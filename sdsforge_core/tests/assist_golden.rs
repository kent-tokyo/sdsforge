//! Golden acceptance fixture for assist v1 (Section 4 / first-aid
//! measures): a short fictional supplier SDS excerpt run through
//! `run_section4_assist` against a scripted backend, covering all four
//! exposure-route fields plus one hallucinated candidate that must be
//! rejected. Fully offline -- no network, no live model.

use std::collections::HashMap;
use std::sync::Mutex;

use sdsforge_core::{run_section4_assist, ConfidenceLevel, EvidenceLevel, LlmBackend, SdsError};

/// A short, fictional supplier SDS excerpt -- not a real product.
const GOLDEN_SOURCE_TEXT: &str = "\
SAFETY DATA SHEET (fictional, for testing only)
Product: GoldenFix Industrial Degreaser

SECTION 4: FIRST-AID MEASURES
Inhalation: Move person to fresh air. If breathing is difficult, give oxygen.
Skin contact: Remove contaminated clothing. Wash skin thoroughly with soap and water for at least 15 minutes.
Eye contact: Rinse cautiously with water for at least 15 minutes. Remove contact lenses if present.
Ingestion: Do NOT induce vomiting. Rinse mouth with water. Seek immediate medical attention.
";

const GOLDEN_SOURCE_SHA: &str = "golden-fixture-sha256-stand-in";

/// The scripted model response for the golden fixture: four candidates
/// quoting the source verbatim, plus one hallucinated candidate (an
/// excerpt that does not appear anywhere in `GOLDEN_SOURCE_TEXT`) to prove
/// the golden case also exercises rejection, not just the happy path.
fn golden_llm_response() -> String {
    serde_json::json!([
        {
            "path": "FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText",
            "proposed_value": "Move person to fresh air. If breathing is difficult, give oxygen.",
            "source_page": 1,
            "source_excerpt": "Move person to fresh air. If breathing is difficult, give oxygen.",
            "rationale": "quoted from the Inhalation line"
        },
        {
            "path": "FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText",
            "proposed_value": "Remove contaminated clothing. Wash skin thoroughly with soap and water for at least 15 minutes.",
            "source_page": 1,
            "source_excerpt": "Remove contaminated clothing. Wash skin thoroughly with soap and water for at least 15 minutes.",
            "rationale": "quoted from the Skin contact line"
        },
        {
            "path": "FirstAidMeasures.ExposureRoute.FirstAidEye.FullText",
            "proposed_value": "Rinse cautiously with water for at least 15 minutes. Remove contact lenses if present.",
            "source_page": 1,
            "source_excerpt": "Rinse cautiously with water for at least 15 minutes. Remove contact lenses if present.",
            "rationale": "quoted from the Eye contact line"
        },
        {
            "path": "FirstAidMeasures.ExposureRoute.FirstAidIngestion.FullText",
            "proposed_value": "Do NOT induce vomiting. Rinse mouth with water. Seek immediate medical attention.",
            "source_page": 1,
            "source_excerpt": "Do NOT induce vomiting. Rinse mouth with water. Seek immediate medical attention.",
            "rationale": "quoted from the Ingestion line"
        },
        {
            "path": "FirstAidMeasures.MedicalAttentionAndSpecialTreatmentNeeded.FullText",
            "proposed_value": "Administer atropine immediately.",
            "source_page": 1,
            "source_excerpt": "Administer atropine immediately.",
            "rationale": "hallucinated -- does not appear in the source document"
        }
    ])
    .to_string()
}

struct ScriptedBackend {
    response: String,
    captured_user_prompt: Mutex<Option<String>>,
}

impl LlmBackend for ScriptedBackend {
    async fn complete(&self, _system: &str, user: &str) -> Result<String, SdsError> {
        *self.captured_user_prompt.lock().unwrap() = Some(user.to_string());
        Ok(self.response.clone())
    }
}

#[tokio::test]
async fn golden_section4_fixture_emits_verified_candidates_and_rejects_the_hallucination() {
    let backend = ScriptedBackend {
        response: golden_llm_response(),
        captured_user_prompt: Mutex::new(None),
    };

    let run = run_section4_assist(
        &backend,
        "goldenfix-industrial-degreaser-sds.pdf",
        GOLDEN_SOURCE_SHA,
        GOLDEN_SOURCE_TEXT,
        "anthropic",
        "claude-golden-test",
    )
    .await
    .expect("golden fixture response is well-formed JSON");

    assert_eq!(run.proposals.len(), 4, "the four verbatim candidates must all survive");
    assert_eq!(run.warnings.len(), 1, "the hallucinated candidate must be rejected, not silently dropped");
    assert!(run.warnings[0].contains("not found in extracted source text"));

    assert_eq!(run.source_evidence_level, EvidenceLevel::SupplierSds);
    assert_eq!(run.extraction_method, "llm_extraction");
    assert_eq!(run.schema_version, "1");

    let by_path: HashMap<&str, &sdsforge_core::AssistProposal> =
        run.proposals.iter().map(|p| (p.path.as_str(), p)).collect();
    assert!(by_path.contains_key("FirstAidMeasures.ExposureRoute.FirstAidInhalation.FullText"));
    assert!(by_path.contains_key("FirstAidMeasures.ExposureRoute.FirstAidSkin.FullText"));
    assert!(by_path.contains_key("FirstAidMeasures.ExposureRoute.FirstAidEye.FullText"));
    assert!(by_path.contains_key("FirstAidMeasures.ExposureRoute.FirstAidIngestion.FullText"));
    // The hallucinated MedicalAttention candidate must not appear.
    assert!(!by_path.contains_key("FirstAidMeasures.MedicalAttentionAndSpecialTreatmentNeeded.FullText"));

    // Every surviving proposal is Medium confidence, has a unique host-assigned
    // id, and is byte-reproducible from the same scripted input.
    let mut ids = std::collections::HashSet::new();
    for p in &run.proposals {
        assert_eq!(p.confidence, ConfidenceLevel::Medium);
        assert!(p.id.starts_with("assist-"));
        assert!(ids.insert(p.id.clone()), "proposal ids must be unique: {}", p.id);
        // The scripted response claims source_page: 1 for every candidate --
        // extract_text has no page boundaries, so that claim must never
        // survive into the emitted proposal.
        assert_eq!(p.source_page, None);
    }

    let rerun = run_section4_assist(
        &backend,
        "goldenfix-industrial-degreaser-sds.pdf",
        GOLDEN_SOURCE_SHA,
        GOLDEN_SOURCE_TEXT,
        "anthropic",
        "claude-golden-test",
    )
    .await
    .unwrap();
    let first_json = serde_json::to_string_pretty(&run).unwrap();
    let rerun_json = serde_json::to_string_pretty(&rerun).unwrap();
    assert_eq!(first_json, rerun_json, "repeated runs with the same input must be byte-equivalent");
}
