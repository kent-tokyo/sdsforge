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

/// Heuristically detect the language of an SDS document from its extracted text.
///
/// Detection order:
/// 0. Fewer than 30 non-whitespace characters → [`Language::Japanese`] (default, not enough text)
/// 1. Hiragana or katakana present → [`Language::Japanese`]
/// 2. No CJK ideographs AND text is substantially Latin (≥ 30 % of meaningful chars are
///    ASCII printable a-z/A-Z/0-9) → [`Language::English`]
///    This avoids misclassifying Japanese PDFs whose pdftotext output contains only garbage
///    ASCII (e.g. garbled CID/Shift-JIS font metrics) as English.
/// 3. Fewer than 20 CJK characters → [`Language::Japanese`] (not enough CJK to distinguish)
/// 4. Traditional-Chinese-only characters outnumber simplified-only → [`Language::ChineseTraditional`]
/// 5. Otherwise → [`Language::ChineseSimplified`]
///
/// Works on as little as ~200 characters of text. No LLM or network call required.
pub fn detect_language(text: &str) -> Language {
    // Not enough text to analyse reliably (e.g. image-only or encrypted PDF returned empty).
    // Fall back to the default language (Japanese) rather than incorrectly guessing English.
    let meaningful_chars = text.chars().filter(|c| !c.is_whitespace()).count();
    if meaningful_chars < 30 {
        return Language::default();
    }

    // Hiragana (あ…ん) and katakana (ア…ン) are unique to Japanese.
    let japanese_kana = text
        .chars()
        .filter(|&c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'))
        .count();
    if japanese_kana > 5 {
        return Language::Japanese;
    }

    // Count CJK unified ideographs.
    let cjk_total = text
        .chars()
        .filter(|&c| matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'))
        .count();

    // If there are no CJK ideographs, check whether the text is substantially Latin
    // before concluding it is English.
    //
    // Rationale: pdftotext applied to CID/Shift-JIS Japanese PDFs can produce garbled ASCII
    // (font metric codes, fallback glyphs, etc.) that contains no kana and no CJK characters.
    // Without this guard, `detect_language` would incorrectly return English for those PDFs.
    //
    // We require ≥ 30 % of meaningful chars to be basic Latin letters/digits to call English.
    // For real English SDS documents this threshold is easily exceeded (typically > 80 %).
    // For garbled CID font output the "text" is mostly punctuation/specials, not letters/digits.
    if cjk_total < 20 {
        let latin_alphanum = text
            .chars()
            .filter(|c| c.is_ascii_alphabetic() || c.is_ascii_digit())
            .count();
        // Need ≥ 30 % of meaningful chars to be ASCII alphanumeric to call English.
        if latin_alphanum * 100 >= meaningful_chars * 30 {
            return Language::English;
        } else {
            // Not enough Latin signal — garbled CID font output or near-empty text.
            return Language::default(); // Japanese
        }
    }

    // Distinguish Simplified vs Traditional Chinese by counting characters that diverge
    // between the two writing systems.  Each entry is a Simplified char whose Traditional
    // counterpart is the corresponding entry in TRADITIONAL_MARKERS (same index).
    const SIMPLIFIED_MARKERS: &[char] = &[
        '国', '语', '时', '书', '来', '这', '过', '东', '样', '从',
        '实', '动', '产', '问', '给', '长', '发', '规', '药', '标',
        '剂', '险', '质', '现', '处', '须', '经', '联', '则', '级',
        '为', '与', '对', '气', '无', '变', '数', '间', '应', '关',
    ];
    const TRADITIONAL_MARKERS: &[char] = &[
        '國', '語', '時', '書', '來', '這', '過', '東', '樣', '從',
        '實', '動', '產', '問', '給', '長', '發', '規', '藥', '標',
        '劑', '險', '質', '現', '處', '須', '經', '聯', '則', '級',
        '為', '與', '對', '氣', '無', '變', '數', '間', '應', '關',
    ];

    let simplified_score = text.chars().filter(|c| SIMPLIFIED_MARKERS.contains(c)).count();
    let traditional_score = text.chars().filter(|c| TRADITIONAL_MARKERS.contains(c)).count();

    if traditional_score > simplified_score {
        Language::ChineseTraditional
    } else {
        Language::ChineseSimplified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_kana_detected() {
        let text = "安全データシート\n製品名：テストシリカ\nSection 1 化学品及び会社情報";
        assert_eq!(detect_language(text), Language::Japanese);
    }

    #[test]
    fn english_sds_detected() {
        let text = "Safety Data Sheet\nSection 1: Identification\nProduct name: Test Chemical\n\
                    Supplier: ABC Corp, 123 Industrial Blvd, City, Country\n\
                    Emergency telephone: +1-800-555-1234";
        assert_eq!(detect_language(text), Language::English);
    }

    #[test]
    fn simplified_chinese_detected() {
        let text = "安全技术说明书\n产品名称：测试化学品\n公司名称：某某化工有限公司\n\
                    危险性概述：本产品为易燃液体，可能导致皮肤刺激。";
        assert_eq!(detect_language(text), Language::ChineseSimplified);
    }

    #[test]
    fn traditional_chinese_detected() {
        let text = "安全資料表\n產品名稱：測試化學品\n公司名稱：某某化工有限公司\n\
                    危害辨識資料：本產品為易燃液體，可能導致皮膚刺激。";
        assert_eq!(detect_language(text), Language::ChineseTraditional);
    }

    /// Garbled CID/Shift-JIS font output from pdftotext should NOT be classified as English.
    /// Such output typically contains lots of punctuation / special chars but few a-z letters.
    #[test]
    fn garbled_cid_font_output_defaults_to_japanese() {
        // Simulate pdftotext output for a Shift-JIS PDF where only font metrics survive:
        // lots of punctuation/brackets but no meaningful Latin words or CJK characters.
        let garbled = "(.)(.)(.)(.)(.)(.)(.)[][][][][]{}{}{}^^^***///\\\\\\###@@@";
        // 30+ non-whitespace chars, no kana, no CJK, but also < 30% alphabetic
        assert_eq!(
            detect_language(garbled),
            Language::Japanese,
            "garbled CID font output should default to Japanese"
        );
    }

    #[test]
    fn empty_text_defaults_to_japanese() {
        assert_eq!(detect_language(""), Language::Japanese);
        assert_eq!(detect_language("   "), Language::Japanese);
    }
}
