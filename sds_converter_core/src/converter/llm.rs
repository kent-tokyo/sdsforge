use reqwest::Client;
use serde_json::Value;

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

// ---------------------------------------------------------------------------
// LlmBackend trait (rig-inspired, stable async fn in trait since Rust 1.75)
// ---------------------------------------------------------------------------

/// Abstraction over LLM completion providers.
///
/// Implement this trait to use any LLM backend with `sds-converter-core`.
/// The library ships with [`AnthropicBackend`] and [`OpenAiCompatBackend`].
pub trait LlmBackend {
    /// Send a system + user message pair and return the raw text response.
    fn complete(
        &self,
        system: &str,
        user: &str,
    ) -> impl std::future::Future<Output = Result<String, SdsError>> + Send;
}

// ---------------------------------------------------------------------------
// Built-in Anthropic backend
// ---------------------------------------------------------------------------

const DEFAULT_MODEL: &str = "claude-sonnet-4-6";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// LLM completion configuration.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub model: String,
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            max_tokens: 8192,
        }
    }
}

/// Anthropic Claude API backend.
///
/// # Example
/// ```no_run
/// use sds_converter_core::converter::llm::{AnthropicBackend, LlmConfig};
/// let backend = AnthropicBackend::new("sk-ant-...", LlmConfig::default());
/// ```
pub struct AnthropicBackend {
    client: Client,
    api_key: String,
    config: LlmConfig,
}

impl AnthropicBackend {
    pub fn new(api_key: impl Into<String>, config: LlmConfig) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            config,
        }
    }

    /// Create from the `ANTHROPIC_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, SdsError> {
        let key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| SdsError::Config("ANTHROPIC_API_KEY environment variable not set".into()))?;
        Ok(Self::new(key, LlmConfig::default()))
    }
}

impl LlmBackend for AnthropicBackend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "system": system,
            "messages": [
                {"role": "user", "content": user}
            ]
        });

        let response = self.client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(SdsError::LlmApi { status, message });
        }

        let resp: Value = response.json().await?;
        let text = resp["content"][0]["text"]
            .as_str()
            .ok_or_else(|| SdsError::LlmParse("missing content[0].text".to_string()))?;

        Ok(strip_code_fences(text))
    }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible backend (works with OpenAI GPT, Google Gemini, etc.)
// ---------------------------------------------------------------------------

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const GEMINI_OPENAI_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";

/// Backend for any OpenAI-compatible chat completions API (OpenAI GPT, Google Gemini, etc.).
///
/// # Example — OpenAI GPT
/// ```no_run
/// use sds_converter_core::converter::llm::{OpenAiCompatBackend, LlmConfig};
/// let config = LlmConfig { model: "gpt-4o".into(), max_tokens: 8192 };
/// let backend = OpenAiCompatBackend::openai("sk-...", config);
/// ```
///
/// # Example — Google Gemini
/// ```no_run
/// use sds_converter_core::converter::llm::{OpenAiCompatBackend, LlmConfig};
/// let config = LlmConfig { model: "gemini-2.0-flash".into(), max_tokens: 8192 };
/// let backend = OpenAiCompatBackend::gemini("AIza...", config);
/// ```
pub struct OpenAiCompatBackend {
    client: Client,
    api_key: String,
    config: LlmConfig,
    base_url: String,
}

impl OpenAiCompatBackend {
    pub fn new(api_key: impl Into<String>, config: LlmConfig, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            config,
            base_url: base_url.into(),
        }
    }

    /// OpenAI GPT backend (api.openai.com).
    pub fn openai(api_key: impl Into<String>, config: LlmConfig) -> Self {
        Self::new(api_key, config, OPENAI_API_URL)
    }

    /// Google Gemini backend via OpenAI-compatible endpoint.
    pub fn gemini(api_key: impl Into<String>, config: LlmConfig) -> Self {
        Self::new(api_key, config, GEMINI_OPENAI_URL)
    }

    /// Create OpenAI backend from the `OPENAI_API_KEY` environment variable.
    pub fn openai_from_env() -> Result<Self, SdsError> {
        let key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| SdsError::Config("OPENAI_API_KEY environment variable not set".into()))?;
        Ok(Self::openai(key, LlmConfig { model: "gpt-4o".into(), max_tokens: 8192 }))
    }

    /// Create Gemini backend from the `GEMINI_API_KEY` environment variable.
    pub fn gemini_from_env() -> Result<Self, SdsError> {
        let key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| SdsError::Config("GEMINI_API_KEY environment variable not set".into()))?;
        Ok(Self::gemini(key, LlmConfig { model: "gemini-2.0-flash".into(), max_tokens: 8192 }))
    }
}

