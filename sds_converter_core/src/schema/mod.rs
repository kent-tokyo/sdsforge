mod generated;

pub use generated::*;

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
