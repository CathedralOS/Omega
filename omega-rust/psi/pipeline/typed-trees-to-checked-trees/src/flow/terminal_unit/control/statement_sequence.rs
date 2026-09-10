//! Authored-order scalar bindings, primitive storage, and structural bindings.
//! Constructors and calls share structural binding ordinals. Completion selects
//! one existing result or constructs its final expression in this same sequence;
//! surrounding supported effects do not select a different producer family.

use super::*;
use checked_trees::CheckedUnitStructuralReturnPlan;

pub(in crate::flow::terminal_unit) struct StatementSequence {
    pub(in crate::flow::terminal_unit) scalar_result: Option<CheckedUnitScalarResultBindingPlan>,
    pub(in crate::flow::terminal_unit) structural_result: Option<CheckedUnitStructuralReturnPlan>,
    pub(in crate::flow::terminal_unit) operations: Vec<CheckedUnitEffectOperationPlan>,
    pub(in crate::flow::terminal_unit) local_count: usize,
    pub(in crate::flow::terminal_unit) structural_local_symbols: Vec<SymbolHandle>,
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
    if validation::is_closed_primitive_array_type(program, local.type_reference) {
        return validation::scalar_array_elements(
            program,
            machine.symbol,
            local.initial_value,
            local.type_reference,
        )
        .is_some()
            || matches!(
                program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(_)
            );
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
            StatementNode::Call(_) | StatementNode::Assignment(_) => true,
            StatementNode::Expression(_) => {
                call_occurrences::tail_call(program, state, index).is_some()
                    || (index + 1
                        == program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len()
                        && (validation::is_closed_primitive_array_type(program, state.return_type)
                            || program
                                .primitive_type_reference(state.return_type)
                                .is_some()))
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

pub(in crate::flow::terminal_unit) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
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
    let mut array_bindings = Vec::<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>::new();
    let mut returned_call = None;
    let mut returned_scalar_call = None;
    // Only whole claim-free affine results participate in move custody.
    // Unrestricted boundary results keep their separate non-moving route.
    let mut structural_results = Vec::new();
    let mut call_count = 0_usize;
    let binders = machine_binders(program, machine);
    let mut stores =
        super::super::structural_scalar_store::build_structural_scalar_field_store_sequence(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            construction_statement_count,
        )?
        .into_iter();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .skip(construction_statement_count)
        .take_while(|(_, statement)| {
            !matches!(statement, StatementNode::Transition(_))
                && !matches!(statement, StatementNode::Expression(expression)
                    if !is_unit(program, state.return_type)
                        && !matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_)))
        })
    {
        let statement_index = u32::try_from(index).ok()?;
        let completes_machine = matches!(statement, StatementNode::Expression(_))
            && !is_unit(program, state.return_type);
        let mut structural_result = None;
        let result = match statement {
            StatementNode::Assignment(_) => {
                let store = stores.next()?;
                let ordinal = match &store {
                    CheckedUnitEffectOperationPlan::ByteSequenceWrite(store) => {
                        store.statement_index
                    }
                    CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                        statement_index,
                        ..
                    } => *statement_index,
                    CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                        store.statement_index
                    }
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                        store.statement_index
                    }
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                        store.statement_index
                    }
                    _ => return None,
                };
                if ordinal != statement_index {
                    return None;
                }
                operations.push(store);
                continue;
            }
            StatementNode::LocalData(local) => {
                if !program
                    .expression_table
                    .expression_is_valid(local.initial_value)
                {
                    return None;
                }
                local_count = local_count.checked_add(1)?;
                if validation::is_closed_primitive_array_type(program, local.type_reference)
                    && !matches!(program.expression_table.expression(local.initial_value), ExpressionNode::Call(_)) {
                    if local.is_mutable {
                        return None;
                    }
                    let result = CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(structural_count).ok()?,
                        type_identity: shapes.add_type(local.type_reference, &binders, &[])?,
                        multiplicity: Multiplicity::Unrestricted,
                    };
                    let elements = super::scalar_arrays::elements(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        checked_trees::CheckedArrayConstructionSource::Statement,
                        local.initial_value,
                        local.type_reference,
                    )?;
                    structural_count = structural_count.checked_add(1)?;
                    array_bindings.push((local.symbol, result.clone()));
                    structural_results.push((result.clone(), facts::PlaceRoot::Symbol(local.symbol)));
                    structural_local_symbols.push(local.symbol);
                    operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source: checked_trees::CheckedArrayConstructionSource::Statement,
                        result,
                        elements,
                    });
                    continue;
                }
                if let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
                {
                    if local.is_mutable {
                        if !matches!(
                            program
                                .type_reference_table
                                .type_reference(local.type_reference),
                            TypeReferenceNode::Named { .. }
                        ) {
                            return None;
                        }
                        let (binding, value) =
                            facts.values.scalar_expressions.bound_expression_at(
                                state.symbol,
                                statement_index,
                                CheckedScalarExpressionRole::StorageInitializer,
                            )?;
                        if binding.expression != local.initial_value
                            || binding.destination != local.symbol
                            || crate::values::scalar_expression_type(value) != Some(primitive_type)
                            || !super::super::primitive_store::scalar_custody_is_exact(
                                program,
                                facts,
                                state,
                                binding,
                                value,
                                primitive_type,
                            )
                            || matches!(value, CheckedScalarExpression::Boolean(expression) if checked_boolean_contains_short_circuit(expression))
                        {
                            return None;
                        }
                        operations.push(CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                            statement_index,
                            symbol: local.symbol,
                            type_identity: shapes.add_type(local.type_reference, &binders, &[])?,
                            primitive_type,
                            value: value.clone(),
                        });
                        continue;
                    }
                    let binding_ordinal = u32::try_from(scalar_count).ok()?;
                    scalar_count = scalar_count.checked_add(1)?;
                    if !matches!(
                        program.expression_table.expression(local.initial_value),
                        ExpressionNode::Call(_)
                    ) {
                        let (result, value) = scalar_computation_local_at(
                            program,
                            facts,
                            machine.symbol,
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
                    if local.is_mutable {
                        return None;
                    }
                    let (mut result, symbol) = checked_unit_structural_result_local(
                        program,
                        shapes,
                        std::slice::from_ref(statement),
                        &binders,
                    )?;
                    result.statement_index = statement_index;
                    result.binding_ordinal = u32::try_from(structural_count).ok()?;
                    structural_result = Some((result, Some(symbol)));
                    None
                }
            }
            StatementNode::Call(call) if call.discards_result => {
                let result_type =
                    crate::flow::call_target_return_type(program, call.target_symbol)?;
                if !is_unit(program, result_type) {
                    let mut result =
                        checked_structural_result_type(program, shapes, result_type, &binders)?;
                    result.statement_index = statement_index;
                    // No source local exists for an explicit discard. The call
                    // still produces its own typed result and disposal debt.
                    structural_result = Some((result, None));
                }
                None
            }
            StatementNode::Call(_) => None,
            StatementNode::Expression(_) if completes_machine
                && program.primitive_type_reference(state.return_type).is_some() =>
            {
                let result = CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(scalar_count).ok()?,
                    primitive_type: program.primitive_type_reference(state.return_type)?,
                };
                scalar_count = scalar_count.checked_add(1)?;
                returned_scalar_call = Some(result);
                Some(result)
            }
            StatementNode::Expression(_) if completes_machine => {
                let mut result = checked_structural_result_type(program, shapes, state.return_type, &binders)?;
                result.statement_index = statement_index;
                structural_result = Some((result, None));
                None
            }
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
        for operand in structural_operands::operations_for_call(program, facts, machine, state, call)? {
            let nested = match operand {
                structural_operands::Operand::Array(array) => {
                    let result = CheckedUnitStructuralResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(structural_count).ok()?,
                        type_identity: shapes.add_type(array.type_reference, &[], &[])?,
                        multiplicity: Multiplicity::Unrestricted,
                    };
                    let elements = super::scalar_arrays::elements(program, facts, machine.symbol,
                        state.symbol, statement_index, array.source, array.expression, array.type_reference)?;
                    structural_count = structural_count.checked_add(1)?;
                    structural_results.push((result.clone(), facts::PlaceRoot::Expression(array.expression)));
                    operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        source: array.source, result, elements,
                    });
                    continue;
                }
                structural_operands::Operand::Call(nested) => nested,
            };
            let target = structural_operands::result(
                program,
                facts,
                machine.symbol,
                nested.authored_expression,
                shapes,
            )?;
            let result = CheckedUnitStructuralResultBindingPlan {
                statement_index,
                binding_ordinal: u32::try_from(structural_count).ok()?,
                type_identity: target.type_identity,
                multiplicity: target.multiplicity,
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
            let operation = bind_structural_call_result(operation, result.clone())?;
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
        // The existing sole-call partial-return route remains available to
        // native consumers. Wider statement schedules use a dying continuation.
        let partial_temporary = if program
            .statement_table
            .statements(state.statement_nodes)
            .len()
            > 1
            && entry_claims.is_empty()
        {
            structural_results.last().and_then(|(result, root)| {
                if result.statement_index != statement_index
                    || !matches!(root, facts::PlaceRoot::Expression(_))
                {
                    return None;
                }
                let candidate = super::super::cleanup::anonymous::binding_at(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    index,
                    result.binding_ordinal,
                )?;
                (candidate.0 == *result && candidate.1 == *root).then_some(candidate)
            })
        } else {
            None
        };
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
            partial_temporary.is_some(),
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
        if let Some((result, None)) = &structural_result
            && !completes_machine {
            // An explicit discard still invokes the value-returning machine.
            // Its anonymous result cannot enter the named-local operand roster.
            // Dispose plain affine contents on this normal continuation, before
            // the next authored effect, rather than extending custody to return.
            operation = bind_structural_call_result(operation, result.clone())?;
            let (CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                discard_result_on_return,
                ..
            }) = &mut operation
            else {
                return None;
            };
            *discard_result_on_return = false;
            let coordinate = *coordinate;
            consume_results(&mut operations, &operation)?;
            operations.push(operation);
            if result.multiplicity == Multiplicity::Affine {
                operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards: vec![CheckedUnitPartialAffineDiscardPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path: Vec::new(),
                        type_identity: result.type_identity.clone(),
                    }],
                });
            }
            structural_count = structural_count.checked_add(1)?;
            continue;
        }
        if let Some((result, symbol)) = structural_result {
            operation = bind_structural_call_result(operation, result.clone())?;
            if completes_machine {
                returned_call = Some(result.clone());
            }
            if let Some(symbol) = symbol {
                structural_local_symbols.push(symbol);
                if matches!(statement, StatementNode::LocalData(local)
                    if validation::is_closed_primitive_array_type(program, local.type_reference)) {
                    array_bindings.push((symbol, result.clone()));
                }
                if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                ) && (result.multiplicity == Multiplicity::Affine
                    || array_bindings.iter().any(|(binding, _)| *binding == symbol))
                {
                    structural_results.push((result, facts::PlaceRoot::Symbol(symbol)));
                }
            }
            structural_count = structural_count.checked_add(1)?;
        }
        consume_results(&mut operations, &operation)?;
        operations.push(match result {
            Some(result) => bind_scalar_call_result(facts, operation, result, !completes_machine)?,
            None => operation,
        });
        if let Some((result, root)) = partial_temporary {
            super::super::cleanup::anonymous::append_continuation(
                program,
                facts,
                shapes,
                machine,
                state,
                &result,
                root,
                &mut operations,
            )?;
        } else {
            append_call_cleanup(&mut operations, statement_index, &structural_results)?;
        }
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
    let structural_result =
        if validation::is_closed_primitive_array_type(program, state.return_type) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let StatementNode::Expression(expression) = statements.last()? else {
                return None;
            };
            let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
            if let ExpressionNode::Name(path) = program.expression_table.expression(*expression) {
                if let Some((_, binding)) = array_bindings
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                {
                    if binding.type_identity
                        != program.normalized_type_identity(state.return_type).as_str()
                    {
                        return None;
                    }
                    Some(binding.clone().into())
                } else {
                    let source_parameters = program.state_parameters(state);
                    let (parameter_index, parameter) = structural_parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| {
                            source_parameters
                                .get(parameter.position as usize)
                                .is_some_and(|source| source.symbol == path.symbol)
                        })?;
                    if parameter.access != CheckedStructuralAccess::Owned
                        || parameter.multiplicity != Multiplicity::Unrestricted
                        || !parameter.qualifications.is_empty()
                        || parameter.type_identity
                            != program.normalized_type_identity(state.return_type).as_str()
                    {
                        return None;
                    }
                    Some(CheckedUnitStructuralReturnPlan {
                        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: u32::try_from(parameter_index).ok()?,
                        },
                        type_identity: parameter.type_identity.clone(),
                        multiplicity: parameter.multiplicity,
                    })
                }
            } else if matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Call(_)
            ) {
                returned_call.map(Into::into)
            } else {
                let elements = super::scalar_arrays::elements(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    checked_trees::CheckedArrayConstructionSource::Statement,
                    *expression,
                    state.return_type,
                )?;
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(structural_count).ok()?,
                    type_identity: shapes.add_type(state.return_type, &binders, &[])?,
                    multiplicity: Multiplicity::Unrestricted,
                };
                operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    source: checked_trees::CheckedArrayConstructionSource::Statement,
                    result: result.clone(),
                    elements,
                });
                Some(result.into())
            }
        } else {
            None
        };
    if returned_scalar_call.is_none()
        && let Some(primitive_type) = program.primitive_type_reference(state.return_type)
    {
        let statements = program.statement_table.statements(state.statement_nodes);
        let StatementNode::Expression(expression) = statements.last()? else {
            return None;
        };
        let statement_index = u32::try_from(statements.len().checked_sub(1)?).ok()?;
        let role = CheckedScalarExpressionRole::Return;
        let computations = &facts.values.scalar_computations;
        let mut roots = computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| {
                root.state == state.symbol
                    && root.statement_ordinal == statement_index
                    && root.role == role
            });
        let value = if let Some(root) = roots.next() {
            if roots.next().is_some()
                || root.machine != machine.symbol
                || !computations.nodes.is_valid(root.root)
                || computations.nodes.get(root.root).authored_root != *expression
                || computations.nodes.get(root.root).primitive_type != primitive_type
                || facts
                    .values
                    .scalar_expressions
                    .expression_at(state.symbol, statement_index, role)
                    .is_some()
            {
                return None;
            }
            checked_trees::CheckedCallScalarArgument::Computation(root.root)
        } else {
            let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                state.symbol,
                statement_index,
                role,
            )?;
            if binding.expression != *expression
                || crate::values::scalar_expression_type(value) != Some(primitive_type)
            {
                return None;
            }
            checked_trees::CheckedCallScalarArgument::Pure(value.clone())
        };
        let result = CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(scalar_count).ok()?,
            primitive_type,
        };
        // Completion evaluates its actual expression after the preceding
        // operations. A final name reuses its value without replaying its call.
        operations.push(CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value });
        returned_scalar_call = Some(result);
    }
    (call_count == calls.len()).then_some(StatementSequence {
        scalar_result: returned_scalar_call,
        structural_result,
        operations,
        local_count,
        structural_local_symbols,
    })
}

