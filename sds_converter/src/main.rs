use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use futures::stream::{self, StreamExt};
use walkdir::WalkDir;
use sds_converter_core::{
    converter::{AnthropicBackend, LlmBackend, LlmConfig, OpenAiCompatBackend, openai_compat_url},
    convert_from_json, convert_from_template, convert_to_json, convert_url_to_json,
    enrich_composition, extract_text, extract_text_from_url, validate,
    ConvertConfig, Language,
    SdsError, SdsRoot,
};

// ---------------------------------------------------------------------------
// Quality presets
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, ValueEnum, Default)]
enum CliQuality {
    /// Fast & cheap: 15,000 chars, Haiku
    Low,
    /// Balanced (default): 30,000 chars, Haiku
    #[default]
    Medium,
    /// High accuracy: 60,000 chars, Sonnet
    High,
}

struct QualityPreset {
    max_chars: usize,
    model: &'static str,
    max_tokens: u32,
}

impl CliQuality {
    fn preset(self) -> QualityPreset {
        match self {
            CliQuality::Low => QualityPreset {
                max_chars: 15_000,
                model: "claude-haiku-4-5-20251001",
                max_tokens: 16_384,
            },
            CliQuality::Medium => QualityPreset {
                max_chars: 30_000,
                model: "claude-haiku-4-5-20251001",
                max_tokens: 16_384,
            },
            CliQuality::High => QualityPreset {
                max_chars: 60_000,
                model: "claude-sonnet-4-6",
                max_tokens: 16_384,
            },
        }
    }

    fn name(self) -> &'static str {
        match self {
            CliQuality::Low => "low",
            CliQuality::Medium => "medium",
            CliQuality::High => "high",
        }
    }
}

// ---------------------------------------------------------------------------
// Backend enum — allows creating the backend once and sharing via Arc
// ---------------------------------------------------------------------------

enum BackendKind {
    Anthropic(AnthropicBackend),
    OpenAiCompat(OpenAiCompatBackend),
}

impl LlmBackend for BackendKind {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        match self {
            Self::Anthropic(b) => b.complete(system, user).await,
            Self::OpenAiCompat(b) => b.complete(system, user).await,
        }
    }
}

fn build_backend(
    provider: CliProvider,
    api_key: String,
    llm_config: LlmConfig,
    base_url: Option<String>,
) -> BackendKind {
    match provider {
        CliProvider::Anthropic => BackendKind::Anthropic(AnthropicBackend::new(api_key, llm_config)),
        CliProvider::Gemini => {
            BackendKind::OpenAiCompat(OpenAiCompatBackend::gemini(api_key, llm_config))
        }
        other => {
            let key = provider_url_key(other);
            let url = base_url
                .or_else(|| key.and_then(openai_compat_url).map(str::to_string))
                .unwrap_or_default();
            BackendKind::OpenAiCompat(OpenAiCompatBackend::new(api_key, llm_config, url))
        }
    }
}

fn provider_url_key(provider: CliProvider) -> Option<&'static str> {
    match provider {
        CliProvider::Openai   => Some("openai"),
        CliProvider::Mistral  => Some("mistral"),
        CliProvider::Groq     => Some("groq"),
        CliProvider::Cohere   => Some("cohere"),
        CliProvider::Local    => Some("local"),
        CliProvider::Anthropic | CliProvider::Gemini => None,
    }
}

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
    /// Mistral AI (api.mistral.ai) — requires MISTRAL_API_KEY
    Mistral,
    /// Groq (api.groq.com) — requires GROQ_API_KEY, very fast inference
    Groq,
    /// Cohere (api.cohere.com) — requires COHERE_API_KEY
    Cohere,
    /// Ollama or any local/custom OpenAI-compatible server (requires --base-url)
    Local,
}

