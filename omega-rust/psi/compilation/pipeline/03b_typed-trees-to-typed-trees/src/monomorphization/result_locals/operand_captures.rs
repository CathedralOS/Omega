//! Store the value consumed by an indexed operand, not an accidental alias.
//!
//! Resolution captures selectors before their type or operator is known. An
//! ordinary inferred initializer preserves a reference, while a builtin scalar
//! selector observes its referee. Select that distinction here using the live
//! consumer and the same formal adaptation as operator matching. Custom index
//! operands retain their declared reference or generic contract.
//!
//! The join is by the generated local's symbol, never its diagnostic name. No
//! second consumer graph is retained: specialization rewrites the actual body,
//! and copying a metadata expression would silently leave stale callees behind.
//! Read all raw result types before adapting any capture. This keeps a range's
//! complete operand tuple stable and never evaluates an initializer twice.

use super::InferredUpdate;
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::operator;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

struct Capture {
    symbol: SymbolHandle,
    raw_type: TypeReferenceHandle,
    selected: Option<TypeReferenceHandle>,
    disagrees: bool,
}

pub(super) fn adapt(program: &TypedTrees, updates: &mut [InferredUpdate]) {
    if updates.is_empty() {
        return;
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let mut captures = statements
                .iter()
                .enumerate()
                .filter_map(|(offset, statement)| {
                    let local = statement.as_local_data()?;
                    if !local.is_operand_capture || !local.type_is_inferred {
                        return None;
                    }
                    let raw_type = updates
                        .iter()
                        .find(|update| {
                            update.body == state.statement_nodes && update.offset == offset
                        })
                        .map(|update| update.type_reference)
                        .or_else(|| {
                            validation::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                local.initial_value,
                            )
                        })?;
                    Some(Capture {
                        symbol: local.symbol,
                        raw_type,
                        selected: None,
                        disagrees: false,
                    })
                })
                .collect::<Vec<_>>();
            if captures.is_empty() {
                continue;
            }
            let mut expressions = Vec::new();
            for statement in statements {
                for (_, root) in program.tables.expression_roots(statement) {
                    if root.is_valid() {
                        validation::expression_traversal::collect_expression_tree(
                            program,
                            root,
                            &mut expressions,
                        );
                    }
                }
            }
            let mut operand_uses = Vec::new();
            for &expression in &expressions {
                let ExpressionNode::Indexed(indexed) =
                    program.expression_table.expression(expression)
                else {
                    continue;
                };
                let (spelling, operands) = match program.expression_table.expression(indexed.index)
                {
                    ExpressionNode::Range(range) => (
                        OperatorSpelling::Range,
                        vec![indexed.collection, range.start, range.end],
                    ),
                    _ => (
                        OperatorSpelling::Index,
                        vec![indexed.collection, indexed.index],
                    ),
                };
                let operand_types = operands
                    .iter()
                    .map(|&operand| {
                        capture_index(program, operand, &captures)
                            .map(|index| captures[index].raw_type)
                            .or_else(|| {
                                operand
                                    .is_valid()
                                    .then(|| {
                                        validation::expression_result_type_reference(
                                            program, machine, state, operand,
                                        )
                                    })
                                    .flatten()
                            })
                    })
                    .collect::<Vec<_>>();
                for (position, &operand) in operands.iter().enumerate().skip(1) {
                    let Some(index) = capture_index(program, operand, &captures) else {
                        continue;
                    };
                    operand_uses.push(operand);
                    let selected = consumed_type(
                        program,
                        machine.symbol,
                        expression,
                        spelling,
                        &operand_types,
                        position,
                        captures[index].raw_type,
                    );
                    let capture = &mut captures[index];
                    if capture
                        .selected
                        .is_some_and(|previous| previous != selected)
                    {
                        capture.disagrees = true;
                    }
                    capture.selected = Some(selected);
                }
            }
            // The provenance flag cannot authorize changing an unrelated use.
            // Shared copies of an indexed operand are fine when all agree.
            for &expression in &expressions {
                if let Some(index) = capture_index(program, expression, &captures)
                    && !operand_uses.contains(&expression)
                {
                    captures[index].disagrees = true;
                }
            }
            for update in updates
                .iter_mut()
                .filter(|update| update.body == state.statement_nodes)
            {
                let Some(local) = statements[update.offset].as_local_data() else {
                    continue;
                };
                if let Some(capture) = captures
                    .iter()
                    .find(|capture| capture.symbol == local.symbol)
                    && !capture.disagrees
                    && let Some(selected) = capture.selected
                {
                    update.type_reference = selected;
                }
            }
        }
    }
}

fn capture_index(
    program: &TypedTrees,
    expression: ExpressionHandle,
    captures: &[Capture],
) -> Option<usize> {
    if !expression.is_valid() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    captures
        .iter()
        .position(|capture| capture.symbol == path.symbol)
}

fn consumed_type(
    program: &TypedTrees,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    spelling: OperatorSpelling,
    operands: &[Option<TypeReferenceHandle>],
    position: usize,
    actual: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let trait_meanings =
        operator::selected_trait_operator_meanings(program, owner, spelling, operands);
    let expected = if trait_meanings.is_empty() {
        operator::resolve_indexed_spelling_for_operands(
            program,
            spelling,
            operands,
            program.expression_table.source_span(expression),
        )
        .into_iter()
        .filter_map(|candidate| {
            program
                .operator_parameters(candidate.operator)
                .get(position)
                .map(|parameter| parameter.type_reference)
        })
        .collect::<Vec<_>>()
    } else {
        trait_meanings
            .into_iter()
            .filter_map(|meaning| {
                program
                    .state_signature_parameters(meaning.requirement)
                    .get(position)
                    .map(|parameter| parameter.type_reference)
            })
            .collect::<Vec<_>>()
    };
    if let Some(&first) = expected.first() {
        let selected = operator::scalar_operand_read_type(program, actual, first);
        return if expected.iter().all(|&expected| {
            operator::scalar_operand_read_type(program, actual, expected) == selected
        }) {
            selected
        } else {
            actual
        };
    }
    if !operator::has_builtin_spelled_expression_meaning(
        program, owner, expression, spelling, operands,
    ) {
        return actual;
    }
    let builtin_collection = operands
        .first()
        .copied()
        .flatten()
        .and_then(|collection| validation::unwrapped_type_reference(program, collection))
        .is_some_and(|collection| {
            matches!(
                program.type_reference_table.type_reference(collection),
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
            )
        });
    let Some(reference) = validation::reference_shell(program, actual) else {
        return actual;
    };
    if !builtin_collection
        || !matches!(
            program.primitive_type_reference(reference.referee),
            Some(
                PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
            )
        )
    {
        return actual;
    }
    // Reuse ordinary argument adaptation, preserving the referee's complete
    // range/domain and refusing write-only observation. No bounds are inferred.
    operator::scalar_operand_read_type(program, actual, reference.referee)
}
