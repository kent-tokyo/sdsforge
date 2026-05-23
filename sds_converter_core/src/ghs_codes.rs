//! GHS Rev.10 H-code and P-code validation and descriptions.
//!
//! Codes are validated against the actual GHS code list (not just range checks).
//! Compound P-codes like `P301+P330+P331` are split on `+` before validation.

static H_CODES: &[&str] = &[
    "H200", "H201", "H202", "H203", "H204", "H205", "H206", "H207", "H208",
    "H220", "H221", "H222", "H223", "H224", "H225", "H226", "H227", "H228",
    "H229", "H230", "H231", "H232",
    "H240", "H241", "H242",
    "H250", "H251", "H252",
    "H260", "H261",
    "H270", "H271", "H272",
    "H280", "H281", "H282", "H283", "H284",
    "H290",
    "H300", "H301", "H302", "H303", "H304", "H305",
    "H310", "H311", "H312", "H313",
    "H314", "H315", "H316", "H317", "H318", "H319", "H320",
    "H330", "H331", "H332", "H333", "H334", "H335", "H336",
    "H340", "H341",
    "H350", "H351",
    "H360", "H361", "H362",
    "H370", "H371", "H372", "H373",
    "H400", "H401", "H402",
    "H410", "H411", "H412", "H413",
    "H420",
];

static P_CODES: &[&str] = &[
    "P101", "P102", "P103",
    "P201", "P202",
    "P210", "P211", "P212",
    "P220", "P221", "P222", "P223",
    "P230", "P231", "P232", "P233", "P234", "P235",
    "P240", "P241", "P242", "P243", "P244",
    "P250", "P251",
    "P260", "P261", "P262", "P263", "P264",
    "P270", "P271", "P272", "P273",
    "P280", "P281", "P282", "P283", "P284", "P285",
    "P301", "P302", "P303", "P304", "P305", "P306", "P307", "P308",
    "P310", "P311", "P312", "P313", "P314", "P315",
    "P320", "P321", "P322",
    "P330", "P331", "P332", "P333", "P334", "P335", "P336", "P337", "P338",
    "P340", "P341", "P342",
    "P350", "P351", "P352", "P353",
    "P360", "P361", "P362", "P363", "P364",
    "P370", "P371", "P372", "P373", "P374", "P375", "P376", "P377", "P378",
    "P380", "P381",
    "P390", "P391",
    "P401", "P402", "P403", "P404", "P405", "P406", "P407",
    "P410", "P411", "P412", "P413",
    "P420", "P422",
    "P501", "P502", "P503",
];

pub fn is_valid_h_code(code: &str) -> bool {
    H_CODES.binary_search(&code).is_ok()
}

/// Validates a P-code, including compound codes like `P301+P330+P331`.
pub fn is_valid_p_code(code: &str) -> bool {
    code.split('+').all(|part| P_CODES.binary_search(&part.trim()).is_ok())
}

