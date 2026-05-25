mod generated;

pub use generated::*;

/// Flexible serde helpers for fields whose JSON type varies between LLM responses.
pub mod serde_flex {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::Deserializer;
    use std::fmt;

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
