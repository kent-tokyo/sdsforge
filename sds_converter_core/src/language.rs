/// Document language for SDS extraction and output generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// Japanese (日本語) — default
    #[default]
    Japanese,
    /// English
    English,
    /// Simplified Chinese (简体中文)
    ChineseSimplified,
    /// Traditional Chinese (繁體中文)
    ChineseTraditional,
}

impl Language {
    /// BCP-47 language tag.
    pub fn bcp47(&self) -> &'static str {
        match self {
            Self::Japanese => "ja",
            Self::English => "en",
            Self::ChineseSimplified => "zh-CN",
            Self::ChineseTraditional => "zh-TW",
        }
    }

    /// Human-readable name of the language in English.
    pub fn name_en(&self) -> &'static str {
        match self {
            Self::Japanese => "Japanese",
            Self::English => "English",
            Self::ChineseSimplified => "Simplified Chinese",
            Self::ChineseTraditional => "Traditional Chinese",
        }
    }
}
