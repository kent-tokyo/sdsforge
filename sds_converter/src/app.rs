use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;

use crate::config::AppConfig;
use crate::tasks::{
    LogFn, Provider, Quality, ToDocxParams, ToHtmlParams, ToJsonParams, ToPdfParams,
};

// ---------------------------------------------------------------------------
// i18n string table
// ---------------------------------------------------------------------------

struct Strings {
    // Menus
    menu_file: &'static str,
    menu_quit: &'static str,
    menu_help: &'static str,
    menu_about: &'static str,
    // Tabs
    tab_convert: &'static str,
    tab_generate: &'static str,
    tab_validate: &'static str,
    tab_settings: &'static str,
    // Convert tab
    heading_convert: &'static str,
    lbl_input: &'static str,
    lbl_output_json: &'static str,
    lbl_provider: &'static str,
    lbl_quality: &'static str,
    lbl_lang: &'static str,
    lbl_enrich: &'static str,
    lbl_files: &'static str,
    btn_browse: &'static str,
    btn_browse_multi: &'static str,
    btn_browse_dir: &'static str,
    btn_save_to: &'static str,
    btn_convert: &'static str,
    btn_converting: &'static str,
    btn_clear_files: &'static str,
    lbl_output_dir: &'static str,
    // Generate tab
    heading_generate: &'static str,
    lbl_input_json: &'static str,
    lbl_output_file: &'static str,
    lbl_format: &'static str,
    btn_generate: &'static str,
    btn_generating: &'static str,
    // Validate tab
    heading_validate: &'static str,
    btn_validate: &'static str,
    btn_validating: &'static str,
    msg_no_issues: &'static str,
    // Settings tab
    heading_settings: &'static str,
    lbl_def_provider: &'static str,
    lbl_def_lang: &'static str,
    lbl_def_quality: &'static str,
    lbl_api_key: &'static str,
    lbl_ui_lang: &'static str,
    lbl_def_enrich: &'static str,
    btn_save: &'static str,
    msg_api_key_warn: &'static str,
    msg_saved: &'static str,
    // Log panel
    lbl_log: &'static str,
    btn_clear: &'static str,
    // Errors
    err_no_api_key: &'static str,
    err_no_input: &'static str,
    // About / Manual
    about_title: &'static str,
    about_body: &'static str,
    menu_manual: &'static str,
    manual_title: &'static str,
    manual_body: &'static str,
}

