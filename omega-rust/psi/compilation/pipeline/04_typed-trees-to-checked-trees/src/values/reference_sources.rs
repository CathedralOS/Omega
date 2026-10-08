//! Reference establishments share their source join across expression positions.
//! Slice descriptors lend existing collection storage: their checked loan names
//! that backing, so resolving a static selection through the descriptor preserves
//! the original array's geometry. Stored reference carriers are different: their
//! runtime address must remain the source, never a replay of their initializer.
//! This joins admitted custody; borrow checking still owns access and lifetime.

use checked_trees::{
    BorrowFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn plan(
    program: &dyn validation::CheckingInput,
    borrow_facts: &BorrowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
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
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        borrow.target,
    )?;
    let mut visited = Vec::new();
    loop {
        let root = place.root.as_symbol()?;
        let Some((declaration_index, local)) = program.state_local_declaration(state, root) else {
            break;
        };
        if validation::slice_view_shell(program, local.type_reference, &[]).is_none() {
            break;
        }
        // Only a loan with this declaration's exact formation may substitute
        // a descriptor. A different occurrence or a cycle supplies no origin.
        if declaration_index >= statement_index || visited.contains(&root) {
            return None;
        }
        visited.push(root);
        let borrowing =
            borrow_facts.states.iter().map(|(_, row)| row).find(|row| {
                row.machine_symbol == machine.symbol && row.state_symbol == state.symbol
            })?;
        let mut loans = borrow_facts.loans.iter().filter(|(handle, loan)| {
            borrow_facts.state_owns_loan(borrowing, *handle)
                && loan.owner_symbol == root
                && loan.statement_index == declaration_index
                && borrow_facts.loan_owner_path(loan).is_empty()
        });
        let (_, loan) = loans.next()?;
        let supplied = match loan.kind {
            checked_trees::BorrowAccessKind::Read => CheckedStructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Mutable => CheckedStructuralAccess::MutableBorrow,
            checked_trees::BorrowAccessKind::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
        };
        if loans.next().is_some() || !access_can_supply(supplied, access) {
            return None;
        }
        let mut segments = borrow_facts.loan_segments(loan).to_vec();
        segments.extend(place.segments);
        place = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(loan.root_symbol),
            segments,
        };
    }
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
                root_type_identity: program
                    .normalized_type_identity(shell.referee)
                    .into_string(),
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
        crate::execution::terminal_unit::calls::projected_borrowed_path(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &place,
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
        type_identity: program.normalized_type_identity(referent).into_string(),
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
