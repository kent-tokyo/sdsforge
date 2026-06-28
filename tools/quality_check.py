#!/usr/bin/env python3
"""
SDS JSON Quality Check Script — r33

# r33: 3 FP fixes (seed=555 diverse50 findings — 53 manufacturers, 49/50 success)
#       FIX: S9-FLASH-POINT-NOT-NUMERIC — add EN 'None detected'/'not detected'/
#            'no flash point detected'/'not measurable' to flash-point N/A suppression.
#            en/mgchemicals sds-4120-l had fp='None detected' (no flammable H-codes)
#            firing spurious HIGH. (Note: '該当しない'+H226 in cerij correctly kept firing.)
#       FIX: S4-NO-PHYSICIAN — add zh-cn Simplified '求医'/'就诊' (求 U+6C42 + 医 U+533B;
#            就 U+5C31 + 诊 U+8BCA). Previous regex had Traditional '求醫'/'就診' (醫 U+91AB,
#            診 U+8A3A) but not the Simplified forms. zh-cn/tci_cn had '求医/就诊' = "seek
#            medical treatment/go to the doctor" that was missed.
#       FIX: S8-OEL-NO-NUMERIC — add '使用粉尘的职业接触限值|在控制.*职业接触限值'
#            (zh-cn: "use dust OEL" / "when controlling exposure, use dust OEL").
#            zh-cn/fpcusa uses this phrase to reference general dust standards without
#            listing a specific numeric value — this is a legitimate compliance statement.
# r32: 3 FP fixes + 1 new HIGH rule (seed=314 diverse50 findings — 53 manufacturers)
#       FIX: S3-CAS-FORMAT — suppress when CAS text is a known proprietary/trade-secret
#            placeholder: 'Proprietary', '未特定', 'Trade Secret', 'Confidential',
#            '機密', '専売品', '不明', etc. GHS SDS guidelines allow withholding CAS
#            for trade-secret ingredients; these are not format violations.
#       FIX: S8-OEL-NO-NUMERIC — add EN 'No biological limit'/'not allocated',
#            zh-cn Simplified '不含有职业接触限值|不含.*职业接触限值'
#            (职 U+804C ≠ Traditional 職 U+8077 — r31 added Traditional only),
#            zh-tw '無資料' (無 U+7121 ≠ 无 U+65E0), standalone '無'/'无' via
#            word-boundary match \b無\b|\b无\b (safe: CJK chars form \b boundaries
#            adjacent to non-CJK; confirmed \b無\b doesn't match in '無資料').
#       FIX: S4-NO-PHYSICIAN — add zh-tw/zh-cn '送醫'/'送医' (send to hospital/doctor);
#            confirmed in sfchem '立即送醫' and ttl '立即送醫' which were FPs.
#       NEW: S3-MAJORITY-CONC-MISSING (HIGH) — mixture with >3 components where >50%
#            (but NOT all) lack numeric concentration values. Bridges existing
#            S3-MIXTURE-ALL-CONC-MISSING (all-absent) and per-component
#            S3-CONC-UNIT-NO-VALUE (individual MED). Confirmed by extract-text on
#            ja/eneos: source PDF has '3質量%以上5質量%未満' ranges the LLM fails to
#            capture. When this rule fires, per-component S3-CONC-UNIT-NO-VALUE MEDs
#            are suppressed (exactly one HIGH per file, following r28 precedent).
# r31: 5 FP fixes + central Unicode normalization + 1 enhanced rule (seed=77 diverse50 findings)
#       CENTRAL: walk_text now applies NFKC normalization on every string value.
#                Fullwidth Latin (ｍ→m, ３→3, ℃→°C) are converted to ASCII equivalents.
#                Tested across all 110 JSONs (seeds 99/202/diverse50): zero regressions,
#                -1 MED FP (nipponkinzoku OEL fullwidth ｍ3), +N true-positive detections
#                (flash point = ' ℃' with no numeric value now correctly fires HIGH).
#       FIX: S9-FLASH-POINT-NOT-NUMERIC — change 'if not fp_val' → 'if fp_val is None'
#            (Python 'not 0.0' is True — akzonobel has ExactValue=0.0 which was treated
#            as absent, causing spurious HIGH); apply same fix to boiling point check.
#       FIX: S9-FLASH-POINT-NOT-NUMERIC suppression — add '沸騰するまで引火せず'/'引火しない'
#            for substances that genuinely don't ignite (d_kasei, no flammable H-codes).
#       FIX: S4-EYE-HCODE-NO-EYE-AID — suppress when FirstAidEye sub-field is populated
#            (6/6 FPs: ja SDSs use '水で...洗う'+'コンタクトレンズ' without '目/眼/rinse');
#            also add 'コンタクトレンズ|まぶた|contact.*lens' to keyword list.
#       FIX: S8-OEL-NO-NUMERIC — add new zh-tw/ja suppression phrases: '未設定', '無相關',
#            '不含.*職業接觸限值', '不含.*職業暴露極限値', '無相關職業接觸限値資料'
#       FIX: S13-NO-DISPOSAL-KEYWORDS — add gas-specific disposal: 'dissipat', '消散',
#            '气瓶'/'氣瓶', '返.*供应商'/'返.*供應商'; add zh-tw Traditional '法規'/'環保'
#       ENHANCED: S2-INVALID-SIGNAL-WORD → detects concatenated signal words (e.g. '危険、警告')
#                 which violate GHS (exactly one signal word required); fires HIGH instead of MED;
#                 avoids double-count with existing MED rule by branching.
# r30: 4 FP fixes + 1 new HIGH rule (seed=202 30-file roundtrip findings)
#       FIX: S7-FLAMMABLE-NO-STORAGE-TEMP — add EN 'keep cool'/'cool place'/'cool.*dry'
#            and ZH-CN '阴凉' / ZH-TW '陰涼' (U+9670+U+6DBC ≠ Simplified U+9634+U+51C9)
#            to storage-temperature suppression. These are legitimate "cool storage"
#            instructions without numeric limit. Eliminated 8 FPs per run.
#       FIX: S9-CORROSIVE-NO-PH — tighten trigger from {H314,H290,H318,H319} to
#            {H314,H290} only. H318 (serious eye damage) and H319 (eye irritation) don't
#            imply acidic/basic chemistry (acetone oxime, IPA, ethanol all trigger H319
#            but have no relevance to pH). 6 of 7 FPs eliminated; 1 true positive kept
#            (W01W0103-0115JGHEJP with H314+H290+H318 — real corrosive compound).
#       FIX: S2-NON-ENGLISH-SIGNAL-WORD — case-insensitive comparison; mgchemicals EN
#            SDSs write 'DANGER'/'WARNING' (all caps) which failed the exact-match check
#            {"Danger","Warning","N/A"}. Changed to lowercase comparison. 2 FPs → 0.
#       FIX: S12-NO-LOGP — also search Section 9 (PhysicalChemicalProperties) for numeric
#            LogP values. JIS Z 7253 allows partition coefficient in Sec 9 or Sec 12.
#            fujifilm_wako SDSs list 'n-オクタノール/水分配係数: 2.64' in OtherPhysicalChemical
#            Property, not in EcologicalInformation. Only suppresses on NUMERIC values
#            (not "Not specified"/"データなし"), preserving true positives. 1 FP → 0.
#       NEW: S1-ALL-SUPPLIER-FIELDS-ABSENT (HIGH) — hazardous product where SupplierInformation
#            key is completely absent AND the product has active H-codes. Confirmed in all
#            ichemistry zh-cn files: source PDF has blank 供应商名称/电话/地址 fields (verified
#            by extract-text on 2933.pdf). Distinguished from S1-NO-COMPANY-NAME (fires when
#            company only is missing) — this fires when the entire supplier block is gone.
# r29: 3 FP fixes (Unicode encoding gaps) + 1 new HIGH rule
#       FIX: S8-OEL-NO-NUMERIC — add Traditional Chinese "no OEL" phrases:
#            '無職業接觸限值'/'未建立職業接觸限值'/'無職業接觸限值資料' (zh-tw, U+7121/U+81E5
#            differ from Simplified 无/职 U+65E0/U+804C — distinct Unicode code points);
#            add zh-cn '不要求' (not required by Chinese OEL standards);
#            fix numeric detection for footnote-annotated values like "0.05* mg/m³"
#            (asterisk footnote marker between number and unit was breaking regex)
#       FIX: S4-H314-NO-REMOVE-CLOTHING — add Traditional Chinese 脫(U+812B) variants:
#            '脫掉'/'脫去'/'脫.*衣' (U+812B differs from Simplified 脱 U+8131)
#       FIX: S8-SKIN-NO-GLOVE-MATERIAL — add Traditional Chinese 橡膠(U+81A0)
#            (differs from Simplified 橡胶 胶 U+80F6 — was generating FP for
#            '合成橡膠材質手套' in zh-tw SDSs)
#       NEW: CX-JA-TEXT-IN-ZH-SDS (HIGH) — zh-cn/zh-tw SDS has Japanese hiragana in
#            HazardIdentification or Composition (LLM language contamination, e.g.
#            Classification values "分類できない" injected into Chinese SDS corpus;
#            confirmed in strem zh-cn SDSs where LLM sourced JA classification logic)
# r28: 6 FP fixes (Unicode CJK variants, EN keyword gaps) + 1 new HIGH rule
#       FIX: S2 specific GHS0x pictogram rules — suppress when ALL pictograms absent
#            (S2-HAZARD-NO-PICTOGRAM already covers it; was double/triple-counting)
#       FIX: S3-CONC-UNIT-NO-VALUE — suppress per-component MED when new aggregate
#            HIGH S3-MIXTURE-ALL-CONC-MISSING fires (avoid counting same defect N times)
#       FIX: S4-NO-PHYSICIAN — add zh-tw '就醫'/'醫師'/'就診'/'求醫' and
#            zh-cn '医生'/'医疗' (was only matching zh-cn '就医' and JA '医師')
#       FIX: S6-NO-CLEANUP-KEYWORDS — add Chinese-character variants that differ from
#            Japanese Kanji in Unicode: '吸收' (U+6536 vs JA 吸収 U+53CE), '收集'/'回收',
#            '沙' (sand, U+6C99 vs JA 砂 U+7802), '围堵'/'收容'/'处置'
#       FIX: S10-NO-STABILITY-KEYWORDS — add English ('temperature','oxidiz','decompos',
#            'reaction') and Chinese ('稳定'/'穩定','氧化','反应'/'反應','不相容','避免')
#       FIX: S13-NO-DISPOSAL-KEYWORDS — add zh-tw Traditional Chinese forms:
#            '廢棄物'/'廢棄'/'廢物', '焚燒', '處置'/'处置', '废弃'/'丢弃'
#       FIX: S15-NO-LAW-KEYWORDS — add Chinese regulatory terms:
#            '办法'/'管理办法', '规范'/'規範', '名录'/'名錄', '管制', 'IECSC'
#       NEW: S3-MIXTURE-ALL-CONC-MISSING (HIGH) — mixture with >2 components where
#            ALL components lack any numeric concentration value (unit present or absent)
# r27: FP fixes + new rules from 30-file random roundtrip test
#       FIX: VALID_SIGNAL_WORDS — add '危險' (zh-tw Danger) and 'Not applicable' (en)
#       FIX: S14 UN detection — extend regex to match zh-tw format
#            '聯合國編號(UN No.)：XXXX' and simplified '联合国编号.*XXXX'
#       FIX: S14-NO-PACKING-GROUP — add '包裝類別' (zh-tw) + Unicode Roman numerals [ⅠⅡⅢⅣ]
#       FIX: S14-NO-SHIPPING-NAME — add '聯合國運輸名稱' / '运输名称' (zh-tw/zh-cn)
#       NEW: S2-HAZARD-NO-PICTOGRAM (MED) — active signal+H-codes but Pictogram field empty
# r26: new pictogram rules S2-FLAMMABLE-NO-GHS02, S2-CORROSIVE-NO-GHS05,
#       S2-ACUTETOX-NO-GHS06; new S4-H314-NO-REMOVE-CLOTHING
# r25: fix S2-EXPLOSIVE-NO-GHS01/S2-ENV-NO-GHS09 spurious substring false negatives,
#       new: S3-NAME-IS-CAS, S16-REVISION-BEFORE-ISSUE
# r24: S5-EMPTY threshold 30→15, S8-OEL-NO-NUMERIC false positive fix,
#       new: S1-ZH-NO-EMERGENCY, S8-NO-ENG-CONTROLS, S7-FLAMMABLE-STORAGE-TEMP,
#            S10-NO-INCOMPATIBLE, CROSS-STALE-DATE

Usage:
    python3 quality_check.py <json_file> <lang> [--jsonl]

  lang: ja | en | zh-cn | zh-tw
  --jsonl: additionally print one JSON-Lines record at the end of stdout

Exit code = total issue count (0 = OK, 1 = WARN, 2+ = FAIL).

When --jsonl is used the QC- lines AND summary are printed normally, then
a single JSONL line is appended so that `tail -1 | jq` works in pipelines.
"""

