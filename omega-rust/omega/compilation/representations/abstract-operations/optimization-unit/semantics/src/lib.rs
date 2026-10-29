#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Independent meaning and rewrite validation for the optimization-unit representation.
//! This crate neither lowers Terminal Psi nor sequences optimization passes or publication.
//! Candidate producers do not participate in acceptance. The caller retains
//! Terminal admission and supplies any independently admitted cycle roster;
//! structural success alone grants no execution or publication authority.
//!
//! The exports below are the whole public surface. Whole-unit acceptance
//! starts at `unit_validation.rs`: `validate_psi_optimization_unit` is the
//! entry, and the file shows the acceptance order -- identity and fact
//! indexes, catalogs with every function validated in place, retained affine
//! authority, final authorities -- with each invariant family owned by a
//! named module beside it. Rewrite acceptance starts at `candidates/mod.rs`:
//! `validate_psi_rewrite_candidate` routes each typed candidate to the exact
//! validator of the pass family that proposed it, and those family validators
//! are exported by name for callers that hold one family's candidate.
//! `current_ownership` reconstructs the ownership frontiers of the current
//! revision and `current_value_ranges` validates its value-range facts;
//! `error` is the failure vocabulary.

mod candidates;
mod current_ownership;
mod current_value_ranges;
pub mod descriptor_results;
mod error;
pub mod placed_references;
mod unit_validation;

pub use error::{OperationSite, OptimizationUnitValidationError};

/// The empty-block threads' shared binding composition: substitute the
/// threaded edge's parameters through its incoming bindings and forward that
/// replacement into the outgoing bindings. Proposing rules and the replay
/// checkers below share this predicate; it decides values only, so it grants
/// neither acceptance nor the provenance accounting a rewrite still owes.
pub use candidates::rewrite_accounting::threading::reconstruct_linear_thread_bindings;

/// The empty-block threads' shared ownership predicate: the six frontier sites
/// the thread touches must carry one snapshot at the unit's identity. Proposing
/// rules and the replay checkers share it, so neither restates the site list;
/// like the bindings predicate it decides only, granting neither acceptance nor
/// the provenance accounting a rewrite still owes.
pub use candidates::rewrite_accounting::threading::reconstruct_linear_thread_ownership_is_identity;

// Declaration custody shared by validation and target realization. This query
// grants neither current availability nor permission for a particular access.
pub use unit_validation::function_structure::{byte_field_root, declared_descriptor_home};

pub use current_ownership::references::CallRegion;
pub use unit_validation::references::reference_projection;
pub use unit_validation::structural_catalog::{
    StructuralStorageProjection, structural_borrowed_storage_projection,
    structural_storage_projection,
};

/// Every structural place consumed by an operation, including its ordered
/// cleanup operands. Current-graph analyses share the exhaustive inventory;
/// this query establishes no availability, custody, or permission.
pub fn operation_place_inputs(
    operation: &abstract_operations::AbstractOperation,
) -> Vec<semantic_vocabulary::PlaceId> {
    unit_validation::function_structure::operation_place_inputs(operation)
}

/// Current declaration and producer authority for one structural place.
/// Availability, the selected projection and use-site loan compatibility are
/// separate judgments; this query does not bypass their receiving validators.
pub fn structural_place_access(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<terminal_psi::StructuralAccess> {
    unit_validation::operation_contracts::structural_source_contract(function, place)
        .map(|source| source.access)
}

/// Intersect an operand's requested permission with the home and every stored
/// reference or view ceiling. This describes declared access, not liveness,
/// bounds, or the absence of overlapping active loans.
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
        && terminal_semantics::descriptor_source_projection_accessible(
            types.iter(),
            source.structural_type,
            source.multiplicity,
            source.access,
            &argument.path,
            requested,
        )
}

/// Descriptor extent is metadata, not payload. A write-only source still
/// resolves its selected descriptor's geometry under its own authority: the
/// path must traverse without a stored-referent crossing, and the selected
/// descriptor's backing admission stays the payload contract this observation
/// never reaches.
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
        && terminal_semantics::reference_source_projection_accessible(
            types.iter(),
            source.structural_type,
            source.multiplicity,
            source.access,
            &argument.path,
            requested,
        )
}

/// Backing access is distinct from owning a descriptor's carrier. Resolve it
/// from the same admitted source contract used by call and reference checking;
/// exact projection, current availability and loan compatibility remain owed.
pub fn structural_place_backing_access<'types>(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
) -> Option<terminal_psi::StructuralAccess> {
    // An arrival retains outer storage access separately from its contained
    // descriptor ceiling. Query the actual row; absent contained permission
    // cannot become a readable grant from the outer storage loan.
    if let Some(parameter) = function.arrival_parameter(place)
        && terminal_semantics::descriptor_results::is_descriptor(
            types.clone(),
            parameter.structural_type,
        )
    {
        return terminal_semantics::descriptor_parameter_access(parameter, types);
    }
    unit_validation::operation_contracts::structural_source_contract(function, place)
        .map(|source| source.backing_access(types))
}

/// Check a write against the current root's complete membership roster. The
/// path stays in the original carrier's coordinates, including every Referent
/// crossing and the selected typed field; peeling a referent must not forget
/// its enclosing predicates. This supplies no storage or live-loan authority.
pub fn structural_write_preserves_memberships(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
) -> bool {
    unit_validation::operation_contracts::structural_source_contract(function, place).is_some_and(
        |source| {
            let (whole, projected) = source.selected_qualifications(&[]);
            terminal_semantics::store_preserves_qualifications(
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
        },
    )
}

/// A checked scalar mutation may expire incoming predicate facts. Its exact
/// root still owes both carried domain Content and independent entry claims;
/// source checking, not a declaration roster, owns later predicate consumption.
pub fn structural_write_preserves_content(
    function: &optimization_unit::PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
) -> bool {
    unit_validation::operation_contracts::structural_source_contract(function, place).is_some_and(
        |source| {
            let (whole, projected) = source.selected_qualifications(&[]);
            terminal_semantics::store_preserves_domain_content(
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
        },
    )
}

/// Declaration-side owned or exclusive storage for whole-referent restoration.
/// Producer-derived view access follows its actual backing contract; an owned
/// descriptor alone does not grant referent access. The receiving custody walk
/// separately proves the origin live and the captured loan mutable.
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
                // Receiving memberships are checked by the store contract;
                // they are value facts, not a loss of storage authority.
                && source.operation_result().is_none_or(|result| result.claims.is_empty())
        },
    )
}

// Whole-unit acceptance.
pub(crate) use unit_validation::validate_psi_optimization_unit;
pub use unit_validation::{
    reconstruct_ownership_frontier_facts_with_admitted_cycle_machines,
    validate_psi_optimization_unit_with_admitted_cycle_machines,
};

// Rewrite-candidate acceptance: the typed dispatch and the result it returns.
pub use candidates::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

// The exact validators the dispatch routes to, by producing pass family.
pub use candidates::case_membership_specialization::validate_case_membership_specialization_candidate;
pub(crate) use candidates::dead_scalar_elimination::validate_dead_scalar_node_candidate;
pub use candidates::field_value_specialization::validate_field_value_specialization_candidate;
pub use candidates::state_specialization::validate_state_argument_specialization_candidate;

// The closed scalar observation boundary scalar rewrites are checked against.
pub(crate) use candidates::observation::reconstruct_closed_scalar_node_boundary;

// Current-revision value-range facts.
pub use current_value_ranges::validate_current_value_range_fact_at;
