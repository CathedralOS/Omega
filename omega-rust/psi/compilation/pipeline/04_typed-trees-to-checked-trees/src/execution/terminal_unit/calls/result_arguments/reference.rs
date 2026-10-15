//! Calls borrow through the stored descriptor published by an ordinary result
//! binding. Source checking already owns formation and lifetime admission;
//! this operation retains the descriptor crossing and selected referent path.
use crate::execution::terminal_unit::calls::argument_paths::projected_borrowed_path;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
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
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;
use validation::reference_result_custody;

/// Slice views have their own descriptor argument route.
pub(crate) fn is_carrier(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    reference_result_custody::stored_loan_parts(program, reference).is_some()
}

pub(crate) fn argument(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: &State,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    owner: facts::PlaceRoot,
    reference: TypeReferenceHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: typed_trees::type_identity::TypeIdentityId,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let phase = |description| {
        trace.phase(description);
        trace.statement(u32::try_from(statement_index).ok());
    };
    phase("stored reference argument: carrier");
    let (referent, carrier_access) =
        reference_result_custody::stored_loan_parts(program, reference)?;
    let access = structural_access_for_type_reference(program, parameter.type_reference)?;
    phase("stored reference argument: access");
    if access == CheckedStructuralAccess::Owned
        || (carrier_access != CheckedStructuralAccess::MutableBorrow && carrier_access != access)
    {
        return None;
    }
    phase("stored reference argument: producer custody");
    if place.root != owner || result.multiplicity != Multiplicity::Affine {
        return None;
    }
    phase("stored reference argument: producer identity");
    if result.type_identity != program.normalized_type_identity(reference) {
        return None;
    }
    phase("stored reference argument: projected referent");
    let mut path = vec![CheckedUnitStructuralPathSegment::Referent];
    let selected = if place.segments.is_empty() {
        referent
    } else {
        let (selected, suffix) =
            projected_borrowed_path(program, machine, state.symbol, statement_index, place)?;
        path.extend(suffix);
        selected
    };
    phase("stored reference argument: lent endpoint");
    let (selected, path) = super::super::argument_paths::borrowed_argument_endpoint(
        program,
        selected,
        path,
        parameter.type_reference,
    )?;
    phase("stored reference argument: receiving identity");
    if base_type_identity(program, selected, &[], &[])? != target_identity {
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
