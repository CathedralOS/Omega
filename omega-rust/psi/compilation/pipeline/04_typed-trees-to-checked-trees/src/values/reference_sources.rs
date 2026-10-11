//! Reference establishments share their source join across expression positions.
//! A local reference or view names its current carrier. Its declaration's loan
//! is provenance, not an executable replacement for that carrier: reassignment
//! can change a view's backing and window without changing the local symbol.
//! Keep the logical root and occurrence's captured selectors until publication
//! joins the current home. View indexing consumes its retained descriptor;
//! stored references cross their actual pointer. Neither route replays an
//! initializer. Borrow checking still owns bounds, access and lifetime.

use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn plan(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    selector: &dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let shell = validation::reference_shell(program, reference)?;
    let (referent, access) = (shell.referee, CheckedStructuralAccess::from(shell.access));
    let Some(borrow) = program.expression_table.borrow_expression(expression) else {
        return validation::reference_result_custody::initializer_source(
            program, state, expression, reference,
        );
    };
    if CheckedStructuralAccess::from(borrow.access) != access {
        return None;
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        borrow.target,
    )?;
    let root = place.root.as_symbol()?;
    let source = if let Some((position, parameter)) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == root)
    {
        let supplied = validation::reference_shell(program, parameter.type_reference)
            .map(|shell| CheckedStructuralAccess::from(shell.access))
            .unwrap_or(CheckedStructuralAccess::Owned);
        if !access_can_supply(supplied, access) {
            return None;
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(position).ok()?,
        }
    } else {
        let (declaration_index, local) = program.state_local_declaration(state, root)?;
        if declaration_index >= statement_index || local.relevance.is_erased() {
            return None;
        }
        if let Some(shell) = validation::reference_shell(program, local.type_reference) {
            if !access_can_supply(shell.access.into(), access) {
                return None;
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                symbol: root,
                root_type_identity: program.normalized_type_identity(shell.referee),
            }
        } else {
            // Owned contents lend their admitted access independently of
            // permission to replace the source binding.
            if local.is_mutable
                && program
                    .primitive_type_reference(local.type_reference)
                    .is_some()
            {
                CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol: root }
            } else {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: root }
            }
        }
    };
    let (mut projected, mut path) =
        crate::execution::terminal_unit::calls::projected_evaluated_path(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &place,
            Some(selector),
        )?;
    // A reference formal already names its referent. At a whole-formal
    // borrow the type query still returns that ingress shell; only a stored
    // reference endpoint contributes a physical Referent crossing.
    if place.segments.is_empty()
        && matches!(
            source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        )
        && let Some(shell) = validation::reference_shell(program, projected)
    {
        projected = shell.referee;
    }
    while let Some(shell) = validation::reference_shell(program, projected) {
        if !access_can_supply(shell.access.into(), access) {
            return None;
        }
        path.push(CheckedUnitStructuralPathSegment::Referent);
        projected = shell.referee;
    }
    if program.normalized_type_identity(projected) != program.normalized_type_identity(referent) {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source,
        path,
        type_identity: program.normalized_type_identity(referent),
        access,
    })
}

fn access_can_supply(source: CheckedStructuralAccess, result: CheckedStructuralAccess) -> bool {
    source == result
        || matches!(
            source,
            CheckedStructuralAccess::Owned | CheckedStructuralAccess::MutableBorrow
        )
}