fn default_model(provider: CliProvider, quality: CliQuality) -> &'static str {
    match provider {
        CliProvider::Anthropic => quality.preset().model,
        CliProvider::Openai    => "gpt-4o-mini",
        CliProvider::Gemini    => "gemini-2.0-flash",
        CliProvider::Mistral   => "mistral-small-latest",
        CliProvider::Groq      => "llama-3.3-70b-versatile",
        CliProvider::Cohere    => "command-r-plus",
        CliProvider::Local     => "llama3",
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a PDF, Word document, HTML file, or URL to MHLW standard JSON
    ToJson {
        /// Input file (PDF/DOCX/TXT/XLSX/HTML) or URL (http:// / https://)
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<String>,

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

        /// LLM model (default: claude-haiku-4-5-20251001 / gpt-4o-mini / gemini-2.0-flash per provider)
        #[arg(long)]
        model: Option<String>,

        /// LLM provider
        #[arg(long, value_enum, default_value = "anthropic")]
        provider: CliProvider,

        /// Custom OpenAI-compatible base URL (e.g. http://localhost:11434/v1 for Ollama)
        #[arg(long)]
        base_url: Option<String>,

        /// Extraction quality: low=15k chars/Haiku, medium=30k/Haiku (default), high=60k/Sonnet
        #[arg(long, value_enum, default_value = "medium")]
        quality: CliQuality,

        /// Number of files to convert in parallel (batch mode only)
        #[arg(long, default_value = "4")]
        concurrency: usize,

        /// Look up each CAS number in PubChem and report mismatches (requires network)
        #[arg(long)]
        enrich: bool,
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

        /// Output document language (used only without --template)
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,

        /// Word template (.docx) with {{FieldName}} placeholders.
        /// When set, fills the template instead of generating from scratch.
        #[arg(long)]
        template: Option<PathBuf>,
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

    /// Convert MHLW standard JSON to an HTML document
    ToHtml {
        /// Input JSON file (single-file mode)
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,

        /// Input directory — process all .json files (batch mode)
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,

        /// Output HTML file (required in single-file mode)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory for batch mode (created if absent)
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,

        /// Output document language
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,
    },

    /// Convert MHLW standard JSON to a PDF via LibreOffice (requires `soffice` in PATH)
    ToPdf {
        /// Input JSON file (single-file mode)
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,

        /// Input directory — process all .json files (batch mode)
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,

        /// Output PDF file (required in single-file mode)
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,

        /// Output directory for batch mode (created if absent)
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,

        /// Output document language
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,
    },

    /// Extract raw text from a PDF, DOCX file, or URL (no LLM — for inspection/debugging)
    ExtractText {
        /// Input file (PDF/DOCX/TXT/XLSX/HTML) or URL (http:// / https://)
        #[arg(short, long)]
        input: String,

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
            quality,
            concurrency,
            enrich,
        } => {
            let preset = quality.preset();
            let api_key = resolve_api_key(api_key, provider)?;
            let model = model.unwrap_or_else(|| default_model(provider, quality).to_string());
            eprintln!(
                "Quality: {} (max_chars={}, max_tokens={}, model={})",
                quality.name(), preset.max_chars, preset.max_tokens, model
            );
            let llm_config = LlmConfig { model, max_tokens: preset.max_tokens };
            let convert_config = ConvertConfig {
                source_language: lang.map(Language::from),
                output_language: Language::default(),
                max_chars: preset.max_chars,
            };

            let backend = Arc::new(build_backend(provider, api_key, llm_config, base_url));

            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| {
                        anyhow::anyhow!("--output is required when using --input")
                    })?;
                    use anyhow::Context as _;
                    eprintln!("Extracting text from {input} ...");
                    let is_url = input.starts_with("http://") || input.starts_with("https://");
                    let (sds, warnings) = if is_url {
                        convert_url_to_json(&input, &*backend, &convert_config).await?
                    } else {
                        convert_to_json(std::path::Path::new(&input), &*backend, &convert_config).await?
                    };
                    for w in &warnings {
                        eprintln!("WARN: {w}");
                    }
                    if enrich {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(30))
                            .build()?;
                        let cas_warnings = enrich_composition(&sds, &client).await;
                        for w in &cas_warnings {
                            eprintln!("CAS: {w}");
                        }
                    }
                    std::fs::write(&output, serde_json::to_string_pretty(&sds)?)
                        .with_context(|| format!("writing {}", output.display()))?;
                    eprintln!("Saved JSON to {}", output.display());
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| {
                        anyhow::anyhow!("--output-dir is required when using --input-dir")
                    })?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_json(&dir, &out_dir, &convert_config, backend, concurrency).await?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::ToDocx { input, input_dir, output, output_dir, lang, template } => {
            let config = ConvertConfig {
                source_language: None,
                output_language: Language::from(lang),
                ..Default::default()
            };

            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| {
                        anyhow::anyhow!("--output is required when using --input")
                    })?;
                    use anyhow::Context as _;
                    eprintln!("Reading JSON from {} ...", input.display());
                    check_json_file_size(&input)?;
                    let json = std::fs::read_to_string(&input)
                        .with_context(|| format!("reading {}", input.display()))?;
                    let sds: SdsRoot = serde_json::from_str(&json)?;
                    if let Some(tmpl) = &template {
                        eprintln!("Filling template {} ...", tmpl.display());
                        convert_from_template(&sds, tmpl, &output)?;
                    } else {
                        convert_from_json(&sds, &output, &config)?;
                    }
                    eprintln!("Saved DOCX to {}", output.display());
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| {
                        anyhow::anyhow!("--output-dir is required when using --input-dir")
                    })?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_docx(&dir, &out_dir, &config, template.as_deref())?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::Validate { input, json } => {
            use anyhow::Context as _;
            check_json_file_size(&input)?;
            let raw = std::fs::read_to_string(&input)
                .with_context(|| format!("reading {}", input.display()))?;
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
            use anyhow::Context as _;
            let is_url = input.starts_with("http://") || input.starts_with("https://");
            let text = if is_url {
                extract_text_from_url(&input).await?
            } else {
                extract_text(std::path::Path::new(&input)).await?
            };
            match output {
                Some(path) => {
                    std::fs::write(&path, &text)
                        .with_context(|| format!("writing {}", path.display()))?;
                    eprintln!("Saved text to {}", path.display());
                }
                None => print!("{text}"),
            }
        }

        Commands::ToHtml { input, input_dir, output, output_dir, lang } => {
            use anyhow::Context as _;
            let config = ConvertConfig {
                source_language: None,
                output_language: Language::from(lang),
                ..Default::default()
            };
            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| {
                        anyhow::anyhow!("--output is required when using --input")
                    })?;
                    check_json_file_size(&input)?;
                    let json = std::fs::read_to_string(&input)
                        .with_context(|| format!("reading {}", input.display()))?;
                    let sds: SdsRoot = serde_json::from_str(&json)?;
                    let html = sds_converter_core::converter::html::generate_html(&sds, config.output_language)?;
                    std::fs::write(&output, html)
                        .with_context(|| format!("writing {}", output.display()))?;
                    eprintln!("Saved HTML to {}", output.display());
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| {
                        anyhow::anyhow!("--output-dir is required when using --input-dir")
                    })?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_html(&dir, &out_dir, &config)?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::ToPdf { input, input_dir, output, output_dir, lang } => {
            use anyhow::Context as _;
            let config = ConvertConfig {
                source_language: None,
                output_language: Language::from(lang),
                ..Default::default()
            };
            // Verify soffice is available
            if std::process::Command::new("soffice").arg("--version").output().is_err() {
                anyhow::bail!(
                    "PDF output requires LibreOffice. Install from https://www.libreoffice.org/ \
                     and ensure `soffice` is in your PATH."
                );
            }
            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| {
                        anyhow::anyhow!("--output is required when using --input")
                    })?;
                    check_json_file_size(&input)?;
                    let json = std::fs::read_to_string(&input)
                        .with_context(|| format!("reading {}", input.display()))?;
                    let sds: SdsRoot = serde_json::from_str(&json)?;
                    docx_to_pdf(&sds, &output, &config)?;
                    eprintln!("Saved PDF to {}", output.display());
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| {
                        anyhow::anyhow!("--output-dir is required when using --input-dir")
                    })?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_pdf(&dir, &out_dir, &config)?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
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
        eprintln!(
            "Warning: passing --api-key on the command line may expose it to other local \
             processes. Prefer setting the environment variable instead."
        );
        return Ok(k);
    }
    let env_var = match provider {
        CliProvider::Anthropic => "ANTHROPIC_API_KEY",
        CliProvider::Openai    => "OPENAI_API_KEY",
        CliProvider::Gemini    => "GEMINI_API_KEY",
        CliProvider::Mistral   => "MISTRAL_API_KEY",
        CliProvider::Groq      => "GROQ_API_KEY",
        CliProvider::Cohere    => "COHERE_API_KEY",
        CliProvider::Local     => {
            return Ok(std::env::var("LOCAL_LLM_API_KEY").unwrap_or_else(|_| "ollama".to_string()))
        }
    };
    std::env::var(env_var)
        .map_err(|_| anyhow::anyhow!("API key not provided. Set --api-key or {env_var}"))
}