import argparse
import json
import re
import sys
import unicodedata
from collections import defaultdict

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

SECTION_KEYS_ALL = [
    "Datasheet",
    "Identification",
    "HazardIdentification",
    "Composition",
    "FirstAidMeasures",
    "FireFightingMeasures",
    "AccidentalReleaseMeasures",
    "HandlingAndStorage",
    "ExposureControlPersonalProtection",
    "PhysicalChemicalProperties",
    "StabilityReactivity",
    "ToxicologicalInformation",
    "EcologicalInformation",
    "DisposalConsiderations",
    "TransportInformation",
    "RegulatoryInformation",
    "OtherInformation",
]

# The 16 JIS sections (excludes Datasheet which is admin metadata)
SECTION_KEYS_16 = [k for k in SECTION_KEYS_ALL if k != "Datasheet"]

# Deviation from spec (r22): spec lists only ja/en/zh standard words.
# Extended with Japanese "N/A" variants encountered in real samples (e.g. output08, output12)
# so that non-hazardous products with a "none" signal word don't generate false MED issues.
VALID_SIGNAL_WORDS = {
    "危険", "警告",          # ja active
    "Danger", "Warning",     # en active
    "N/A", "不適用",          # ja/en N/A
    "Not applicable",         # en non-hazardous (r27)
    "不适用", "危险", "警告", "无资料",  # zh-cn
    "危險",                   # zh-tw Danger (r27: Traditional Chinese)
    "なし", "該当区分なし", "該当しない", "なし（GHSの危険有害性なし）",  # ja "none" variants
    "危険性なし", "警告なし",
}

# Cat 1/2 H-codes that warrant a Danger signal word
CAT12_CODES = {
    "H200", "H201", "H202", "H203", "H204", "H205",
    "H220", "H221", "H222", "H223", "H224", "H225",
    "H260", "H261", "H270", "H271",
    "H280", "H281", "H290",
    "H300", "H301", "H310", "H311",
    "H330", "H331", "H314", "H317", "H318", "H334",
    "H340", "H341", "H350", "H351",
    "H360", "H361", "H370", "H371",
    "H400", "H410", "H420",
}

VALID_PICTOGRAMS = {
    "GHS01", "GHS02", "GHS03", "GHS04", "GHS05",
    "GHS06", "GHS07", "GHS08", "GHS09",
    # Japanese label variants
    "絵表示01", "絵表示02", "絵表示03", "絵表示04", "絵表示05",
    "絵表示06", "絵表示07", "絵表示08", "絵表示09",
    # Chinese variants sometimes written as GHS-01 etc.
    "GHS-01", "GHS-02", "GHS-03", "GHS-04", "GHS-05",
    "GHS-06", "GHS-07", "GHS-08", "GHS-09",
}

# H-codes that trigger UN number check in Section 14
DG_H_CODES = {
    "H224", "H225", "H226",
    "H300", "H301", "H302",
    "H310", "H311",
    "H314",
    "H330", "H331", "H332",
    "H270", "H271", "H272",
}

NOT_REGULATED_PATTERNS = re.compile(
    r"not regulated|非危険物|not dangerous|無資料|規制されていない|規制対象外|"
    r"危険物に該当しない|not subject|no regulation|非危险|该当なし|该当しない|"
    r"not classify|not applicable|n/a",
    re.IGNORECASE,
)

H_CODE_RE = re.compile(r"\bH[2-4]\d{2}[A-Z]?\b")
P_CODE_RE = re.compile(r"\bP\d{3}(?:\+P\d{3})*\b")

# ---------------------------------------------------------------------------
# Utility helpers
# ---------------------------------------------------------------------------


def walk_text(obj) -> str:
    """Recursively collect all string values from any nested structure.

    r31: NFKC-normalizes every string so fullwidth Latin chars (ｍ→m, ℃→°C,
    ３→3) don't break unit/numeric regexes. CJK characters are unaffected.
    """
    if isinstance(obj, str):
        return unicodedata.normalize("NFKC", obj)
    if isinstance(obj, dict):
        return " ".join(walk_text(v) for v in obj.values())
    if isinstance(obj, list):
        return " ".join(walk_text(item) for item in obj)
    return ""


def to_str(v) -> str:
    """Coerce any JSON value (str | list | None | number) to a plain string.

    Handles the case where the LLM produces a list instead of a scalar string
    for fields like Use, TradeNameJP, Phone, etc.
    """
    if v is None:
        return ""
    if isinstance(v, list):
        return " ".join(to_str(item) for item in v)
    if isinstance(v, dict):
        return walk_text(v)
    return str(v).strip()


def section_text(root: dict, key: str) -> str:
    """Return all text from the named top-level section."""
    return walk_text(root.get(key) or "")


def has_katakana(s: str) -> bool:
    return bool(re.search(r"[゠-ヿ]", s))


def has_digits(s: str) -> bool:
    return bool(re.search(r"\d", s))


def collect_h_codes(root: dict) -> set:
    """Collect all well-formed H-codes from HazardIdentification."""
    text = section_text(root, "HazardIdentification")
    return set(H_CODE_RE.findall(text))


def collect_p_codes(root: dict) -> set:
    """Collect all P-codes (including combined P210+P220 style) from HazardIdentification."""
    text = section_text(root, "HazardIdentification")
    raw = P_CODE_RE.findall(text)
    codes = set()
    for match in raw:
        for part in match.split("+"):
            codes.add(part.strip())
    return codes


def is_hazardous(root: dict) -> bool:
    return len(collect_h_codes(root)) > 0


def get_signal_word(root: dict) -> str:
    haz = root.get("HazardIdentification") or {}
    if isinstance(haz, list):
        haz = haz[0] if haz else {}
    labelling = haz.get("HazardLabelling") or {}
    return to_str(labelling.get("SignalWord"))


def extract_numeric_values(obj, path_hint: str = "") -> list:
    """
    Recursively extract float values from nested NumericRangeWithUnitAndQualifier
    structures (ExactValue.Value, LowerValue.Value, UpperValue.Value).
    Returns list of floats found.
    """
    values = []
    if isinstance(obj, dict):
        nrwuq = obj.get("NumericRangeWithUnitAndQualifier")
        if nrwuq and isinstance(nrwuq, dict):
            for sub_key in ("ExactValue", "LowerValue", "UpperValue"):
                sub = nrwuq.get(sub_key) or {}
                if isinstance(sub, dict) and "Value" in sub:
                    try:
                        values.append(float(sub["Value"]))
                    except (TypeError, ValueError):
                        pass
        for v in obj.values():
            values.extend(extract_numeric_values(v, path_hint))
    elif isinstance(obj, list):
        for item in obj:
            values.extend(extract_numeric_values(item, path_hint))
    return values


def get_flash_point_value(root: dict):
    """Return the first numeric flash point value, or None."""
    phys = root.get("PhysicalChemicalProperties") or {}
    fp_list = phys.get("FlashPoint") or []
    if isinstance(fp_list, dict):
        fp_list = [fp_list]
    for fp in fp_list:
        vals = extract_numeric_values(fp)
        if vals:
            return vals[0]
    return None


def get_boiling_point_value(root: dict):
    """Return the first numeric boiling point value, or None."""
    phys = root.get("PhysicalChemicalProperties") or {}
    bp_list = phys.get("BoilingPointRelated") or []
    if isinstance(bp_list, dict):
        bp_list = [bp_list]
    for bp in bp_list:
        vals = extract_numeric_values(bp)
        if vals:
            return vals[0]
    return None


def get_composition_type(root: dict) -> str:
    comp = root.get("Composition") or {}
    return to_str(comp.get("CompositionType"))


def is_mixture(root: dict) -> bool:
    ct = get_composition_type(root).lower()
    return ct in ("混合物", "mixture")


def cas_check_digit_valid(cas: str) -> bool:
    """Validate CAS check digit. Returns True if valid, False if invalid."""
    # CAS format: XXXXXXX-XX-X
    parts = cas.strip().split("-")
    if len(parts) != 3:
        return False
    digits_str = parts[0] + parts[1]
    check_digit_str = parts[2]
    if not digits_str.isdigit() or not check_digit_str.isdigit():
        return False
    digits = [int(c) for c in digits_str]
    check_digit = int(check_digit_str)
    # Multiply right-to-left by 1,2,3,...
    total = sum((i + 1) * d for i, d in enumerate(reversed(digits)))
    return (total % 10) == check_digit


def section_populated(root: dict, key: str) -> bool:
    """Return True if the section has any non-trivial content (>10 chars of text)."""
    val = root.get(key)
    if val is None:
        return False
    text = walk_text(val).strip()
    return len(text) > 10


def count_populated_sections(root: dict) -> int:
    return sum(1 for k in SECTION_KEYS_16 if section_populated(root, k))


def issue(level: str, rule_id: str, message: str) -> tuple:
    return (level, rule_id, message)


# ---------------------------------------------------------------------------
# Section 1: Identification
# ---------------------------------------------------------------------------