impl LlmBackend for OpenAiCompatBackend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        });

        let response = self.client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(SdsError::LlmApi { status, message });
        }

        let resp: Value = response.json().await?;
        let text = resp["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| SdsError::LlmParse("missing choices[0].message.content".to_string()))?;

        Ok(strip_code_fences(text))
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn strip_code_fences(text: &str) -> String {
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text);
    text.strip_suffix("```").unwrap_or(text).trim().to_string()
}

// ---------------------------------------------------------------------------
// SDS extraction
// ---------------------------------------------------------------------------

const MHLW_SCHEMA_HINT: &str = r#"Output a JSON object. CRITICAL: Use EXACTLY these key names — they must match the MHLW schema precisely.
{
  "Datasheet": { "IssueDate": "YYYY-MM-DD", "SDS-SchemaVersionNo": "1.0" },
  "Identification": {
    "TradeProductIdentity": { "TradeNameJP": "...", "TradeNameEN": "..." },
    "SupplierInformation": { "CompanyName": "...", "Phone": "...", "Address": "..." },
    "RecommendedUseAndRestrictions": "..."
  },
  "HazardIdentification": {
    "Classification": { "FlammableLiquids": "区分2" },
    "HazardLabelling": {
      "SignalWord": "危険",
      "HazardStatement": [{ "HazardStatementCode": "H225", "FullText": "引火性の高い液体および蒸気" }],
      "PrecautionaryStatements": {
        "Prevention": [{ "PrecautionaryStatementCode": "P210", "FullText": "熱から遠ざけること。" }],
        "Response": [{ "PrecautionaryStatementCode": "P370+P378", "FullText": "火災の場合：消火に適切な手段を用いること。" }],
        "Storage": [{ "PrecautionaryStatementCode": "P403+P235", "FullText": "換気の良い場所で保管すること。涼しい場所に置くこと。" }],
        "Disposal": [{ "PrecautionaryStatementCode": "P501", "FullText": "内容物・容器を法規に従って廃棄すること。" }]
      }
    }
  },
  "Composition": {
    "CompositionType": "単一物質",
    "CompositionAndConcentration": [{
      "SubstanceIdentifiers": {
        "SubstanceNames": { "IupacName": "エタノール", "GenericName": "エチルアルコール" },
        "SubstanceIdentity": { "CASno": { "FullText": ["64-17-5"] } }
      },
      "Concentration": { "NumericRangeWithUnitAndQualifier": { "ExactValue": { "Value": 99.5 }, "Unit": "%" } }
    }]
  },
  "FirstAidMeasures": {
    "ExposureRoute": {
      "FirstAidInhalation": { "FullText": "新鮮な空気の場所に移動させ安静を保つ。" },
      "FirstAidSkin": { "FullText": "石鹸と水で洗い流す。" },
      "FirstAidEye": { "FullText": "水で十分に洗い流す。" },
      "FirstAidIngestion": { "FullText": "口をすすぐ。多量に飲み込んだ場合は医師に連絡する。" }
    }
  },
  "FireFightingMeasures": {
    "MediaToBeUsed": { "FullText": "粉末消火剤、二酸化炭素、泡、水噴霧" },
    "FireAndExplosionHazards": { "FullText": "..." },
    "FireFightingProcedures": { "FullText": "..." },
    "SpecialProtectiveEquipmentForFirefighters": { "FullText": "..." }
  },
  "AccidentalReleaseMeasures": {
    "HumanExposureAndEmergencyMeasuress": { "FullText": "..." },
    "EnvironmentalPrecautions": { "FullText": "..." },
    "ContainmentAndCleaningUp": { "FullText": "..." }
  },
  "HandlingAndStorage": {
    "SafeHandling": {
      "HandlingPrecautions": "火気厳禁。換気の良い場所で取り扱う。",
      "TechnicalMeasuresAndStorageConditions": { "FullText": "..." }
    },
    "Storage": {
      "ConditionsForSafeStorage": { "TechnicalMeasuresAndStorageConditions": "冷暗所に保管する。容器を密閉する。" }
    }
  },
  "ExposureControlPersonalProtection": {
    "AppropriateEngineeringControls": ["局所排気装置を設置する。"],
    "PersonalProtectionEquipment": {
      "EyeProtection": [{ "FullText": "保護眼鏡を着用する。" }],
      "SkinProtection": [{ "FullText": "保護手袋を着用する。" }],
      "RespiratoryProtection": [{ "FullText": "蒸気が高濃度の場合は有機ガス用防毒マスクを着用する。" }],
      "HandProtection": [{ "FullText": "保護手袋を着用する。" }]
    }
  },
  "PhysicalChemicalProperties": {
    "BasePhysicalChemicalProperties": { "PhysicalState": "液体", "Colour": "無色", "Odour": "特異臭" },
    "FlashPoint": [{ "NumericRangeWithUnitAndQualifier": { "ExactValue": { "Value": 13.0 }, "Unit": "°C" } }],
    "BoilingPointRelated": [{ "ItemName": "沸点", "NumericRangeWithUnitAndQualifier": { "ExactValue": { "Value": 78.4 }, "Unit": "°C" } }]
  },
  "StabilityReactivity": {
    "ReactivityDescription": "反応性についての説明",
    "StabilityDescription": "通常の保管条件で安定",
    "HazardousReactions": { "FullText": "強酸化剤と反応する。" },
    "ConditionsToAvoid": "熱、火花、炎",
    "MaterialsToAvoid": "強酸化剤",
    "HazardousDecompositionProducts": { "Substance": "炭素酸化物" }
  },
  "ToxicologicalInformation": [{
    "AcuteToxicity": { "ExposureRoute": [{ "RouteOfExposure": "経口", "FullText": "LD50 ラット 経口 7060 mg/kg" }] },
    "SkinCorrosionIrritation": { "TestResult": [{ "FullText": "軽度の皮膚刺激性" }] },
    "EyeDamageOrIrritation": { "TestResult": [{ "FullText": "軽度の眼刺激性" }] },
    "AdditionalToxicologicalInformation": "その他の毒性情報"
  }],
  "EcologicalInformation": [{
    "EcotoxicologicalInformation": { "AquaticToxicity": [{ "FullText": "..." }] },
    "PersistenceDegradability": { "FullText": "生分解性あり" },
    "AdditionalEcotoxInformation": "その他の生態影響情報"
  }],
  "DisposalConsiderations": {
    "FullText": "廃棄物は関係法令に従って処理する。",
    "ProductWaste": ["廃棄物処理法に従い処理する"],
    "PackagingWaste": ["容器はリサイクルまたは適切に廃棄する"]
  },
  "TransportInformation": {
    "InternationalRegulations": [{
      "RegulationName": [{ "TransportationType": "海上輸送(IMDG)", "FullText": "国連番号：UN1170 品名：ETHANOL" }]
    }],
    "DomesticRegulations": [{ "TransportationType": "陸上輸送", "FullText": "消防法：第4類 アルコール類" }]
  },
  "RegulatoryInformation": {
    "OtherLegislation": {
      "Legislation": [{ "LegislationName": "労働安全衛生法", "Regulations": [{ "FullText": "..." }] }]
    }
  },
  "OtherInformation": {
    "RelatedDocuments": "参考資料リスト",
    "RevisionInformation": [{ "LastUpdateDate": "YYYY-MM-DD", "FullText": "改訂内容の説明" }],
    "Desclaimer": "免責事項テキスト"
  }
}"#;

