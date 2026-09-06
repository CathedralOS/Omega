//! Authored-order immutable scalar and structural bindings in one Unit state.

use super::*;

pub(super) struct StatementSequence {
    pub(super) operations: Vec<CheckedUnitEffectOperationPlan>,
    pub(super) local_count: usize,
    pub(super) structural_local_symbols: Vec<SymbolHandle>,
}

pub(super) fn has_structural_result(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    statement: &StatementNode,
) -> bool {
    let StatementNode::LocalData(local) = statement else {
        return false;
    };
    if !local.initial_value.is_valid() {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(local.initial_value)
    else {
        return false;
    };
    facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .any(|plan| plan.state == call.target_symbol)
        || (program
            .primitive_type_reference(local.type_reference)
            .is_none()
            && validation::has_plain_owned_contents(program, local.type_reference)
            && validation::unit_result_initializer_call_is_supported(
                program,
                machine,
                local.initial_value,
            ))
}

pub(super) fn has_statement_shape(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    construction_statement_count: usize,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .skip(construction_statement_count)
        .all(|(index, statement)| match statement {
            StatementNode::Call(_) => true,
            StatementNode::Expression(_) => {
                call_occurrences::tail_call(program, state, index).is_some()
            }
            StatementNode::LocalData(local) => {
                program
                    .primitive_type_reference(local.type_reference)
                    .is_some()
                    || has_structural_result(program, facts, machine, statement)
            }
            _ => false,
        })
}

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    calls: &[&checked_trees::FlowCallFact],
    trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    affine_scalar_record_locals: &[AffineScalarRecordLocal],
    construction_statement_count: usize,
) -> Option<StatementSequence> {
    let mut operations = Vec::new();
    let mut local_count = construction_statement_count;
    let mut scalar_count = 0_usize;
    let mut structural_count = 0_usize;
    let mut structural_local_symbols = Vec::new();
    // Only whole claim-free affine results participate in move custody.
    // Unrestricted boundary results keep their separate non-moving route.
    let mut structural_results = Vec::new();
    let mut call_count = 0_usize;
    let binders = machine_binders(program, machine);
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .skip(construction_statement_count)
    {
        let statement_index = u32::try_from(index).ok()?;
        let mut structural_result = None;
        let result = match statement {
            StatementNode::LocalData(local) => {
                if local.is_mutable
                    || !program
                        .expression_table
                        .expression_is_valid(local.initial_value)
                {
                    return None;
                }
                local_count = local_count.checked_add(1)?;
                if let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
                {
                    let binding_ordinal = u32::try_from(scalar_count).ok()?;
                    scalar_count = scalar_count.checked_add(1)?;
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
                } else {
                    let (mut result, symbol) = checked_unit_structural_result_local(
                        program,
                        shapes,
                        std::slice::from_ref(statement),
                        &binders,
                    )?;
                    result.statement_index = statement_index;
                    result.binding_ordinal = u32::try_from(structural_count).ok()?;
                    structural_result = Some((result, facts::PlaceRoot::Symbol(symbol)));
                    None
                }
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
        for nested in structural_operands::for_call(program, facts, machine, state, call)? {
            let target = facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .iter()
                .find(|target| target.state == nested.target_symbol)?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(structural_count).ok()?,
                type_identity: target.result.type_identity.clone(),
                multiplicity: Multiplicity::Affine,
            };
            let operation = build_call_operation(
                program,
                facts,
                machine,
                state,
                structural_parameters,
                trivial_affine_locals,
                affine_scalar_record_locals,
                entry_claims,
                nested,
                false,
                Some(ExpectedCallValueResult::Structural(&result)),
                &structural_results,
            )?;
            if !matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCall { .. }
            ) {
                return None;
            }
            consume_results(&mut operations, &operation)?;
            operations.push(operation);
            structural_results.push((
                result,
                facts::PlaceRoot::Expression(nested.authored_expression),
            ));
            structural_count = structural_count.checked_add(1)?;
        }
        if let Some((result, _)) = &mut structural_result {
            result.binding_ordinal = u32::try_from(structural_count).ok()?;
        }
        let mut operation = build_call_operation(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            affine_scalar_record_locals,
            entry_claims,
            call,
            false,
            result
                .as_ref()
                .map(|result| ExpectedCallValueResult::Scalar(result.primitive_type))
                .or_else(|| {
                    structural_result
                        .as_ref()
                        .map(|(result, _)| ExpectedCallValueResult::Structural(result))
                }),
            &structural_results,
        )?;
        if let Some((result, symbol)) = structural_result {
            operation = bind_structural_call_result(operation, result.clone())?;
            if let facts::PlaceRoot::Symbol(symbol) = symbol {
                structural_local_symbols.push(symbol);
                if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                ) && result.multiplicity == Multiplicity::Affine
                {
                    structural_results.push((result, facts::PlaceRoot::Symbol(symbol)));
                }
            } else {
                return None;
            }
            structural_count = structural_count.checked_add(1)?;
        }
        consume_results(&mut operations, &operation)?;
        operations.push(match result {
            Some(result) => bind_scalar_call_result(facts, operation, result, true)?,
            None => operation,
        });
    }
    if operations.iter().any(|operation| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::StructuralCall { .. }
        )
    }) {
        for plan in &facts.flow.terminal_structural_returns.structural_types {
            if shapes
                .types
                .get(&plan.identity)
                .is_some_and(|existing| existing != plan)
            {
                return None;
            }
            shapes.types.insert(plan.identity.clone(), plan.clone());
        }
    }
    (call_count == calls.len()).then_some(StatementSequence {
        operations,
        local_count,
        structural_local_symbols,
    })
}

fn consume_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    if let CheckedUnitEffectOperationPlan::StructuralCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::CallUnit {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        structural_arguments,
        ..
    } = consumer
    {
        for binding_ordinal in structural_arguments
            .iter()
            .filter_map(|argument| argument.source_structural_result_binding_ordinal())
        {
            let mut producers = operations.iter_mut().filter(|operation| {
                matches!(operation,
                CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                    if result.binding_ordinal == binding_ordinal)
            });
            let producer = producers.next()?;
            if producers.next().is_some() {
                return None;
            }
            let (CheckedUnitEffectOperationPlan::StructuralCall {
                discard_result_on_return,
                result,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                discard_result_on_return,
                result,
                ..
            }) = producer
            else {
                unreachable!()
            };
            if result.multiplicity != Multiplicity::Affine || !*discard_result_on_return {
                return None;
            }
            *discard_result_on_return = false;
        }
    }
    Some(())
}
