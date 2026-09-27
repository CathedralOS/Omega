//! Non-authorizing join between one checked Atomic result contract and one
//! already-specialized provider-backed resident access request.
//!
//! This carrier neither establishes a source placed value nor attempts an
//! Atomic operation. It only retains the exact checked contract beside the
//! sealed runtime request whose placement authority already owns resident
//! custody.

use access_plans::{AtomicAccessOperation, AtomicPrimitiveAccessRequest};
use diagnostics::Diagnostic;
use language_core::atomic::AtomicObservingCompareExchangeOperation;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::typed_trees::{
    PlacedAtomicObservingResultContract, PlacedAtomicResidentContract, PlacedFieldPlan,
    PlacedViewPlan,
};

use super::plan_replay::{exact_access_entry_for_field, validate_plans};

#[derive(Debug)]
struct ReplayedCheckedAtomicContract<'checked> {
    contract: &'checked PlacedAtomicResidentContract,
    result: &'checked PlacedAtomicObservingResultContract,
}

/// One exact checked observing-result contract joined to the sealed Atomic
/// request that retains runtime resident custody.
#[derive(Debug)]
#[must_use = "checked Atomic resident access retains a non-Clone runtime request"]
pub struct CheckedAtomicResidentAccess<'checked, 'view, 'extent> {
    program: &'checked TypedTrees,
    view_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
    contract: &'checked PlacedAtomicResidentContract,
    result: &'checked PlacedAtomicObservingResultContract,
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
}

impl<'checked, 'view, 'extent> CheckedAtomicResidentAccess<'checked, 'view, 'extent> {
    pub const fn atomic_access(&self) -> &AtomicPrimitiveAccessRequest<'view, 'extent> {
        &self.access
    }

    pub const fn resident_contract(&self) -> &'checked PlacedAtomicResidentContract {
        self.contract
    }

    pub const fn observing_result(&self) -> &'checked PlacedAtomicObservingResultContract {
        self.result
    }

    /// Replay both the checked contract and complete provider-backed Atomic
    /// request before a future result-custody consumer proceeds. Failure only
    /// borrows this carrier; the exact non-Clone request remains retained.
    pub fn validate_for_result_custody(&self) -> Result<(), Vec<Diagnostic>> {
        let replayed = replay_binding(
            self.program,
            self.view_symbol,
            self.field_symbol,
            &self.access,
        )?;
        if !std::ptr::eq(replayed.contract, self.contract)
            || !std::ptr::eq(replayed.result, self.result)
        {
            return Err(vec![Diagnostic::error(
                "checked Atomic resident binding replay selected different checked contract authority",
            )]);
        }
        Ok(())
    }

    /// Remove only this checked-contract join. The original specialized
    /// request and all provider-backed resident custody remain intact.
    pub fn into_atomic_access(self) -> AtomicPrimitiveAccessRequest<'view, 'extent> {
        self.access
    }
}

/// Failed checked/runtime binding returns the exact non-Clone Atomic request.
#[derive(Debug)]
pub struct CheckedAtomicResidentAccessRejection<'view, 'extent> {
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
    diagnostics: Vec<Diagnostic>,
}

impl<'view, 'extent> CheckedAtomicResidentAccessRejection<'view, 'extent> {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(
        self,
    ) -> (
        AtomicPrimitiveAccessRequest<'view, 'extent>,
        Vec<Diagnostic>,
    ) {
        (self.access, self.diagnostics)
    }
}

/// Join one independently replayed checked observing-result contract to an
/// already-specialized Atomic request carrying exact resident custody.
///
/// `view_symbol` and `field_symbol` are compiler-internal semantic identities,
/// never source names. Rejection returns `access` unchanged and performs no
/// Atomic attempt.
pub fn bind_checked_atomic_resident_access<'checked, 'view, 'extent>(
    program: &'checked TypedTrees,
    view_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
    access: AtomicPrimitiveAccessRequest<'view, 'extent>,
) -> Result<
    CheckedAtomicResidentAccess<'checked, 'view, 'extent>,
    CheckedAtomicResidentAccessRejection<'view, 'extent>,