/// Build the system prompt for SDS extraction in the given document language.
fn build_system_prompt(lang: Option<Language>) -> String {
    let lang_hint = match lang {
        Some(l) => format!(
            "The source document is written in {} ({}).\n",
            l.name_en(),
            l.bcp47()
        ),
        None => "The source document may be in Japanese, English, Simplified Chinese, or Traditional Chinese — detect the language automatically.\n".to_string(),
    };

    format!(
        "You are an expert in extracting Safety Data Sheet (SDS) information.\n\
         {lang_hint}\
         Read the document text and output all SDS information as a JSON object conforming to the \
         Japanese Ministry of Health, Labour and Welfare (MHLW) SDS data exchange format v1.0.\n\
         Rules:\n\
         - Output raw JSON only — no markdown, no code fences, no explanation\n\
         - Omit keys that have no information (empty strings and null are forbidden)\n\
         - Dates in YYYY-MM-DD format\n\
         - Numeric values as numeric types (not strings) inside NumericRangeWithUnitAndQualifier\n\
         - Reproduce text exactly as written in the source document; do not infer or fill in missing data\n\
         - JSON keys must match EXACTLY the key names shown in the schema example below\n\
         \nSchema example (use these EXACT key names):\n{MHLW_SCHEMA_HINT}"
    )
}

