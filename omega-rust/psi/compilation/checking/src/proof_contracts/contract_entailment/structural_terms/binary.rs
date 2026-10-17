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

/// Boolean negation is the builtin equality of its Boolean operand to false.
/// Keep that typed value relationship so captured false guards survive both
/// declaration substitution and runtime observation. An opaque source string
/// cannot carry this relationship, and no numerical complement law is implied.
pub(crate) fn logical_not_term_in_scope(
    program: &dyn crate::CheckingInput,
    scope: Option<(&Machine, &State, TypeReferenceHandle)>,
    frame: Option<(SymbolHandle, &[StateParameter])>,
    expression: ExpressionHandle,
    operand: StructuralTerm,
) -> Option<StructuralTerm> {
    let ExpressionNode::Unary(unary) = program.expression_table.expression(expression) else {
        return None;
    };
    if unary.operator != typed_trees::expression::UnaryOperator::LogicalNot {
        return None;
    }
    let reference = if let Some((owner, parameters)) = frame {
        crate::parameter_expression_result_type_reference(program, owner, parameters, unary.operand)
    } else {
        let (machine, state, _) = scope?;
        crate::expression_result_type_reference(program, machine, state, unary.operand)
    }?;
    if program.primitive_type_reference(reference) != Some(typed_trees::types::PrimitiveType::Bool)
    {
        return None;
    }
    let meaning = super::scalar_operand_meaning(program, reference, &operand)?;
    Some(StructuralTerm::ScalarBinary {
        references: [reference; 2],
        operator: BinaryOperator::Equal,
        meaning: [meaning; 2],
        left: Box::new(operand),
        right: Box::new(StructuralTerm::Constructor {
            data: "bool".to_owned(),
            case: "false".to_owned(),
            fields: Vec::new(),
        }),
    })
}

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
    if let Some(term) = super::builtin_anonymous_comparison_term(program, expression) {
        return Some(term);
    }
    // The selected builtin mathematical operation has no finite carrier or
    // overflow policy. Preserve it separately instead of borrowing an i64/u64
    // meaning merely to transport its captured operands through the judge.
    if super::super::proof_integer::mathematical_integer_operands(
        program,
        [binary.left, binary.right],
    ) {
        return Some(StructuralTerm::MathematicalBinary {
            operator: binary.operator,
            left: Box::new(left),
            right: Box::new(right),
        });
    }
    let references = crate::value_custody::expression_types::builtin_scalar_operand_references(
        program, binary, references, expected,
    )?;
    let meaning = [
        super::scalar_operand_meaning(program, references[0], &left)?,
        super::scalar_operand_meaning(program, references[1], &right)?,
    ];
    Some(StructuralTerm::ScalarBinary {
        references,
        operator: binary.operator,
        meaning,
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// Walk an anchored name's symbol upward to its declaring machine and state.
pub(super) fn scoped_machine_state<'program>(
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

pub(super) fn expression_scope(
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
        ExpressionNode::Indexed(indexed) => {
            expression_scope(program, indexed.collection, depth + 1)
                .or_else(|| expression_scope(program, indexed.index, depth + 1))
        }
        ExpressionNode::Cast(cast) => expression_scope(program, cast.value, depth + 1),
        _ => None,
    }
}
