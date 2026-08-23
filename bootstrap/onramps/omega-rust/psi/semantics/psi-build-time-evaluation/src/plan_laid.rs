//! PLAN-LAID VALUE TYPES -- L4 of the LAYOUTS ladder
//! (design_briefs/programmable_layouts.md §5): a field `gdt: CLayout<Gdt>;`
//! applies a layout POLICY (ordinary data with a build-time-admissible `plan` machine)
//! to a SCHEMA (a plain record of primitives) in type position. The value
//! behaves exactly like the schema type -- same fields, ZII, projections --
//! but its native in-memory placement comes from the validated plan instead
//! of the compiler's own packing.
//!
//! Two passes, both driven from the pipeline:
//!
//! 1. `desugar_plan_laid_value_types` (PRE-RESOLUTION, on the merged syntax
//!    trees): synthesizes `data CLayout<Gdt> { <schema fields> }` and rewrites
//!    every occurrence of the generic spelling to that plain name, so fields,
//!    parameters, returns, locals, nested generic arguments, symbol resolution,
//!    typing, validation, proof, and the interpreter all see one ordinary
//!    record identity. The interpreter is name-keyed, so it needs nothing else.
//! 2. `compute_plan_laid_layouts` (POST-TYPING, after const-length
//!    substitution): evaluates the policy at build time through the existing
//!    L2/L3 pipeline (`compute_layout_plan` -- contract gate, plan validation),
//!    requires the plan be FULLY STATIC (a dynamic plan cannot be a value
//!    type: values need offsets, bytes need mints), and records the placement
//!    on `TypedTrees::plan_laid_layouts` for the native layout builder.
//!
//! v0 boundaries (documented, all clean errors): schemas are plain records of
//! primitives; construction is ZII + per-field writes (a
//! `CLayout<Gdt> { ... }` literal is not spellable).

mod desugaring;
mod layout_installation;

pub use desugaring::desugar_plan_laid_value_types;
pub use layout_installation::compute_plan_laid_layouts;

/// One plan-laid instantiation discovered by the desugar: the synthesized
/// data definition plus the (policy, schema) pair whose validated plan will
/// dictate its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLaidRecord {
    /// Name of the synthesized data definition (`CLayout<GdtEntryish>`).
    pub synthetic_name: String,
    /// Qualified policy machine (`CLayout::plan`).
    pub policy_machine: String,
    /// The schema data definition the plan places.
    pub schema_data: String,
}
