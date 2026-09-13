//! Sequence scalar bindings, writes, calls and structural locals at their exact
//! authored statements. Each effect completes before the following binding;
//! backward continuation assembly never publishes an initializer prematurely.

use super::*;

pub(super) struct Prepared {
    pub(super) value_types: Vec<QualifiedScalarType>,
    pub(super) scalar_bindings: storage::ScalarBindings,
    parameter_types: Vec<QualifiedScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    prefixes: Vec<PendingStep>,
}

enum PendingStep {
    Value(PendingComputation),
    Effect(PendingEffect),
}

struct PendingEffect {
    parameter_types: Vec<QualifiedScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    value_types: Vec<QualifiedScalarType>,
    scalar_bindings: storage::ScalarBindings,
    prepared: PreparedOperation,
}

enum PreparedOperation {
    Unit(unit_operations::Prepared),
    Structural(structural_values::Prepared),
}

struct PendingComputation {
    parameter_types: Vec<QualifiedScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    value_types: Vec<QualifiedScalarType>,
    scalar_bindings: storage::ScalarBindings,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
    destination: symbols::SymbolHandle,
    result_type: QualifiedScalarType,
    value: PendingValue,
    store: Option<primitive_locals::StoreDestination>,
}

enum PendingValue {
    Computation,
    Expression(LoweredDirectExpression),
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedScalarStateGraph,
    parameter_types: Vec<QualifiedScalarType>,
    structural_parameters: &[StructuralParameterDeclaration],
    primitive_locals: &[primitive_locals::PrimitiveLocal],
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Prepared, LoweringError> {
    let (source_machine, source_state) = source_custody::authored_state(checked, state.state)?;
    let authored_prefix = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                checked_trees::statement::StatementNode::LocalData(_)
                    | checked_trees::statement::StatementNode::Assignment(_)
                    | checked_trees::statement::StatementNode::Call(_)
            )
        })
        .count();
    let terminator_ordinal = match &state.terminator {
        CheckedScalarStateTerminator::Return { statement_ordinal }
        | CheckedScalarStateTerminator::Crash { statement_ordinal } => *statement_ordinal,
        CheckedScalarStateTerminator::Jump(successor) => successor.statement_ordinal,
        CheckedScalarStateTerminator::Conditional {
            guard_statement_ordinal,
            ..
        } => *guard_statement_ordinal,
    };
    if source_machine.symbol != machine
        || state.bindings.len() + state.unit_operations.len() != authored_prefix
        || usize::try_from(terminator_ordinal).ok() != Some(authored_prefix)
    {
        return unsupported("scalar graph lost its complete authored binding prefix");
    }
    let statements = checked
        .statement_table
        .statements(source_state.statement_nodes);
    let mut binding_rows = state.bindings.iter();
    let mut unit_rows = state.unit_operations.iter();
    for (ordinal, statement) in statements[..authored_prefix].iter().enumerate() {
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| LoweringError::Unsupported("scalar statement ordinal exceeds u32"))?;
        match statement {
            checked_trees::statement::StatementNode::Call(_) => {
                let operation = unit_rows.next().ok_or(LoweringError::Unsupported(
                    "scalar graph lost an authored Unit call",
                ))?;
                let coordinate = unit_operations::coordinate(operation)?;
                if coordinate.statement_index != ordinal || coordinate.call_ordinal != 0 {
                    return unsupported(
                        "scalar graph Unit operation moved from its authored statement",
                    );
                }
            }
            checked_trees::statement::StatementNode::LocalData(local)
                if checked
                    .primitive_type_reference(local.type_reference)
                    .is_none() =>
            {
                let operation = unit_rows.next().ok_or(LoweringError::Unsupported(
                    "scalar graph lost an authored structural local",
                ))?;
                if !matches!(operation, checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } if result.statement_index == ordinal)
                {
                    return unsupported(
                        "scalar graph structural local moved from its authored statement",
                    );
                }
            }
            _ => {
                if binding_rows
                    .next()
                    .is_none_or(|binding| binding.statement_ordinal != ordinal)
                {
                    return unsupported("scalar graph binding moved from its authored statement");
                }
            }
        }
    }
    if binding_rows.next().is_some() || unit_rows.next().is_some() {
        return unsupported("scalar graph duplicates its authored statement roster");
    }
    let mut parameter_types = parameter_types;
    let mut prefixes = Vec::new();
    let mut value_types = parameter_types.clone();
    let structural_namespace = state
        .structural_parameters
        .iter()
        .zip(structural_parameters)
        .map(|(source, emitted)| (source.position, emitted.clone()))
        .collect::<Vec<_>>();
    let mut scalar_bindings = storage::ScalarBindings::new(parameter_types.len())
        .with_structural_parameters(&structural_namespace)
        .with_structural_observations(structural_types);
    for parameter in source_custody::parameter_storage(checked, machine, state)? {
        scalar_bindings.initialize_parameter(
            parameter.symbol,
            terminal_scalar_type(parameter.primitive_type)?,
            state
                .scalar_parameters
                .iter()
                .position(|scalar| scalar.source_position == parameter.parameter_ordinal)
                .ok_or(LoweringError::Unsupported(
                    "scalar parameter ordinal is absent from the entry namespace",
                ))?,
        )?;
    }
    let mut immutable_ordinal = 0u32;
    let mut bindings = Vec::with_capacity(state.bindings.len());
    let mut units = state.unit_operations.iter().peekable();
    for binding in &state.bindings {
        while units.peek().is_some_and(|operation| {
            unit_operations::statement_index(operation)
                .is_ok_and(|statement| statement < binding.statement_ordinal)
        }) {
            let operation = units.next().expect("peeked Unit operation exists");
            let statement = unit_operations::statement_index(operation)?;
            scalar_bindings = scalar_bindings.with_primitive_storage(
                &primitive_locals::storage_before(primitive_locals, statement),
            );
            let source_bindings = scalar_bindings.clone();
            let prepared = prepare_operation(
                checked,
                qualifications,
                machine,
                state,
                operation,
                &mut scalar_bindings,
                &value_types,
                structural_types,
                next_place,
            )?;
            prefixes.push(PendingStep::Effect(PendingEffect {
                parameter_types,
                bindings: std::mem::take(&mut bindings),
                value_types: value_types.clone(),
                scalar_bindings: source_bindings,
                prepared,
            }));
            parameter_types = value_types.clone();
        }
        use checked_trees::CheckedScalarBindingDestination;
        scalar_bindings = scalar_bindings.with_primitive_storage(
            &primitive_locals::storage_before(primitive_locals, binding.statement_ordinal),
        );
        let binding_ordinal = immutable_ordinal;
        let role = match binding.destination {
            CheckedScalarBindingDestination::Immutable => {
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
            }
            CheckedScalarBindingDestination::StorageInitialize { .. } => {
                CheckedScalarExpressionRole::StorageInitializer
            }
            CheckedScalarBindingDestination::StorageAssign { .. } => {
                CheckedScalarExpressionRole::AssignmentValue
            }
        };
        if let CheckedScalarBindingDestination::StorageInitialize { symbol }
        | CheckedScalarBindingDestination::StorageAssign { symbol } = binding.destination
            && !matches!(binding.value, CheckedScalarBindingValue::Computation)
        {
            let mut custody = checked
                .facts
                .values
                .scalar_expressions
                .source_bindings
                .iter()
                .filter(|(_, source)| {
                    source.state == state.state
                        && source.statement_ordinal == binding.statement_ordinal
                        && source.role == role
                });
            let Some((_, source)) = custody.next() else {
                return unsupported("scalar storage destination lost its checked source custody");
            };
            if custody.next().is_some()
                || source.destination != symbol
                || !checked
                    .typed
                    .expression_table
                    .expression_is_valid(source.expression)
            {
                return unsupported(
                    "scalar storage destination disagrees with its checked source custody",
                );
            }
        }
        let mut prepared_expression =
            if matches!(binding.value, CheckedScalarBindingValue::Expression) {
                Some(scalar_bindings.expression_at(
                    checked,
                    state.state,
                    binding.statement_ordinal,
                    role,
                )?)
            } else {
                None
            };
        let binding_type = match &binding.value {
            CheckedScalarBindingValue::Computation => {
                let root = checked
                    .facts
                    .values
                    .scalar_computations
                    .root_at(state.state, binding.statement_ordinal, role)
                    .ok_or(LoweringError::Unsupported(
                        "scalar binding has no unique computation root",
                    ))?;
                if root.machine != machine {
                    return unsupported("scalar binding computation belongs to another machine");
                }
                computations::computation_value_type(
                    checked,
                    qualifications,
                    root.root,
                    &scalar_bindings,
                    &value_types,
                )?
            }
            CheckedScalarBindingValue::DirectCall { target_state, .. } => {
                qualifications.scalar_state_types(checked, *target_state)?.1
            }
            CheckedScalarBindingValue::Expression => prepared_expression
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "scalar binding lost its prepared expression",
                ))?
                .value_type(&value_types)?,
        };
        if binding_type.scalar_type != terminal_scalar_type(binding.primitive_type)? {
            return unsupported("scalar binding carrier disagrees with its retained value");
        }
        if let Some(checked_trees::statement::StatementNode::LocalData(local)) =
            statements.get(binding.statement_ordinal as usize)
            && !local.type_is_inferred
            && qualifications.value_type(checked, local.type_reference)? != binding_type
        {
            return unsupported("scalar binding does not retain its authored qualification");
        }
        let store = primitive_locals::destination(primitive_locals, binding)?;
        if store.is_some() && !binding_type.qualifications.is_empty() {
            return unsupported("qualified primitive storage requires qualified storage custody");
        }
        let inline_call = if matches!(binding.value, CheckedScalarBindingValue::Computation)
            && binding.destination == CheckedScalarBindingDestination::Immutable
        {
            computations::lower_inline_call(
                checked,
                qualifications,
                machine,
                state.state,
                binding.statement_ordinal,
                role,
                &scalar_bindings,
                &value_types,
                binding_type,
            )?
        } else {
            None
        };
        if matches!(binding.value, CheckedScalarBindingValue::Computation) && inline_call.is_none()
        {
            prefixes.push(PendingStep::Value(PendingComputation {
                parameter_types,
                bindings: std::mem::take(&mut bindings),
                value_types: value_types.clone(),
                scalar_bindings: scalar_bindings.clone(),
                statement_ordinal: binding.statement_ordinal,
                role,
                destination: match binding.destination {
                    CheckedScalarBindingDestination::StorageInitialize { symbol }
                    | CheckedScalarBindingDestination::StorageAssign { symbol } => symbol,
                    CheckedScalarBindingDestination::Immutable => symbols::SymbolHandle::default(),
                },
                result_type: binding_type,
                value: PendingValue::Computation,
                store,
            }));
            parameter_types = value_types.clone();
            parameter_types.push(binding_type);
        }
        let mut lowered = match &binding.value {
            CheckedScalarBindingValue::Computation => {
                inline_call.map(LoweredScalarBinding::DirectCall)
            }
            CheckedScalarBindingValue::Expression => {
                let expression = prepared_expression
                    .take()
                    .ok_or(LoweringError::Unsupported(
                        "scalar binding lost its prepared expression",
                    ))?;
                if expression.value_type(&value_types)? != binding_type {
                    return unsupported(
                        "checked scalar computed value type must match its binding",
                    );
                }
                validate_direct_parameter_types(&expression, &scalar_carriers(&value_types))?;
                Some(LoweredScalarBinding::Expression(expression))
            }
            CheckedScalarBindingValue::DirectCall {
                target_machine,
                target_state,
                call_ordinal,
                argument_count,
            } => {
                if binding.destination != CheckedScalarBindingDestination::Immutable {
                    return unsupported("scalar storage computations do not admit direct calls");
                }
                Some(LoweredScalarBinding::DirectCall(
                    lower_checked_direct_call_binding(
                        checked,
                        qualifications,
                        machine,
                        state.state,
                        binding.statement_ordinal,
                        binding_ordinal,
                        *target_machine,
                        *target_state,
                        *call_ordinal,
                        *argument_count,
                        binding_type,
                        &value_types,
                        &scalar_bindings,
                    )?,
                ))
            }
        };
        if let Some(store) = store
            && let Some(LoweredScalarBinding::Expression(expression)) = lowered.take()
        {
            prefixes.push(PendingStep::Value(PendingComputation {
                parameter_types,
                bindings: std::mem::take(&mut bindings),
                value_types: value_types.clone(),
                scalar_bindings: scalar_bindings.clone(),
                statement_ordinal: binding.statement_ordinal,
                role,
                destination: match binding.destination {
                    CheckedScalarBindingDestination::StorageInitialize { symbol }
                    | CheckedScalarBindingDestination::StorageAssign { symbol } => symbol,
                    CheckedScalarBindingDestination::Immutable => {
                        return unsupported("primitive store lost its destination");
                    }
                },
                result_type: binding_type,
                value: PendingValue::Expression(expression),
                store: Some(store),
            }));
            parameter_types = value_types.clone();
            parameter_types.push(binding_type);
        }
        scalar_bindings.append(
            binding.destination,
            binding_type.scalar_type,
            value_types.len(),
        )?;
        if binding.destination == CheckedScalarBindingDestination::Immutable {
            immutable_ordinal =
                immutable_ordinal
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "scalar immutable local count exceeds u32",
                    ))?;
        }
        if let Some(lowered) = lowered {
            bindings.push(lowered);
        }
        value_types.push(binding_type);
    }
    for operation in units {
        let statement = unit_operations::statement_index(operation)?;
        scalar_bindings = scalar_bindings.with_primitive_storage(
            &primitive_locals::storage_before(primitive_locals, statement),
        );
        let source_bindings = scalar_bindings.clone();
        let prepared = prepare_operation(
            checked,
            qualifications,
            machine,
            state,
            operation,
            &mut scalar_bindings,
            &value_types,
            structural_types,
            next_place,
        )?;
        prefixes.push(PendingStep::Effect(PendingEffect {
            parameter_types,
            bindings: std::mem::take(&mut bindings),
            value_types: value_types.clone(),
            scalar_bindings: source_bindings,
            prepared,
        }));
        parameter_types = value_types.clone();
    }
    scalar_bindings = scalar_bindings.with_primitive_storage(&primitive_locals::storage_before(
        primitive_locals,
        terminator_ordinal,
    ));
    Ok(Prepared {
        value_types,
        scalar_bindings,
        parameter_types,
        bindings,
        prefixes,
    })
}

