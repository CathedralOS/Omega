//! Prepare scalar bindings and writes, retaining completed values across computations.

use super::*;

pub(super) struct Prepared {
    pub(super) value_types: Vec<ScalarType>,
    pub(super) scalar_bindings: storage::ScalarBindings,
    parameter_types: Vec<ScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    prefixes: Vec<PendingComputation>,
}

struct PendingComputation {
    parameter_types: Vec<ScalarType>,
    bindings: Vec<LoweredScalarBinding>,
    value_types: Vec<ScalarType>,
    scalar_bindings: storage::ScalarBindings,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
    destination: symbols::SymbolHandle,
    result_type: ScalarType,
    value: PendingValue,
    store: Option<primitive_locals::StoreDestination>,
}

enum PendingValue {
    Computation,
    Expression(LoweredDirectExpression),
}

pub(super) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedScalarStateGraph,
    parameter_types: Vec<ScalarType>,
    structural_parameters: &[StructuralParameterDeclaration],
    primitive_locals: &[primitive_locals::PrimitiveLocal],
    structural_types: &[StructuralTypeDeclaration],
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
        || state.bindings.len() != authored_prefix
        || usize::try_from(terminator_ordinal).ok() != Some(authored_prefix)
    {
        return unsupported("scalar graph lost its complete authored binding prefix");
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
        .with_structural_fields(structural_types);
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
    for (binding_index, binding) in state.bindings.iter().enumerate() {
        use checked_trees::CheckedScalarBindingDestination;
        scalar_bindings = scalar_bindings.with_primitive_storage(
            &primitive_locals::storage_before(primitive_locals, binding.statement_ordinal),
        );
        if usize::try_from(binding.statement_ordinal).ok() != Some(binding_index) {
            return unsupported("scalar computations drifted from statement order");
        }
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
        let binding_type = terminal_scalar_type(binding.primitive_type)?;
        let store = primitive_locals::destination(primitive_locals, binding)?;
        if matches!(binding.value, CheckedScalarBindingValue::Computation) {
            prefixes.push(PendingComputation {
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
            });
            parameter_types = value_types.clone();
            parameter_types.push(binding_type);
        }
        let mut lowered = match &binding.value {
            CheckedScalarBindingValue::Computation => None,
            CheckedScalarBindingValue::Expression => {
                let expression = scalar_bindings.expression_at(
                    checked,
                    state.state,
                    binding.statement_ordinal,
                    role,
                )?;
                if expression.scalar_type() != binding_type {
                    return unsupported(
                        "checked scalar computed value type must match its binding",
                    );
                }
                validate_direct_parameter_types(&expression, &value_types)?;
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
            prefixes.push(PendingComputation {
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
            });
            parameter_types = value_types.clone();
            parameter_types.push(binding_type);
        }
        scalar_bindings.append(binding.destination, binding_type, value_types.len())?;
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
    scalar_bindings = scalar_bindings.with_primitive_storage(&primitive_locals::storage_before(
        primitive_locals,
        u32::try_from(state.bindings.len())
            .map_err(|_| LoweringError::Unsupported("scalar statement count exceeds u32"))?,
    ));
    Ok(Prepared {
        value_types,
        scalar_bindings,
        parameter_types,
        bindings,
        prefixes,
    })
}

impl Prepared {
    pub(super) fn finish(
        self,
        state: symbols::SymbolHandle,
        terminator: LoweredScalarBranchTerminator,
        computations: &mut computations::Expansion<'_>,
    ) -> Result<LoweredScalarBranchState, LoweringError> {
        let mut continuation = LoweredScalarBranchState {
            parameter_types: self.parameter_types,
            bindings: self.bindings,
            terminator,
        };
        // Construct backward so each computation sends the completed prefix and
        // its result directly to the following statements, without placeholders.
        for prefix in self.prefixes.into_iter().rev() {
            let mut target = computations.push(continuation);
            if let Some(destination) = prefix.store {
                let mut completed_types = prefix.value_types.clone();
                completed_types.push(prefix.result_type);
                target = computations.push(LoweredScalarBranchState {
                    parameter_types: completed_types.clone(),
                    bindings: vec![LoweredScalarBinding::StoredValue {
                        value: LoweredDirectExpression::Parameter {
                            position: prefix.value_types.len(),
                            scalar_type: prefix.result_type,
                        },
                        destination,
                    }],
                    terminator: LoweredScalarBranchTerminator::Jump {
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
                        parameter_types: prefix.value_types.clone(),
                        bindings: vec![LoweredScalarBinding::Expression(expression)],
                        terminator: LoweredScalarBranchTerminator::Jump {
                            structural_arguments: Vec::new(),
                            target,
                            arguments: computations::parameters(&completed_types),
                        },
                    })
                }
            };
            continuation = LoweredScalarBranchState {
                parameter_types: prefix.parameter_types,
                bindings: prefix.bindings,
                terminator: LoweredScalarBranchTerminator::Jump {
                    structural_arguments: Vec::new(),
                    target,
                    arguments: computations::parameters(&prefix.value_types),
                },
            };
        }
        Ok(continuation)
    }
}
