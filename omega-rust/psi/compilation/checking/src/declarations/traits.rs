//! Traits: each trait's requirements, the conformance bounds that traits and
//! generic machines declare, the exact satisfaction edges machines claim, the
//! named implementations that own complete row maps, external `via`
//! realizations, and local dynamic trait values.
//!
//! There is no single entrance; `program_validation::validate` calls these
//! validators. Before its per-machine loop, in this order:
//! `validate_trait_requirements` (`requirements`),
//! `validate_trait_conformance_bounds` for each trait, `validate_conformances`
//! for named implementations (`named_implementations`), and
//! `collect_dynamic_implementation_selections` (`dynamic`). For each machine, in
//! this order: `validate_generic_conformance_bounds`,
//! `validate_machine_trait_conformances`, `validate_external_via_expression`
//! for each conformance, and `validate_external_leaf_native_shapes` for an
//! external leaf with one `via` binding. The bound, machine-conformance and
//! external checks come from `conformance`.
//!
//! Other readers. Call validation in `machine_calls::calls` uses the generic
//! bound queries from `conformance`, `dynamic_selector_application` and
//! `dynamic_requirement_ineligibility`. Typed preparation binds service calls;
//! specialized producers and build evaluation bind only their selected roots.
//! The immutable checking stage reads the dynamic implementation
//! selections, `compose_forwarded_trait_arguments` and
//! `generic_bound_operator_requirement`. Package review calls
//! `revalidate_top_level_requirement_realization`.
//!
//! `trait_definition_by_symbol` is the trait lookup that the `conformance`,
//! `named_implementations` and `requirements` children share.
//! Inherited row and law consumers retain `AppliedTraitScope`: projecting an
//! argument node alone discards the outer telescope needed to interpret it.

use symbols::SymbolHandle;
use typed_trees::trait_definition::TraitDefinition;

mod conformance;
pub mod conformance_applications;
mod dynamic;
mod named_implementations;
mod provider_regions;
pub mod refinement_fit;
mod requirements;
mod trait_implementation_arguments;

pub use conformance::GenericBoundRequirement;
pub use conformance::compose_forwarded_trait_arguments;
pub use conformance::generic_bound_operator_requirement;
pub use conformance::resolve_closed_requirement_application;
pub use conformance::revalidate_top_level_requirement_realization;
pub(crate) use conformance::{
    named_conformance_argument_matches, validate_external_leaf_native_shapes,
    validate_external_via_expression, validate_generic_conformance_bounds,
    validate_machine_trait_conformances, validate_trait_application_obligations,
    validate_trait_conformance_bounds,
};
pub use dynamic::{
    DynamicImplementationSelection, DynamicSlotDemand, collect_dynamic_implementation_selections,
    collect_dynamic_slot_demands, dynamic_call_demand, dynamic_expression_call_demand,
    qualified_trait_scope, resolve_dynamic_call_targets, resolve_selected_dynamic_call_targets,
};
pub(crate) use dynamic::{
    dynamic_requirement_ineligibility, dynamic_selector_application,
    report_dynamic_receiver_call_error,
};
pub(crate) use named_implementations::validate_conformances;
pub use provider_regions::{ProviderRegionCorrespondence, provider_region_correspondence};
pub(crate) use requirements::validate_trait_requirements;

/// The trait `symbol` declares, or `None` for an invalid symbol or a symbol
/// that names no trait. Every conformance, requirement, and data-conformance
/// validator below resolves its trait through this one lookup.
fn trait_definition_by_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program.trait_definition_by_symbol(symbol)
}