pub fn h_code_description(code: &str) -> Option<&'static str> {
    match code {
        "H200" => Some("Unstable explosive"),
        "H201" => Some("Explosive; mass explosion hazard"),
        "H202" => Some("Explosive; severe projection hazard"),
        "H203" => Some("Explosive; fire, blast or projection hazard"),
        "H204" => Some("Fire or projection hazard"),
        "H205" => Some("May mass explode in fire"),
        "H206" => Some("Fire, blast or projection hazard; increased risk of explosion if desensitizing agent is reduced"),
        "H207" => Some("Fire or projection hazard; increased risk of explosion if desensitizing agent is reduced"),
        "H208" => Some("Fire hazard; increased risk of explosion if desensitizing agent is reduced"),
        "H220" => Some("Extremely flammable gas"),
        "H221" => Some("Flammable gas"),
        "H222" => Some("Extremely flammable aerosol"),
        "H223" => Some("Flammable aerosol"),
        "H224" => Some("Extremely flammable liquid and vapour"),
        "H225" => Some("Highly flammable liquid and vapour"),
        "H226" => Some("Flammable liquid and vapour"),
        "H227" => Some("Combustible liquid"),
        "H228" => Some("Flammable solid"),
        "H229" => Some("Pressurized container: may burst if heated"),
        "H230" => Some("May react explosively even in the absence of air"),
        "H231" => Some("May react explosively even in the absence of air at elevated pressure and/or temperature"),
        "H232" => Some("May ignite spontaneously if exposed to air"),
        "H240" => Some("Heating may cause an explosion"),
        "H241" => Some("Heating may cause a fire or explosion"),
        "H242" => Some("Heating may cause a fire"),
        "H250" => Some("Catches fire spontaneously if exposed to air"),
        "H251" => Some("Self-heating; may catch fire"),
        "H252" => Some("Self-heating in large quantities; may catch fire"),
        "H260" => Some("In contact with water releases flammable gases which may ignite spontaneously"),
        "H261" => Some("In contact with water releases flammable gas"),
        "H270" => Some("May cause or intensify fire; oxidizer"),
        "H271" => Some("May cause fire or explosion; strong oxidizer"),
        "H272" => Some("May intensify fire; oxidizer"),
        "H280" => Some("Contains gas under pressure; may explode if heated"),
        "H281" => Some("Contains refrigerated gas; may cause cryogenic burns or injury"),
        "H282" => Some("Extremely flammable chemical under pressure: may explode if heated"),
        "H283" => Some("Flammable chemical under pressure: may explode if heated"),
        "H284" => Some("Chemical under pressure: may explode if heated"),
        "H290" => Some("May be corrosive to metals"),
        "H300" => Some("Fatal if swallowed"),
        "H301" => Some("Toxic if swallowed"),
        "H302" => Some("Harmful if swallowed"),
        "H303" => Some("May be harmful if swallowed"),
        "H304" => Some("May be fatal if swallowed and enters airways"),
        "H305" => Some("May be harmful if swallowed and enters airways"),
        "H310" => Some("Fatal in contact with skin"),
        "H311" => Some("Toxic in contact with skin"),
        "H312" => Some("Harmful in contact with skin"),
        "H313" => Some("May be harmful in contact with skin"),
        "H314" => Some("Causes severe skin burns and eye damage"),
        "H315" => Some("Causes skin irritation"),
        "H316" => Some("Causes mild skin irritation"),
        "H317" => Some("May cause an allergic skin reaction"),
        "H318" => Some("Causes serious eye damage"),
        "H319" => Some("Causes serious eye irritation"),
        "H320" => Some("Causes eye irritation"),
        "H330" => Some("Fatal if inhaled"),
        "H331" => Some("Toxic if inhaled"),
        "H332" => Some("Harmful if inhaled"),
        "H333" => Some("May be harmful if inhaled"),
        "H334" => Some("May cause allergy or asthma symptoms or breathing difficulties if inhaled"),
        "H335" => Some("May cause respiratory irritation"),
        "H336" => Some("May cause drowsiness or dizziness"),
        "H340" => Some("May cause genetic defects"),
        "H341" => Some("Suspected of causing genetic defects"),
        "H350" => Some("May cause cancer"),
        "H351" => Some("Suspected of causing cancer"),
        "H360" => Some("May damage fertility or the unborn child"),
        "H361" => Some("Suspected of damaging fertility or the unborn child"),
        "H362" => Some("May cause harm to breast-fed children"),
        "H370" => Some("Causes damage to organs"),
        "H371" => Some("May cause damage to organs"),
        "H372" => Some("Causes damage to organs through prolonged or repeated exposure"),
        "H373" => Some("May cause damage to organs through prolonged or repeated exposure"),
        "H400" => Some("Very toxic to aquatic life"),
        "H401" => Some("Toxic to aquatic life"),
        "H402" => Some("Harmful to aquatic life"),
        "H410" => Some("Very toxic to aquatic life with long lasting effects"),
        "H411" => Some("Toxic to aquatic life with long lasting effects"),
        "H412" => Some("Harmful to aquatic life with long lasting effects"),
        "H413" => Some("May cause long lasting harmful effects to aquatic life"),
        "H420" => Some("Harms public health and the environment by destroying ozone in the upper atmosphere"),
        _ => None,
    }
}

