//! Scalar operand evaluation within an existing machine's structural frontier.

use super::*;
use checked_trees::CheckedCallScalarArgument;

mod scalar_control;
mod source_values;

fn prepare_shared_qualifications(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    values: &[ValueDeclaration],
) -> Result<crate::scalar_qualifications::PreparedScalarQualifications, LoweringError> {
    let qualifications =
        crate::scalar_qualifications::PreparedScalarQualifications::prepare(checked, &[machine])?;
    if !qualifications.catalog().domains.is_empty()
        || values.iter().any(|value| !value.qualifications.is_empty())
    {
        return unsupported(
            "attached scalar qualifications require the enclosing catalog namespace",
        );
    }
    Ok(qualifications)
}

pub(crate) struct Evaluation {
    pub(crate) structural_value_owners: Vec<StructuralValueOwner>,
    pub(crate) selection_cleanups: Vec<SelectionCleanup>,
    pub(crate) structural_locals: Vec<(symbols::SymbolHandle, StructuralArgument)>,
    pub(crate) local_cases: Vec<crate::scalar_bindings::structural_cases::LocalCaseBinding>,
    pub(crate) arrays: Vec<crate::scalar_computations::arrays::Slot>,
    pub(crate) cases: Vec<crate::scalar_computations::cases::Slot>,
    pub(crate) record_fields: Vec<crate::scalar_computations::fields::Binding>,
    pub primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
    /// State-local storage has its own namespace; it is not an immutable slot.
    /// Other callers retain the ordinary dense source-prefix mapping.
    pub scalar_bindings: Option<crate::scalar_bindings::ScalarBindings>,
    pub structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
    pub structural_fields: Vec<crate::scalar_bindings::StructuralScalarFieldBinding>,
    pub structural_cases: Vec<crate::scalar_bindings::structural_cases::StructuralCaseBinding>,
    pub entry: BlockId,
    pub current: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    /// Parameters established at the current private join, distinct from the
    /// authored structural namespace used to resolve source operands.
    pub block_structural_parameters: Vec<StructuralParameterDeclaration>,
    pub operation_start: usize,
    pub blocks: Vec<Block>,
}

/// Replace candidate roots only at their normal death edge. Both rosters are
/// in reverse establishment order; physical source declarations stay intact.
pub(crate) struct SelectionCleanup {
    pub(crate) selected: PlaceId,
    pub(crate) sources: Vec<PlaceId>,
    pub(crate) remaining: Vec<PlaceId>,
    pub(crate) pass_through: Vec<(PlaceId, PlaceId)>,
    pub(crate) next_operation: u64,
}

/// Current physical owners in establishment order. An anonymous residual keeps
/// its receipt correspondence in SelectionCleanup, never a fabricated symbol.
#[derive(Clone)]
pub(crate) struct StructuralValueOwner {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) statement: u32,
    pub(crate) value: terminal_psi::StructuralOperationResult,
}