fn prepare_operation(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedScalarStateGraph,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
    bindings: &mut storage::ScalarBindings,
    value_types: &[QualifiedScalarType],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<PreparedOperation, LoweringError> {
    if matches!(
        operation,
        checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
    ) {
        let prepared = structural_values::prepare(
            checked,
            qualifications,
            machine,
            state.state,
            operation,
            bindings,
            value_types,
            types,
            next_place,
        )?;
        bindings.establish_structural_local(prepared.symbol, prepared.place)?;
        Ok(PreparedOperation::Structural(prepared))
    } else {
        Ok(PreparedOperation::Unit(unit_operations::prepare(
            checked,
            machine,
            state,
            operation,
            bindings,
            value_types,
        )?))
    }
}

impl Prepared {
    pub(super) fn finish(
        self,
        state: symbols::SymbolHandle,
        terminator: LoweredScalarBranchTerminator,
        computations: &mut computations::Expansion<'_>,
    ) -> Result<LoweredScalarBranchState, LoweringError> {
        let mut continuation = LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: self.parameter_types,
            bindings: self.bindings,
            terminator,
        };
        // Construct backward so each computation sends the completed prefix and
        // its result directly to the following statements, without placeholders.
        for step in self.prefixes.into_iter().rev() {
            let prefix = match step {
                PendingStep::Value(prefix) => prefix,
                PendingStep::Effect(prefix) => {
                    let target = computations.push(continuation);
                    let prepared = match prefix.prepared {
                        PreparedOperation::Unit(prepared) => prepared,
                        PreparedOperation::Structural(prepared) => {
                            let target = prepared.finish(
                                state,
                                &prefix.scalar_bindings,
                                &prefix.value_types,
                                target,
                                computations,
                            )?;
                            continuation = LoweredScalarBranchState {
                                structural_parameters: Vec::new(),
                                parameter_types: prefix.parameter_types,
                                bindings: prefix.bindings,
                                structural_effects: Vec::new(),
                                terminator: LoweredScalarBranchTerminator::Jump {
                                    target,
                                    arguments: computations::parameters(&prefix.value_types),
                                    structural_arguments: Vec::new(),
                                    trivial_affine_discards: Vec::new(),
                                },
                            };
                            continue;
                        }
                    };
                    let mut argument_types = prefix.value_types.clone();
                    argument_types.extend_from_slice(&prepared.argument_types);
                    let call_block = computations.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        parameter_types: argument_types,
                        bindings: Vec::new(),
                        structural_effects: vec![LoweredScalarEffect::CallUnit(prepared.call)],
                        terminator: LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            target,
                            arguments: computations::parameters(&prefix.value_types),
                            structural_arguments: Vec::new(),
                        },
                    });
                    let target = computations.call_arguments(
                        state,
                        prepared.coordinate,
                        false,
                        &prepared.arguments,
                        0,
                        &prefix.scalar_bindings,
                        &prefix.value_types,
                        call_block,
                    )?;
                    continuation = LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        parameter_types: prefix.parameter_types,
                        bindings: prefix.bindings,
                        structural_effects: Vec::new(),
                        terminator: LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            target,
                            arguments: computations::parameters(&prefix.value_types),
                            structural_arguments: Vec::new(),
                        },
                    };
                    continue;
                }
            };
            let mut target = computations.push(continuation);
            if let Some(destination) = prefix.store {
                let mut completed_types = prefix.value_types.clone();
                completed_types.push(prefix.result_type);
                target = computations.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    structural_effects: Vec::new(),
                    parameter_types: completed_types.clone(),
                    bindings: vec![LoweredScalarBinding::StoredValue {
                        value: LoweredDirectExpression::Parameter {
                            position: prefix.value_types.len(),
                            scalar_type: prefix.result_type.scalar_type,
                        },
                        destination,
                    }],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        trivial_affine_discards: Vec::new(),
                        structural_arguments: Vec::new(),
                        target,
                        arguments: computations::parameters(&completed_types),
                    },
                });
            }
            let target = match prefix.value {
                PendingValue::Computation => computations.retained_value(
                    state,
                    prefix.statement_ordinal,
                    prefix.role,
                    prefix.destination,
                    &prefix.scalar_bindings,
                    &prefix.value_types,
                    prefix.result_type,
                    target,
                )?,
                PendingValue::Expression(expression) => {
                    let mut completed_types = prefix.value_types.clone();
                    completed_types.push(prefix.result_type);
                    computations.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        structural_effects: Vec::new(),
                        parameter_types: prefix.value_types.clone(),
                        bindings: vec![LoweredScalarBinding::Expression(expression)],
                        terminator: LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            structural_arguments: Vec::new(),
                            target,
                            arguments: computations::parameters(&completed_types),
                        },
                    })
                }
            };
            continuation = LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                structural_effects: Vec::new(),
                parameter_types: prefix.parameter_types,
                bindings: prefix.bindings,
                terminator: LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    structural_arguments: Vec::new(),
                    target,
                    arguments: computations::parameters(&prefix.value_types),
                },
            };
        }
        Ok(continuation)
    }
}