pub fn p_code_description(code: &str) -> Option<&'static str> {
    match code {
        "P101" => Some("If medical advice is needed, have product container or label at hand"),
        "P102" => Some("Keep out of reach of children"),
        "P103" => Some("Read carefully and follow all instructions"),
        "P201" => Some("Obtain special instructions before use"),
        "P202" => Some("Do not handle until all safety precautions have been read and understood"),
        "P210" => Some("Keep away from heat, hot surfaces, sparks, open flames and other ignition sources. No smoking"),
        "P211" => Some("Do not spray on an open flame or other ignition source"),
        "P212" => Some("Avoid heating under confinement or reduction of desensitized agent"),
        "P220" => Some("Keep away from clothing and other combustible materials"),
        "P221" => Some("Take precautionary measures against mixing with combustibles"),
        "P222" => Some("Do not allow contact with air"),
        "P223" => Some("Keep away from any possible contact with water"),
        "P230" => Some("Keep wetted with specified agent"),
        "P231" => Some("Handle under inert gas"),
        "P232" => Some("Protect from moisture"),
        "P233" => Some("Keep container tightly closed"),
        "P234" => Some("Keep only in original packaging"),
        "P235" => Some("Keep cool"),
        "P240" => Some("Ground and bond container and receiving equipment"),
        "P241" => Some("Use explosion-proof equipment"),
        "P242" => Some("Use non-sparking tools"),
        "P243" => Some("Take precautionary measures against static discharge"),
        "P244" => Some("Keep valves and fittings free from oil and grease"),
        "P250" => Some("Do not subject to grinding/shock/friction"),
        "P251" => Some("Do not pierce or burn, even after use"),
        "P260" => Some("Do not breathe dust/fume/gas/mist/vapours/spray"),
        "P261" => Some("Avoid breathing dust/fume/gas/mist/vapours/spray"),
        "P262" => Some("Do not get in eyes, on skin, or on clothing"),
        "P263" => Some("Avoid contact during pregnancy and while nursing"),
        "P264" => Some("Wash thoroughly after handling"),
        "P270" => Some("Do not eat, drink or smoke when using this product"),
        "P271" => Some("Use only outdoors or in a well-ventilated area"),
        "P272" => Some("Contaminated work clothing should not be allowed out of the workplace"),
        "P273" => Some("Avoid release to the environment"),
        "P280" => Some("Wear protective gloves/protective clothing/eye protection/face protection"),
        "P281" => Some("Use personal protective equipment as required"),
        "P282" => Some("Wear cold insulating gloves and either face shield or eye protection"),
        "P283" => Some("Wear fire-resistant/flame-retardant clothing"),
        "P284" => Some("In case of inadequate ventilation wear respiratory protection"),
        "P285" => Some("In case of inadequate ventilation use respiratory protection"),
        "P301" => Some("IF SWALLOWED"),
        "P302" => Some("IF ON SKIN"),
        "P303" => Some("IF ON SKIN (or hair)"),
        "P304" => Some("IF INHALED"),
        "P305" => Some("IF IN EYES"),
        "P306" => Some("IF ON CLOTHING"),
        "P307" => Some("IF exposed"),
        "P308" => Some("IF exposed or concerned"),
        "P310" => Some("Immediately call a POISON CENTER or doctor/physician"),
        "P311" => Some("Call a POISON CENTER or doctor/physician"),
        "P312" => Some("Call a POISON CENTER or doctor/physician if you feel unwell"),
        "P313" => Some("Get medical advice/attention"),
        "P314" => Some("Get medical advice/attention if you feel unwell"),
        "P315" => Some("Get immediate medical advice/attention"),
        "P320" => Some("Specific treatment is urgent"),
        "P321" => Some("Specific treatment (see supplemental first aid instructions)"),
        "P322" => Some("Specific measures (see supplemental instructions)"),
        "P330" => Some("Rinse mouth"),
        "P331" => Some("Do NOT induce vomiting"),
        "P332" => Some("If skin irritation occurs: Get medical advice/attention"),
        "P333" => Some("If skin irritation or rash occurs: Get medical advice/attention"),
        "P334" => Some("Immerse in cool water or wrap in wet bandages"),
        "P335" => Some("Brush off loose particles from skin"),
        "P336" => Some("Thaw frosted parts with lukewarm water"),
        "P337" => Some("If eye irritation persists: Get medical advice/attention"),
        "P338" => Some("Remove contact lenses, if present and easy to do. Continue rinsing"),
        "P340" => Some("Remove victim to fresh air and keep at rest in a position comfortable for breathing"),
        "P341" => Some("If breathing is difficult, remove victim to fresh air and keep at rest"),
        "P342" => Some("If experiencing respiratory symptoms: Call a POISON CENTER or doctor/physician"),
        "P350" => Some("Gently wash with plenty of soap and water"),
        "P351" => Some("Rinse cautiously with water for several minutes"),
        "P352" => Some("Wash with plenty of water"),
        "P353" => Some("Rinse skin with water or shower"),
        "P360" => Some("Rinse immediately contaminated clothing and skin with plenty of water before removing clothes"),
        "P361" => Some("Take off immediately all contaminated clothing"),
        "P362" => Some("Take off contaminated clothing and wash before reuse"),
        "P363" => Some("Wash contaminated clothing before reuse"),
        "P364" => Some("And wash it before reuse"),
        "P370" => Some("In case of fire"),
        "P371" => Some("In case of major fire and large quantities"),
        "P372" => Some("Explosion risk"),
        "P373" => Some("DO NOT fight fire when fire reaches explosives"),
        "P374" => Some("Fight fire with normal precautions from a reasonable distance"),
        "P375" => Some("Fight fire remotely due to the risk of explosion"),
        "P376" => Some("Stop leak if safe to do so"),
        "P377" => Some("Leaking gas fire: Do not extinguish, unless leak can be stopped safely"),
        "P378" => Some("Use appropriate fire-fighting media"),
        "P380" => Some("Evacuate area"),
        "P381" => Some("In case of leakage, eliminate all ignition sources"),
        "P390" => Some("Absorb spillage to prevent material damage"),
        "P391" => Some("Collect spillage"),
        "P401" => Some("Store in accordance with applicable regulations"),
        "P402" => Some("Store in a dry place"),
        "P403" => Some("Store in a well-ventilated place"),
        "P404" => Some("Store in a closed container"),
        "P405" => Some("Store locked up"),
        "P406" => Some("Store in corrosive-resistant container with a resistant inner liner"),
        "P407" => Some("Maintain air gap between stacks or pallets"),
        "P410" => Some("Protect from sunlight"),
        "P411" => Some("Store at temperatures not exceeding specified temperature"),
        "P412" => Some("Do not expose to temperatures exceeding 50°C/122°F"),
        "P413" => Some("Store bulk masses greater than specified mass at temperatures not exceeding specified temperature"),
        "P420" => Some("Store separately"),
        "P422" => Some("Store contents under specified conditions"),
        "P501" => Some("Dispose of contents/container in accordance with local regulations"),
        "P502" => Some("Refer to manufacturer or supplier for information on recovery or recycling"),
        "P503" => Some("Refer to manufacturer/supplier/competent authority for information on recovery or recycling"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_h_codes() {
        assert!(is_valid_h_code("H200"));
        assert!(is_valid_h_code("H315"));
        assert!(is_valid_h_code("H420"));
        assert!(!is_valid_h_code("H999"));
        assert!(!is_valid_h_code("H"));
        assert!(!is_valid_h_code(""));
    }

    #[test]
    fn valid_p_codes() {
        assert!(is_valid_p_code("P101"));
        assert!(is_valid_p_code("P501"));
        assert!(!is_valid_p_code("P999"));
    }

    #[test]
    fn compound_p_codes() {
        assert!(is_valid_p_code("P301+P330+P331"));
        assert!(is_valid_p_code("P305+P351+P338"));
        assert!(!is_valid_p_code("P301+P999"));
    }

    #[test]
    fn h_code_descriptions() {
        assert_eq!(h_code_description("H315"), Some("Causes skin irritation"));
        assert_eq!(h_code_description("H400"), Some("Very toxic to aquatic life"));
        assert!(h_code_description("H999").is_none());
    }

    #[test]
    fn p_code_descriptions() {
        assert!(p_code_description("P101").is_some());
        assert!(p_code_description("P501").is_some());
        assert!(p_code_description("P999").is_none());
    }

    #[test]
    fn h_codes_sorted() {
        // Ensure binary_search works (slice must be sorted)
        let mut sorted = H_CODES.to_vec();
        sorted.sort_unstable();
        assert_eq!(H_CODES, sorted.as_slice());
    }

    #[test]
    fn p_codes_sorted() {
        let mut sorted = P_CODES.to_vec();
        sorted.sort_unstable();
        assert_eq!(P_CODES, sorted.as_slice());
    }
}