fn get_strings(ui_lang: &str) -> Strings {
    match ui_lang {
        "en" => Strings {
            menu_file:        "File",
            menu_quit:        "Quit",
            menu_help:        "Help",
            menu_about:       "About",
            tab_convert:      "Convert (to-json)",
            tab_generate:     "Generate (docx/html)",
            tab_validate:     "Validate",
            tab_settings:     "Settings",
            heading_convert:  "SDS Document → MHLW Standard JSON",
            lbl_input:        "Input (file/URL):",
            lbl_output_json:  "Output JSON:",
            lbl_provider:     "Provider:",
            lbl_quality:      "Quality:",
            lbl_lang:         "Language:",
            lbl_enrich:       "PubChem lookup (--enrich)",
            lbl_files:        "file(s) selected",
            btn_browse:       "Browse...",
            btn_browse_multi: "Select files...",
            btn_browse_dir:   "Select folder...",
            btn_save_to:      "Save to...",
            btn_convert:      "Convert",
            btn_converting:   "Converting...",
            btn_clear_files:  "Clear selection",
            lbl_output_dir:   "Output folder:",
            heading_generate: "MHLW JSON → Document",
            lbl_input_json:   "Input JSON:",
            lbl_output_file:  "Output:",
            lbl_format:       "Format:",
            btn_generate:     "Generate",
            btn_generating:   "Generating...",
            heading_validate: "JSON Validation",
            btn_validate:     "Validate",
            btn_validating:   "Validating...",
            msg_no_issues:    "OK: no issues found",
            heading_settings: "Settings",
            lbl_def_provider: "Default Provider:",
            lbl_def_lang:     "Default Language:",
            lbl_def_quality:  "Default Quality:",
            lbl_api_key:      "API Key:",
            lbl_ui_lang:      "UI Language:",
            lbl_def_enrich:   "PubChem lookup by default",
            btn_save:         "Save",
            msg_api_key_warn: "⚠ API key is stored in plain text",
            msg_saved:        "Saved",
            lbl_log:          "Log",
            btn_clear:        "Clear",
            err_no_api_key:   "[ERROR] API key not set. Enter it in Settings.",
            err_no_input:     "[ERROR] Please specify an input file.",
            about_title:      "About sds-converter",
            about_body:       concat!(
                "sds-converter v", env!("CARGO_PKG_VERSION"),
                "\n\nConverts SDS documents to/from MHLW standard JSON.\nhttps://github.com/kent-tokyo/sds-converter"
            ),
            menu_manual: "Manual",
            manual_title: "How to use sds-converter",
            manual_body: "\
【Convert tab (to-json)】
Convert SDS documents (PDF, Word, XLSX, HTML) to MHLW standard JSON.
1. Enter the file path or URL in the Input field (or click Browse)
2. Specify the output JSON path
3. Select the LLM provider and enter the API key in Settings
4. Click Convert
For batch processing, use 'Select files...' or 'Select folder...'

【Generate tab (docx/html)】
Generate a Word/HTML/PDF document from MHLW standard JSON.
1. Select the input JSON file
2. Specify the output path and format
3. Click Generate

【Validate tab】
Check if a JSON file conforms to the MHLW SDS standard.
Select a JSON file and click Validate to see any warnings.
Multiple files can be selected at once.

【Settings tab】
• API Key: Enter the key for your chosen LLM provider
• Default Provider/Language/Quality: Used when opening the app
• PubChem lookup: Enriches composition data via PubChem API
• UI Language: Change the interface language

【Tips】
• Set RUST_LOG=info for verbose CLI logging
• Use --help for CLI usage: sds-converter --help",
        },
        "zh-cn" => Strings {
            menu_file:        "文件",
            menu_quit:        "退出",
            menu_help:        "帮助",
            menu_about:       "关于",
            tab_convert:      "转换 (to-json)",
            tab_generate:     "生成 (docx/html)",
            tab_validate:     "验证",
            tab_settings:     "设置",
            heading_convert:  "SDS文档 → MHLW标准JSON",
            lbl_input:        "输入 (文件/URL):",
            lbl_output_json:  "输出 JSON:",
            lbl_provider:     "提供商:",
            lbl_quality:      "质量:",
            lbl_lang:         "语言:",
            lbl_enrich:       "PubChem查询 (--enrich)",
            lbl_files:        "个文件已选择",
            btn_browse:       "浏览...",
            btn_browse_multi: "选择文件...",
            btn_browse_dir:   "选择文件夹...",
            btn_save_to:      "保存到...",
            btn_convert:      "开始转换",
            btn_converting:   "转换中...",
            btn_clear_files:  "清除选择",
            lbl_output_dir:   "输出文件夹:",
            heading_generate: "MHLW JSON → 文档生成",
            lbl_input_json:   "输入 JSON:",
            lbl_output_file:  "输出文件:",
            lbl_format:       "格式:",
            btn_generate:     "开始生成",
            btn_generating:   "生成中...",
            heading_validate: "JSON验证",
            btn_validate:     "验证",
            btn_validating:   "验证中...",
            msg_no_issues:    "OK: 未发现问题",
            heading_settings: "设置",
            lbl_def_provider: "默认提供商:",
            lbl_def_lang:     "默认语言:",
            lbl_def_quality:  "默认质量:",
            lbl_api_key:      "API密钥:",
            lbl_ui_lang:      "界面语言:",
            lbl_def_enrich:   "默认启用PubChem查询",
            btn_save:         "保存",
            msg_api_key_warn: "⚠ API密钥以明文保存",
            msg_saved:        "已保存",
            lbl_log:          "日志",
            btn_clear:        "清除",
            err_no_api_key:   "[ERROR] 未设置API密钥，请在设置中输入。",
            err_no_input:     "[ERROR] 请指定输入文件。",
            about_title:      "关于 sds-converter",
            about_body:       concat!(
                "sds-converter v", env!("CARGO_PKG_VERSION"),
                "\n\n将SDS文档转换为MHLW标准JSON。\nhttps://github.com/kent-tokyo/sds-converter"
            ),
            menu_manual: "使用手册",
            manual_title: "sds-converter 使用说明",
            manual_body: "\
【转换标签 (to-json)】
将SDS文档（PDF、Word、XLSX、HTML）转换为MHLW标准JSON。
1. 在输入栏输入文件路径或URL（或点击浏览）
2. 指定输出JSON路径
3. 在设置中选择LLM提供商并输入API密钥
4. 点击「开始转换」
批量处理：使用「选择文件...」或「选择文件夹...」

【生成标签 (docx/html)】
从MHLW标准JSON生成Word/HTML/PDF文档。
1. 选择输入JSON文件
2. 指定输出路径和格式
3. 点击「开始生成」

【验证标签】
检查JSON文件是否符合MHLW SDS标准。
选择JSON文件并点击「验证」查看警告。

【设置标签】
• API密钥：输入所选LLM提供商的密钥
• 默认提供商/语言/质量：启动时的默认值
• PubChem查询：通过PubChem API丰富成分数据",
        },
        _ => Strings {  // Japanese (ja, default)
            menu_file:        "ファイル",
            menu_quit:        "終了",
            menu_help:        "ヘルプ",
            menu_about:       "バージョン情報",
            tab_convert:      "変換 (to-json)",
            tab_generate:     "生成 (docx/html)",
            tab_validate:     "検証",
            tab_settings:     "設定",
            heading_convert:  "SDS文書 → MHLW標準JSON",
            lbl_input:        "入力 (ファイル/URL):",
            lbl_output_json:  "出力 JSON:",
            lbl_provider:     "プロバイダ:",
            lbl_quality:      "品質:",
            lbl_lang:         "言語:",
            lbl_enrich:       "PubChem照合 (--enrich)",
            lbl_files:        "ファイル選択済み",
            btn_browse:       "参照...",
            btn_browse_multi: "複数選択...",
            btn_browse_dir:   "フォルダ選択...",
            btn_save_to:      "保存先...",
            btn_convert:      "変換開始",
            btn_converting:   "変換中...",
            btn_clear_files:  "選択解除",
            lbl_output_dir:   "出力フォルダ:",
            heading_generate: "MHLW JSON → 文書生成",
            lbl_input_json:   "入力 JSON:",
            lbl_output_file:  "出力ファイル:",
            lbl_format:       "形式:",
            btn_generate:     "生成開始",
            btn_generating:   "生成中...",
            heading_validate: "JSON バリデーション",
            btn_validate:     "検証実行",
            btn_validating:   "検証中...",
            msg_no_issues:    "OK: 問題は見つかりませんでした",
            heading_settings: "設定",
            lbl_def_provider: "デフォルトプロバイダ:",
            lbl_def_lang:     "デフォルト言語:",
            lbl_def_quality:  "デフォルト品質:",
            lbl_api_key:      "API Key:",
            lbl_ui_lang:      "UI言語:",
            lbl_def_enrich:   "PubChem照合をデフォルトで有効にする",
            btn_save:         "保存",
            msg_api_key_warn: "⚠ APIキーはプレーンテキストで設定ファイルに保存されます",
            msg_saved:        "保存しました",
            lbl_log:          "ログ",
            btn_clear:        "クリア",
            err_no_api_key:   "[ERROR] APIキーが未設定です。設定タブで入力してください。",
            err_no_input:     "[ERROR] 入力ファイルを指定してください。",
            about_title:      "sds-converter について",
            about_body:       concat!(
                "sds-converter v", env!("CARGO_PKG_VERSION"),
                "\n\nSDS文書をMHLW標準JSONへ変換します。\nhttps://github.com/kent-tokyo/sds-converter"
            ),
            menu_manual: "マニュアル",
            manual_title: "sds-converter 使い方",
            manual_body: "\
【変換タブ (to-json)】
SDS文書（PDF・Word・XLSX・HTML）をMHLW標準JSONに変換します。
1. 入力欄にファイルパスまたはURLを入力（または「参照...」で選択）
2. 出力JSONの保存先を指定
3. 設定タブでLLMプロバイダとAPIキーを設定
4. 「変換開始」をクリック
複数ファイルをまとめて変換する場合は「複数選択...」または「フォルダ選択...」を使用

【生成タブ (docx/html)】
MHLW標準JSONからWord・HTML・PDF文書を生成します。
1. 入力JSONファイルを選択
2. 出力先と形式（DOCX/HTML/PDF）を指定
3. 「生成開始」をクリック

【検証タブ】
JSONファイルがMHLW SDS標準に準拠しているか確認します。
JSONファイルを選択して「検証実行」をクリックすると警告が表示されます。
複数ファイルの一括検証も可能です。

【設定タブ】
• APIキー: LLMプロバイダのAPIキーを入力
• デフォルトプロバイダ/言語/品質: 起動時のデフォルト値
• PubChem照合: PubChem APIで組成情報を補完（--enrichオプション）
• UI言語: インターフェースの表示言語を切り替え

【ヒント】
• RUST_LOG=info を設定すると詳細ログが表示されます（CUIモード）
• CLIの使い方: sds-converter --help",
        },
    }
}

