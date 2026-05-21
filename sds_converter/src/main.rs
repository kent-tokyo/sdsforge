use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use sds_converter_core::{
    converter::{AnthropicBackend, LlmConfig, OpenAiCompatBackend},
    convert_from_json, convert_to_json, extract_text, validate, ConvertConfig, Language,
    OutputFormat, SdsRoot,
};

#[derive(Parser)]
#[command(name = "sds-converter", about = "Convert between SDS documents and MHLW standard JSON")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum)]
enum CliLanguage {
    Ja,
    En,
    ZhCn,
    ZhTw,
}

impl From<CliLanguage> for Language {
    fn from(l: CliLanguage) -> Self {
        match l {
            CliLanguage::Ja => Language::Japanese,
            CliLanguage::En => Language::English,
            CliLanguage::ZhCn => Language::ChineseSimplified,
            CliLanguage::ZhTw => Language::ChineseTraditional,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Default)]
enum CliProvider {
    #[default]
    Anthropic,
    Openai,
    Gemini,
}

fn default_model(provider: CliProvider) -> &'static str {
    match provider {
        CliProvider::Anthropic => "claude-sonnet-4-6",
        CliProvider::Openai => "gpt-4o",
        CliProvider::Gemini => "gemini-2.0-flash",
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a PDF or Word document to MHLW standard JSON
    ToJson {
        /// Input PDF, DOCX, or TXT file (single-file mode)
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,

        /// Input directory — process all .pdf/.docx files (batch mode)
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,

        /// Output JSON file (required in single-file mode)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory for batch mode (created if absent)
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,

        /// API key (or set ANTHROPIC_API_KEY / OPENAI_API_KEY / GEMINI_API_KEY)
        #[arg(long, hide_env_values = true)]
        api_key: Option<String>,

        /// Source document language (default: auto-detect)
        #[arg(long, value_enum)]
        lang: Option<CliLanguage>,

        /// LLM model (default: claude-sonnet-4-6 / gpt-4o / gemini-2.0-flash per provider)
        #[arg(long)]
        model: Option<String>,

        /// LLM provider
        #[arg(long, value_enum, default_value = "anthropic")]
        provider: CliProvider,

        /// Custom OpenAI-compatible base URL (e.g. http://localhost:11434/v1 for Ollama)
        #[arg(long)]
        base_url: Option<String>,
    },

    /// Convert MHLW standard JSON to a Word document
    ToDocx {
        /// Input JSON file (single-file mode)
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,

        /// Input directory — process all .json files (batch mode)
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,

        /// Output DOCX file (required in single-file mode)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory for batch mode (created if absent)
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,

        /// Output document language
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,
    },

    /// Validate a MHLW standard JSON file and report structural issues
    Validate {
        /// Input JSON file
        #[arg(short, long)]
        input: PathBuf,

        /// Output warnings as a JSON array (useful for CI)
        #[arg(long)]
        json: bool,
    },

    /// Extract raw text from a PDF or DOCX file (no LLM — for inspection/debugging)
    ExtractText {
        /// Input PDF or DOCX file
        #[arg(short, long)]
        input: PathBuf,

        /// Output .txt file (omit to print to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::ToJson {
            input,
            input_dir,
            output,
            output_dir,
            api_key,
            lang,
            model,
            provider,
            base_url,
        } => {
            let api_key = resolve_api_key(api_key, provider)?;
            let model = model.unwrap_or_else(|| default_model(provider).to_string());
            let llm_config = LlmConfig { model, max_tokens: 8192 };
            let convert_config = ConvertConfig {
                source_language: lang.map(Language::from),
                output_language: Language::default(),
            };

            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| {
                        anyhow::anyhow!("--output is required when using --input")
                    })?;
                    eprintln!("Extracting text from {} ...", input.display());
                    let sds = run_to_json(
                        &input,
                        &convert_config,
                        provider,
                        api_key,
                        llm_config,
                        base_url,
                    )
                    .await?;
                    std::fs::write(&output, serde_json::to_string_pretty(&sds)?)?;
                    eprintln!("Saved JSON to {}", output.display());
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| {
                        anyhow::anyhow!("--output-dir is required when using --input-dir")
                    })?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_json(
                        &dir,
                        &out_dir,
                        &convert_config,
                        api_key,
                        llm_config,
                        provider,
                        base_url,
                    )
                    .await?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::ToDocx { input, input_dir, output, output_dir, lang } => {
            let config = ConvertConfig {
                source_language: None,
                output_language: Language::from(lang),
            };

            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| {
                        anyhow::anyhow!("--output is required when using --input")
                    })?;
                    eprintln!("Reading JSON from {} ...", input.display());
                    let json = std::fs::read_to_string(&input)?;
                    let sds: SdsRoot = serde_json::from_str(&json)?;
                    convert_from_json(&sds, &output, OutputFormat::Docx, &config)?;
                    eprintln!("Saved DOCX to {}", output.display());
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| {
                        anyhow::anyhow!("--output-dir is required when using --input-dir")
                    })?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_docx(&dir, &out_dir, &config)?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::Validate { input, json } => {
            let raw = std::fs::read_to_string(&input)?;
            let sds: SdsRoot = serde_json::from_str(&raw)?;
            let warnings = validate(&sds);

            if json {
                println!("{}", serde_json::to_string_pretty(&warnings)?);
            } else if warnings.is_empty() {
                println!("OK: no issues found");
            } else {
                for w in &warnings {
                    eprintln!("WARN: {w}");
                }
                std::process::exit(1);
            }
        }

        Commands::ExtractText { input, output } => {
            let text = extract_text(&input)?;
            match output {
                Some(path) => {
                    std::fs::write(&path, &text)?;
                    eprintln!("Saved text to {}", path.display());
                }
                None => print!("{text}"),
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_api_key(api_key: Option<String>, provider: CliProvider) -> anyhow::Result<String> {
    if let Some(k) = api_key {
        return Ok(k);
    }
    let env_var = match provider {
        CliProvider::Anthropic => "ANTHROPIC_API_KEY",
        CliProvider::Openai => "OPENAI_API_KEY",
        CliProvider::Gemini => "GEMINI_API_KEY",
    };
    std::env::var(env_var)
        .map_err(|_| anyhow::anyhow!("API key not provided. Set --api-key or {env_var}"))
}

async fn run_to_json(
    input: &Path,
    config: &ConvertConfig,
    provider: CliProvider,
    api_key: String,
    llm_config: LlmConfig,
    base_url: Option<String>,
) -> anyhow::Result<SdsRoot> {
    match provider {
        CliProvider::Anthropic => {
            let backend = AnthropicBackend::new(api_key, llm_config);
            Ok(convert_to_json(input, &backend, config).await?)
        }
        CliProvider::Openai => {
            let backend = match base_url {
                Some(url) => OpenAiCompatBackend::new(api_key, llm_config, &url),
                None => OpenAiCompatBackend::openai(api_key, llm_config),
            };
            Ok(convert_to_json(input, &backend, config).await?)
        }
        CliProvider::Gemini => {
            let backend = OpenAiCompatBackend::gemini(api_key, llm_config);
            Ok(convert_to_json(input, &backend, config).await?)
        }
    }
}

async fn batch_to_json(
    input_dir: &Path,
    output_dir: &Path,
    config: &ConvertConfig,
    api_key: String,
    llm_config: LlmConfig,
    provider: CliProvider,
    base_url: Option<String>,
) -> anyhow::Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(input_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("pdf" | "docx")
            )
        })
        .collect();
    files.sort();

    let total = files.len();
    if total == 0 {
        eprintln!("No .pdf or .docx files found in {}", input_dir.display());
        return Ok(());
    }

    let (mut ok, mut failed) = (0usize, 0usize);
    for (i, path) in files.iter().enumerate() {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let out_path = output_dir.join(format!("{stem}.json"));
        eprintln!("[{}/{}] {}", i + 1, total, path.display());
        match run_to_json(path, config, provider, api_key.clone(), llm_config.clone(), base_url.clone()).await {
            Ok(sds) => {
                std::fs::write(&out_path, serde_json::to_string_pretty(&sds)?)?;
                ok += 1;
            }
            Err(e) => {
                eprintln!("[ERROR] {}: {e}", path.display());
                failed += 1;
            }
        }
    }
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}

fn batch_to_docx(
    input_dir: &Path,
    output_dir: &Path,
    config: &ConvertConfig,
) -> anyhow::Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(input_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    files.sort();

    let total = files.len();
    if total == 0 {
        eprintln!("No .json files found in {}", input_dir.display());
        return Ok(());
    }

    let (mut ok, mut failed) = (0usize, 0usize);
    for (i, path) in files.iter().enumerate() {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let out_path = output_dir.join(format!("{stem}.docx"));
        eprintln!("[{}/{}] {}", i + 1, total, path.display());
        let result = std::fs::read_to_string(path)
            .map_err(anyhow::Error::from)
            .and_then(|raw| serde_json::from_str::<SdsRoot>(&raw).map_err(anyhow::Error::from))
            .and_then(|sds| {
                convert_from_json(&sds, &out_path, OutputFormat::Docx, config)
                    .map_err(anyhow::Error::from)
            });
        match result {
            Ok(_) => ok += 1,
            Err(e) => {
                eprintln!("[ERROR] {}: {e}", path.display());
                failed += 1;
            }
        }
    }
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}
