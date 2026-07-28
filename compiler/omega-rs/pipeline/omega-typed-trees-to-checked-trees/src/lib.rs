mod capabilities;
mod checks;
mod context;
mod facts;
mod field_domain;
mod invariants;
mod labels;
mod lookup;
mod lowerer;
mod monomorphization;
mod operators;
mod validation;
mod values;

use omega_checked_trees::CheckedTrees;

pub fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    lowerer::lower_typed_trees(program)
}

/// Validate and consume compile-time machine-symbol selections, rewriting
/// every complete generic call tuple to direct concrete calls. The ordinary
/// checked-tree path invokes this before validation; orchestration also uses
/// it on a private clone before interpreting build.omg so build-time execution
/// sees the same specialized program as runtime lowering.
pub fn specialize_static_machine_calls(
    program: &mut omega_typed_trees::TypedTrees,
) -> Result<(), Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_static_machine_selections(program)?;
    omega_validation::validate_generic_machine_contract_entailment(program)?;
    monomorphization::monomorphize_generic_machine_value_calls(program)
}

pub use monomorphization::generic_machine_template_fingerprint;
/// The v0 asm-intrinsic discharge gate (asm requires a freestanding boundary
/// root) -- re-exported for the ORCHESTRATION layer, which owns the
/// BuildConfig fact the gate consumes; the other validations run inside
/// `lower_typed_trees` and never see build.omg.
pub use omega_validation::validate_asm_discharge;

mod semantic;
mod semantic_calls;
mod semantic_places;

#[cfg(test)]
pub(crate) use semantic::build_semantic_facts;
pub use semantic::lower_typed_program;
pub(crate) use semantic::{
    CallSite, call_site_argument_expressions, call_target_parameters, call_target_type_parameters,
    find_call_site, find_state, find_state_in_machine,
};

mod proof;
mod qualification_evidence;

#[cfg(test)]
pub(crate) use borrow::build_borrow_facts;
#[cfg(test)]
pub(crate) use flow::{build_domain_facts, build_flow_facts};
#[cfg(test)]
pub(crate) use operators::build_operator_facts;
#[cfg(test)]
pub(crate) use proof::build_proof_facts;
pub(crate) use proof::contract_target_from_state_symbol;
#[cfg(test)]
pub(crate) use values::build_value_facts;
mod borrow;
mod flow;

#[cfg(test)]
mod tests;
