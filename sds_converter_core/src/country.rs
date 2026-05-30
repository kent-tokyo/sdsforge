use crate::language::Language;

/// Country/regulatory-region that governs the SDS requirements.
///
/// Used to select country-specific extraction rules, validation checks,
/// and compliance-gap reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCountry {
    Japan,
    China,
    Taiwan,
    Korea,
}

impl SourceCountry {
    /// Infer the most likely country from a detected language.
    ///
    /// English is ambiguous (US, EU, AU, …) and returns `None`.
    /// The caller may override this inference with an explicit `--country` flag.
    pub fn infer_from_language(lang: Language) -> Option<Self> {
        match lang {
            Language::Japanese => Some(Self::Japan),
            Language::ChineseSimplified => Some(Self::China),
            Language::ChineseTraditional => Some(Self::Taiwan),
            Language::English => None,
        }
    }

    /// Short English name used in log messages and report filenames.
    pub fn name_en(self) -> &'static str {
        match self {
            Self::Japan  => "Japan",
            Self::China  => "China",
            Self::Taiwan => "Taiwan",
            Self::Korea  => "Korea",
        }
    }

    /// Primary regulatory standard governing SDS content for this country.
    pub fn regulatory_standard(self) -> &'static str {
        match self {
            Self::Japan  => "JIS Z 7253 / MHLW",
            Self::China  => "GB/T 16483-2008 / GB/T 17519-2013",
            Self::Taiwan => "CNS 15030",
            Self::Korea  => "K-GHS Rev.6 (산업안전보건법)",
        }
    }

    /// Lowercase ASCII slug used in `_compliance_<slug>.json` filenames.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Japan  => "jp",
            Self::China  => "cn",
            Self::Taiwan => "tw",
            Self::Korea  => "kr",
        }
    }
}
