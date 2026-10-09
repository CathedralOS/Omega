//! Preserve scalar operation meaning while substituting structural operands.
//! This forms congruence terms; it neither evaluates arithmetic nor licenses
//! reassociation. The original declaration supplies operator and carrier facts.

use super::StructuralTerm;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Forms the congruence term for the selected binary operation, with an
/// optional declaring `(machine, state)` fallback plus the expression's
/// expected (landed) result type. Pure literal operands carry no `Name`, so
/// `expression_scope` cannot anchor a scope walk, and fully anonymous operands
/// carry no resolvable result reference; callers that already know the
/// expression's owning state and expected type pass them through instead of
/// losing the term. An explicit declaration frame takes precedence over a
/// name anchor, so contract formals remain in their own telescope. Selected
/// callable operations retain that exact application and result type rather
/// than acquiring builtin arithmetic or identity laws.
pub(crate) fn binary_term_in_scope(
    program: &dyn crate::CheckingInput,
    scope: Option<(&Machine, &State, TypeReferenceHandle)>,
    frame: Option<(SymbolHandle, &[StateParameter])>,
    expression: ExpressionHandle,
    left: StructuralTerm,
    right: StructuralTerm,
) -> Option<StructuralTerm> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    let state_scope = scope
        .map(|(machine, state, _)| (machine, state))
        .or_else(|| {
            expression_scope(program, expression, 0)
                .and_then(|symbol| scoped_machine_state(program, symbol))
        });
    let owner = frame
        .map(|(owner, _)| owner)
        .or_else(|| state_scope.map(|(machine, _)| machine.symbol))?;
    let expected = scope.map(|(_, _, expected)| expected);
    let expression_type = |expression| {
        if let Some((owner, parameters)) = frame {
            crate::parameter_expression_result_type_reference(
                program, owner, parameters, expression,
            )
        } else {
            let (machine, state) = state_scope?;
            crate::expression_result_type_reference(program, machine, state, expression)
        }
    };
    let references = [binary.left, binary.right].map(expression_type);
    if binary.operator == BinaryOperator::CaseMembership {
        return None;
    }
    let spelling = crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
        binary.operator,
    );
    if let Some(spelling) = spelling
        && !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            expression,
            spelling,
            &references,
        )
    {
        if references.iter().any(Option::is_none)
            || !typed_trees::operator::selected_trait_operator_meanings(
                program,
                owner,
                spelling,
                &references,
            )
            .is_empty()
        {
            return None;
        }
        let candidates = typed_trees::operator::resolve_spelling_for_operands(
            program,
            spelling,
            &references,
            program.expression_table.source_span(expression),
        );
        let [selected] = candidates.as_slice() else {
            return None;
        };
        // The application is denotational, not an executed call snapshot.
        // Its selected public result equations still require their premises
        // in the common contract normalizer before supplying any truth.
        if !crate::machine_calls::denotational_calls::selected_operator_has_closed_value_denotation(
            program,
            selected.operator,
        ) {
            return None;
        }
        let result_type = expression_type(expression)?;
        let name = program
            .operator_path_members(selected.operator.name)
            .last()?;
        return Some(StructuralTerm::Application {
            result_type,
            target: selected.operator.symbol,
            selections: Vec::new(),
            machine: super::structural_call_machine_name(name.as_str(), &[], &[]),
            arguments: vec![left, right],
        });
    }
    let parent_reference = || expression_type(expression);
    let reference = |index: usize| {
        references[index]
            .or_else(|| {
                crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
                .then_some(references[1 - index])
                .flatten()
            })
            .or_else(|| {
                crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
                .then(parent_reference)
                .flatten()
            })
            .or_else(|| {
                // Fully anonymous operand trees inherit the expression's
                // expected (landed) type -- the same resolution integer landing
                // applies when the operation is checked.
                crate::value_custody::literals::has_anonymous_numeric_results(
                    program,
                    [binary.left, binary.right][index],
                )
                .then_some(expected)
                .flatten()
            })
    };
    let meaning = [reference(0)?, reference(1)?].map(|reference| {
        Some((
            program.primitive_type_reference(reference)?,
            program.arithmetic_domain_for_type_reference(reference),
        ))
    });
    Some(StructuralTerm::ScalarBinary {
        operator: binary.operator,
        meaning: [meaning[0]?, meaning[1]?],
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// Walk an anchored name's symbol upward to its declaring machine and state.
fn scoped_machine_state<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<(&'program Machine, &'program State)> {
    let mut owner = symbol;
    let mut visited = Vec::new();
    loop {
        if !owner.is_valid() || visited.contains(&owner) {
            return None;
        }
        visited.push(owner);
        if let Some(selected) = program.machines().iter().find_map(|machine| {
            program
                .state_in_machine(machine, owner)
                .map(|state| (machine, state))
        }) {
            return Some(selected);
        }
        if let Some(machine) = program.machine(owner) {
            return Some((machine, program.machine_states(machine).first()?));
        }
        owner = program.symbols.get(owner).parent;
    }
}

fn expression_scope(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<SymbolHandle> {
    if depth >= 32 {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => path.symbol.is_valid().then_some(path.symbol),
        ExpressionNode::Binary(binary) => expression_scope(program, binary.left, depth + 1)
            .or_else(|| expression_scope(program, binary.right, depth + 1)),
        ExpressionNode::Member(member) => expression_scope(program, member.receiver, depth + 1),
        ExpressionNode::Cast(cast) => expression_scope(program, cast.value, depth + 1),
        _ => None,
    }
}