def check_sec1(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        ident = root.get("Identification") or {}

        # Product name
        tpi = ident.get("TradeProductIdentity") or {}
        name_jp = to_str(tpi.get("TradeNameJP"))
        name_en = to_str(tpi.get("TradeNameEN"))
        if not name_jp and not name_en:
            issues.append(issue("CRIT", "S1-NO-PRODUCT-NAME",
                                "Sec1: Both TradeNameJP and TradeNameEN are absent or empty"))
        else:
            if lang != "ja":
                if has_katakana(name_jp) or has_katakana(name_en):
                    issues.append(issue("CRIT", "S1-KATAKANA-PRODUCT-NAME",
                                        f"Sec1: Katakana in product name for non-Japanese SDS (lang={lang})"))

        # r30-NEW: SupplierInformation block entirely absent on a hazardous product.
        # Confirmed by extract-text on ichemistry zh-cn PDFs: source document has blank
        # 供应商名称/电话/地址 fields — the entire supplier section was never filled in.
        # More severe than S1-NO-COMPANY-NAME (missing one field) because no contact
        # info at all on a dangerous chemical SDS violates JIS Z 7253 Section 1 mandatory reqs.
        _raw_supplier = ident.get("SupplierInformation")
        if _raw_supplier is None and h_codes:
            issues.append(issue("HIGH", "S1-ALL-SUPPLIER-FIELDS-ABSENT",
                                "Sec1: SupplierInformation block is completely absent for a hazardous product "
                                "(source SDS has no supplier contact data — JIS Z 7253 Sec.1 violation)"))

        # Company name
        supplier = _raw_supplier or {}
        company = to_str(supplier.get("CompanyName"))
        if not company:
            issues.append(issue("HIGH", "S1-NO-COMPANY-NAME",
                                "Sec1: SupplierInformation.CompanyName is absent or empty"))
        elif lang != "ja" and has_katakana(company):
            issues.append(issue("HIGH", "S1-KATAKANA-COMPANY-NAME",
                                f"Sec1: Katakana CompanyName in non-Japanese SDS (lang={lang})"))

        # Phone (r23-NEW)
        phone = to_str(supplier.get("Phone"))
        if not phone:
            issues.append(issue("MED", "S1-NO-PHONE",
                                "Sec1: SupplierInformation.Phone is absent"))
        else:
            digit_count = sum(1 for c in phone if c.isdigit())
            if digit_count < 7:
                issues.append(issue("MED", "S1-SHORT-PHONE",
                                    f"Sec1: SupplierInformation.Phone has fewer than 7 digits: '{phone}'"))

        # Use field
        uuaa = ident.get("UseAndUseAdvisedAgainst") or {}
        use_val = to_str(uuaa.get("Use"))
        if not use_val:
            issues.append(issue("MED", "S1-NO-USE",
                                "Sec1: UseAndUseAdvisedAgainst.Use is absent or empty"))

        # Emergency contact phone digits
        emergency = ident.get("EmergencyContact") or []
        if isinstance(emergency, dict):
            emergency = [emergency]
        for ec in emergency:
            ec_text = walk_text(ec)
            if ec_text.strip() and not has_digits(ec_text):
                issues.append(issue("MED", "S1-EMERGENCY-NO-PHONE",
                                    "Sec1: EmergencyContact entry has no phone digits"))
                break

        # zh-cn/zh-tw: EmergencyContact required by GB/T 16483 / CNS 15030
        if lang in ("zh-cn", "zh-tw") and company:  # only when SupplierInformation exists
            ec_list = ident.get("EmergencyContact") or supplier.get("EmergencyContact") or []
            if isinstance(ec_list, dict):
                ec_list = [ec_list]
            has_emergency = any(walk_text(ec).strip() for ec in ec_list)
            if not has_emergency:
                issues.append(issue("MED", "S1-ZH-NO-EMERGENCY",
                                    f"Sec1: {lang} SDS has no EmergencyContact (required by GB/T 16483 / CNS 15030)"))

    except Exception as e:
        issues.append(issue("MED", "S1-INTERNAL", f"Sec1 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 2: HazardIdentification
# ---------------------------------------------------------------------------

def check_sec2(root: dict, lang: str, h_codes: set, p_codes: set) -> list:
    issues = []
    try:
        haz = root.get("HazardIdentification") or {}
        if isinstance(haz, list):
            haz = haz[0] if haz else {}
        labelling = haz.get("HazardLabelling") or {}
        signal_word = to_str(labelling.get("SignalWord"))

        # SignalWord validation
        if signal_word:
            if signal_word not in VALID_SIGNAL_WORDS and signal_word.lower() not in {
                sw.lower() for sw in VALID_SIGNAL_WORDS
            }:
                # r31: detect concatenated signal words (e.g. '危険、警告' or 'Danger/Warning').
                # GHS requires exactly ONE signal word. Concatenation is a data-quality violation
                # worse than just an unrecognized word — fires HIGH to distinguish.
                # Splits on 、,，/／ delimiters; avoids double-count with S2-INVALID-SIGNAL-WORD.
                _VALID_SW_SET = {"危険", "警告", "Danger", "Warning", "危险", "危險", "DANGER", "WARNING"}
                _sw_parts = [p.strip() for p in re.split(r"[、,，/／]+", signal_word) if p.strip()]
                if len(_sw_parts) >= 2 and all(p in _VALID_SW_SET or p.lower() in {s.lower() for s in _VALID_SW_SET} for p in _sw_parts):
                    issues.append(issue("HIGH", "S2-MULTIPLE-SIGNAL-WORDS",
                                        f"Sec2: SignalWord '{signal_word}' concatenates multiple GHS signal words "
                                        f"— GHS requires exactly one (Danger OR Warning, not both)"))
                else:
                    issues.append(issue("MED", "S2-INVALID-SIGNAL-WORD",
                                        f"Sec2: SignalWord '{signal_word}' not in valid set"))
            if lang in ("zh-cn", "zh-tw") and has_katakana(signal_word):
                issues.append(issue("HIGH", "S2-KATAKANA-SIGNAL-WORD",
                                    f"Sec2: Katakana SignalWord in Chinese SDS (lang={lang}): '{signal_word}'"))
            # r30: case-insensitive — mgchemicals writes 'DANGER'/'WARNING' (all caps)
            if lang == "en" and signal_word.lower() not in ("danger", "warning", "n/a", "not applicable"):
                issues.append(issue("MED", "S2-NON-ENGLISH-SIGNAL-WORD",
                                    f"Sec2: Non-English SignalWord in English SDS: '{signal_word}'"))
            # H224 + Danger check
            if "H224" in h_codes and signal_word not in ("Danger", "危険", "危险"):
                issues.append(issue("HIGH", "S2-H224-NO-DANGER",
                                    "Sec2: H224 (extremely flammable) present but SignalWord is not Danger"))
            # H226 alone with Danger
            if signal_word in ("Danger", "危険", "危险") and "H226" in h_codes:
                cat12_except_h226 = CAT12_CODES - {"H226"}
                if not h_codes.intersection(cat12_except_h226):
                    issues.append(issue("MED", "S2-H226-ALONE-DANGER",
                                        "Sec2: H226 alone but SignalWord is Danger (Cat3 flammable is normally Warning)"))

        # H-code format violations — scan raw text for any H\d{3} that doesn't match
        haz_text = section_text(root, "HazardIdentification")
        for raw_code in re.findall(r"\bH\d{3,4}[A-Z]?\b", haz_text):
            if not H_CODE_RE.match(raw_code):
                issues.append(issue("HIGH", "S2-H-FORMAT",
                                    f"Sec2: H-code format violation: '{raw_code}'"))
                break

        # HazardStatement codes empty
        hs_list = labelling.get("HazardStatement") or []
        if isinstance(hs_list, dict):
            hs_list = [hs_list]
        if hs_list:
            all_codes_empty = all(
                not (entry.get("HazardStatementCode"))
                for entry in hs_list if isinstance(entry, dict)
            )
            # Deviation from spec (r22): spec says "always fire when all codes empty",
            # but N/A / "なし" products legitimately store only FullText (no H-codes).
            # Gating on active signal word prevents false CRITs on non-hazardous products.
            ACTIVE_SIGNAL_WORDS_EARLY = {"Danger", "Warning", "危険", "警告", "危险"}
            if all_codes_empty and signal_word in ACTIVE_SIGNAL_WORDS_EARLY:
                issues.append(issue("CRIT", "S2-HS-CODES-EMPTY",
                                    "Sec2: HazardStatement entries present but all HazardStatementCode are empty"))

        # "Active" signal words (Danger/Warning) imply actual hazard labelling is expected.
        # Deviation from spec: rules S2-SIGNAL-NO-HCODES and S2-SIGNAL-NO-HAZARDSTATEMENT
        # are restricted to active signal words so non-hazardous products (signal=N/A/なし)
        # don't trigger false HIGHs for legitimately absent H-codes.
        ACTIVE_SIGNAL_WORDS = {"Danger", "Warning", "危険", "警告", "危险"}
        is_active_signal = signal_word in ACTIVE_SIGNAL_WORDS

        # SignalWord present but no H-codes (only if active signal word)
        if is_active_signal and not h_codes:
            issues.append(issue("HIGH", "S2-SIGNAL-NO-HCODES",
                                "Sec2: SignalWord present but no H-codes found"))

        # SignalWord present but HazardStatement completely absent (r23-NEW, only active)
        if is_active_signal and not hs_list:
            issues.append(issue("HIGH", "S2-SIGNAL-NO-HAZARDSTATEMENT",
                                "Sec2: SignalWord present but HazardStatement completely absent"))

        # More than 12 H-codes
        if len(h_codes) > 12:
            issues.append(issue("MED", "S2-TOO-MANY-HCODES",
                                f"Sec2: More than 12 H-codes found ({len(h_codes)}) — possible duplication"))

        # Danger signal but no Cat1/2 H-code
        if signal_word in ("Danger", "危険", "危险"):
            if h_codes and not h_codes.intersection(CAT12_CODES):
                issues.append(issue("MED", "S2-DANGER-NO-CAT12",
                                    "Sec2: Danger signal word but no Cat1/2 H-code found"))

        # P-code format violations
        p_text = haz_text
        for raw_p in re.findall(r"\bP\d{3,4}\b", p_text):
            if not re.match(r"^P\d{3}$", raw_p):
                issues.append(issue("HIGH", "S2-P-FORMAT",
                                    f"Sec2: P-code format violation: '{raw_p}'"))
                break

        # Signal + H-codes but zero P-codes
        if signal_word and h_codes and not p_codes:
            issues.append(issue("HIGH", "S2-NO-PCODES",
                                "Sec2: Signal word + H-codes present but zero P-codes extracted"))

        # Danger/Warning P-code minimum
        if signal_word in ("Danger", "危険", "危险") and p_codes and len(p_codes) < 4:
            issues.append(issue("MED", "S2-DANGER-FEW-PCODES",
                                f"Sec2: Danger product has fewer than 4 P-codes ({len(p_codes)})"))
        if signal_word in ("Warning", "警告") and p_codes and len(p_codes) < 3:
            issues.append(issue("MED", "S2-WARNING-FEW-PCODES",
                                f"Sec2: Warning product has fewer than 3 P-codes ({len(p_codes)})"))

        # H-code × P-code consistency
        if h_codes.intersection({"H224", "H225", "H226"}) and "P210" not in p_codes:
            issues.append(issue("MED", "S2-H22X-NO-P210",
                                "Sec2: H224/225/226 present but P210 (away from heat) not found"))
        if h_codes.intersection({"H300", "H301", "H302"}):
            if "P301" not in p_codes and "P330" not in p_codes:
                issues.append(issue("MED", "S2-H3XX-NO-P301",
                                    "Sec2: Oral acute-tox H-code but P301 or P330 not found"))
        if h_codes.intersection({"H330", "H331", "H332"}):
            if "P304" not in p_codes and "P261" not in p_codes:
                issues.append(issue("MED", "S2-H33X-NO-P304",
                                    "Sec2: Inhalation H-code but P304 or P261 not found"))
        if h_codes.intersection({"H318", "H319"}):
            if "P305" not in p_codes:
                issues.append(issue("MED", "S2-H318-NO-P305",
                                    "Sec2: H318/H319 present but P305 (eye rinse) not found"))
        if "H314" in h_codes:
            missing = [p for p in ("P280", "P301", "P305") if p not in p_codes]
            if missing:
                issues.append(issue("MED", "S2-H314-MISSING-P",
                                    f"Sec2: H314 present but P-code(s) {missing} not found"))

        # Pictogram validation
        pictograms = labelling.get("Pictogram") or []
        if isinstance(pictograms, str):
            pictograms = [pictograms]
        for pic in pictograms:
            pic_str = str(pic).strip()
            if pic_str and pic_str not in VALID_PICTOGRAMS:
                # Try partial match (e.g., "GHS01爆弾" type)
                if not any(v in pic_str for v in VALID_PICTOGRAMS):
                    issues.append(issue("MED", "S2-INVALID-PICTOGRAM",
                                        f"Sec2: Pictogram '{pic_str}' outside GHS01-GHS09 set"))

        # H-codes but Classification missing
        if h_codes and not (haz.get("Classification")):
            issues.append(issue("MED", "S2-NO-CLASSIFICATION",
                                "Sec2: H-codes present but Classification section missing"))

        # r27-NEW / r28-refactor: Active signal word + H-codes but Pictogram field is completely empty.
        # r28: Fire this first; if ALL pictograms are absent, skip the specific GHS0x rules below
        # to avoid triple-counting the same defect (S2-HAZARD-NO-PICTOGRAM + 4 specific rules).
        # Specific rules only fire when the list is NON-empty but missing a particular GHS code.
        _all_pictograms_absent = is_active_signal and h_codes and not pictograms
        if _all_pictograms_absent:
            issues.append(issue("MED", "S2-HAZARD-NO-PICTOGRAM",
                                "Sec2: Active signal word + H-codes present but Pictogram list is completely empty — "
                                "pictograms may be image-only in source PDF (not extractable as text)"))

        # Specific pictogram checks — only meaningful when at least one pictogram was extracted.
        # (r28: gate on `pictograms` to suppress when S2-HAZARD-NO-PICTOGRAM already fired)
        if pictograms:
            pic_texts = " ".join(str(p) for p in pictograms)

            # r23-NEW: H200-H205 but no GHS01  (r25-fix: removed false-negative "01" fallback)
            if h_codes.intersection({"H200", "H201", "H202", "H203", "H204", "H205"}):
                if "GHS01" not in pic_texts:
                    issues.append(issue("MED", "S2-EXPLOSIVE-NO-GHS01",
                                        "Sec2: H200-H205 (explosive) present but GHS01 pictogram not found"))

            # r23-NEW: H4xx environmental but no GHS09  (r25-fix: removed false-negative "09" fallback)
            if h_codes.intersection({"H410", "H411", "H412", "H413"}):
                if "GHS09" not in pic_texts:
                    issues.append(issue("MED", "S2-ENV-NO-GHS09",
                                        "Sec2: H410/H411/H412/H413 present but GHS09 (environmental) pictogram not found"))

            # r26-NEW: Flammable H-codes but no GHS02 flame pictogram
            if h_codes.intersection({"H224", "H225", "H226", "H220", "H221", "H222", "H223", "H228", "H242", "H252"}):
                if "GHS02" not in pic_texts:
                    issues.append(issue("MED", "S2-FLAMMABLE-NO-GHS02",
                                        "Sec2: Flammable H-code present but GHS02 (flame) pictogram not found"))

            # r26-NEW: Skin corrosion H314 but no GHS05 corrosion pictogram
            if "H314" in h_codes:
                if "GHS05" not in pic_texts:
                    issues.append(issue("MED", "S2-CORROSIVE-NO-GHS05",
                                        "Sec2: H314 (skin corrosion) present but GHS05 (corrosion) pictogram not found"))

            # r26-NEW: Fatal/toxic acute H-codes (Cat 1-3) but no GHS06 skull pictogram
            if h_codes.intersection({"H300", "H301", "H310", "H311", "H330", "H331"}):
                if "GHS06" not in pic_texts:
                    issues.append(issue("MED", "S2-ACUTETOX-NO-GHS06",
                                        "Sec2: Acute-tox H300/H301/H310/H311/H330/H331 present but GHS06 (skull) pictogram not found"))

    except Exception as e:
        issues.append(issue("MED", "S2-INTERNAL", f"Sec2 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 3: Composition
# ---------------------------------------------------------------------------

def check_sec3(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        comp = root.get("Composition") or {}
        components = comp.get("CompositionAndConcentration") or []
        if isinstance(components, dict):
            components = [components]

        # Empty composition
        if not components:
            issues.append(issue("CRIT", "S3-EMPTY",
                                "Sec3: CompositionAndConcentration is empty — no ingredients extracted"))
            return issues

        comp_type = get_composition_type(root)
        is_mix = is_mixture(root)

        # Mixture but only 1 substance
        if is_mix and len(components) == 1:
            issues.append(issue("MED", "S3-MIXTURE-ONE-COMPONENT",
                                "Sec3: CompositionType is mixture but only 1 substance found"))

        # r23-NEW: Mixture with >10 components
        if is_mix and len(components) > 10:
            issues.append(issue("MED", "S3-MIXTURE-TOO-MANY",
                                f"Sec3: Mixture with {len(components)} components (>10) — likely over-extraction"))

        cas_list = []
        numeric_concentrations = []
        duplicate_cas_set = set()
        _conc_unit_no_value_issues = []  # r28: collected separately for aggregate-HIGH dedup

        for comp_entry in components:
            if not isinstance(comp_entry, dict):
                continue
            ids = (comp_entry.get("SubstanceIdentifiers") or {})
            sub_id = (ids.get("SubstanceIdentity") or {})
            sub_names = (ids.get("SubstanceNames") or {})

            # CAS handling
            cas_node = (sub_id.get("CASno") or {})
            cas_texts = cas_node.get("FullText") or []
            if isinstance(cas_texts, str):
                cas_texts = [cas_texts]

            # r32: proprietary/trade-secret placeholders are legitimate CAS substitutes.
            # GHS guidelines allow withholding CAS numbers for confidential ingredients.
            _PROPRIETARY_CAS_RE = re.compile(
                r"proprietary|trade.?secret|confidential|not.?disclosed|withheld|"
                r"未特定|機密|専売品|不明|秘密|未公開|営業秘密|"
                r"商业机密|保密|不公开|未知",
                re.IGNORECASE,
            )
            cas_found = []
            for cas_raw in cas_texts:
                cas_str = str(cas_raw).strip()
                if not cas_str:
                    continue
                # Skip format check for proprietary/trade-secret placeholders
                if _PROPRIETARY_CAS_RE.search(cas_str):
                    continue
                # CAS format check
                if not re.match(r"^\d{1,7}-\d{2}-\d$", cas_str):
                    issues.append(issue("HIGH", "S3-CAS-FORMAT",
                                        f"Sec3: CAS format violation: '{cas_str}'"))
                elif not cas_check_digit_valid(cas_str):
                    issues.append(issue("HIGH", "S3-CAS-CHECKDIGIT",
                                        f"Sec3: CAS check-digit mismatch: '{cas_str}'"))
                else:
                    cas_found.append(cas_str)
                    if cas_str in duplicate_cas_set:
                        issues.append(issue("MED", "S3-DUPLICATE-CAS",
                                            f"Sec3: Duplicate CAS number: '{cas_str}'"))
                    duplicate_cas_set.add(cas_str)
                    cas_list.append(cas_str)

            # Multi-component missing CAS
            if is_mix and not cas_found:
                issues.append(issue("MED", "S3-MISSING-CAS",
                                    "Sec3: Multi-component product has component without CAS number"))

            # Katakana substance name in non-Japanese SDS
            if lang != "ja":
                generic = (sub_names.get("GenericName") or "")
                iupac = (sub_names.get("IupacName") or "")
                for nm in (generic, iupac):
                    if has_katakana(nm):
                        issues.append(issue("CRIT", "S3-KATAKANA-SUBSTANCE",
                                            f"Sec3: Katakana substance name in non-Japanese SDS: '{nm}'"))
                        break

            # r25-NEW: substance name field contains a bare CAS number (LLM mis-extraction)
            for nm_key in ("GenericName", "IupacName"):
                nm = to_str(sub_names.get(nm_key))
                if nm and re.match(r"^\d{1,7}-\d{2}-\d$", nm):
                    issues.append(issue("HIGH", "S3-NAME-IS-CAS",
                                        f"Sec3: Substance name field '{nm_key}' contains bare CAS number: '{nm}'"))
                    break

            # Molecular weight check
            mw_node = sub_id.get("MolecularWeight") or {}
            mw_vals = extract_numeric_values(mw_node)
            for mw in mw_vals:
                if mw <= 0 or mw > 200000:
                    issues.append(issue("HIGH", "S3-MW-RANGE",
                                        f"Sec3: Molecular weight out of range: {mw}"))

            # Concentration checks
            conc_node = comp_entry.get("Concentration") or {}
            conc_text = walk_text(conc_node)

            # Date string in concentration
            if re.search(r"\b(19|20|21)\d{2}-\d{2}-\d{2}\b", conc_text):
                issues.append(issue("HIGH", "S3-DATE-IN-CONC",
                                    f"Sec3: Date string in Concentration field: '{conc_text[:60]}'"))

            # r23-NEW: year-like string in concentration (e.g., "2024")
            # Do NOT exclude on "年" — "2024年" is exactly the LLM mis-extraction we want to catch
            # Only suppress when the year is clearly a temperature reading (adjacent to °C/℃)
            if re.search(r"\b(19|20|21)\d{2}\b", conc_text):
                if not re.search(r"(19|20|21)\d{2}\s*[℃°]", conc_text):
                    issues.append(issue("HIGH", "S3-YEAR-IN-CONC",
                                        f"Sec3: Year-like string in Concentration field: '{conc_text[:60]}'"))

            conc_vals = extract_numeric_values(conc_node)
            numeric_concentrations.extend(conc_vals)

            # r27-NEW: Concentration has a unit but no numeric value
            # Pattern: {"NumericRangeWithUnitAndQualifier": {"Unit": "%"}} — unit extracted, value missed
            # Gate on mixture to avoid noise from pure substance SDSs with ">99%" style text.
            # r28: Collect separately; suppressed below if S3-MIXTURE-ALL-CONC-MISSING fires.
            if is_mix and isinstance(conc_node, dict):
                nrwuq = conc_node.get("NumericRangeWithUnitAndQualifier") or {}
                if isinstance(nrwuq, dict) and nrwuq.get("Unit") and not conc_vals:
                    # Confirm no AdditionalInfo either (catch ">" / "<" qualifiers in text)
                    if not walk_text(conc_node.get("AdditionalInfo") or {}).strip():
                        _conc_unit_no_value_issues.append(issue(
                            "MED", "S3-CONC-UNIT-NO-VALUE",
                            f"Sec3: Mixture component has concentration unit ('{nrwuq['Unit']}') but no numeric value extracted"))

        # r28-NEW: Mixture where ALL components lack any numeric concentration (aggregate HIGH).
        # r32: Also detect MAJORITY-missing case (>50% but <100%) as HIGH. Both suppress
        # per-component S3-CONC-UNIT-NO-VALUE MEDs to avoid N+1 counting (r28 precedent).
        # Confirmed by extract-text on ja/eneos: PDF has '3質量%以上5質量%未満' ranges that
        # the LLM fails to capture (extraction failure, not intentional withholding).
        _all_conc_missing = (
            is_mix and len(components) > 2
            and not numeric_concentrations
        )
        _n_unit_no_val = len(_conc_unit_no_value_issues)
        _majority_conc_missing = (
            is_mix and len(components) > 3
            and not _all_conc_missing          # all-missing case handled below
            and _n_unit_no_val > 0
            and _n_unit_no_val / len(components) > 0.5
        )

        if _all_conc_missing:
            issues.append(issue("HIGH", "S3-MIXTURE-ALL-CONC-MISSING",
                                f"Sec3: Mixture with {len(components)} components — ALL lack numeric concentration values "
                                f"(LLM may have extracted unit strings only; source concentrations not captured)"))
            # Suppress per-component MEDs — redundant with this HIGH
        elif _majority_conc_missing:
            issues.append(issue("HIGH", "S3-MAJORITY-CONC-MISSING",
                                f"Sec3: Mixture with {len(components)} components — {_n_unit_no_val}/{len(components)} "
                                f"({100*_n_unit_no_val//len(components)}%) lack numeric concentration values "
                                f"(source PDF has range/range values; LLM captured unit only)"))
            # Suppress per-component MEDs — redundant with this HIGH
        else:
            issues.extend(_conc_unit_no_value_issues)

        # Concentration sum > 102%
        if len(numeric_concentrations) > 1:
            total_conc = sum(numeric_concentrations)
            if total_conc > 102:
                issues.append(issue("MED", "S3-CONC-SUM-EXCEEDS",
                                    f"Sec3: Sum of numeric concentrations is {total_conc:.1f}% (>102%)"))

        # Single-component checks
        if not is_mix and len(components) == 1:
            entry = components[0]
            if isinstance(entry, dict):
                ids = entry.get("SubstanceIdentifiers") or {}
                sub_names = ids.get("SubstanceNames") or {}
                generic = to_str(sub_names.get("GenericName"))
                iupac = to_str(sub_names.get("IupacName"))
                if not generic and not iupac:
                    issues.append(issue("MED", "S3-SINGLE-NO-NAME",
                                        "Sec3: Single-component product with no substance name"))
                conc_text = walk_text(entry.get("Concentration") or {}).strip()
                if not conc_text:
                    issues.append(issue("MED", "S3-SINGLE-NO-CONC",
                                        "Sec3: Single-component product with no concentration/purity"))

    except Exception as e:
        issues.append(issue("MED", "S3-INTERNAL", f"Sec3 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 4: FirstAidMeasures
# ---------------------------------------------------------------------------

def check_sec4(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        fam = root.get("FirstAidMeasures") or {}
        exp_route = fam.get("ExposureRoute") or {}
        sec_text = section_text(root, "FirstAidMeasures")

        ROUTE_KEYS = ("FirstAidEye", "FirstAidIngestion", "FirstAidInhalation", "FirstAidSkin")
        non_empty_routes = 0
        for rk in ROUTE_KEYS:
            val = exp_route.get(rk) or {}
            if walk_text(val).strip():
                non_empty_routes += 1

        if non_empty_routes == 0 and section_populated(root, "FirstAidMeasures"):
            issues.append(issue("HIGH", "S4-NO-ROUTES",
                                "Sec4: ExposureRoute has no non-empty route texts"))

        if is_hazardous(root) and non_empty_routes < 2:
            issues.append(issue("MED", "S4-FEW-ROUTES",
                                f"Sec4: Hazardous product with fewer than 2 first-aid routes ({non_empty_routes})"))

        if is_hazardous(root):
            # r28: added zh-tw '就醫'/'醫師'/'就診'/'求醫' and zh-cn '医生'/'医疗'/'医院'
            # r32: added '送醫'/'送医' (zh-tw/zh-cn: send to hospital/doctor)
            # r33: added zh-cn Simplified '求医'/'就诊' (医 U+533B ≠ 醫 U+91AB; 诊 U+8BCA ≠ 診 U+8A3A)
            #      confirmed in tci_cn '求医/就诊' = "seek medical treatment/go to the doctor"
            if not re.search(
                r"doctor|physician|medical|医師|醫師|就医|就醫|医生|醫生|医疗|醫療|"
                r"就診|求醫|求医|就诊|医院|seek medical|診断|手当|送醫|送医",
                sec_text, re.IGNORECASE
            ):
                issues.append(issue("MED", "S4-NO-PHYSICIAN",
                                    "Sec4: No physician/doctor/medical mention for hazardous product"))

        if h_codes.intersection({"H318", "H319", "H314"}):
            _eye_route_text = walk_text(exp_route.get("FirstAidEye") or {}).strip()
            # r31: suppress when FirstAidEye sub-field is populated (contains actual guidance).
            # 6/6 FPs were ja SDSs using '水で...洗う' + 'コンタクトレンズ' without '目/眼/rinse'.
            # Also added コンタクトレンズ|まぶた|contact.*lens for keyword-based detection.
            # Akzonobel (eye={}, H314/H319) still fires — true positive guard preserved.
            if not _eye_route_text:
                if not re.search(
                    r"eye|眼|rinse|洗眼|目|look\b|コンタクトレンズ|まぶた|contact.*lens|eyelid",
                    sec_text, re.IGNORECASE
                ):
                    issues.append(issue("MED", "S4-EYE-HCODE-NO-EYE-AID",
                                        "Sec4: H318/H319/H314 but no eye first-aid keywords found"))

        if h_codes.intersection({"H330", "H331", "H332", "H333", "H334", "H335"}):
            if not re.search(r"inhal|吸入|fresh air|新鮮な空気|空気|換気|通風|呼吸", sec_text, re.IGNORECASE):
                issues.append(issue("MED", "S4-INHAL-HCODE-NO-INHAL-AID",
                                    "Sec4: Inhalation H-code but no inhalation first-aid keywords found"))

        if h_codes.intersection({"H314", "H315"}):
            if not re.search(r"skin|皮膚|wash|洗|水|water", sec_text, re.IGNORECASE):
                issues.append(issue("MED", "S4-SKIN-HCODE-NO-SKIN-AID",
                                    "Sec4: H314/H315 but no skin contact first-aid keywords found"))

        # r26-NEW: H314 (severe skin corrosion) but no "remove clothing" instruction (P361 equivalent)
        if "H314" in h_codes:
            skin_aid_text = walk_text(exp_route.get("FirstAidSkin") or {})
            combined = (skin_aid_text or sec_text)
            if not re.search(
                # r29: added Traditional Chinese 脫(U+812B) variants — differs from Simplified 脱(U+8131)
                r"remov.*cloth|take.?off.*cloth|contaminat.*cloth|衣類.*脱|脱.*衣|汚染.*衣|除去.*衣|脱去.*衣|立即.*脱|立刻.*脱|脱掉|"
                r"脫掉|脫去|脫.*衣",
                combined, re.IGNORECASE
            ):
                issues.append(issue("MED", "S4-H314-NO-REMOVE-CLOTHING",
                                    "Sec4: H314 present but no 'remove contaminated clothing' instruction found (P361 requirement)"))

    except Exception as e:
        issues.append(issue("MED", "S4-INTERNAL", f"Sec4 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 5: FireFightingMeasures
# ---------------------------------------------------------------------------

def check_sec5(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        sec_text = section_text(root, "FireFightingMeasures")
        if len(sec_text.strip()) < 15:
            issues.append(issue("HIGH", "S5-EMPTY",
                                "Sec5: FireFightingMeasures section is empty (< 15 chars)"))
            return issues

        ext_keywords = re.compile(
            r"foam|water|CO2|carbon dioxide|powder|sand|dry chemical|halon|nitrogen|"
            r"inert gas|extinguish|泡|二酸化炭素|炭酸|粉末|砂|消火|灭火|水雾|dry sand|surrounding|appropriate|"
            r"水|泡沫|粉|ABC|foam|spray",
            re.IGNORECASE
        )
        if not ext_keywords.search(sec_text):
            issues.append(issue("MED", "S5-NO-EXTINGUISHING-AGENT",
                                "Sec5: No extinguishing agent keywords found"))

    except Exception as e:
        issues.append(issue("MED", "S5-INTERNAL", f"Sec5 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 6: AccidentalReleaseMeasures
# ---------------------------------------------------------------------------

def check_sec6(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        sec_text = section_text(root, "AccidentalReleaseMeasures")
        if len(sec_text.strip()) < 30:
            issues.append(issue("MED", "S6-EMPTY",
                                "Sec6: AccidentalReleaseMeasures section is empty (< 30 chars)"))
            return issues

        # r28: added Chinese character variants — JA Kanji differ from ZH characters in Unicode:
        # 吸收(zh U+6536) ≠ 吸収(JA U+53CE), 收集(zh) ≠ 収集(JA), 回收(zh) ≠ 回収(JA)
        # 沙(zh sand U+6C99) ≠ 砂(JA U+7802). Also added 围堵/收容/处置 (zh-tw/zh-cn containment).
        cleanup_kw = re.compile(
            r"absorb|collect|sweep|dike|sand|berm|ventilat|"
            r"吸収|吸收|回収|回收|吸附|収集|收集|围堤|围堵|通风|通風|盛土|"
            r"乾燥砂|おがくず|乾燥|砂|沙|吸着|囲|收容|处置|處置",
            re.IGNORECASE
        )
        if not cleanup_kw.search(sec_text):
            issues.append(issue("MED", "S6-NO-CLEANUP-KEYWORDS",
                                "Sec6: No cleanup/containment keywords found"))

    except Exception as e:
        issues.append(issue("MED", "S6-INTERNAL", f"Sec6 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 7: HandlingAndStorage
# ---------------------------------------------------------------------------

def check_sec7(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        hs = root.get("HandlingAndStorage") or {}
        handling_text = walk_text(hs.get("SafeHandling") or {})
        storage_text = walk_text(hs.get("Storage") or {})
        sec_text = section_text(root, "HandlingAndStorage")

        if not handling_text.strip() and not storage_text.strip():
            issues.append(issue("HIGH", "S7-BOTH-ABSENT",
                                "Sec7: Both Handling and Storage information are completely absent"))
            return issues

        if h_codes.intersection({"H224", "H225", "H226"}):
            if not re.search(r"cool|heat|ignition|flame|spark|火気|冷所|远离|冷暗|炎|点火", sec_text, re.IGNORECASE):
                issues.append(issue("MED", "S7-FLAMMABLE-NO-HEAT-KW",
                                    "Sec7: H224/225/226 but no heat/ignition source keywords"))

        # Flammable: storage should mention a specific temperature limit or cool-storage directive
        # r30: added EN 'keep cool'/'cool place'/'cool.*dry' and ZH '阴凉'/'陰涼' (Traditional
        # U+9670+U+6DBC differs from Simplified U+9634+U+51C9) to suppress generic cool-storage
        # instructions that lack a numeric limit but are legitimate guidance (8 FPs → 0).
        if h_codes.intersection({"H224", "H225", "H226"}):
            if not re.search(r"\d+\s*[°℃]|\d+\s*°C|\d+\s*degrees?|below\s+\d+", storage_text, re.IGNORECASE):
                if not re.search(
                    r"涼しい|冷所|冷暗|low temperature|冷凉处|低温|"
                    r"keep cool|cool place|cool.*area|cool.*dry|cool.*location|"
                    r"阴凉|陰涼",
                    storage_text, re.IGNORECASE
                ):
                    issues.append(issue("MED", "S7-FLAMMABLE-NO-STORAGE-TEMP",
                                        "Sec7: Flammable H-code but no specific storage temperature found"))

        if h_codes.intersection({"H260", "H261", "H250"}):
            if not re.search(r"dry|moisture|water|乾燥|防湿|水分|湿気|dry", sec_text, re.IGNORECASE):
                issues.append(issue("MED", "S7-WATER-REACTIVE-NO-DRY",
                                    "Sec7: H260/261/250 but no dry/moisture keywords"))

        if h_codes.intersection({"H330", "H331", "H332", "H333", "H334", "H335", "H224", "H225", "H226"}):
            if not re.search(r"ventilat|exhaust|fume hood|換気|局排|通风|排気|ventilation", sec_text, re.IGNORECASE):
                issues.append(issue("MED", "S7-VOLATILE-NO-VENTILATION",
                                    "Sec7: Volatile/toxic H-code but no ventilation keywords"))

    except Exception as e:
        issues.append(issue("MED", "S7-INTERNAL", f"Sec7 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 8: ExposureControlPersonalProtection
# ---------------------------------------------------------------------------

def check_sec8(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        ec = root.get("ExposureControlPersonalProtection") or {}
        eng_controls = ec.get("AppropriateEngineeringControls") or ec.get("EngineeringControls") or []
        ppe = ec.get("PersonalProtectionEquipment") or {}
        oel = ec.get("OccupationalExposureLimits") or ec.get("OEL") or []

        ec_text = section_text(root, "ExposureControlPersonalProtection")

        if not walk_text(eng_controls).strip() and not walk_text(ppe).strip() and not walk_text(oel).strip():
            issues.append(issue("HIGH", "S8-ALL-ABSENT",
                                "Sec8: EngineeringControls, PPE, and OEL all absent"))
            return issues

        # PPE sub-fields
        if is_hazardous(root):
            ppe_fields = [
                "EyeProtection", "HandProtection", "RespiratoryProtection",
                "SkinProtection", "BodyProtection"
            ]
            populated_ppe = sum(1 for f in ppe_fields if walk_text(ppe.get(f) or {}).strip())
            if populated_ppe < 2:
                issues.append(issue("MED", "S8-FEW-PPE-FIELDS",
                                    f"Sec8: Hazardous product with fewer than 2 PPE sub-fields populated ({populated_ppe})"))

        # Engineering controls absent when PPE exists
        if is_hazardous(root) and not walk_text(eng_controls).strip():
            if walk_text(ppe).strip():
                issues.append(issue("MED", "S8-NO-ENG-CONTROLS",
                                    "Sec8: Hazardous product has PPE but no engineering controls (local exhaust/ventilation) specified"))

        # OEL for single-substance hazardous
        if is_hazardous(root) and not is_mixture(root):
            if not walk_text(oel).strip():
                issues.append(issue("MED", "S8-NO-OEL",
                                    "Sec8: Hazardous single-substance product with no OEL"))

        # r23-NEW / r24-fix: OEL present but no numeric value
        oel_text = walk_text(oel)
        # Detect numeric value — handles "5 ppm", "MAC(mg/m3)：5" (unit before value),
        # and footnote-annotated values like "0.05* mg/m³" (r29: asterisk between num and unit)
        _oel_has_num = bool(
            re.search(r"\d+\.?\d*[*＊]*\s*(mg/m|ppm|mg/L|f/cc|µg)", oel_text, re.IGNORECASE) or
            re.search(r"(mg/m\d*|ppm|mg/L)[^0-9A-Za-z]{0,5}\d+\.?\d*", oel_text, re.IGNORECASE)
        )
        if oel_text.strip() and not _oel_has_num:
            if not re.search(
                    r"設定されていない|not established|not set|no limit|not available|情報なし|なし|N/A|"
                    r"does not contain|含有していない|含まれていない|限界値.*含有|"
                    r"no hazardous material|no applicable|not required|no substances.*limit|"
                    r"没有.*接触限值|无职业接触限值|不适用|无需监控|"
                    r"未制订|无资料|不监控|监视.*不.*含|该产品不含|"
                    r"不要求|"                                      # zh-cn: OEL not required
                    r"未設定|"                                      # ja: not established (r31)
                    r"使用粉尘的职业接触限值|在控制.*职业接触限值|"      # zh-cn: "refer to dust OEL" (r33)
                    r"不含.*職業接觸限值|不含.*職業暴露極限值|"         # zh-tw Traditional (r31)
                    r"不含有职业接触限值|不含.*职业接触限值|"            # zh-cn Simplified (r32: 职 U+804C ≠ 職 U+8077)
                    r"无.*职业接触限值的物质|"                         # zh-cn: no substances with OEL (r32)
                    r"無相關|無相關職業接觸限值|無相關職業暴露|"          # zh-tw: no relevant OEL data (r31)
                    r"無資料|"                                       # zh-tw Traditional 'no data' (r32: 無 U+7121 ≠ 无 U+65E0)
                    r"\b無\b|\b无\b|"                               # standalone 'none' (r32: \b safe for CJK; won't match 無資料)
                    r"No biological limit|no.*biological.*limit|not.*allocated|"  # EN (r32: shell/en)
                    r"無職業接觸限值|未建立職業接觸限值|無職業接觸限值資料|"  # zh-tw Traditional (U+7121 ≠ U+65E0)
                    r"[：:]\s*[-－—]\s*[；;]|[：:]\s*[-－—]\s*$",  # dash as N/A: "TWA：－"
                    oel_text, re.IGNORECASE):
                issues.append(issue("MED", "S8-OEL-NO-NUMERIC",
                                    "Sec8: OEL present but contains no numeric value (ppm/mg/m³ etc.)"))

        # H314 but no face shield/goggles
        if "H314" in h_codes:
            if not re.search(r"face shield|goggles|フェイス|ゴーグル|面罩|护目", ec_text, re.IGNORECASE):
                issues.append(issue("MED", "S8-H314-NO-FACE-SHIELD",
                                    "Sec8: H314 (corrosive) but no face shield/goggles keywords"))

        # Skin/corrosive H-code but no glove material
        if h_codes.intersection({"H314", "H315", "H316", "H317"}):
            glove_kw = re.compile(
                r"nitrile|butyl|neoprene|rubber|latex|viton|PVC|polyethylene|"
                r"ニトリル|ブチル|ネオプレン|ゴム|丁腈|丁基|氯丁|橡胶|橡膠",  # r29: 橡膠 Traditional (U+81A0 ≠ U+80F6)
                re.IGNORECASE
            )
            hand_prot = walk_text(ppe.get("HandProtection") or {})
            if hand_prot.strip() and not glove_kw.search(hand_prot):
                issues.append(issue("MED", "S8-SKIN-NO-GLOVE-MATERIAL",
                                    "Sec8: Skin/corrosive H-code but glove material not specified"))

        # Inhalation H-code but no respirator type
        if h_codes.intersection({"H330", "H331", "H332", "H333", "H334", "H335"}):
            resp_kw = re.compile(
                r"P1|P2|P3|A1|ABEK|FFP|half mask|full face|SCBA|P100|organic vapor|"
                r"防毒|防じん|送気|有机蒸气|防尘|organic vapour|half-face|full-face",
                re.IGNORECASE
            )
            resp_prot = walk_text(ppe.get("RespiratoryProtection") or {})
            if resp_prot.strip() and not resp_kw.search(resp_prot):
                issues.append(issue("MED", "S8-INHAL-NO-RESP-TYPE",
                                    "Sec8: Inhalation H-code but respirator type not specified"))

    except Exception as e:
        issues.append(issue("MED", "S8-INTERNAL", f"Sec8 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 9: PhysicalChemicalProperties
# ---------------------------------------------------------------------------

def check_sec9(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        phys = root.get("PhysicalChemicalProperties") or {}
        base = phys.get("BasePhysicalChemicalProperties") or {}
        sec_text = section_text(root, "PhysicalChemicalProperties")

        colour = to_str(base.get("Colour") or base.get("Appearance"))
        physical_state = to_str(base.get("PhysicalState"))

        if not colour and not physical_state:
            issues.append(issue("HIGH", "S9-NO-COLOUR-STATE",
                                "Sec9: Both Colour/Appearance and PhysicalState are absent"))

        if is_hazardous(root) and not to_str(base.get("Odour")):
            issues.append(issue("MED", "S9-NO-ODOUR",
                                "Sec9: Odour not extracted for hazardous product"))

        # Density/relative density
        densities = phys.get("Densities") or phys.get("Density") or []
        density_text = walk_text(densities) + walk_text(base.get("RelativeDensity") or {})
        if not density_text.strip() or density_text.strip() in ("情報なし", "N/A", "no data"):
            # Check if it has numeric
            density_vals = extract_numeric_values(densities)
            if not density_vals:
                issues.append(issue("MED", "S9-NO-DENSITY",
                                    "Sec9: Density/RelativeDensity not extracted"))

        # Water solubility
        sol = phys.get("Solubilities") or {}
        water_sol = sol.get("WaterSolubility") or []
        if not walk_text(water_sol).strip():
            # Also check OtherPhysicalChemicalProperty for 水溶性/溶解度
            other_props_text = walk_text(phys.get("OtherPhysicalChemicalProperty") or {})
            if not re.search(r"水溶|water solub|水分散|water\s*solub|溶解度|solubility", other_props_text, re.IGNORECASE):
                issues.append(issue("MED", "S9-NO-WATER-SOL",
                                    "Sec9: Water solubility not extracted"))

        # Flash point
        fp_val = get_flash_point_value(root)
        fp_list = phys.get("FlashPoint") or []
        if isinstance(fp_list, dict):
            fp_list = [fp_list]

        if fp_list:
            fp_text = walk_text(fp_list)
            # Check if value is non-numeric (string instead of number)
            # r31: 'if fp_val is None' replaces 'if not fp_val' — Python 'not 0.0' is True,
            # which caused akzonobel (ExactValue=0.0) to incorrectly fire this rule.
            # r31: added '沸騰するまで引火せず'/'引火しない' for substances with no flash point.
            if fp_val is None and fp_text.strip() and not re.search(
                r"情報なし|not applicable|n/a|なし|no data|"
                r"沸騰するまで引火せず|引火しない|非引火性|不燃|"
                r"None detected|not detected|no flash point detected|not measurable",  # r33: EN N/A phrases
                fp_text, re.IGNORECASE
            ):
                if re.search(r"[a-zA-Z぀-ヿ]", fp_text) and not re.search(r"\d", fp_text):
                    issues.append(issue("HIGH", "S9-FLASH-POINT-NOT-NUMERIC",
                                        f"Sec9: Flash point value is not numeric: '{fp_text[:60]}'"))

        if fp_val is not None:
            if not (-220 <= fp_val <= 400):
                issues.append(issue("MED", "S9-FLASH-POINT-RANGE",
                                    f"Sec9: Flash point {fp_val}°C is outside -220 to 400°C range"))

        flammable_codes = h_codes.intersection({"H224", "H225", "H226"})
        if flammable_codes and fp_val is None and not fp_list:
            issues.append(issue("MED", "S9-FLAMMABLE-NO-FP",
                                f"Sec9: {flammable_codes} present but no flash point extracted"))

        if "H224" in h_codes and fp_val is not None and fp_val >= 23:
            issues.append(issue("MED", "S9-H224-FP-TOO-HIGH",
                                f"Sec9: H224 but flash point {fp_val}°C >= 23°C (GHS requires < 23°C)"))

        if "H226" in h_codes and h_codes.isdisjoint({"H224", "H225"}):
            if fp_val is not None and not (23 <= fp_val < 60):
                issues.append(issue("MED", "S9-H226-FP-RANGE",
                                    f"Sec9: H226 alone but flash point {fp_val}°C is outside 23-60°C range"))

        # Boiling point
        bp_val = get_boiling_point_value(root)
        if fp_val is not None and bp_val is not None:
            if fp_val >= bp_val:
                issues.append(issue("MED", "S9-FP-GTE-BP",
                                    f"Sec9: Flash point {fp_val}°C >= boiling point {bp_val}°C (physically impossible)"))

        # r23-NEW: Boiling point range
        if bp_val is not None:
            if not (-200 <= bp_val <= 3000):
                issues.append(issue("MED", "S9-BP-RANGE",
                                    f"Sec9: Boiling point {bp_val}°C outside -200 to 3000°C"))

        # Melting point
        mp_list = phys.get("MeltingPointRelated") or []
        mp_vals = extract_numeric_values(mp_list)

        ps_lower = physical_state.lower()
        if "liquid" in ps_lower or "液体" in physical_state or "液" in physical_state:
            if not bp_val and not walk_text(phys.get("BoilingPointRelated") or {}).strip():
                issues.append(issue("MED", "S9-LIQUID-NO-BP",
                                    "Sec9: PhysicalState is liquid but no boiling point extracted"))

        if any(kw in ps_lower or kw in physical_state for kw in ("solid", "固体", "固形", "crystalline", "結晶", "晶")):
            if not mp_vals and not walk_text(mp_list).strip():
                issues.append(issue("MED", "S9-SOLID-NO-MP",
                                    "Sec9: PhysicalState is solid/crystalline but no melting point extracted"))

        # Auto-ignition temperature
        auto_ign = phys.get("AutoIgnitionTemperature") or []
        # Also check OtherPhysicalChemicalProperty for auto-ignition
        other_props = phys.get("OtherPhysicalChemicalProperty") or []
        auto_ign_text = walk_text(auto_ign)
        for prop in other_props:
            if isinstance(prop, dict):
                item_name = (prop.get("ItemName") or "").lower()
                if re.search(r"auto.ignit|autoignit|自然発火|自燃|引火", item_name, re.IGNORECASE):
                    auto_ign_text += walk_text(prop)

        if flammable_codes and not auto_ign_text.strip():
            issues.append(issue("MED", "S9-FLAMMABLE-NO-AUTOIGN",
                                "Sec9: Flammable H-code but no auto-ignition temperature extracted"))

        # r23-NEW: Auto-ignition below flash point
        auto_ign_vals = extract_numeric_values(auto_ign)
        if not auto_ign_vals:
            for prop in other_props:
                if isinstance(prop, dict):
                    item_name = (prop.get("ItemName") or "").lower()
                    if re.search(r"auto.ignit|自然発火|自燃|引火点", item_name, re.IGNORECASE):
                        auto_ign_vals = extract_numeric_values(prop)
        if auto_ign_vals and fp_val is not None:
            for ai_val in auto_ign_vals:
                if ai_val < fp_val:
                    issues.append(issue("MED", "S9-AUTOIGN-BELOW-FP",
                                        f"Sec9: Auto-ignition temperature {ai_val}°C is below flash point {fp_val}°C"))
                    break

        # Vapour pressure
        vp = phys.get("VapourPressure") or phys.get("VaporPressure") or []
        vp_text = walk_text(vp)
        if h_codes.intersection({"H224", "H225", "H226", "H330", "H331", "H332"}):
            if not vp_text.strip():
                issues.append(issue("MED", "S9-NO-VAPOUR-PRESSURE",
                                    "Sec9: Volatile/flammable H-code but no vapour pressure extracted"))

        # pH
        ph_found = False
        for prop in other_props:
            if isinstance(prop, dict):
                item_name = (prop.get("ItemName") or "").lower()
                if "ph" in item_name or "pH" in (prop.get("ItemName") or ""):
                    ph_found = True
                    ph_vals = extract_numeric_values(prop)
                    # r23-NEW: pH outside 0-14
                    for ph_v in ph_vals:
                        if not (0 <= ph_v <= 14):
                            issues.append(issue("MED", "S9-PH-RANGE",
                                                f"Sec9: pH value {ph_v} is outside 0 to 14"))
                    break
        # Also check BasePhysicalChemicalProperties for pH
        ph_base = base.get("pH") or base.get("ph") or {}
        if ph_base:
            ph_found = True
            ph_vals_b = extract_numeric_values(ph_base)
            for ph_v in ph_vals_b:
                if not (0 <= ph_v <= 14):
                    issues.append(issue("MED", "S9-PH-RANGE",
                                        f"Sec9: pH value {ph_v} is outside 0 to 14"))

        # r30: tightened from {H314,H290,H318,H319} to {H314,H290} only.
        # H318 (serious eye damage) and H319 (eye irritation) don't imply acidic/basic chemistry —
        # confirmed: acetone oxime (H318), IPA/ethanol/2-propanol (H319) all fired FP.
        # H314 (skin corrosion) and H290 (corrosive to metals) require acidic/basic chemistry.
        if h_codes.intersection({"H314", "H290"}):
            if not ph_found:
                issues.append(issue("MED", "S9-CORROSIVE-NO-PH",
                                    "Sec9: Corrosive/metal-corrosive H-code (H314/H290) but no pH extracted"))

        # r23-NEW: Density value range
        density_vals = extract_numeric_values(densities)
        for dv in density_vals:
            if not (0.1 <= dv <= 25):
                issues.append(issue("MED", "S9-DENSITY-RANGE",
                                    f"Sec9: Density value {dv} g/cm³ is outside 0.1 to 25"))
                break

    except Exception as e:
        issues.append(issue("MED", "S9-INTERNAL", f"Sec9 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 10: StabilityReactivity
# ---------------------------------------------------------------------------

def check_sec10(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        sec_text = section_text(root, "StabilityReactivity")
        if len(sec_text.strip()) < 30:
            issues.append(issue("MED", "S10-EMPTY",
                                "Sec10: StabilityReactivity section is empty (< 30 chars)"))
            return issues

        # r28: added EN terms ('temperature','oxidiz','decompos','reaction','normal processing',
        #      'oxidizing agent') and ZH terms ('稳定'/'穩定' stable, '氧化' oxidation,
        #      '反应'/'反應' reaction, '不相容' incompatible, '避免' avoid).
        # Rationale: EN fujifilm SDSs use "Extremes of temperature", "Strong oxidizing agents",
        # "None under normal processing" which are valid stability content missed by old pattern.
        # ZH (zh-cn/zh-tw) use '稳定'/'穩定' for stable (not JA '安定') and '氧化' not '酸化'.
        stability_kw = re.compile(
            r"avoid|heat|temperature|incompatible|oxidiz|decompos|acid|reaction|processing|"
            r"酸化|禁止|分解|stable|stability|"
            r"安定|不安定|稳定|穩定|反応|反应|反應|氧化|不相容|避免|"
            r"conditions|条件|materials|物質",
            re.IGNORECASE
        )
        if not stability_kw.search(sec_text):
            issues.append(issue("MED", "S10-NO-STABILITY-KEYWORDS",
                                "Sec10: No stability/reactivity keywords found"))

        # Reactive/oxidizing products: incompatible materials
        reactive_ox = h_codes.intersection({"H260", "H261", "H270", "H271", "H272",
                                             "H240", "H241", "H242", "H290"})
        if reactive_ox:
            sr = root.get("StabilityReactivity") or {}
            incompat_text = walk_text(sr.get("IncompatibleMaterials") or {})
            if not incompat_text.strip():
                if not re.search(r"incompatible|禁水|water|acid|alkali|oxidiz|避ける|禁止|回避|不相容",
                                 sec_text, re.IGNORECASE):
                    issues.append(issue("MED", "S10-NO-INCOMPATIBLE",
                                        f"Sec10: Reactive H-codes {reactive_ox} present but no incompatible materials mentioned"))

        # Decomposition products for explosive/flammable
        if h_codes.intersection({"H200", "H201", "H202", "H203", "H204", "H205",
                                   "H240", "H241", "H242"}):
            sr = root.get("StabilityReactivity") or {}
            decomp = (sr.get("HazardousDecompositionProducts") or {})
            if not walk_text(decomp).strip():
                issues.append(issue("MED", "S10-NO-DECOMP-PRODUCTS",
                                    "Sec10: H200-H205/H240-H242 but decomposition products absent"))

    except Exception as e:
        issues.append(issue("MED", "S10-INTERNAL", f"Sec10 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 11: ToxicologicalInformation
# ---------------------------------------------------------------------------

def check_sec11(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        tox_data = root.get("ToxicologicalInformation") or []
        if isinstance(tox_data, dict):
            tox_data = [tox_data]
        sec_text = section_text(root, "ToxicologicalInformation")

        if not sec_text.strip() or len(sec_text.strip()) < 20:
            issues.append(issue("HIGH", "S11-EMPTY",
                                "Sec11: ToxicologicalInformation section is completely empty"))
            return issues

        # Consolidate all tox entries
        tox_combined = {}
        for entry in tox_data:
            if isinstance(entry, dict):
                for k, v in entry.items():
                    if k not in tox_combined:
                        tox_combined[k] = v

        acute_tox_h = h_codes.intersection({
            "H300", "H301", "H302", "H310", "H311", "H312", "H330", "H331", "H332"
        })
        if acute_tox_h:
            at = tox_combined.get("AcuteToxicity")
            if not at or not walk_text(at).strip():
                issues.append(issue("MED", "S11-ACUTE-TOX-MISSING",
                                    f"Sec11: Acute-tox H-code {acute_tox_h} but AcuteToxicity not extracted"))
            elif not re.search(r"LD50|LC50|mg/kg|ml/kg|mg/l|mg/L|µg|lethal", sec_text, re.IGNORECASE):
                issues.append(issue("MED", "S11-NO-LD50",
                                    "Sec11: Acute-tox H-code but no LD50/LC50 value text found"))

        if "H315" in h_codes:
            sci = tox_combined.get("SkinCorrosionIrritation")
            if not sci or not walk_text(sci).strip():
                issues.append(issue("MED", "S11-H315-NO-SCI",
                                    "Sec11: H315 present but SkinCorrosionIrritation not extracted"))

        if h_codes.intersection({"H319", "H318"}):
            edi = tox_combined.get("EyeDamageOrIrritation")
            if not edi or not walk_text(edi).strip():
                issues.append(issue("MED", "S11-H319-NO-EDI",
                                    "Sec11: H319/H318 present but EyeDamageOrIrritation not extracted"))

        if "H334" in h_codes:
            sens = tox_combined.get("Sensitization")
            if not sens or not walk_text(sens).strip():
                issues.append(issue("MED", "S11-H334-NO-SENSITIZ",
                                    "Sec11: H334 present but Sensitization not extracted"))

        if h_codes.intersection({"H350", "H351"}):
            carc = tox_combined.get("Carcinogenicity")
            if not carc or not walk_text(carc).strip():
                issues.append(issue("MED", "S11-H350-NO-CARC",
                                    "Sec11: H350/H351 present but Carcinogenicity not extracted"))
            else:
                # r23-NEW: H350/H351 but no carcinogenicity classification reference
                if not re.search(r"IARC|NTP|ACGIH|WHO|NIOSH|Group|Class|カテゴリ|分类|分類",
                                 walk_text(carc), re.IGNORECASE):
                    issues.append(issue("MED", "S11-H350-NO-CARC-CLASS",
                                        "Sec11: H350/H351 present but no IARC/NTP/ACGIH/WHO classification found"))

        if h_codes.intersection({"H360", "H361"}):
            rt = tox_combined.get("ReproductiveToxicity")
            if not rt or not walk_text(rt).strip():
                issues.append(issue("MED", "S11-H360-NO-REPTOX",
                                    "Sec11: H360/H361 present but ReproductiveToxicity not extracted"))

        stot_codes = h_codes.intersection({"H370", "H371", "H372", "H373"})
        if stot_codes:
            sto = tox_combined.get("SpecificTargetOrgan")
            if not sto or not walk_text(sto).strip():
                issues.append(issue("MED", "S11-H370-NO-STOT",
                                    f"Sec11: {stot_codes} present but SpecificTargetOrgan not extracted"))

    except Exception as e:
        issues.append(issue("MED", "S11-INTERNAL", f"Sec11 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 12: EcologicalInformation
# ---------------------------------------------------------------------------

def check_sec12(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        eco_data = root.get("EcologicalInformation") or []
        if isinstance(eco_data, dict):
            eco_data = [eco_data]
        sec_text = section_text(root, "EcologicalInformation")

        env_h = h_codes.intersection({"H400", "H401", "H402", "H410", "H411",
                                       "H412", "H413", "H420"})

        if env_h and len(sec_text.strip()) < 20:
            issues.append(issue("HIGH", "S12-ENV-HCODE-EMPTY",
                                f"Sec12: Environmental H-code {env_h} present but section is empty"))
            return issues

        if env_h.difference({"H420"}):  # H4xx except H420
            aquatic_kw = re.compile(
                r"aquatic|fish|daphnia|algae|LC50|EC50|水生|魚|甲殻|藻類|水产|毒性",
                re.IGNORECASE
            )
            if not aquatic_kw.search(sec_text):
                issues.append(issue("MED", "S12-NO-AQUATIC-KEYWORDS",
                                    "Sec12: Environmental H-code but no aquatic toxicity keywords"))

        if h_codes.intersection({"H410", "H411"}):
            biodeg_kw = re.compile(
                r"biodeg|bioaccum|BCF|PersistenceDeg|生分解|生物濃縮|生物蓄積|持続性|生态|降解|蓄积",
                re.IGNORECASE
            )
            if not biodeg_kw.search(sec_text):
                issues.append(issue("MED", "S12-H410-NO-BIODEG",
                                    "Sec12: H410/H411 but no biodegradability/bioaccumulation keywords"))

        if env_h.difference({"H420"}):
            logp_kw = re.compile(
                r"LogP|Kow|BCF|partition coefficient|分配係数|辛醇|logkow|log P|正辛醇|オクタノール",
                re.IGNORECASE
            )
            _logp_in_sec12 = logp_kw.search(sec_text)
            # r30: also search Section 9 OtherPhysicalChemicalProperty for numeric LogP.
            # JIS Z 7253 allows partition coefficient in Sec 9 OR Sec 12. fujifilm_wako
            # SDSs list 'n-オクタノール/水分配係数' in OtherPhysicalChemicalProperty with a
            # numeric value (e.g. 2.64). Only suppress when Section 9 has a NUMERIC value
            # (not "Not specified"/"データなし") to preserve true positives.
            _logp_in_sec9 = False
            if not _logp_in_sec12:
                _phys = root.get("PhysicalChemicalProperties") or {}
                _other = _phys.get("OtherPhysicalChemicalProperty") or []
                if isinstance(_other, dict):
                    _other = [_other]
                for _prop in _other:
                    if not isinstance(_prop, dict):
                        continue
                    _iname = _prop.get("ItemName") or ""
                    if logp_kw.search(_iname):
                        if extract_numeric_values(_prop):
                            _logp_in_sec9 = True
                            break
            if not _logp_in_sec12 and not _logp_in_sec9:
                issues.append(issue("MED", "S12-NO-LOGP",
                                    "Sec12: Environmental H-code but no LogP/Kow/BCF value (checked Sec9 and Sec12)"))

        if is_hazardous(root) and len(sec_text.strip()) < 20:
            issues.append(issue("MED", "S12-HAZARDOUS-EMPTY",
                                "Sec12: Hazardous product with empty EcologicalInformation"))

        # r23-NEW: H420 (ozone depleter) but no ODP/ozone keywords
        if "H420" in h_codes:
            ozone_kw = re.compile(
                r"ODP|ozone|stratosph|オゾン|臭氧|オゾン破壊|ODS",
                re.IGNORECASE
            )
            if not ozone_kw.search(sec_text):
                issues.append(issue("MED", "S12-H420-NO-OZONE",
                                    "Sec12: H420 (ozone depleter) present but no ODP/ozone keywords"))

    except Exception as e:
        issues.append(issue("MED", "S12-INTERNAL", f"Sec12 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 13: DisposalConsiderations
# ---------------------------------------------------------------------------

def check_sec13(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        sec_text = section_text(root, "DisposalConsiderations")
        if not sec_text.strip():
            issues.append(issue("MED", "S13-EMPTY",
                                "Sec13: DisposalConsiderations section is empty"))
            return issues

        # r28: added zh-tw Traditional Chinese disposal terms and zh-cn variants.
        # r31: added gas-specific disposal keywords (sandiego/fpcc/sh_gas compressed-gas SDSs):
        #   dissipat/消散 (release to atmosphere), 气瓶/氣瓶 (gas cylinder return),
        #   返.*供应商/返.*供應商 (return to supplier).
        #   Added zh-tw Traditional '法規' (U+898F ≠ Simplified 法规 U+89C4) and '環保'.
        disposal_kw = re.compile(
            r"inciner|landfill|waste|regulation|dissipat|"
            r"廃棄|廢棄|廃液|廢液|焼却|焚烧|焚燒|"
            r"废物|廢物|廢棄物|処分|処理|處理|"
            r"处置|處置|废弃|廢棄|丢弃|弃置|"
            r"官方.*规章|规章|法规|法律.*处置|处置.*法规|"
            r"消散|气瓶|氣瓶|返.*供[应應]商|"          # gas disposal (r31)
            r"法規|環保法規",                           # zh-tw Traditional regulation (r31)
            re.IGNORECASE
        )
        if not disposal_kw.search(sec_text):
            issues.append(issue("MED", "S13-NO-DISPOSAL-KEYWORDS",
                                "Sec13: No disposal method or regulation keywords found"))

    except Exception as e:
        issues.append(issue("MED", "S13-INTERNAL", f"Sec13 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 14: TransportInformation
# ---------------------------------------------------------------------------

def check_sec14(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        if not section_populated(root, "TransportInformation"):
            issues.append(issue("MED", "S14-MISSING",
                                "Sec14: TransportInformation section is missing"))
            return issues

        sec_text = section_text(root, "TransportInformation")

        # UN number detection — r27: extended to catch zh-tw/zh-cn formats
        # Standard: "UN 1234" or "UN1234"
        # zh-tw: "聯合國編號(UN No.)：1990"  or "UN No.)：1990"
        # zh-cn: "联合国编号：1990" or "联合国危险货物编号 1990"
        UN_RE = re.compile(
            r"\bUN\s?\d{4}\b"                          # standard: UN 1234
            r"|UN\s*[Nn][Oo][\.\s]*[)）]?\s*[：:]\s*\d{4}"  # UN No.)：1990
            r"|聯合國編號[^0-9]{0,10}\d{4}"             # 聯合國編號(UN No.)：1990
            r"|联合国编号[^0-9]{0,10}\d{4}"             # 联合国编号：1990
            r"|联合国危险货物编号[^0-9]{0,10}\d{4}",    # zh-cn extended
            re.IGNORECASE,
        )
        un_match = UN_RE.search(sec_text)
        not_regulated = NOT_REGULATED_PATTERNS.search(sec_text)

        # Dangerous goods H-codes present but no UN number
        dg_h = h_codes.intersection(DG_H_CODES)
        if dg_h and not un_match and not not_regulated:
            issues.append(issue("MED", "S14-DG-NO-UN",
                                f"Sec14: Dangerous goods H-codes {dg_h} present but no UN number found"))

        if un_match:
            # UN format check — only apply to western-format tokens (avoid false hits on Chinese text)
            for bad_match in re.finditer(r"\bUN[-\s]?\d+\b", sec_text, re.IGNORECASE):
                token = bad_match.group()
                if not re.match(r"^UN\s?\d{4}$", token, re.IGNORECASE):
                    issues.append(issue("MED", "S14-UN-FORMAT",
                                        f"Sec14: UN number format not matching UN+4digits: '{token}'"))
                    break

            # Packing group — r27: added zh-tw '包裝類別'/'包裝等級' and Unicode Roman numerals Ⅰ-Ⅳ
            if not re.search(
                r"packing group|PG\s?[IVXivx]+|危険物容器|容器等級|容器包装等級|"
                r"包裝類別|包裝等級|包装类别|[ⅠⅡⅢⅣⅤ]",
                sec_text, re.IGNORECASE
            ):
                issues.append(issue("MED", "S14-NO-PACKING-GROUP",
                                    "Sec14: UN number found but Packing Group not extracted"))

            # Proper shipping name — r27: added zh-tw '聯合國運輸名稱' / zh-cn '运输名称'
            if not re.search(
                r"proper shipping|品名|品番|shipping name|品目名|正式品名|"
                r"聯合國運輸名稱|运输名称|運輸名稱",
                sec_text, re.IGNORECASE
            ):
                issues.append(issue("MED", "S14-NO-SHIPPING-NAME",
                                    "Sec14: UN number found but Proper Shipping Name not extracted"))

    except Exception as e:
        issues.append(issue("MED", "S14-INTERNAL", f"Sec14 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 15: RegulatoryInformation
# ---------------------------------------------------------------------------

def check_sec15(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        sec_text = section_text(root, "RegulatoryInformation")
        if not sec_text.strip():
            issues.append(issue("MED", "S15-EMPTY",
                                "Sec15: RegulatoryInformation section is empty"))
            return issues

        # Note: walk_text flattens *values*, not keys — so JSON key names like
        # "LegislationName" won't appear in sec_text; only their values do.
        # r28: added Chinese regulatory terms missing from previous pattern:
        # '办法'/'管理办法' (administrative regulations), '规范'/'規範' (standards/specifications),
        # '名录'/'名錄' (chemical inventories like IECSC), '管制' (control/regulated),
        # 'IECSC' (China Existing Chemicals Inventory). These appear in strem zh-cn SDSs
        # ("新化学物质环境管理办法", "中国现有化学物质名录") which are legitimate regulatory refs.
        law_kw = re.compile(
            r"law|regulation|安全衛生|化審法|消防法|毒劇法|化管法|GB|REACH|OSHA|RoHS|CLP|"
            r"規制|法令|法律|规定|立法|条例|基準|指令|directive|労働基準|化学物質|"
            r"办法|管理办法|规范|規範|名录|名錄|管制|IECSC|化学品|危险化学|"
            r"安全生产|化学品登记|环境管理",
            re.IGNORECASE
        )
        if not law_kw.search(sec_text):
            issues.append(issue("MED", "S15-NO-LAW-KEYWORDS",
                                "Sec15: No recognizable law or regulation keywords"))

        if lang == "ja":
            ja_law_kw = re.compile(
                r"労働安全衛生法|安衛法|化審法|毒劇法|消防法|化管法|PRTR|安全衛生",
                re.IGNORECASE
            )
            if not ja_law_kw.search(sec_text):
                issues.append(issue("MED", "S15-JA-NO-JA-LAW",
                                    "Sec15: Japanese SDS but no Japanese law reference found"))

        if lang == "zh-cn":
            gb_kw = re.compile(r"GB\s?\d|GBZ|GB/T|GB\s+\d|国家标准", re.IGNORECASE)
            if not gb_kw.search(sec_text):
                issues.append(issue("MED", "S15-ZHCN-NO-GB",
                                    "Sec15: zh-cn SDS but no GB standard reference found"))

        if lang == "ja":
            prtr_codes = h_codes.intersection({"H350", "H351", "H340", "H341", "H400", "H410"})
            if prtr_codes:
                if not re.search(r"PRTR|化管法|化学物質管理促進", sec_text):
                    issues.append(issue("MED", "S15-JA-NO-PRTR",
                                        f"Sec15: Japanese SDS with {prtr_codes} but no PRTR/化管法 reference"))

    except Exception as e:
        issues.append(issue("MED", "S15-INTERNAL", f"Sec15 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Section 16: OtherInformation / Datasheet
# ---------------------------------------------------------------------------

def check_sec16(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        datasheet = root.get("Datasheet") or {}
        other = root.get("OtherInformation") or {}

        # Collect dates from both Datasheet and OtherInformation
        dates_found = []
        issue_date = to_str(datasheet.get("IssueDate"))
        if issue_date:
            dates_found.append(issue_date)

        rev_dates = datasheet.get("RevisionDate") or []
        if isinstance(rev_dates, str):
            rev_dates = [rev_dates]
        for rd in rev_dates:
            if rd and str(rd).strip():
                dates_found.append(str(rd).strip())

        # Also look in OtherInformation text
        other_text = walk_text(other)
        date_matches = re.findall(r"\d{4}-\d{2}-\d{2}", other_text)
        dates_found.extend(date_matches)

        if not dates_found:
            issues.append(issue("MED", "S16-NO-DATE",
                                "Sec16: SDS date (IssueDate/RevisionDate) not extracted"))
        else:
            for date_str in dates_found[:3]:  # check up to 3 dates
                if not re.match(r"^\d{4}-\d{2}-\d{2}$", date_str):
                    issues.append(issue("MED", "S16-DATE-FORMAT",
                                        f"Sec16: Date format is not YYYY-MM-DD: '{date_str}'"))
                    break
                year = int(date_str[:4])
                if not (2000 <= year <= 2030):
                    issues.append(issue("MED", "S16-DATE-YEAR-RANGE",
                                        f"Sec16: Date year {year} outside 2000-2030 range"))
                    break
                if year < 2020:
                    issues.append(issue("MED", "S16-DATE-OLD",
                                        f"Sec16: SDS date {date_str} is before 2020 (older than 5 years)"))
                    break

        # r25-NEW: RevisionDate precedes IssueDate (impossible ordering — likely LLM swap)
        if issue_date and re.match(r"^\d{4}-\d{2}-\d{2}$", issue_date):
            valid_rev_dates = [rd for rd in rev_dates
                               if isinstance(rd, str) and re.match(r"^\d{4}-\d{2}-\d{2}$", rd)]
            for rd in valid_rev_dates:
                if rd < issue_date:
                    issues.append(issue("HIGH", "S16-REVISION-BEFORE-ISSUE",
                                        f"Sec16: RevisionDate ({rd}) precedes IssueDate ({issue_date}) — likely date swap"))
                    break

    except Exception as e:
        issues.append(issue("MED", "S16-INTERNAL", f"Sec16 check failed: {e}"))
    return issues


# ---------------------------------------------------------------------------
# Cross-field checks
# ---------------------------------------------------------------------------

def check_cross_field(root: dict, lang: str, h_codes: set) -> list:
    issues = []
    try:
        all_text = walk_text(root)

        # Placeholder detection
        placeholder_re = re.compile(
            r"\[insert|\[記入|PLACEHOLDER|TODO\b|TBD\b",
            re.IGNORECASE
        )
        if placeholder_re.search(all_text):
            issues.append(issue("HIGH", "CX-PLACEHOLDER",
                                "Cross: Placeholder text detected ([insert / [記入 / PLACEHOLDER / TODO / TBD)"))

        # Populated section counts
        populated = count_populated_sections(root)
        if populated < 10:
            issues.append(issue("HIGH", "CX-FEW-SECTIONS-HIGH",
                                f"Cross: Fewer than 10 of 16 JIS sections are populated ({populated}/16)"))
        elif populated < 13:
            issues.append(issue("MED", "CX-FEW-SECTIONS-MED",
                                f"Cross: Fewer than 13 of 16 JIS sections are populated ({populated}/16)"))

        # H290 corrosive to metals but no acid/halide in composition
        if "H290" in h_codes:
            comp_text = section_text(root, "Composition")
            if not re.search(r"acid|chloride|chloro|fluor|sulfate|hydro|酸|塩化|フッ化|硫酸|塩酸|硝酸|酸|氯|氟|硫",
                              comp_text, re.IGNORECASE):
                issues.append(issue("MED", "CX-H290-NO-ACID",
                                    "Cross: H290 (corrosive to metals) but no acid/halide keywords in composition"))

        # r29-NEW: zh-cn/zh-tw SDS with Japanese hiragana in critical chemical sections.
        # Hiragana is purely Japanese (not used in Chinese) — its presence indicates the LLM
        # sourced classification logic from Japanese templates, producing wrong-language content.
        # Pattern confirmed: strem zh-cn SDSs had "分類できない" (hiragana き/な/い) in Classification.
        # Only scan HazardIdentification and Composition (highest-risk sections for this artefact).
        if lang in ("zh-cn", "zh-tw"):
            _hiragana_re = re.compile(r"[ぁ-ゖ]")
            for sec_key in ("HazardIdentification", "Composition"):
                _sec_text = section_text(root, sec_key)
                if _hiragana_re.search(_sec_text):
                    issues.append(issue("HIGH", "CX-JA-TEXT-IN-ZH-SDS",
                                        f"Cross: Japanese hiragana found in {sec_key} of {lang} SDS "
                                        f"(LLM language contamination — e.g. '分類できない' in Classification)"))
                    break

        # r23-NEW: Identical text (>100 chars) in two different sections
        section_long_texts = {}
        for key in SECTION_KEYS_16:
            val = root.get(key)
            if val is None:
                continue
            # Collect individual strings > 100 chars
            long_strings = []
            _collect_long_strings(val, long_strings, min_len=100)
            section_long_texts[key] = long_strings

        seen_texts = {}  # text -> first section key
        for sec_key, texts in section_long_texts.items():
            for txt in texts:
                txt_norm = txt.strip()
                if txt_norm in seen_texts and seen_texts[txt_norm] != sec_key:
                    issues.append(issue("MED", "CX-DUPLICATE-SECTION-TEXT",
                                        f"Cross: Identical text (>100 chars) found in {seen_texts[txt_norm]} and {sec_key} (copy-paste artefact)"))
                    # Only report once
                    break
                seen_texts[txt_norm] = sec_key

        # r23-NEW: All H-codes from one family for mixture with >3 components
        comp = root.get("Composition") or {}
        components = comp.get("CompositionAndConcentration") or []
        if isinstance(components, dict):
            components = [components]
        if is_mixture(root) and len(components) > 3 and len(h_codes) >= 2:
            families = set()
            for hc in h_codes:
                if len(hc) >= 2:
                    families.add(hc[1])  # H2xx -> '2', H3xx -> '3', H4xx -> '4'
            if len(families) == 1:
                issues.append(issue("MED", "CX-SINGLE-HCODE-FAMILY",
                                    f"Cross: All {len(h_codes)} H-codes are from one family (H{list(families)[0]}xx) "
                                    f"for a mixture with {len(components)} components — possible partial extraction"))

    except Exception as e:
        issues.append(issue("MED", "CX-INTERNAL", f"Cross-field check failed: {e}"))

    # Stale revision date (> 5 years)
    try:
        from datetime import date
        datasheet = root.get("Datasheet") or {}
        rev_date_str = datasheet.get("RevisionDate") or ""
        if rev_date_str and len(rev_date_str) >= 4:
            rev_year = int(rev_date_str[:4])
            if date.today().year - rev_year > 5:
                issues.append(issue("MED", "CROSS-STALE-DATE",
                                    f"Revision date {rev_date_str} is over 5 years old"))
    except Exception:
        pass

    return issues


def _collect_long_strings(obj, result: list, min_len: int = 100):
    """Recursively collect prose strings longer than min_len.
    Excludes strings with no spaces (chemical names, identifiers).
    """
    if isinstance(obj, str):
        # Only flag strings that look like prose (contain spaces and have multiple words)
        # This avoids false positives from long IUPAC names / CAS IDs appearing in multiple sections
        if len(obj) >= min_len and obj.count(" ") >= 5:
            result.append(obj)
    elif isinstance(obj, dict):
        for v in obj.values():
            _collect_long_strings(v, result, min_len)
    elif isinstance(obj, list):
        for item in obj:
            _collect_long_strings(item, result, min_len)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def run_checks(root: dict, lang: str) -> list:
    """Run all QC checks and return list of (level, rule_id, message) tuples."""
    h_codes = collect_h_codes(root)
    p_codes = collect_p_codes(root)

    all_issues = []
    all_issues.extend(check_sec1(root, lang, h_codes))
    all_issues.extend(check_sec2(root, lang, h_codes, p_codes))
    all_issues.extend(check_sec3(root, lang, h_codes))
    all_issues.extend(check_sec4(root, lang, h_codes))
    all_issues.extend(check_sec5(root, lang, h_codes))
    all_issues.extend(check_sec6(root, lang, h_codes))
    all_issues.extend(check_sec7(root, lang, h_codes))
    all_issues.extend(check_sec8(root, lang, h_codes))
    all_issues.extend(check_sec9(root, lang, h_codes))
    all_issues.extend(check_sec10(root, lang, h_codes))
    all_issues.extend(check_sec11(root, lang, h_codes))
    all_issues.extend(check_sec12(root, lang, h_codes))
    all_issues.extend(check_sec13(root, lang, h_codes))
    all_issues.extend(check_sec14(root, lang, h_codes))
    all_issues.extend(check_sec15(root, lang, h_codes))
    all_issues.extend(check_sec16(root, lang, h_codes))
    all_issues.extend(check_cross_field(root, lang, h_codes))
    return all_issues


def sort_issues(issues: list) -> list:
    order = {"CRIT": 0, "HIGH": 1, "MED": 2}
    return sorted(issues, key=lambda x: order.get(x[0], 9))


def main():
    parser = argparse.ArgumentParser(
        description="SDS JSON Quality Check — r23",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("json_file", help="Path to the SDS JSON file")
    parser.add_argument("lang", choices=["ja", "en", "zh-cn", "zh-tw"],
                        help="Language of the SDS")
    parser.add_argument("--jsonl", action="store_true",
                        help="Append one JSON-Lines record at end of stdout")
    args = parser.parse_args()

    try:
        with open(args.json_file, "r", encoding="utf-8") as f:
            root = json.load(f)
    except FileNotFoundError:
        print(f"ERROR: File not found: {args.json_file}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"ERROR: Invalid JSON: {e}", file=sys.stderr)
        sys.exit(1)

    issues = sort_issues(run_checks(root, args.lang))

    n_crit = sum(1 for i in issues if i[0] == "CRIT")
    n_high = sum(1 for i in issues if i[0] == "HIGH")
    n_med = sum(1 for i in issues if i[0] == "MED")
    total = n_crit + n_high + n_med

    if total == 0:
        print("QC-OK: all quality checks passed")
    else:
        for level, rule_id, message in issues:
            print(f"QC-{level}: {message}")

    print(f"QC-SUMMARY: {n_crit} CRIT + {n_high} HIGH + {n_med} MED = {total} total issues")

    if args.jsonl:
        jsonl_record = {
            "file": args.json_file,
            "lang": args.lang,
            "crit": n_crit,
            "high": n_high,
            "med": n_med,
            "total": total,
            "issues": [
                {"level": lv, "rule": rid, "message": msg}
                for lv, rid, msg in issues
            ],
        }
        print(json.dumps(jsonl_record, ensure_ascii=False))

    sys.exit(total)


if __name__ == "__main__":
    main()
