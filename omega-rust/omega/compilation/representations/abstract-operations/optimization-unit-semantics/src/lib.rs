#![forbid(unsafe_code)]

mod operation_metadata;

/*
We provide independent semantic and rewrite validation for the Psi optimization
unit representation. We neither lower Terminal Psi nor sequence optimization
passes or publication. Candidate producers do not participate in acceptance:
our callers retain Terminal admission and supply any independently admitted
control-cycle roster, while structural validation alone grants no execution or
publication authority.

Our public surface divides into whole-unit acceptance, rewrite-candidate replay,
current-revision fact reconstruction, and structural authority queries. Whole-
unit validation enters through `unit_validation` and checks canonical identity
and fact indexes, structural and service catalogs, per-function control flow
and operation contracts, retained affine cleanup authority, and final frontier
and service reach. Rewrite acceptance enters through `candidates`, routing each
typed `PsiRewriteCandidate` to the independent replay validator of the pass
family that proposed it. Current-revision ownership and loan frontiers are
reconstructed in `current_ownership`, value-range interval and dominance facts
are checked in `current_value_ranges`, cohort descriptor and reference flow
graphs are adapted in `descriptor_results`, native access-view backing
correspondence is verified in `access_references`, and `error` defines our
diagnostic rejection vocabulary.
*/

/*
We keep semantic and rewrite validation in a dedicated crate separate from both
the `optimization-unit` data representation and the optimization pipeline so
every transformation stage can verify candidate rewrites and unit invariants
without trusting producer summaries or coupling data carriers to pass logic.
When an optimization pass proposes a `PsiRewriteCandidate`, we do not trust its
claimed post-state or provenance accounting: we replay the patch from the
admitted input unit, recompute node definitions, uses, effect chains, fact
indexes, declared places, and root service reach, re-validate the entire unit,
and verify that every block and function outside the declared rewrite region
remains byte-for-byte intact.

We enforce the same fail-closed discipline across structural and ownership
queries. Rather than reading stale source summaries or cached `OwnershipEvent`
rows, we reconstruct live claims, owned places, partial-affine move paths, and
outstanding reference loans directly from the current revision's executable
operations and signatures. That separation ensures that any rewrite bug, leaked
linear claim, disturbed shared-join origin, or broken provenance settlement is
caught immediately at the candidate boundary before a commit can enter the
transformation ledger.
*/

/*
Downstream pipeline stages consume our validators and queries across the
abstract and target realization boundaries:
- `unit_validation`: whole-unit, structural-catalog, and ownership-frontier
  validation (`validate_psi_optimization_unit_with_admitted_cycle_machines`,
  `validate_psi_structural_catalog`, and
  `reconstruct_ownership_frontier_facts_with_admitted_cycle_machines`).
- `candidates`: typed rewrite dispatch (`validate_psi_rewrite_candidate`),
  family-specific specialization validators, linear empty-block thread
  predicates, and `ValidatedPsiRewrite` receipts.
- `current_ownership` and `current_value_ranges`: CFG loan/claim frontier replay
  (`CallRegion`) and interval dominance verification
  (`validate_current_value_range_fact_at`).
- `descriptor_results` and `access_references`: cohort descriptor flow graphs,
  call-origin reconstruction, and placed native backing checks.
- `error`: `OperationSite` coordinates and `OptimizationUnitValidationError`.
*/

pub mod access_references;
mod candidates;
mod current_ownership;
mod current_value_ranges;
pub mod descriptor_results;
mod error;
mod unit_validation;

pub use error::{OperationSite, OptimizationUnitValidationError};

pub use candidates::rewrite_accounting::threading::reconstruct_linear_thread_bindings;

pub use candidates::rewrite_accounting::threading::reconstruct_linear_thread_ownership_is_identity;

pub use unit_validation::function_structure::{byte_field_root, declared_descriptor_home};

pub use current_ownership::references::CallRegion;
pub use unit_validation::references::reference_projection;
pub use unit_validation::structural_catalog::{
    StructuralStorageProjection, structural_borrowed_storage_projection,
    structural_storage_projection,
};

pub fn operation_place_inputs(
    operation: &abstract_operations::AbstractOperation,
) -> Vec<semantic_vocabulary::PlaceId> {
    unit_validation::function_structure::operation_place_inputs(operation)
}

