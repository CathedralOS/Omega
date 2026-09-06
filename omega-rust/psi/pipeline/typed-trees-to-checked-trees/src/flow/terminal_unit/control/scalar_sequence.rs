//! Authored-order immutable scalar bindings and calls in one Unit state.

use super::*;

pub(super) struct ScalarSequence {
    pub(super) operations: Vec<CheckedUnitEffectOperationPlan>,
    pub(super) local_count: usize,
}

pub(super) fn has_scalar_statement_shape(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .all(|(index, statement)| match statement {
            StatementNode::Call(_) => true,
            StatementNode::Expression(_) => {
                call_occurrences::tail_call(program, state, index).is_some()
            }
            StatementNode::LocalData(local) => program
                .primitive_type_reference(local.type_reference)
                .is_some(),
            _ => false,
        })
}

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    calls: &[&checked_trees::FlowCallFact],
) -> Option<ScalarSequence> {
    let mut operations = Vec::new();
    let mut local_count = 0_usize;
    let mut call_count = 0_usize;
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let statement_index = u32::try_from(index).ok()?;
        let result = match statement {
            StatementNode::LocalData(local) => {
                if local.is_mutable
                    || !program
                        .expression_table
                        .expression_is_valid(local.initial_value)
                {
                    return None;
                }
                let binding_ordinal = u32::try_from(local_count).ok()?;
                let primitive_type = program.primitive_type_reference(local.type_reference)?;
                local_count = local_count.checked_add(1)?;
                if !matches!(
                    program.expression_table.expression(local.initial_value),
                    ExpressionNode::Call(_)
                ) {
                    let (result, value) = scalar_expression_local_at(
                        program,
                        facts,
                        state,
                        statement_index,
                        binding_ordinal,
                        local,
                    )?;
                    operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value,
                    });
                    continue;
                }
                Some(CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal,
                    primitive_type,
                })
            }
            StatementNode::Call(_) => None,
            StatementNode::Expression(_)
                if call_occurrences::tail_call(program, state, index).is_some() =>
            {
                None
            }
            _ => return None,
        };
        let mut matching = calls
            .iter()
            .copied()
            .filter(|call| call.statement_index == index && call.call_ordinal == 0);
        let call = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let authored_expression = match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            StatementNode::Expression(expression) => Some(*expression),
            _ => None,
        };
        if let Some(expression) = authored_expression {
            let ExpressionNode::Call(authored) = program.expression_table.expression(expression)
            else {
                return None;
            };
            if call.authored_expression != expression
                || call.target_symbol != authored.target_symbol
            {
                return None;
            }
        }
        call_count = call_count.checked_add(1)?;
        let operation = build_call_operation(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            &[],
            &[],
            entry_claims,
            call,
            false,
            result
                .as_ref()
                .map(|result| ExpectedCallValueResult::Scalar(result.primitive_type)),
            &[],
        )?;
        operations.push(match result {
            Some(result) => bind_scalar_call_result(facts, operation, result, true)?,
            None => operation,
        });
    }
    (call_count == calls.len()).then_some(ScalarSequence {
        operations,
        local_count,
    })
}
