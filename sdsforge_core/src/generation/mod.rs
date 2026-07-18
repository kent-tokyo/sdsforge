//! Formulation input domain model for the `generate` feature.
//!
//! This is the *input* side only — raw product/component/supplier data
//! supplied by a caller, plus deterministic validation. See
//! `docs/sdsforge-architecture.md`'s "Generation architecture" section for
//! the full roadmap: Section 1/3 draft generation, provenance/unresolved
//! tracking, and chematic integration are later, separate commits.

mod draft;
mod input;
mod validate;

pub use draft::{draft_sections_from_resolved_input, generate_section_1_and_3, SectionDraftResult};
pub use input::{ComponentInput, ConcentrationRange, ProductInput, SupplierInput};
pub use validate::validate_product_input;
