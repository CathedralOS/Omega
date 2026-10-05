//! Calls borrow through the stored descriptor published by an ordinary result
//! binding. Source checking already owns formation and lifetime admission;
//! this operation retains the descriptor crossing and selected referent path.
use crate::execution::terminal_unit::calls::argument_paths::projected_borrowed_path;
use crate::execution::terminal_unit::types::{
    base_type_identity, structural_access_for_type_reference,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan,
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::TableLocalData;
use typed_trees::types::TypeReferenceHandle;
use validation::reference_result_custody;

/// Slice views have their own descriptor argument route.
pub(super) fn is_carrier(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    reference_result_custody::stored_loan_parts(program, reference).is_some()
}

pub(super) fn argument(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &State,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    local: &TableLocalData,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: &str,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let (referent, carrier_access) =
        reference_result_custody::stored_loan_parts(program, local.type_reference)?;
    let access = structural_access_for_type_reference(program, parameter.type_reference)?;
    if access == CheckedStructuralAccess::Owned
        || (carrier_access != CheckedStructuralAccess::MutableBorrow && carrier_access != access)
        || place.root != facts::PlaceRoot::Symbol(local.symbol)
        || result.multiplicity != Multiplicity::Affine
        || result.type_identity
            != program
                .normalized_type_identity(local.type_reference)
                .as_str()
    {
        return None;
    }
    let mut path = vec![CheckedUnitStructuralPathSegment::Referent];
    let selected = if place.segments.is_empty() {
        referent
    } else {
        let (selected, suffix) =
            projected_borrowed_path(program, machine, state.symbol, statement_index, place)?;
        path.extend(suffix);
        selected
    };
    if base_type_identity(program, selected, &[])? != target_identity {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: target_identity.to_owned(),
        access,
    })
}
