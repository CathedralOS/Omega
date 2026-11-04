//! Calls borrow through the stored descriptor published by an ordinary result
//! binding. Source checking already owns formation and lifetime admission;
//! this operation retains the descriptor crossing and selected referent path.
use super::super::argument_paths::borrowed_argument_endpoint;
use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
use crate::execution::terminal_unit::calls::argument_paths::projected_evaluated_path;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::structural_access_for_type_reference;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan,
};
use facts::PlaceRoot;
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::state::State;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::TypeReferenceHandle;
use validation::{CanonicalPlace, reference_result_custody, stored_structural_type_identity};

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
    place: &CanonicalPlace,
    owner: PlaceRoot,
    reference: TypeReferenceHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: TypeReferenceHandle,
    target_identity: TypeIdentityId,
    trace: &LocalConstructionTrace,
    selectors: Option<&dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let phase = |description| {
        trace.phase(description);
        trace.statement(u32::try_from(statement_index).ok());
    };
    phase("stored reference argument: carrier");
    let (referent, carrier_access) =
        reference_result_custody::stored_loan_parts(program, reference)?;
    phase("result reference argument: formal access");
    let access = structural_access_for_type_reference(program, parameter)?;
    phase("stored reference argument: access");
    if access == CheckedStructuralAccess::Owned {
        return None;
    }
    phase("stored reference argument: producer custody");
    if place.root != owner || result.multiplicity != Multiplicity::Affine {
        return None;
    }
    phase("stored reference argument: producer identity");
    // This binding names executable storage. The exact source occurrence,
    // access and formation custody remain the independent joins above; a
    // runtime index may be absent from storage after member independence was
    // proved, without making differently indexed source applications equal.
    if result.type_identity != stored_structural_type_identity(program, reference, &[], &[]) {
        return None;
    }
    phase("stored reference argument: projected referent");
    let lends_descriptor = place.segments.is_empty()
        && stored_structural_type_identity(program, reference, &[], &[]) == target_identity;
    if !lends_descriptor
        && carrier_access != CheckedStructuralAccess::MutableBorrow
        && carrier_access != access
    {
        return None;
    }
    let (selected, path) = if lends_descriptor {
        (reference, Vec::new())
    } else if place.segments.is_empty() {
        // A saved pointer has storage of its own. Select the complete typed
        // endpoint before loading it, so `&mut &Cell` lends the writable slot
        // while `&Cell` still loads its shared pointee. Source admission owns
        // slot mutability; the old pointer's access cannot replace it.
        borrowed_argument_endpoint(
            program,
            referent,
            vec![CheckedUnitStructuralPathSegment::Referent],
            parameter,
        )?
    } else {
        phase("result reference argument: projected referent path");
        // This operand owns its selector evaluation. Stored-reference roots
        // use that occurrence's captured scalar slots just like other result
        // projections, rather than reloading an invocation-entry parameter.
        let (_, mut path) = borrowed_argument_endpoint(
            program,
            referent,
            vec![CheckedUnitStructuralPathSegment::Referent],
            parameter,
        )?;
        let ProjectedArgumentPath {
            storage_type: selected,
            path: suffix,
            ..
        } = projected_evaluated_path(
            program,
            machine,
            state.symbol,
            statement_index,
            place,
            selectors,
        )?;
        path.extend(suffix);
        borrowed_argument_endpoint(program, selected, path, parameter)?
    };
    phase("stored reference argument: lent endpoint");
    phase("stored reference argument: receiving identity");
    if stored_structural_type_identity(program, selected, &[], &[]) != target_identity {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        projection_evaluations: Default::default(),
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: target_identity.to_owned(),
        access,
    })
}