fn scalar_computation_local_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    binding_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<(
    CheckedUnitScalarResultBindingPlan,
    checked_trees::CheckedCallScalarArgument,
)> {
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal };
    let computations = &facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.symbol
                && root.statement_ordinal == statement_index
                && root.role == role
        });
    let Some(root) = roots.next() else {
        let (result, value) = scalar_expression_local_at(
            program,
            facts,
            state,
            statement_index,
            binding_ordinal,
            local,
        )?;
        return Some((
            result,
            checked_trees::CheckedCallScalarArgument::Pure(value),
        ));
    };
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    if roots.next().is_some()
        || root.machine != machine
        || local.is_mutable
        || !computations.nodes.is_valid(root.root)
        || computations.nodes.get(root.root).authored_root != local.initial_value
        || computations.nodes.get(root.root).primitive_type != primitive_type
        || facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, statement_index, role)
            .is_some()
    {
        return None;
    }
    Some((
        CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        },
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    ))
}

fn append_call_cleanup(
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    statement_index: u32,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
) -> Option<()> {
    let Some(CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        structural_arguments,
        ..
    }) = operations.last()
    else {
        return Some(());
    };
    let coordinate = *coordinate;
    let mut discards = Vec::new();
    for (result, root) in results.iter().rev() {
        if !matches!(root, facts::PlaceRoot::Expression(_))
            || result.statement_index != statement_index
            || !structural_arguments.iter().any(|argument| {
                argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                    && argument.access == CheckedStructuralAccess::SharedBorrow
            })
        {
            continue;
        }
        discards.push(checked_trees::CheckedUnitPartialAffineDiscardPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: result.type_identity.clone(),
        });
    }
    if discards.is_empty() {
        return Some(());
    }
    for discard in &discards {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } = discard.source
        else {
            return None;
        };
        let producer = operations.iter_mut().find(|operation| {
            matches!(operation,
            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                if result.binding_ordinal == binding_ordinal)
        })?;
        let (CheckedUnitEffectOperationPlan::StructuralCall {
            discard_result_on_return,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            discard_result_on_return,
            ..
        }) = producer
        else {
            return None;
        };
        if !*discard_result_on_return {
            return None;
        }
        *discard_result_on_return = false;
    }
    operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards: discards,
    });
    Some(())
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
    | CheckedUnitEffectOperationPlan::ScalarCall {
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
        for (binding_ordinal, access, projected) in
            structural_arguments.iter().filter_map(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .map(|ordinal| (ordinal, argument.access, !argument.path.is_empty()))
            })
        {
            let mut producers = operations.iter_mut().filter(|operation| {
                matches!(operation,
                CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                    if result.binding_ordinal == binding_ordinal)
            });
            let producer = producers.next()?;
            if producers.next().is_some() {
                return None;
            }
            // Unrestricted whole-value arguments copy their payload. They retain
            // a live producer without acquiring an affine disposal obligation.
            if matches!(producer,
                CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
                | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                    if result.multiplicity == Multiplicity::Unrestricted)
            {
                if access != CheckedStructuralAccess::Owned || projected {
                    return None;
                }
                continue;
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
                return None;
            };
            if result.multiplicity != Multiplicity::Affine || !*discard_result_on_return {
                return None;
            }
            match access {
                // A projected transfer leaves its root owner alive until the
                // exact complement is committed on the consumer continuation.
                CheckedStructuralAccess::Owned if projected => {}
                CheckedStructuralAccess::Owned => *discard_result_on_return = false,
                CheckedStructuralAccess::SharedBorrow => {}
                CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow => return None,
            }
        }
    }
    Some(())
}