/// Extract SDS data from document text using the provided LLM backend.
pub async fn extract_sds_from_text<B: LlmBackend>(
    backend: &B,
    text: &str,
    source_language: Option<Language>,
) -> Result<SdsRoot, SdsError> {
    let system = build_system_prompt(source_language);

    let lang_instruction = match source_language {
        Some(l) => format!("This document is in {}. ", l.name_en()),
        None => String::new(),
    };
    let user = format!(
        "{lang_instruction}Extract all SDS information from the following document text and output as JSON:\n\n{text}"
    );

    let json_str = backend.complete(&system, &user).await?;
    tracing::debug!("Raw LLM JSON output:\n{}", json_str);
    lenient_deserialize(&json_str)
}

/// Deserialize LLM JSON output section-by-section, skipping sections with type errors.
///
/// The MHLW schema is deeply nested (~200 structs). If the LLM outputs a valid top-level
/// JSON object but gets a subsection's structure wrong, this lets us still return all
/// correctly-structured sections rather than failing entirely.
fn lenient_deserialize(json_str: &str) -> Result<SdsRoot, SdsError> {
    use crate::schema::*;
    use tracing::warn;

    let val: Value = serde_json::from_str(json_str).map_err(|e| {
        let preview: String = json_str.chars().take(500).collect();
        SdsError::LlmParse(format!("Invalid JSON: {e}\nRaw (first 500 chars): {preview}"))
    })?;

    let obj = val
        .as_object()
        .ok_or_else(|| SdsError::LlmParse("LLM output is not a JSON object".into()))?;

    macro_rules! section {
        ($key:literal, $type:ty) => {
            obj.get($key).and_then(|v| {
                serde_json::from_value::<$type>(v.clone())
                    .map_err(|e| warn!("Section '{}' skipped (schema mismatch): {}", $key, e))
                    .ok()
            })
        };
    }

    Ok(SdsRoot {
        datasheet: section!("Datasheet", Datasheet),
        identification: section!("Identification", Identification),
        hazard_identification: section!("HazardIdentification", HazardIdentification),
        composition: section!("Composition", Composition),
        first_aid_measures: section!("FirstAidMeasures", FirstAidMeasures),
        fire_fighting_measures: section!("FireFightingMeasures", FireFightingMeasures),
        accidental_release_measures: section!("AccidentalReleaseMeasures", AccidentalReleaseMeasures),
        handling_and_storage: section!("HandlingAndStorage", HandlingAndStorage),
        exposure_control_personal_protection: section!(
            "ExposureControlPersonalProtection",
            ExposureControlPersonalProtection
        ),
        physical_chemical_properties: section!("PhysicalChemicalProperties", PhysicalChemicalProperties),
        stability_reactivity: section!("StabilityReactivity", StabilityReactivity),
        toxicological_information: section!("ToxicologicalInformation", Vec<ToxicologicalInformation>),
        ecological_information: section!("EcologicalInformation", Vec<EcologicalInformation>),
        disposal_considerations: section!("DisposalConsiderations", DisposalConsiderations),
        transport_information: section!("TransportInformation", TransportInformation),
        regulatory_information: section!("RegulatoryInformation", RegulatoryInformation),
        other_information: section!("OtherInformation", OtherInformation),
    })
}
