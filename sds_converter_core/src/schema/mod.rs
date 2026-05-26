mod generated;

pub use generated::*;

/// Flexible serde helpers for fields whose JSON type varies between LLM responses.
pub mod serde_flex {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::Deserializer;
    use std::fmt;

    /// Deserialise `Option<String>` from a JSON string, array-of-strings, or null.
    ///
    /// Many free-text fields in the MHLW schema are typed as plain `String`, but LLMs
    /// sometimes return them as arrays (e.g. `["CO2", "NH3"]` for Substance, or
    /// `["text line 1", "text line 2"]` for FullText).  This helper accepts both forms:
    /// a bare string is used as-is, while an array is joined with "\n".
    pub fn flex_string_opt<'de, D>(d: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FlexStringVisitor;

        impl<'de> Visitor<'de> for FlexStringVisitor {
            type Value = Option<String>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a string, an array of strings, or null")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.is_empty() { Ok(None) } else { Ok(Some(v.to_string())) }
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                if v.is_empty() { Ok(None) } else { Ok(Some(v)) }
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut parts = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    if !s.is_empty() {
                        parts.push(s);
                    }
                }
                if parts.is_empty() { Ok(None) } else { Ok(Some(parts.join("\n"))) }
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> { Ok(None) }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> { Ok(None) }
        }

        d.deserialize_any(FlexStringVisitor)
    }

    /// Deserialise `Option<Vec<String>>` from a JSON string, array-of-strings, or null.
    ///
    /// The MHLW schema defines `AdditionalInfo.FullText` as `Vec<String>`, but LLMs
    /// sometimes emit a bare string.  This helper wraps a bare string in a one-element vec.
    pub fn flex_vec_string_opt<'de, D>(d: D) -> Result<Option<Vec<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FlexVecVisitor;

        impl<'de> Visitor<'de> for FlexVecVisitor {
            type Value = Option<Vec<String>>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a string, an array of strings, or null")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(vec![v.to_string()]))
                }
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                if v.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(vec![v]))
                }
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    if !s.is_empty() {
                        items.push(s);
                    }
                }
                if items.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(items))
                }
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
        }

        d.deserialize_any(FlexVecVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FireFightingMeasures.FireAndExplosionHazards.FullText returned as array
    /// previously caused "invalid type: sequence, expected a string" and
    /// skipped the entire FireFightingMeasures section.
    #[test]
    fn fire_fighting_full_text_accepts_array() {
        let json = r#"{
            "FireFightingMeasures": {
                "FireAndExplosionHazards": {
                    "FullText": ["有害燃焼副産物：ケイ素酸化物", "炭素酸化物"]
                },
                "FireFightingProcedures": {
                    "FullText": "現場の状況に応じて適切な消火手段を用いる。"
                }
            }
        }"#;
        let sds: SdsRoot = serde_json::from_str(json)
            .expect("FireFightingMeasures with array FullText should deserialize");
        let ffm = sds.fire_fighting_measures.as_ref().expect("FireFightingMeasures must be Some");
        let hazards = ffm.fire_and_explosion_hazards.as_ref().expect("FireAndExplosionHazards must be Some");
        let full_text = hazards.full_text.as_deref().expect("FullText must be Some");
        // array elements joined with \n
        assert!(full_text.contains("有害燃焼副産物"), "FullText content lost: {full_text}");
        assert!(full_text.contains("炭素酸化物"), "second element lost: {full_text}");
    }

    /// StabilityReactivity.HazardousDecompositionProducts.Substance returned as array
    /// previously caused "invalid type: sequence, expected a string" and
    /// skipped the entire StabilityReactivity section.
    #[test]
    fn stability_reactivity_substance_accepts_array() {
        let json = r#"{
            "StabilityReactivity": {
                "HazardousDecompositionProducts": {
                    "Substance": ["二酸化炭素（CO2）", "アンモニア", "窒素酸化物(NOx)"]
                }
            }
        }"#;
        let sds: SdsRoot = serde_json::from_str(json)
            .expect("StabilityReactivity with array Substance should deserialize");
        let sr = sds.stability_reactivity.as_ref().expect("StabilityReactivity must be Some");
        let hdp = sr.hazardous_decomposition_products.as_ref().expect("HazardousDecompositionProducts must be Some");
        let substance = hdp.substance.as_deref().expect("Substance must be Some");
        assert!(substance.contains("二酸化炭素"), "first element lost: {substance}");
        assert!(substance.contains("アンモニア"), "second element lost: {substance}");
        assert!(substance.contains("窒素酸化物"), "third element lost: {substance}");
    }

    #[test]
    fn sds_root_round_trip_empty() {
        let sds = SdsRoot::default();
        let json = serde_json::to_string(&sds).unwrap();
        assert_eq!(json, "{}");
        let sds2: SdsRoot = serde_json::from_str("{}").unwrap();
        let json2 = serde_json::to_string(&sds2).unwrap();
        assert_eq!(json2, "{}");
    }

    #[test]
    fn sds_root_round_trip_partial() {
        let json = r#"{"Datasheet":{"IssueDate":"2024-03-31","SDS-SchemaVersionNo":"1.0"}}"#;
        let sds: SdsRoot = serde_json::from_str(json).unwrap();
        assert_eq!(sds.datasheet.as_ref().unwrap().issue_date.as_deref(), Some("2024-03-31"));
        let out = serde_json::to_string(&sds).unwrap();
        let v1: serde_json::Value = serde_json::from_str(json).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v1, v2);
    }
}
