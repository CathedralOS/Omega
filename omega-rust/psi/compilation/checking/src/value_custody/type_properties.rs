//! Invocation-owned frontier and establishment judgments over exact handles.
//!
//! Each live type or declaration has a row. Independent queries may initialize
//! different rows concurrently; recursion stays within the domain's fresh walk,
//! not a shared stack or a partially initialized answer from another root.

use crate::value_custody::claim_frontier::ClaimFrontierClaim;
use std::sync::OnceLock;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

pub(crate) struct ClaimFrontierQueries {
    rows: Vec<(TypeReferenceHandle, OnceLock<Vec<ClaimFrontierClaim>>)>,
}

impl ClaimFrontierQueries {
    fn of(program: &dyn crate::CheckingInput) -> Self {
        let mut rows: Vec<_> = program
            .type_reference_table
            .type_reference_rows()
            .map(|(handle, _)| (handle, OnceLock::new()))
            .collect();
        rows.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
        Self { rows }
    }
}

pub(crate) fn memoized_claim_frontier(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Vec<ClaimFrontierClaim> {
    let compute = || super::claim_frontier::linear_claim_frontier_uncached(program, type_reference);
    let Some(memos) = program.checking_memos() else {
        return compute();
    };
    let queries = memos
        .claim_frontiers
        .get_or_init(|| ClaimFrontierQueries::of(program));
    let Ok(position) = queries.rows.binary_search_by_key(
        &(type_reference.arena_index(), type_reference.generation()),
        |(handle, _)| (handle.arena_index(), handle.generation()),
    ) else {
        return compute();
    };
    queries.rows[position].1.get_or_init(compute).clone()
}

pub(crate) fn memoized_owns_drop_hook(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> bool {
    program.attached_drop_machine_exists(symbol)
}

pub(crate) struct EstablishmentQueries {
    rows: Vec<(super::data::EstablishmentIdentity, OnceLock<bool>)>,
    type_rows: Vec<(TypeReferenceHandle, OnceLock<bool>)>,
}

impl EstablishmentQueries {
    fn of(program: &dyn crate::CheckingInput) -> Self {
        let mut identities = program
            .data_definitions()
            .iter()
            .map(super::data::EstablishmentIdentity::of)
            .collect::<Vec<_>>();
        identities.sort_by_key(|identity| identity.order());
        identities.dedup();
        let mut type_rows: Vec<_> = program
            .type_reference_table
            .type_reference_rows()
            .map(|(handle, _)| (handle, OnceLock::new()))
            .collect();
        type_rows.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
        Self {
            rows: identities
                .into_iter()
                .map(|identity| (identity, OnceLock::new()))
                .collect(),
            type_rows,
        }
    }
}

pub(crate) fn memoized_type_requires_establishment(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    compute: impl FnOnce() -> bool,
) -> bool {
    let Some(memos) = program.checking_memos() else {
        return compute();
    };
    let queries = memos
        .establishment
        .get_or_init(|| EstablishmentQueries::of(program));
    let Ok(position) = queries.type_rows.binary_search_by_key(
        &(type_reference.arena_index(), type_reference.generation()),
        |(handle, _)| (handle.arena_index(), handle.generation()),
    ) else {
        return compute();
    };
    *queries.type_rows[position].1.get_or_init(compute)
}

pub(crate) fn memoized_data_requires_establishment(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    compute: impl FnOnce() -> bool,
) -> bool {
    let Some(memos) = program.checking_memos() else {
        return compute();
    };
    let queries = memos
        .establishment
        .get_or_init(|| EstablishmentQueries::of(program));
    let Ok(position) = queries.rows.binary_search_by_key(
        &super::data::EstablishmentIdentity::of(definition).order(),
        |(identity, _)| identity.order(),
    ) else {
        return compute();
    };
    *queries.rows[position].1.get_or_init(compute)
}
