//! Primitive call operands borrow exact established storage, including projected
//! fields. Address/access replay is shared with aggregate operands; primitive
//! carrier identity and exclusive forwarding remain this consumer's contract.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::state::State;
use checked_trees::statement::StatementNode;
use checked_trees::types::TypeReferenceHandle;
use checked_trees::{
    BorrowAccessKind, BorrowCallFact, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

use super::borrow_rows;
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

pub(crate) fn validate(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    target_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    borrow_call: &BorrowCallFact,
    access_position: Option<usize>,
) -> Result<usize, LoweringError> {
    let (_, target_access) = primitive_reference(checked, target_type)?;
    let declared_access = match target_access {
        CheckedStructuralAccess::SharedBorrow => ReferenceAccess::Shared,
        CheckedStructuralAccess::MutableBorrow => ReferenceAccess::Mutable,
        CheckedStructuralAccess::WriteOnlyBorrow => ReferenceAccess::WriteOnly,
        CheckedStructuralAccess::Owned => {
            return unsupported("primitive argument requires borrowed access");
        }
    };
    let borrowed = super::borrowed_storage::source(
        checked,
        state,
        statement,
        declared_access,
        expression,
        argument,
    )?;
    let explicit_access = match checked.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => Some(access(borrow.access)),
        _ => None,
    };
    if let CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. } = argument.source
        && explicit_access.is_none()
    {
        return unsupported("computed primitive local lacks its authored borrow");
    }
    // A bare exclusive parameter forwards an existing reference, rather than
    // forming a new loan from an owned field. Preserve its restoration boundary.
    if let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = argument.source
        && explicit_access.is_none()
        && target_access != CheckedStructuralAccess::SharedBorrow
    {
        let parameter = super::structural_parameter_at(
            checked,
            checked.state_parameters(state),
            parameter_index,
            "computed primitive argument lost its source parameter",
        )?;
        let (_, source_access) = primitive_reference(checked, parameter.type_reference)?;
        if !borrowed.place.path.is_empty()
            || source_access != CheckedStructuralAccess::MutableBorrow
            || target_access != CheckedStructuralAccess::MutableBorrow
        {
            return unsupported("computed primitive argument lacks an authored exclusive borrow");
        }
        validate_direct_forwarding(checked, state, statement, borrowed.place.root)?;
    }
    // The shared source join retained the complete declaration and projection.
    // Qualifications remain on the formal and the established source field;
    // matching the primitive carrier does not synthesize either qualification.
    let expected_kind = borrow_kind(explicit_access.unwrap_or(argument.access))?;
    let position = match access_position {
        Some(position) => position,
        None => borrow_rows::validate_argument(
            checked,
            borrow_call,
            borrowed.place.captured_place().root_symbol,
            expected_kind.clone(),
        )?,
    };
    Ok(position)
}

fn primitive_reference(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Result<(TypeReferenceHandle, CheckedStructuralAccess), LoweringError> {
    let shell = validation::reference_shell(&checked.typed, reference).ok_or(
        LoweringError::Unsupported("computed structural argument requires a primitive reference"),
    )?;
    if checked.primitive_type_reference(shell.referee).is_none()
        || !crate::content_classes::plain_owned_with_represented_constraints(checked, shell.referee)
    {
        return unsupported(
            "computed structural argument requires a represented primitive referent",
        );
    }
    Ok((shell.referee, access(shell.access)))
}

fn access(access: ReferenceAccess) -> CheckedStructuralAccess {
    match access {
        ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    }
}

fn borrow_kind(access: CheckedStructuralAccess) -> Result<BorrowAccessKind, LoweringError> {
    Ok(match access {
        CheckedStructuralAccess::SharedBorrow => BorrowAccessKind::Read,
        CheckedStructuralAccess::MutableBorrow => BorrowAccessKind::Mutable,
        CheckedStructuralAccess::WriteOnlyBorrow => BorrowAccessKind::WriteOnly,
        CheckedStructuralAccess::Owned => {
            return unsupported("computed primitive argument requires borrowed access");
        }
    })
}

fn validate_direct_forwarding(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    symbol: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    // This slice does not carry restoration certificates for a descendant
    // loan. A bare carrier cannot stand in for that missing resource custody.
    for (_, loan) in checked.facts.borrow.loans.iter() {
        if loan.statement_index <= statement as usize
            && loan.last_use_statement_index >= statement as usize
            && (loan.root_symbol == symbol
                || loan.owner_symbol == symbol
                || loan.source_owner_symbol == symbol)
        {
            return unsupported(
                "computed primitive forwarding requires unsupported loan restoration",
            );
        }
    }
    let prefix = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement as usize)
        .ok_or(LoweringError::Unsupported(
            "computed primitive forwarding has no authored prefix",
        ))?;
    for statement in prefix {
        if let StatementNode::Assignment(assignment) = statement
            && let ExpressionNode::Name(name) =
                checked.expression_table.expression(assignment.target)
            && (name.symbol == symbol || name.head_symbol == symbol)
        {
            return unsupported("computed primitive forwarding requires unchanged entry custody");
        }
    }
    Ok(())
}
