use std::collections::HashSet;

use crate::converter::validator::{validate_cas, CasValidation, Finding};

use super::input::ProductInput;

/// Deterministic, offline validation of a [`ProductInput`] before any
/// `SdsRoot` fragment is generated from it. Checks only what's derivable
/// from the raw input itself: CAS format/check-digit, duplicate CAS across
/// components, and per-component/aggregate concentration sanity.
///
/// This does *not* duplicate the Section 1/3 QC rules in
/// `docs/quality-check.md` (product-name presence, supplier-phone digit
/// count, etc.) — those apply to the *generated* `SdsRoot` and are already
/// covered for free by [`crate::converter::validator::validate_typed`] once
/// a draft exists.
pub fn validate_product_input(input: &ProductInput) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen_cas = HashSet::new();
    let mut concentration_sum = 0.0_f64;

    for (i, component) in input.components.iter().enumerate() {
        let label = component
            .name
            .clone()
            .unwrap_or_else(|| format!("component[{i}]"));

        if let Some(cas) = &component.cas_number {
            match validate_cas(cas) {
                CasValidation::Ok => {
                    if !seen_cas.insert(cas.clone()) {
                        findings.push(Finding {
                            level: "MED".into(),
                            rule: "GEN-CAS-DUPLICATE".into(),
                            message: format!(
                                "{label}: CAS '{cas}' appears more than once in this product."
                            ),
                        });
                    }
                }
                CasValidation::InvalidFormat => {
                    findings.push(Finding {
                        level: "HIGH".into(),
                        rule: "GEN-CAS-FORMAT".into(),
                        message: format!(
                            "{label}: CAS '{cas}' does not match the expected format (e.g. 7732-18-5)."
                        ),
                    });
                }
                CasValidation::InvalidCheckDigit { expected } => {
                    findings.push(Finding {
                        level: "HIGH".into(),
                        rule: "GEN-CAS-CHECKDIGIT".into(),
                        message: format!(
                            "{label}: CAS '{cas}' has an invalid check digit (expected {expected})."
                        ),
                    });
                }
            }
        }

        let conc = &component.concentration;
        if conc.unit.trim().is_empty() {
            findings.push(Finding {
                level: "MED".into(),
                rule: "GEN-CONC-NO-UNIT".into(),
                message: format!("{label}: concentration has no unit."),
            });
        }
        if conc.exact.is_some() && (conc.lower.is_some() || conc.upper.is_some()) {
            findings.push(Finding {
                level: "MED".into(),
                rule: "GEN-CONC-AMBIGUOUS".into(),
                message: format!(
                    "{label}: concentration has both an exact value and a range — ambiguous."
                ),
            });
        }
        if let (Some(lower), Some(upper)) = (conc.lower, conc.upper) {
            if lower > upper {
                findings.push(Finding {
                    level: "HIGH".into(),
                    rule: "GEN-CONC-RANGE-INVALID".into(),
                    message: format!(
                        "{label}: concentration lower bound ({lower}) exceeds upper bound ({upper})."
                    ),
                });
            }
        }
        if let Some(exact) = conc.exact {
            concentration_sum += exact;
        }
    }

    if concentration_sum > 102.0 {
        findings.push(Finding {
            level: "MED".into(),
            rule: "GEN-CONC-SUM-EXCEEDS-100".into(),
            message: format!(
                "Sum of exact-value concentrations is {concentration_sum:.2}%, exceeding the 102% tolerance."
            ),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::input::{ComponentInput, ConcentrationRange, SupplierInput};

    fn supplier() -> SupplierInput {
        SupplierInput {
            company_name: "Example Chemical Co.".into(),
            address: None,
            phone: None,
            email: None,
        }
    }

    fn component(cas: &str, name: &str, exact: f64) -> ComponentInput {
        ComponentInput {
            cas_number: Some(cas.into()),
            name: Some(name.into()),
            concentration: ConcentrationRange {
                exact: Some(exact),
                lower: None,
                upper: None,
                unit: "%".into(),
            },
        }
    }

    #[test]
    fn valid_product_input_has_no_findings() {
        let input = ProductInput {
            trade_name: "Test Solvent".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![
                component("7732-18-5", "Water", 60.0),
                component("64-17-5", "Ethanol", 40.0),
            ],
        };
        let findings = validate_product_input(&input);
        assert!(findings.is_empty(), "unexpected findings: {findings:?}");
    }

    #[test]
    fn invalid_check_digit_is_flagged() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![component("7732-18-4", "Water", 100.0)],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CAS-CHECKDIGIT"));
    }

    #[test]
    fn malformed_cas_format_is_flagged() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![component("not-a-cas", "Mystery", 100.0)],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CAS-FORMAT"));
    }

    #[test]
    fn duplicate_cas_across_components_is_flagged() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![
                component("7732-18-5", "Water", 50.0),
                component("7732-18-5", "Water again", 50.0),
            ],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CAS-DUPLICATE"));
    }

    #[test]
    fn concentration_lower_greater_than_upper_is_flagged() {
        let mut c = component("7732-18-5", "Water", 100.0);
        c.concentration = ConcentrationRange {
            exact: None,
            lower: Some(50.0),
            upper: Some(10.0),
            unit: "%".into(),
        };
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![c],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CONC-RANGE-INVALID"));
    }

    #[test]
    fn exact_and_range_both_set_is_ambiguous() {
        let mut c = component("7732-18-5", "Water", 50.0);
        c.concentration.lower = Some(40.0);
        c.concentration.upper = Some(60.0);
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![c],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CONC-AMBIGUOUS"));
    }

    #[test]
    fn concentration_sum_over_102_percent_is_flagged() {
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![
                component("7732-18-5", "Water", 60.0),
                component("64-17-5", "Ethanol", 45.0),
            ],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CONC-SUM-EXCEEDS-100"));
    }

    #[test]
    fn missing_unit_is_flagged() {
        let mut c = component("7732-18-5", "Water", 100.0);
        c.concentration.unit = String::new();
        let input = ProductInput {
            trade_name: "Test".into(),
            other_names: vec![],
            supplier: supplier(),
            components: vec![c],
        };
        let findings = validate_product_input(&input);
        assert!(findings.iter().any(|f| f.rule == "GEN-CONC-NO-UNIT"));
    }
}