// ---------------------------------------------------------------------------
// Tab / format enums
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
enum Tab {
    Convert,
    Generate,
    Validate,
    Settings,
}

#[derive(PartialEq, Clone, Copy)]
enum GenFormat {
    Docx,
    Html,
    Pdf,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct SdsApp {
    config: AppConfig,
    rt: tokio::runtime::Runtime,
    log: Arc<Mutex<Vec<String>>>,
    busy: Arc<AtomicBool>,
    tab: Tab,
    show_about: bool,
    show_manual: bool,

    // Convert tab — batch-capable
    conv_input: String,          // URL or single file path (text box)
    conv_inputs: Vec<PathBuf>,   // multi-file selection (non-empty → batch mode)
    conv_output: String,         // single-file output path
    conv_output_dir: String,     // batch output directory
    conv_provider: String,
    conv_quality: String,
    conv_lang: String,
    conv_enrich: bool,

    // Generate tab
    gen_input: String,
    gen_output: String,
    gen_format: GenFormat,
    gen_lang: String,

    // Validate tab — batch-capable
    val_input: String,
    val_inputs: Vec<PathBuf>,
    val_results: Vec<String>,
    val_pending: Arc<Mutex<Option<Vec<String>>>>,

    // Settings tab
    settings_saved_msg: Option<String>,
}

impl SdsApp {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            conv_provider: config.provider.clone(),
            conv_quality:  config.quality.clone(),
            conv_lang:     config.language.clone(),
            conv_enrich:   config.enrich,
            gen_lang:      config.language.clone(),
            config,
            rt: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            log:          Arc::new(Mutex::new(Vec::new())),
            busy:         Arc::new(AtomicBool::new(false)),
            tab:          Tab::Convert,
            show_about:   false,
            show_manual:  false,
            conv_input:   String::new(),
            conv_inputs:  Vec::new(),
            conv_output:  String::new(),
            conv_output_dir: String::new(),
            gen_input:    String::new(),
            gen_output:   String::new(),
            gen_format:   GenFormat::Docx,
            val_input:    String::new(),
            val_inputs:   Vec::new(),
            val_results:  Vec::new(),
            val_pending:  Arc::new(Mutex::new(None)),
            settings_saved_msg: None,
        }
    }

    fn log_push(&self, msg: impl Into<String>) {
        if let Ok(mut v) = self.log.lock() { v.push(msg.into()); }
    }

    fn make_log_fn(&self) -> LogFn {
        let log = Arc::clone(&self.log);
        Arc::new(move |msg| { if let Ok(mut v) = log.lock() { v.push(msg); } })
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    fn s(&self) -> Strings {
        get_strings(&self.config.ui_lang)
    }

    // -----------------------------------------------------------------------
    // Convert tab
    // -----------------------------------------------------------------------

    fn ui_convert_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_convert);
        ui.add_space(6.0);

        let batch = !self.conv_inputs.is_empty();

        if batch {
            // Batch mode: show file count + clear button
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", self.conv_inputs.len(), s.lbl_files));
                if ui.small_button(s.btn_clear_files).clicked() {
                    self.conv_inputs.clear();
                }
            });
            ui.horizontal(|ui| {
                ui.label(s.lbl_output_dir);
                ui.add_sized([260.0, 20.0], egui::TextEdit::singleline(&mut self.conv_output_dir));
                if ui.button(s.btn_browse_dir).clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.conv_output_dir = p.to_string_lossy().into_owned();
                    }
                }
            });
        } else {
            // Single mode: text input + browse
            ui.horizontal(|ui| {
                ui.label(s.lbl_input);
                ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.conv_input));
                if ui.button(s.btn_browse).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("SDS", &["pdf", "docx", "xlsx", "txt", "html"])
                        .pick_file()
                    {
                        self.conv_input = path.to_string_lossy().into_owned();
                        if self.conv_output.is_empty() {
                            if let Some(stem) = path.file_stem() {
                                let mut out = path.clone();
                                out.set_file_name(format!("{}.json", stem.to_string_lossy()));
                                self.conv_output = out.to_string_lossy().into_owned();
                            }
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(s.lbl_output_json);
                ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.conv_output));
                if ui.button(s.btn_save_to).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                        self.conv_output = p.to_string_lossy().into_owned();
                    }
                }
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(s.btn_browse_multi).clicked() {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("SDS", &["pdf", "docx", "xlsx", "txt", "html"])
                    .pick_files()
                {
                    self.conv_inputs = paths;
                    self.conv_output.clear();
                }
            }
            if ui.button(s.btn_browse_dir).clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    let exts = ["pdf", "docx", "xlsx", "txt", "html"];
                    self.conv_inputs = crate::tasks::collect_files(&dir, &exts);
                    self.conv_output_dir = dir.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(s.lbl_provider);
            egui::ComboBox::from_id_salt("conv_provider")
                .selected_text(&self.conv_provider)
                .width(130.0)
                .show_ui(ui, |ui| {
                    for &p in Provider::all() {
                        ui.selectable_value(&mut self.conv_provider, p.to_string(), p);
                    }
                });
            ui.add_space(8.0);
            ui.label(s.lbl_quality);
            egui::ComboBox::from_id_salt("conv_quality")
                .selected_text(&self.conv_quality)
                .width(85.0)
                .show_ui(ui, |ui| {
                    for &q in Quality::all() {
                        ui.selectable_value(&mut self.conv_quality, q.to_string(), q);
                    }
                });
            ui.add_space(8.0);
            ui.label(s.lbl_lang);
            lang_combo(ui, "conv_lang", &mut self.conv_lang);
        });

        ui.checkbox(&mut self.conv_enrich, s.lbl_enrich);
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_converting } else { s.btn_convert };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_convert(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });
    }

    fn start_convert(&mut self, ctx: &egui::Context) {
        let provider = Provider::from_str(&self.conv_provider);
        let quality  = Quality::from_str(&self.conv_quality);
        let lang     = lang_from_str(&self.conv_lang);
        let enrich   = self.conv_enrich;

        let api_key = {
            let k = self.config.api_key.clone();
            if k.is_empty() {
                std::env::var(provider.api_key_env()).unwrap_or_default()
            } else { k }
        };
        if api_key.is_empty() {
            self.log_push(self.s().err_no_api_key);
            return;
        }

        let model = provider.default_model(quality).to_string();
        let log_fn  = self.make_log_fn();
        let log_err = Arc::clone(&self.log);
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        busy.store(true, Ordering::Relaxed);

        if !self.conv_inputs.is_empty() {
            // ----- Batch mode -----
            let inputs = self.conv_inputs.clone();
            let out_dir = PathBuf::from(if self.conv_output_dir.is_empty() {
                inputs.first().and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
                    .unwrap_or_default()
            } else {
                self.conv_output_dir.clone()
            });
            self.log_push(format!("[START] batch {} files", inputs.len()));

            self.rt.spawn(async move {
                if let Err(e) = std::fs::create_dir_all(&out_dir) {
                    if let Ok(mut v) = log_err.lock() {
                        v.push(format!("[ERROR] 出力フォルダを作成できません: {e}"));
                    }
                    busy.store(false, Ordering::Relaxed);
                    ctx2.request_repaint();
                    return;
                }
                let total = inputs.len();
                let mut ok = 0usize;
                for path in &inputs {
                    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    let output = out_dir.join(format!("{stem}.json"));
                    let res = crate::tasks::run_to_json(ToJsonParams {
                        input: path.to_string_lossy().into_owned(),
                        output,
                        provider, api_key: api_key.clone(), model: model.clone(),
                        quality, lang, base_url: None, enrich,
                    }, Arc::clone(&log_fn)).await;
                    match res {
                        Ok(_)  => ok += 1,
                        Err(e) => { if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); } }
                    }
                }
                if let Ok(mut v) = log_err.lock() {
                    v.push(format!("[DONE] {ok}/{total} converted"));
                }
                busy.store(false, Ordering::Relaxed);
                ctx2.request_repaint();
            });
        } else {
            // ----- Single mode -----
            let input  = self.conv_input.trim().to_string();
            let output = PathBuf::from(self.conv_output.trim());
            if input.is_empty() {
                self.log_push(self.s().err_no_input);
                busy.store(false, Ordering::Relaxed);
                return;
            }
            if output.as_os_str().is_empty() {
                self.log_push("[ERROR] 出力パスを指定してください。");
                busy.store(false, Ordering::Relaxed);
                return;
            }
            self.log_push(format!("[START] {} → {}", input, output.display()));

            self.rt.spawn(async move {
                if let Err(e) = crate::tasks::run_to_json(ToJsonParams {
                    input, output, provider, api_key, model, quality, lang, base_url: None, enrich,
                }, log_fn).await {
                    if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); }
                }
                busy.store(false, Ordering::Relaxed);
                ctx2.request_repaint();
            });
        }
    }

    // -----------------------------------------------------------------------
    // Generate tab
    // -----------------------------------------------------------------------

    fn ui_generate_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_generate);
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(s.lbl_input_json);
            ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.gen_input));
            if ui.button(s.btn_browse).clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                    self.gen_input = path.to_string_lossy().into_owned();
                    if self.gen_output.is_empty() {
                        let ext = match self.gen_format {
                            GenFormat::Docx => "docx", GenFormat::Html => "html", GenFormat::Pdf => "pdf",
                        };
                        if let Some(stem) = path.file_stem() {
                            let mut out = path.clone();
                            out.set_file_name(format!("{}.{ext}", stem.to_string_lossy()));
                            self.gen_output = out.to_string_lossy().into_owned();
                        }
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(s.lbl_output_file);
            ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.gen_output));
            if ui.button(s.btn_save_to).clicked() {
                let (desc, exts): (&str, Vec<&str>) = match self.gen_format {
                    GenFormat::Docx => ("Word", vec!["docx"]),
                    GenFormat::Html => ("HTML", vec!["html"]),
                    GenFormat::Pdf  => ("PDF",  vec!["pdf"]),
                };
                if let Some(p) = rfd::FileDialog::new().add_filter(desc, &exts).save_file() {
                    self.gen_output = p.to_string_lossy().into_owned();
                }
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(s.lbl_format);
            ui.selectable_value(&mut self.gen_format, GenFormat::Docx, "DOCX");
            ui.selectable_value(&mut self.gen_format, GenFormat::Html, "HTML");
            ui.selectable_value(&mut self.gen_format, GenFormat::Pdf,  "PDF");
            ui.add_space(12.0);
            ui.label(s.lbl_lang);
            lang_combo(ui, "gen_lang", &mut self.gen_lang);
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_generating } else { s.btn_generate };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_generate(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });
    }

    fn start_generate(&mut self, ctx: &egui::Context) {
        if self.gen_input.is_empty() {
            self.log_push(self.s().err_no_input);
            return;
        }
        let input  = PathBuf::from(self.gen_input.trim());
        let output = PathBuf::from(self.gen_output.trim());
        let lang   = lang_from_str(&self.gen_lang).unwrap_or(sds_converter_core::Language::Japanese);
        let format = self.gen_format;

        let log_fn  = self.make_log_fn();
        let log_err = Arc::clone(&self.log);
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        busy.store(true, Ordering::Relaxed);
        self.log_push(format!("[START] {} → {}", input.display(), output.display()));

        self.rt.spawn(async move {
            let result = match format {
                GenFormat::Docx => crate::tasks::run_to_docx(
                    ToDocxParams { input, output, lang, template: None }, log_fn).await,
                GenFormat::Html => crate::tasks::run_to_html(
                    ToHtmlParams { input, output, lang }, log_fn).await,
                GenFormat::Pdf  => crate::tasks::run_to_pdf(
                    ToPdfParams { input, output, lang }, log_fn).await,
            };
            if let Err(e) = result {
                if let Ok(mut v) = log_err.lock() { v.push(format!("[ERROR] {e}")); }
            }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    // -----------------------------------------------------------------------
    // Validate tab
    // -----------------------------------------------------------------------

    fn ui_validate_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let s = self.s();
        ui.heading(s.heading_validate);
        ui.add_space(6.0);

        let batch = !self.val_inputs.is_empty();

        if batch {
            ui.horizontal(|ui| {
                ui.label(format!("{} {}", self.val_inputs.len(), s.lbl_files));
                if ui.small_button(s.btn_clear_files).clicked() {
                    self.val_inputs.clear();
                    self.val_results.clear();
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(s.lbl_input_json);
                ui.add_sized([280.0, 20.0], egui::TextEdit::singleline(&mut self.val_input));
                if ui.button(s.btn_browse).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                        self.val_input = path.to_string_lossy().into_owned();
                        self.val_results.clear();
                    }
                }
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(s.btn_browse_multi).clicked() {
                if let Some(paths) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_files() {
                    self.val_inputs = paths;
                    self.val_results.clear();
                }
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let label = if self.is_busy() { s.btn_validating } else { s.btn_validate };
            if ui.add_enabled(!self.is_busy(), egui::Button::new(label)).clicked() {
                self.start_validate(ctx);
            }
            if self.is_busy() { ui.spinner(); }
        });

        if !self.val_results.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for w in &self.val_results {
                    let color = if w.starts_with("OK") || w.starts_with("[OK]") {
                        egui::Color32::GREEN
                    } else if w.starts_with("[ERROR]") {
                        egui::Color32::RED
                    } else {
                        egui::Color32::YELLOW
                    };
                    ui.colored_label(color, w);
                }
            });
        }
    }

    fn start_validate(&mut self, ctx: &egui::Context) {
        self.val_results.clear();
        if let Ok(mut slot) = self.val_pending.lock() { *slot = None; }
        let log_fn  = self.make_log_fn();
        let busy    = Arc::clone(&self.busy);
        let ctx2    = ctx.clone();
        let pending = Arc::clone(&self.val_pending);
        busy.store(true, Ordering::Relaxed);
        let s = self.s();

        let inputs: Vec<PathBuf> = if !self.val_inputs.is_empty() {
            self.val_inputs.clone()
        } else if !self.val_input.is_empty() {
            vec![PathBuf::from(self.val_input.trim())]
        } else {
            self.log_push(s.err_no_input);
            busy.store(false, Ordering::Relaxed);
            return;
        };

        self.rt.spawn(async move {
            let mut all_results: Vec<String> = Vec::new();
            for path in &inputs {
                let prefix = if inputs.len() > 1 {
                    format!("[{}] ", path.file_name().unwrap_or_default().to_string_lossy())
                } else {
                    String::new()
                };
                match crate::tasks::run_validate(path.clone(), Arc::clone(&log_fn)).await {
                    Ok(warnings) if warnings.is_empty() => {
                        all_results.push(format!("{}OK: 問題なし", prefix));
                    }
                    Ok(warnings) => {
                        for w in warnings { all_results.push(format!("{prefix}{w}")); }
                    }
                    Err(e) => {
                        all_results.push(format!("{prefix}[ERROR] {e}"));
                    }
                }
            }
            if let Ok(mut slot) = pending.lock() { *slot = Some(all_results); }
            busy.store(false, Ordering::Relaxed);
            ctx2.request_repaint();
        });
    }

    // -----------------------------------------------------------------------
    // Settings tab
    // -----------------------------------------------------------------------

    fn ui_settings_tab(&mut self, ui: &mut egui::Ui) {
        let s = self.s();
        ui.heading(s.heading_settings);
        ui.add_space(6.0);

        egui::Grid::new("settings_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(s.lbl_def_provider);
            egui::ComboBox::from_id_salt("settings_provider")
                .selected_text(&self.config.provider)
                .width(130.0)
                .show_ui(ui, |ui| {
                    for &p in Provider::all() {
                        ui.selectable_value(&mut self.config.provider, p.to_string(), p);
                    }
                });
            ui.end_row();

            ui.label(s.lbl_def_lang);
            lang_combo(ui, "settings_lang", &mut self.config.language);
            ui.end_row();

            ui.label(s.lbl_def_quality);
            egui::ComboBox::from_id_salt("settings_quality")
                .selected_text(&self.config.quality)
                .width(85.0)
                .show_ui(ui, |ui| {
                    for &q in Quality::all() {
                        ui.selectable_value(&mut self.config.quality, q.to_string(), q);
                    }
                });
            ui.end_row();

            ui.label(s.lbl_api_key);
            ui.add(egui::TextEdit::singleline(&mut self.config.api_key)
                .password(true).desired_width(240.0));
            ui.end_row();

            ui.label(s.lbl_ui_lang);
            let ui_langs = [("ja", "日本語"), ("en", "English"), ("zh-cn", "简体中文")];
            let cur_label = ui_langs.iter().find(|(k, _)| *k == self.config.ui_lang.as_str())
                .map(|(_, v)| *v).unwrap_or("日本語");
            egui::ComboBox::from_id_salt("ui_lang")
                .selected_text(cur_label)
                .width(120.0)
                .show_ui(ui, |ui| {
                    for (k, v) in ui_langs {
                        ui.selectable_value(&mut self.config.ui_lang, k.to_string(), v);
                    }
                });
            ui.end_row();

            ui.label(s.lbl_def_enrich);
            ui.checkbox(&mut self.config.enrich, "");
            ui.end_row();
        });

        ui.add_space(4.0);
        ui.colored_label(egui::Color32::YELLOW, s.msg_api_key_warn);
        ui.add_space(8.0);

        if ui.button(s.btn_save).clicked() {
            match self.config.save() {
                Ok(_)  => {
                    self.conv_enrich = self.config.enrich;
                    self.settings_saved_msg = Some(s.msg_saved.to_string());
                }
                Err(e) => self.settings_saved_msg = Some(format!("Error: {e}")),
            }
        }
        if let Some(msg) = &self.settings_saved_msg {
            ui.label(msg);
        }
    }
}