const MAX_JSON_INPUT_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

fn check_json_file_size(path: &Path) -> anyhow::Result<()> {
    let size = std::fs::metadata(path)
        .map_err(|e| anyhow::anyhow!("file stat failed for {}: {e}", path.display()))?
        .len();
    if size > MAX_JSON_INPUT_BYTES {
        anyhow::bail!(
            "input file too large ({} bytes, limit {} MB): {}",
            size,
            MAX_JSON_INPUT_BYTES / 1024 / 1024,
            path.display()
        );
    }
    Ok(())
}

fn collect_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

async fn batch_to_json(
    input_dir: &Path,
    output_dir: &Path,
    config: &ConvertConfig,
    backend: Arc<BackendKind>,
    concurrency: usize,
) -> anyhow::Result<()> {
    let files = collect_files(input_dir, &["pdf", "docx", "xlsx", "xls"]);

    let total = files.len();
    if total == 0 {
        eprintln!("No .pdf/.docx/.xlsx files found in {}", input_dir.display());
        return Ok(());
    }

    let concurrency = concurrency.max(1);
    eprintln!("Batch: {total} files, concurrency={concurrency}");

    let config = Arc::new(config.clone());
    let output_dir = Arc::new(output_dir.to_path_buf());
    let ok = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let failed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    stream::iter(files.into_iter().enumerate())
        .map(|(i, path)| {
            let config = Arc::clone(&config);
            let backend = Arc::clone(&backend);
            let output_dir = Arc::clone(&output_dir);
            let ok = Arc::clone(&ok);
            let failed = Arc::clone(&failed);
            async move {
                let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
                    Some(s) => s.to_string(),
                    None => {
                        eprintln!("  [SKIP] {}: no file stem", path.display());
                        failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                };
                let out_path = output_dir.join(format!("{stem}.json"));
                eprintln!("[{}/{}] {}", i + 1, total, path.display());
                match convert_to_json(&path, &*backend, &config).await {
                    Ok((sds, warnings)) => {
                        for w in &warnings {
                            eprintln!("  WARN: {w}");
                        }
                        match serde_json::to_string_pretty(&sds) {
                            Ok(json) => match std::fs::write(&out_path, json) {
                                Ok(_) => {
                                    ok.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    eprintln!("  [OK] {}", out_path.display());
                                }
                                Err(e) => {
                                    eprintln!("  [ERROR] write {}: {e}", out_path.display());
                                    failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                }
                            },
                            Err(e) => {
                                eprintln!("  [ERROR] serialize {}: {e}", path.display());
                                failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  [ERROR] {}: {e}", path.display());
                        failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    let ok = ok.load(std::sync::atomic::Ordering::Relaxed);
    let failed = failed.load(std::sync::atomic::Ordering::Relaxed);
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}

fn batch_to_html(
    input_dir: &Path,
    output_dir: &Path,
    config: &ConvertConfig,
) -> anyhow::Result<()> {
    let files = collect_files(input_dir, &["json"]);
    let total = files.len();
    if total == 0 {
        eprintln!("No .json files found in {}", input_dir.display());
        return Ok(());
    }
    let (mut ok, mut failed) = (0usize, 0usize);
    for (i, path) in files.iter().enumerate() {
        let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => { failed += 1; continue; }
        };
        let out_path = output_dir.join(format!("{stem}.html"));
        eprintln!("[{}/{}] {}", i + 1, total, path.display());
        let result = check_json_file_size(path)
            .and_then(|_| std::fs::read_to_string(path).map_err(anyhow::Error::from))
            .and_then(|raw| serde_json::from_str::<SdsRoot>(&raw).map_err(anyhow::Error::from))
            .and_then(|sds| {
                sds_converter_core::converter::html::generate_html(&sds, config.output_language)
                    .map_err(anyhow::Error::from)
            })
            .and_then(|html| std::fs::write(&out_path, html).map_err(anyhow::Error::from));
        match result {
            Ok(_) => ok += 1,
            Err(e) => { eprintln!("[ERROR] {}: {e}", path.display()); failed += 1; }
        }
    }
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}

/// Generate a temporary DOCX then convert it to PDF via `soffice --headless`.
fn docx_to_pdf(sds: &SdsRoot, output: &Path, config: &ConvertConfig) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let tmp_dir = std::env::temp_dir();
    let tmp_docx = tmp_dir.join(format!(
        "sds_converter_{}.docx",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    convert_from_json(sds, &tmp_docx, config)
        .with_context(|| "generating intermediate DOCX")?;

    let out_dir = output
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let status = std::process::Command::new("soffice")
        .args([
            "--headless",
            "--convert-to",
            "pdf",
            "--outdir",
            out_dir.to_str().unwrap_or("."),
            tmp_docx.to_str().unwrap_or(""),
        ])
        .status()
        .with_context(|| "running soffice")?;

    let _ = std::fs::remove_file(&tmp_docx);

    if !status.success() {
        anyhow::bail!("soffice exited with status {status}");
    }

    // soffice places the PDF as <stem>.pdf in out_dir; rename to the requested output path.
    let stem = tmp_docx.file_stem().unwrap_or_default().to_string_lossy();
    let generated = out_dir.join(format!("{stem}.pdf"));
    if generated != output {
        std::fs::rename(&generated, output)
            .with_context(|| format!("renaming {} to {}", generated.display(), output.display()))?;
    }

    Ok(())
}

fn batch_to_pdf(
    input_dir: &Path,
    output_dir: &Path,
    config: &ConvertConfig,
) -> anyhow::Result<()> {
    let files = collect_files(input_dir, &["json"]);
    let total = files.len();
    if total == 0 {
        eprintln!("No .json files found in {}", input_dir.display());
        return Ok(());
    }
    let (mut ok, mut failed) = (0usize, 0usize);
    for (i, path) in files.iter().enumerate() {
        let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => { failed += 1; continue; }
        };
        let out_path = output_dir.join(format!("{stem}.pdf"));
        eprintln!("[{}/{}] {}", i + 1, total, path.display());
        let result = check_json_file_size(path)
            .and_then(|_| std::fs::read_to_string(path).map_err(anyhow::Error::from))
            .and_then(|raw| serde_json::from_str::<SdsRoot>(&raw).map_err(anyhow::Error::from))
            .and_then(|sds| docx_to_pdf(&sds, &out_path, config));
        match result {
            Ok(_) => ok += 1,
            Err(e) => { eprintln!("[ERROR] {}: {e}", path.display()); failed += 1; }
        }
    }
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}

fn batch_to_docx(
    input_dir: &Path,
    output_dir: &Path,
    config: &ConvertConfig,
    template: Option<&Path>,
) -> anyhow::Result<()> {
    let files = collect_files(input_dir, &["json"]);

    let total = files.len();
    if total == 0 {
        eprintln!("No .json files found in {}", input_dir.display());
        return Ok(());
    }

    let (mut ok, mut failed) = (0usize, 0usize);
    for (i, path) in files.iter().enumerate() {
        let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => {
                eprintln!("  [SKIP] {}: no file stem", path.display());
                failed += 1;
                continue;
            }
        };
        let out_path = output_dir.join(format!("{stem}.docx"));
        eprintln!("[{}/{}] {}", i + 1, total, path.display());
        let result = check_json_file_size(path)
            .and_then(|_| std::fs::read_to_string(path).map_err(anyhow::Error::from))
            .and_then(|raw| serde_json::from_str::<SdsRoot>(&raw).map_err(anyhow::Error::from))
            .and_then(|sds| {
                if let Some(tmpl) = template {
                    convert_from_template(&sds, tmpl, &out_path).map_err(anyhow::Error::from)
                } else {
                    convert_from_json(&sds, &out_path, config).map_err(anyhow::Error::from)
                }
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