impl Evaluation {
    /// Publish one completed structural result into the state's existing value
    /// and local namespaces. Calls and constructors share the original home;
    /// registration neither copies payload fields nor changes cleanup custody.
    pub(crate) fn establish_structural_result(
        &mut self,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        produced: terminal_psi::StructuralOperationResult,
        structural_types: &[StructuralTypeDeclaration],
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let multiplicity = match result.multiplicity {
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            _ => return unsupported("structural result has unsupported local custody"),
        };
        let mut types = structural_types
            .iter()
            .filter(|declaration| declaration.id == produced.structural_type);
        let declaration = types.next().ok_or(LoweringError::Unsupported(
            "structural result type was not declared",
        ))?;
        if types.next().is_some()
            || declaration.identity != result.type_identity
            || produced.multiplicity != multiplicity
            || !produced.qualifications.is_empty()
            || !produced.projected_qualifications.is_empty()
            || !produced.claims.is_empty()
        {
            return unsupported("structural result registration changed its exact custody");
        }
        if operations
            .structural_values
            .iter()
            .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
        {
            return unsupported("structural value binding was established twice");
        }
        let (_, source) = crate::scalar_source_custody::authored_state(checked, state)?;
        let local = match checked
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize)
        {
            Some(checked_trees::statement::StatementNode::LocalData(local)) => Some(local),
            Some(_) => None,
            None => return unsupported("structural result lost its source statement"),
        };
        let mut local_symbol = symbols::SymbolHandle::invalid();
        if let Some(local) = local {
            if !local.symbol.is_valid()
                || !checked
                    .expression_table
                    .expression_is_valid(local.initial_value)
                || checked
                    .normalized_type_identity(local.type_reference)
                    .as_str()
                    != result.type_identity
                || checked.type_multiplicity(local.type_reference) != result.multiplicity
                || self
                    .structural_locals
                    .iter()
                    .any(|(symbol, _)| *symbol == local.symbol)
            {
                return unsupported("structural value repeats or substitutes its local binding");
            }
            local_symbol = local.symbol;
            if matches!(declaration.shape, StructuralTypeShape::Sum { .. }) {
                self.local_cases.push(
                    crate::scalar_bindings::structural_cases::LocalCaseBinding::new(
                        checked,
                        local.symbol,
                        local.type_reference,
                        produced.place,
                        structural_types,
                    )?,
                );
            }
            self.structural_locals.push((
                local.symbol,
                StructuralArgument {
                    place: produced.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            ));
        }
        if multiplicity == StructuralMultiplicity::Affine {
            self.structural_value_owners.push(StructuralValueOwner {
                symbol: local_symbol,
                statement: result.statement_index,
                value: produced.clone(),
            });
        }
        operations
            .structural_values
            .push((result.binding_ordinal, produced));
        Ok(())
    }

