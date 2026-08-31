//! Skill enable/disable registry.
//!
//! See `registry.rs` for the full design rationale. Short version:
//! `SkillsRegistry` is an in-memory `(name -> enabled)` map used by
//! `POST /v1/skills/:name/enable|disable` and `GET /v1/skills` (the
//! latter merges the operator's overrides with the catalog that
//! llmd reports).

pub mod registry;

pub use registry::{SharedSkillsRegistry, SkillState, SkillsRegistry};