// ---------------------------------------------------------------------------
// eframe::App impl
// ---------------------------------------------------------------------------

impl eframe::App for SdsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Repaint while busy
        if self.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // Drain async validate results
        if let Ok(mut slot) = self.val_pending.try_lock() {
            if let Some(results) = slot.take() {
                self.val_results = results;
            }
        }

        let s = self.s();

        // --- Menu bar ---
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(s.menu_file, |ui| {
                    if ui.button(s.menu_quit).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button(s.menu_help, |ui| {
                    if ui.button(s.menu_manual).clicked() {
                        self.show_manual = true;
                        ui.close_menu();
                    }
                    if ui.button(s.menu_about).clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // --- Tab bar ---
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Convert,  s.tab_convert);
                ui.selectable_value(&mut self.tab, Tab::Generate, s.tab_generate);
                ui.selectable_value(&mut self.tab, Tab::Validate, s.tab_validate);
                ui.selectable_value(&mut self.tab, Tab::Settings, s.tab_settings);
            });
        });

        // --- Log panel ---
        egui::TopBottomPanel::bottom("log_panel").resizable(true).min_height(60.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(s.lbl_log);
                if ui.small_button(s.btn_clear).clicked() {
                    if let Ok(mut v) = self.log.lock() { v.clear(); }
                }
            });
            ui.separator();
            let lines = self.log.lock().map(|v| v.clone()).unwrap_or_default();
            egui::ScrollArea::vertical().stick_to_bottom(true).max_height(160.0).show(ui, |ui| {
                for line in &lines {
                    let color = if line.starts_with("[ERROR]") { egui::Color32::RED }
                        else if line.starts_with("WARN") || line.starts_with("CAS:") { egui::Color32::YELLOW }
                        else if line.starts_with("[OK]") || line.starts_with("Saved") || line.starts_with("OK") || line.starts_with("[DONE]") { egui::Color32::GREEN }
                        else { ui.visuals().text_color() };
                    ui.colored_label(color, line);
                }
            });
        });

        // --- Main content ---
        egui::CentralPanel::default().show(ctx, |ui| {
            let ctx2 = ctx.clone();
            match self.tab {
                Tab::Convert  => self.ui_convert_tab(ui, &ctx2),
                Tab::Generate => self.ui_generate_tab(ui, &ctx2),
                Tab::Validate => self.ui_validate_tab(ui, &ctx2),
                Tab::Settings => self.ui_settings_tab(ui),
            }
        });

        // --- About dialog ---
        if self.show_about {
            let s = self.s();
            egui::Window::new(s.about_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(s.about_body);
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() { self.show_about = false; }
                });
        }

        // --- Manual window ---
        if self.show_manual {
            let s = self.s();
            egui::Window::new(s.manual_title)
                .collapsible(false)
                .resizable(true)
                .default_size([520.0, 420.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(s.manual_body);
                    });
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() { self.show_manual = false; }
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lang_combo(ui: &mut egui::Ui, id: &str, value: &mut String) {
    let langs = [("ja", "日本語"), ("en", "English"), ("zh-cn", "简体中文"), ("zh-tw", "繁體中文")];
    let label = langs.iter().find(|(k, _)| *k == value.as_str()).map(|(_, v)| *v).unwrap_or("日本語");
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(110.0)
        .show_ui(ui, |ui| {
            for (k, v) in langs {
                ui.selectable_value(value, k.to_string(), v);
            }
        });
}

fn lang_from_str(s: &str) -> Option<sds_converter_core::Language> {
    match s {
        "ja"    => Some(sds_converter_core::Language::Japanese),
        "en"    => Some(sds_converter_core::Language::English),
        "zh-cn" => Some(sds_converter_core::Language::ChineseSimplified),
        "zh-tw" => Some(sds_converter_core::Language::ChineseTraditional),
        _       => None,
    }
}

// ---------------------------------------------------------------------------
// Font + launch
// ---------------------------------------------------------------------------

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ];
    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &[
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/msgothic.ttc",
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let candidates: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
    ];

    for path in candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("jp_font".to_owned(), egui::FontData::from_owned(data));
            // Primary font: insert at position 0 so Latin and CJK share baseline metrics
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().insert(0, "jp_font".to_owned());
            }
            break;
        }
    }

    ctx.set_fonts(fonts);
}

pub fn run_gui() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("sds-converter")
            .with_inner_size([760.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
        "sds-converter",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(SdsApp::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {e}"))
}