    /// Call operand resolvers retain authored result identities. Apply only
    /// transports that precede the operation; earlier calls keep their places.
    pub(crate) fn remap_transported_call_operands(&mut self, operations: &mut OperationBuffer) {
        let cleanups = &self.selection_cleanups;
        let remap = |operation: &mut Operation| {
            let arguments = match &mut operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructural {
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    structural_arguments,
                    ..
                }
                | OperationKind::BoundaryCall {
                    structural_arguments,
                    ..
                } => structural_arguments,
                _ => return,
            };
            for cleanup in cleanups {
                if operation.id.get() < cleanup.next_operation {
                    continue;
                }
                for argument in arguments.iter_mut() {
                    if let Some((_, target)) = cleanup
                        .pass_through
                        .iter()
                        .find(|(source, _)| *source == argument.place)
                    {
                        argument.place = *target;
                    }
                }
            }
        };
        for operation in &mut operations.operations {
            remap(operation);
        }
        for block in &mut self.blocks {
            for operation in &mut block.operations {
                remap(operation);
            }
        }
    }

    pub(crate) fn current_structural_place(&self, mut place: PlaceId) -> PlaceId {
        for cleanup in &self.selection_cleanups {
            if let Some((_, target)) = cleanup
                .pass_through
                .iter()
                .find(|(source, _)| *source == place)
            {
                place = *target;
            }
        }
        place
    }

    pub(crate) fn selection_return_discards(
        &self,
        mut roots: Vec<(PlaceId, bool)>,
    ) -> Result<Vec<PlaceId>, LoweringError> {
        for cleanup in &self.selection_cleanups {
            let selected = roots
                .iter()
                .position(|(place, _)| *place == cleanup.selected)
                .ok_or(LoweringError::Unsupported(
                    "selected cleanup result is absent from its operation roster",
                ))?;
            let start = roots
                .iter()
                .position(|(place, _)| cleanup.sources.contains(place))
                .ok_or(LoweringError::Unsupported(
                    "owned selection normal cleanup has no source roster",
                ))?;
            let end = roots
                .iter()
                .rposition(|(place, _)| cleanup.sources.contains(place))
                .ok_or(LoweringError::Unsupported(
                    "owned selection cleanup source is absent",
                ))?
                + 1;
            if start <= selected
                || !roots[start..end]
                    .iter()
                    .filter(|(place, _)| cleanup.sources.contains(place))
                    .map(|(place, _)| place)
                    .eq(cleanup.sources.iter())
                || roots[start..end].iter().any(|(place, discard)| {
                    !cleanup.sources.contains(place)
                        && (*discard
                            || cleanup
                                .pass_through
                                .iter()
                                .any(|(source, _)| source == place))
                })
            {
                return unsupported(
                    "interleaved selection sources require a shared mixed-root establishment-order cleanup carrier",
                );
            }
            if roots[start..end]
                .iter()
                .any(|(place, discard)| cleanup.sources.contains(place) && *discard)
            {
                return unsupported("owned selection source retains conflicting return disposal");
            }
            roots.splice(
                start..end,
                cleanup.remaining.iter().map(|place| (*place, true)),
            );
            for (place, _) in &mut roots {
                if let Some((_, target)) = cleanup
                    .pass_through
                    .iter()
                    .find(|(source, _)| source == place)
                {
                    *place = *target;
                }
            }
        }
        Ok(roots
            .into_iter()
            .filter_map(|(place, discard)| discard.then_some(place))
            .collect())
    }

    /// Preserve evaluated scalar bindings while committing only the dying
    /// affine owners on the completed call's normal continuation.
    pub(crate) fn cleanup_continuation(
        &mut self,
        discards: Vec<PlaceId>,
        mut residuals: Vec<terminal_psi::StructuralAffineDiscard>,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &OperationBuffer,
    ) -> Result<(), LoweringError> {
        let discards = discards
            .into_iter()
            .map(|place| self.current_structural_place(place))
            .collect();
        for residual in &mut residuals {
            residual.place = self.current_structural_place(residual.place);
        }
        let types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let parameters = declarations(&types, next_value)?;
        let continuation = block_id(allocate_dense(next_block)?);
        self.blocks.push(Block {
            structural_parameters: std::mem::take(&mut self.block_structural_parameters),
            id: self.current,
            parameters: std::mem::take(&mut self.parameters),
            operations: operations[self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: edge_id(allocate_dense(next_edge)?),
                target: continuation,
                arguments: values.iter().map(|value| value.id).collect(),
                residual_affine_discards: residuals,
                trivial_affine_discards: discards,
            },
        });
        *values = parameters.clone();
        self.parameters = parameters;
        self.current = continuation;
        self.operation_start = operations.len();
        Ok(())
    }

    pub(crate) fn new(next_block: &mut u64) -> Result<Self, LoweringError> {
        let entry = block_id(allocate_dense(next_block)?);
        Ok(Self {
            structural_value_owners: Vec::new(),
            selection_cleanups: Vec::new(),
            structural_locals: Vec::new(),
            local_cases: Vec::new(),
            arrays: Vec::new(),
            cases: Vec::new(),
            record_fields: Vec::new(),
            primitive_storage: Vec::new(),
            scalar_bindings: None,
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
            structural_parameters: Vec::new(),
            entry,
            current: entry,
            parameters: Vec::new(),
            block_structural_parameters: Vec::new(),
            operation_start: 0,
            blocks: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn arguments(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
        self.arguments_slice(
            checked,
            machine,
            state,
            operation,
            None,
            values.len(),
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )
    }

    /// Complete one dense scalar operand at its original call coordinate.
    /// The caller retains the result in `values` before evaluating another
    /// operand and uses indices to follow values across private blocks. Only
    /// the pre-group source prefix may be read by authored operand expressions;
    /// later retained slots carry private argument temporaries across the CFG.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn argument_at(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        argument_ordinal: usize,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let arguments = self.arguments_slice(
            checked,
            machine,
            state,
            operation,
            Some(argument_ordinal),
            source_value_count,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match arguments.as_deref() {
            Some([argument]) => Ok(*argument),
            _ => unsupported("call has no single completed scalar operand"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn arguments_slice(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &CheckedUnitEffectOperationPlan,
        argument_ordinal: Option<usize>,
        source_value_count: usize,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Option<Vec<ValueDeclaration>>, LoweringError> {
        if source_value_count > values.len() {
            return unsupported("call argument source prefix exceeds its retained values");
        }
        let source_bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| crate::scalar_bindings::ScalarBindings::new(source_value_count))
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases);
        let source_bindings = source_bindings
            .with_primitive_storage(&self.primitive_storage)
            .with_local_cases(&self.local_cases)
            .with_structural_locals(&self.structural_locals);
        let (coordinate, arguments, boundary) = match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                scalar_arguments,
                ..
            } => (*coordinate, scalar_arguments, false),
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                scalar_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                scalar_arguments,
                ..
            } => (*coordinate, scalar_arguments, true),
            _ => return Ok(None),
        };
        let (arguments, argument_ordinal_start) = match argument_ordinal {
            Some(ordinal) => {
                let argument = arguments.get(ordinal).ok_or(LoweringError::Unsupported(
                    "call scalar operand ordinal is outside its argument list",
                ))?;
                (std::slice::from_ref(argument), ordinal)
            }
            None => (arguments.as_slice(), 0),
        };
        let qualifications = prepare_shared_qualifications(checked, machine, values)?;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let argument_types = arguments
            .iter()
            .map(|argument| match argument {
                CheckedCallScalarArgument::Pure(expression) => source_bindings
                    .expression(expression)?
                    .value_type(&source_types),
                CheckedCallScalarArgument::Computation(root) => {
                    let nodes = &checked.facts.values.scalar_computations.nodes;
                    if !nodes.is_valid(*root) {
                        return unsupported("call argument computation has no live root");
                    }
                    crate::scalar_computations::computation_value_type(
                        checked,
                        &qualifications,
                        *root,
                        &source_bindings,
                        &source_types,
                    )
                }
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let needs_control = arguments.iter().any(|argument| match argument {
            CheckedCallScalarArgument::Computation(_) => true,
            CheckedCallScalarArgument::Pure(expression) => source_bindings
                .expression(expression)
                .is_ok_and(|expression| direct_expression_contains_short_circuit(&expression)),
        });
        if !needs_control {
            return arguments
                .iter()
                .zip(&argument_types)
                .map(|(argument, scalar_type)| {
                    let expression = source_bindings.expression(argument.as_pure().ok_or(
                        LoweringError::Unsupported(
                            "computed call argument requires ordered control",
                        ),
                    )?)?;
                    validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
                    Ok(ValueDeclaration {
                        qualifications: scalar_type.qualifications,
                        id: emit_direct_expression(&expression, values, next_value, operations),
                        scalar_type: scalar_type.scalar_type,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
                .map(Some);
        }

        let mut expansion =
            crate::scalar_computations::Expansion::new(checked, &qualifications, machine, 1)
                .with_arrays(&self.arrays)
                .with_cases(&self.cases)
                .with_fields(&self.record_fields);
        let entry_index = expansion.call_arguments(
            state,
            coordinate,
            boundary,
            arguments,
            argument_ordinal_start,
            &source_bindings,
            &source_types,
            0,
        )?;
        let states = expansion.finish();
        self.complete_expansion(
            &states,
            entry_index,
            &argument_types,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )
        .map(Some)
    }

    /// Finish the selected RHS without changing the destination or source slots.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn field_assignment_value(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| crate::scalar_bindings::ScalarBindings::new(values.len()))
            .with_primitive_storage(&self.primitive_storage)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases);
        let qualifications = prepare_shared_qualifications(checked, machine, values)?;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let scalar_type = terminal_scalar_type(store.primitive_type)?;
        crate::structural_scalar_store_source::computation_root(checked, machine, state, store)?;
        if store.value.as_pure().is_some() {
            let expression = bindings.expression_at(
                checked,
                state,
                store.statement_index,
                CheckedScalarExpressionRole::AssignmentValue,
            )?;
            if expression.scalar_type() != scalar_type
                || direct_expression_contains_short_circuit(&expression)
            {
                return unsupported("field store requires a matching branch-free pure value");
            }
            validate_direct_parameter_types(&expression, &scalar_carriers(&source_types))?;
            return Ok(ValueDeclaration {
                qualifications: Default::default(),
                id: emit_direct_expression(&expression, values, next_value, operations),
                scalar_type,
            });
        }
        let mut expansion =
            crate::scalar_computations::Expansion::new(checked, &qualifications, machine, 1)
                .with_arrays(&self.arrays)
                .with_cases(&self.cases)
                .with_fields(&self.record_fields);
        let entry = expansion.retained_value(
            state,
            store.statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
            symbols::SymbolHandle::invalid(),
            &bindings,
            &source_types,
            scalar_type.into(),
            0,
        )?;
        let states = expansion.finish();
        let result = self.complete_expansion(
            &states,
            entry,
            &[scalar_type.into()],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match result.as_slice() {
            [value] => Ok(*value),
            _ => unsupported("field assignment has no single completed RHS"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_expansion(
        &mut self,
        states: &[LoweredScalarBranchState],
        entry_index: usize,
        result_types: &[QualifiedScalarType],
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<Vec<ValueDeclaration>, LoweringError> {
        if states.iter().any(|state| {
            state
                .parameter_types
                .iter()
                .any(|value_type| !value_type.qualifications.is_empty())
                || matches!(
                    state.terminator,
                    LoweredScalarBranchTerminator::Qualify { .. }
                )
        }) || result_types
            .iter()
            .any(|value_type| !value_type.qualifications.is_empty())
        {
            return unsupported(
                "attached scalar expansion requires a shared qualification catalog",
            );
        }
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let mut completion_types = source_types.clone();
        completion_types.extend_from_slice(result_types);
        let completion_parameters = declarations(&completion_types, next_value)?;
        let completion = block_id(allocate_dense(next_block)?);
        let mut targets = vec![completion];
        let mut parameters = vec![completion_parameters.clone()];
        for state in states {
            targets.push(block_id(allocate_dense(next_block)?));
            parameters.push(declarations(&state.parameter_types, next_value)?);
        }
        self.blocks.push(Block {
            structural_parameters: std::mem::take(&mut self.block_structural_parameters),
            id: self.current,
            parameters: std::mem::take(&mut self.parameters),
            operations: operations[self.operation_start..].to_vec(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: edge_id(allocate_dense(next_edge)?),
                target: *targets.get(entry_index).ok_or(LoweringError::Unsupported(
                    "call computation entry is absent",
                ))?,
                arguments: values.iter().map(|value| value.id).collect(),
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        });
        for (index, state) in states.iter().enumerate() {
            emit_state(
                state,
                targets[index + 1],
                &parameters[index + 1],
                &targets,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
                &mut self.blocks,
            )?;
        }
        operations.byte_lengths.clear();
        let result = completion_parameters[source_types.len()..].to_vec();
        *values = completion_parameters[..source_types.len()].to_vec();
        self.current = completion;
        self.parameters = completion_parameters;
        self.operation_start = operations.len();
        Ok(result)
    }
}

fn declarations(
    types: &[QualifiedScalarType],
    next_value: &mut u64,
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    types
        .iter()
        .map(|scalar_type| {
            Ok(ValueDeclaration {
                qualifications: scalar_type.qualifications,
                id: value_id(allocate_dense(next_value)?),
                scalar_type: scalar_type.scalar_type,
            })
        })
        .collect()
}

pub(crate) fn validated_values(
    values: Option<&[ValueDeclaration]>,
    types: &[ScalarType],
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    let values = values.ok_or(LoweringError::Unsupported(
        "call has no completed scalar operands",
    ))?;
    if values.len() != types.len()
        || values.iter().zip(types).any(|(value, scalar_type)| {
            value.scalar_type != *scalar_type || !value.qualifications.is_empty()
        })
    {
        return unsupported("completed call scalar operands disagree with the target signature");
    }
    Ok(values.to_vec())
}

#[allow(clippy::too_many_arguments)]
fn emit_state(
    state: &LoweredScalarBranchState,
    mut block: BlockId,
    parameters: &[ValueDeclaration],
    targets: &[BlockId],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    blocks: &mut Vec<Block>,
) -> Result<(), LoweringError> {
    // Expansion-state order is not dominance order. Only observations made
    // within this state's path may be reused while emitting its decisions.
    operations.byte_lengths.clear();
    let mut values = parameters.to_vec();
    let mut block_parameters = parameters.to_vec();
    let mut operation_start = operations.len();
    for binding in &state.bindings {
        if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean { expression }) =
            binding
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let count = boolean_decision_block_count(&decision);
            let reserved_root = *next_block;
            *next_block = next_block
                .checked_add(u64::try_from(count).map_err(|_| {
                    LoweringError::Unsupported("call Boolean block count exceeds u64")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "call Boolean block identities exhausted",
                ))?;
            let continuation = block_id(allocate_dense(next_block)?);
            let mut types = values
                .iter()
                .map(|value| value.value_type())
                .collect::<Vec<_>>();
            types.push(binding.value_type(&types)?);
            let continuation_parameters = declarations(&types, next_value)?;
            let prefix = operations[operation_start..].to_vec();
            let mut decisions = Vec::new();
            emit_reserved_boolean_tuple_stage_blocks(
                &decision,
                &values,
                block_parameters,
                continuation,
                &values.iter().map(|value| value.id).collect::<Vec<_>>(),
                reserved_root,
                next_value,
                next_edge,
                operations,
                &mut decisions,
            );
            for (index, decision) in decisions.into_iter().enumerate() {
                let mut decision = decision.ok_or(LoweringError::Unsupported(
                    "call Boolean block is incomplete",
                ))?;
                if index == 0 {
                    decision.id = block;
                    decision.operations.splice(0..0, prefix.iter().cloned());
                }
                blocks.push(decision);
            }
            block = continuation;
            values = continuation_parameters.clone();
            block_parameters = continuation_parameters;
            operation_start = operations.len();
        } else {
            let value_type = binding.value_type(
                &values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>(),
            )?;
            let id = emit_scalar_binding(binding, &values, next_value, operations, calls)?;
            values.push(ValueDeclaration {
                qualifications: value_type.qualifications,
                id,
                scalar_type: value_type.scalar_type,
            });
        }
    }
    crate::scalar_graph_effects::emit(
        &state.structural_effects,
        &values,
        next_value,
        operations,
        calls,
    )?;
    let mut arguments =
        |expressions: &[LoweredDirectExpression]| -> Result<Vec<ValueId>, LoweringError> {
            expressions
                .iter()
                .map(|expression| {
                    validate_direct_parameter_types(
                        expression,
                        &values
                            .iter()
                            .map(|value| value.scalar_type)
                            .collect::<Vec<_>>(),
                    )?;
                    if direct_expression_contains_short_circuit(expression) {
                        return unsupported(
                            "call computation transfer retains unexpanded Boolean control",
                        );
                    }
                    Ok(emit_direct_expression(
                        expression, &values, next_value, operations,
                    ))
                })
                .collect()
        };
    let terminator = match &state.terminator {
        LoweredScalarBranchTerminator::Qualify { .. } => {
            return unsupported(
                "attached scalar computation qualification requires a shared catalog namespace",
            );
        }
        LoweredScalarBranchTerminator::Jump {
            target,
            arguments: outgoing,
            structural_arguments,
            trivial_affine_discards,
        } => Terminator::Jump {
            structural_arguments: structural_arguments.clone(),
            edge: edge_id(allocate_dense(next_edge)?),
            target: *targets.get(*target).ok_or(LoweringError::Unsupported(
                "call computation target is absent",
            ))?,
            arguments: arguments(outgoing)?,
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: trivial_affine_discards.clone(),
        },
        LoweredScalarBranchTerminator::Conditional {
            condition,
            when_true_target,
            when_true_arguments,
            when_false_target,
            when_false_arguments,
        } => {
            let when_true_arguments = arguments(when_true_arguments)?;
            let when_false_arguments = arguments(when_false_arguments)?;
            let condition = emit_boolean_expression(condition, &values, next_value, operations);
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_true_target)
                        .ok_or(LoweringError::Unsupported("call true target is absent"))?,
                    arguments: when_true_arguments,
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_false_target)
                        .ok_or(LoweringError::Unsupported("call false target is absent"))?,
                    arguments: when_false_arguments,
                    trivial_affine_discards: Vec::new(),
                },
            }
        }
        _ => {
            return unsupported(
                "call operand computation cannot return from its enclosing machine",
            );
        }
    };
    blocks.push(Block {
        structural_parameters: Vec::new(),
        id: block,
        parameters: block_parameters,
        operations: operations[operation_start..].to_vec(),
        terminator,
    });
    Ok(())
}