pub fn structural_place_access(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<terminal_psi::StructuralAccess> {
    unit_validation::operation_contracts::structural_source_contract(function, place)
        .map(|source| source.access)
}

pub fn descriptor_argument_accessible(
    function: &optimization_unit::PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    types: &[terminal_psi::StructuralTypeDeclaration],
    requested: terminal_psi::StructuralAccess,
) -> bool {
    let Some(source) =
        unit_validation::operation_contracts::structural_source_contract(function, argument.place)
    else {
        return false;
    };
    requested.admits(argument.access)
        && source.backing_access(types.iter()).is_some()
        && terminal_semantics::descriptor_source_projection_accessible(
            types.iter(),
            source.structural_type,
            source.multiplicity,
            source.access,
            &argument.path,
            requested,
        )
}

pub fn descriptor_metadata_argument_accessible(
    function: &optimization_unit::PsiOptimizationFunction,
    argument: &terminal_psi::StructuralArgument,
    types: &[terminal_psi::StructuralTypeDeclaration],
    requested: terminal_psi::StructuralAccess,
) -> bool {
    let Some(source) =
        unit_validation::operation_contracts::structural_source_contract(function, argument.place)
    else {
        return false;
    };
    requested.admits(argument.access)
        && source.backing_access(types.iter()).is_some()
        && terminal_semantics::reference_source_projection_accessible(
            types.iter(),
            source.structural_type,
            source.multiplicity,
            source.access,
            &argument.path,
            requested,
        )
}

pub fn structural_place_backing_access<'types>(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
) -> Option<terminal_psi::StructuralAccess> {
    if let Some(parameter) = function.arrival_parameter(place)
        && terminal_semantics::descriptor_results::is_descriptor(
            types.clone(),
            parameter.structural_type,
        )
    {
        return terminal_semantics::descriptor_parameter_access(parameter, types);
    }
    unit_validation::operation_contracts::structural_source_contract(function, place)
        .and_then(|source| source.backing_access(types))
}

macro_rules! structural_write_preserves {
    ($(#[$attribute:meta])* $name:ident, $roster:path) => {
        $(#[$attribute])*
        pub fn $name(
            function: &optimization_unit::PsiOptimizationFunction,
            place: semantic_vocabulary::PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
            types: &[terminal_psi::StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            identities: &terminal_psi::TerminalIdentityTable,
        ) -> bool {
            unit_validation::operation_contracts::structural_source_contract(function, place)
                .is_some_and(|source| {
                    let (whole, projected) = source.selected_qualifications(&[]);
                    $roster(
                        types.iter(),
                        domains.iter(),
                        identities,
                        &whole,
                        &projected,
                        path,
                    ) && terminal_semantics::store_preserves_content(
                        types.iter(),
                        domains.iter(),
                        identities,
                        source.structural_type,
                        place,
                        function.content_entry_claims.iter(),
                        path,
                    )
                })
        }
    };
}

structural_write_preserves!(
    structural_write_preserves_memberships,
    terminal_semantics::store_preserves_qualifications
);

structural_write_preserves!(
    structural_write_preserves_content,
    terminal_semantics::store_preserves_domain_content
);

pub fn structural_place_supports_window_restoration(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    unit_validation::operation_contracts::structural_source_contract(function, place).is_some_and(
        |source| {
            matches!(
                source.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) && source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                && source
                    .operation_result()
                    .is_none_or(|result| result.claims.is_empty())
        },
    )
}

pub(crate) use unit_validation::validate_psi_optimization_unit;
pub use unit_validation::{
    reconstruct_ownership_frontier_facts_with_admitted_cycle_machines,
    validate_psi_optimization_unit_with_admitted_cycle_machines, validate_psi_structural_catalog,
};

pub use candidates::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

pub use candidates::case_membership_specialization::validate_case_membership_specialization_candidate;
pub(crate) use candidates::dead_scalar_elimination::validate_dead_scalar_node_candidate;
pub use candidates::field_value_specialization::validate_field_value_specialization_candidate;
pub use candidates::state_specialization::validate_state_argument_specialization_candidate;

pub(crate) use candidates::observation::reconstruct_closed_scalar_node_boundary;

pub use current_value_ranges::validate_current_value_range_fact_at;

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::StructuralTypeDeclaration;

pub(crate) trait StructuralTypes {
    fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration>;
    fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone;
}

impl StructuralTypes for [StructuralTypeDeclaration] {
    fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self[position])
    }

    fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone {
        self.iter()
    }
}