> {
    let replayed = match replay_binding(program, view_symbol, field_symbol, &access) {
        Ok(replayed) => replayed,
        Err(diagnostics) => {
            return Err(CheckedAtomicResidentAccessRejection {
                access,
                diagnostics,
            });
        }
    };
    Ok(CheckedAtomicResidentAccess {
        program,
        view_symbol,
        field_symbol,
        contract: replayed.contract,
        result: replayed.result,
        access,
    })
}

fn replay_binding<'checked>(
    program: &'checked TypedTrees,
    view_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
    access: &AtomicPrimitiveAccessRequest<'_, '_>,
) -> Result<ReplayedCheckedAtomicContract<'checked>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    validate_plans(program, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let view = exactly_one_view(program, view_symbol)?;
    let field = exactly_one_field(view, field_symbol)?;
    access
        .validate_against_checked_placement(&view.placement)
        .map_err(|diagnostic| {
            vec![Diagnostic::error(format!(
                "checked Atomic resident binding rejected runtime authority: {diagnostic}"
            ))]
        })?;
    let contract = field.atomic_resident.as_ref().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` has no checked observing Atomic resident contract",
            view.data_name, field.field_name
        ))]
    })?;
    let exact_access = exact_access_entry_for_field(view, field).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` has no exact canonical access entry",
            view.data_name, field.field_name
        ))]
    })?;

    let request = access.primitive_request();
    if request.plan() != view.placement.identity()
        || request.effective_supply().key() != exact_access.key()
        || request.effective_supply().field() != exact_access.field()
        || request.transfer_width_bits() != contract.transfer_width_bits
    {
        return Err(vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` does not match the Atomic request's exact plan, field key, or transfer width",
            view.data_name, field.field_name
        ))]);
    }
    if request.resident_claim().is_none() || request.placed_occurrence().is_none() {
        return Err(vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` observing Atomic request lacks runtime resident custody",
            view.data_name, field.field_name
        ))]);
    }

    let operation = match access.operation() {
        AtomicAccessOperation::CompareExchange { .. } => {
            AtomicObservingCompareExchangeOperation::Decisive
        }
        AtomicAccessOperation::CompareExchangeOnce { .. } => {
            AtomicObservingCompareExchangeOperation::SingleAttempt
        }
        _ => {
            return Err(vec![Diagnostic::error(format!(
                "placed view `{}` field `{}` checked resident binding accepts only observing decisive or single-attempt compare-exchange",
                view.data_name, field.field_name
            ))]);
        }
    };
    let permitted = match operation {
        AtomicObservingCompareExchangeOperation::Decisive => contract.compare_exchange,
        AtomicObservingCompareExchangeOperation::SingleAttempt => contract.compare_exchange_once,
    };
    if !permitted {
        return Err(vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` checked resident contract does not admit the selected observing operation",
            view.data_name, field.field_name
        ))]);
    }
    let mut results = contract
        .observing_results
        .iter()
        .filter(|row| row.operation == operation);
    let result = results.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` checked resident contract omits the selected observing result",
            view.data_name, field.field_name
        ))]
    })?;
    if results.next().is_some() || result.result_shape != operation.result_shape() {
        return Err(vec![Diagnostic::error(format!(
            "placed view `{}` field `{}` checked resident contract changed the selected observing result shape or cardinality",
            view.data_name, field.field_name
        ))]);
    }

    Ok(ReplayedCheckedAtomicContract { contract, result })
}

fn exactly_one_view(
    program: &TypedTrees,
    view_symbol: SymbolHandle,
) -> Result<&PlacedViewPlan, Vec<Diagnostic>> {
    let mut matches = program
        .placed_view_plans
        .iter()
        .filter(|view| view.data_symbol == view_symbol);
    let view = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "checked Atomic resident binding names no exact placed-view identity",
        )]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(
            "checked Atomic resident binding names a duplicate placed-view identity",
        )]);
    }
    Ok(view)
}

fn exactly_one_field(
    view: &PlacedViewPlan,
    field_symbol: SymbolHandle,
) -> Result<&PlacedFieldPlan, Vec<Diagnostic>> {
    let mut matches = view
        .fields
        .iter()
        .filter(|field| field.field_symbol == field_symbol);
    let field = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "placed view `{}` names no exact checked Atomic field identity",
            view.data_name
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "placed view `{}` repeats the checked Atomic field identity",
            view.data_name
        ))]);
    }
    Ok(field)
}
