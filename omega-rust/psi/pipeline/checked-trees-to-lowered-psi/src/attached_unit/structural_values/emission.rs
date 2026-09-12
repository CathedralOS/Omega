use super::*;
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    structural_types: &[StructuralTypeDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    temporary_places: &mut Vec<StructuralPlaceDeclaration>,
    calls: &mut CallEmissionContext<'_>,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    source_custody::validate(checked, machine, state, operation)?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } = operation
    else {
        return unsupported("structural value producer missing");
    };
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    let multiplicity = match result.multiplicity {
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Linear => {
            return unsupported("fresh structural value cannot create linear custody");
        }
    };
    let mut emission = Emission {
        checked,
        machine,
        state,
        statement: result.statement_index,
        structural_type,
        multiplicity,
        structural_types,
        next_place,
        temporary_places,
        calls,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
    };
    let place = emission.value(*value)?;
    // Private arm producers are not authored result ordinals. Publish exactly
    // one completed place for this binding, whether a direct producer or join.
    let declaration = emission
        .temporary_places
        .iter()
        .position(|item| item.id == place)
        .ok_or(LoweringError::Unsupported(
            "structural value place was not declared",
        ))?;
    let declaration = emission.temporary_places.remove(declaration);
    if emission
        .operations
        .structural_values
        .iter()
        .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
    {
        return unsupported("structural value binding was established twice");
    }
    emission.operations.structural_values.push((
        result.binding_ordinal,
        terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
    ));
    if let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(
            crate::scalar_source_custody::authored_state(checked, state)?
                .1
                .statement_nodes,
        )
        .get(result.statement_index as usize)
    {
        if emission
            .evaluation
            .structural_locals
            .iter()
            .any(|(symbol, _)| *symbol == local.symbol)
        {
            return unsupported("structural value repeats its local binding");
        }
        if structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(declaration.shape, StructuralTypeShape::Sum { .. })
        }) {
            let cases = crate::scalar_bindings::structural_cases::LocalCaseBinding::new(
                checked,
                local.symbol,
                local.type_reference,
                place,
                structural_types,
            )?;
            emission.evaluation.local_cases.push(cases);
        }
        emission.evaluation.structural_locals.push((
            local.symbol,
            StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        ));
    }
    Ok(declaration)
}

struct Emission<'a, 'b, 'calls> {
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    structural_types: &'a [StructuralTypeDeclaration],
    next_place: &'b mut u64,
    temporary_places: &'b mut Vec<StructuralPlaceDeclaration>,
    calls: &'b mut CallEmissionContext<'calls>,
    evaluation: &'b mut argument_evaluation::Evaluation,
    values: &'b mut Vec<ValueDeclaration>,
    next_value: &'b mut u64,
    next_block: &'b mut u64,
    next_edge: &'b mut u64,
    operations: &'b mut OperationBuffer,
}

impl Emission<'_, '_, '_> {
    fn value(&mut self, value: CheckedStructuralValueHandle) -> Result<PlaceId, LoweringError> {
        let node = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(value)
            .clone();
        match node.kind {
            CheckedStructuralValueKind::Record { .. } => {
                let source_count = self.values.len();
                let declaration = super::emit_record(
                    self.checked,
                    self.machine,
                    self.state,
                    self.statement,
                    value,
                    self.structural_type,
                    self.multiplicity,
                    self.structural_types,
                    self.evaluation,
                    self.values,
                    source_count,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.next_place,
                    self.operations,
                    self.calls,
                )?;
                let place = declaration.id;
                self.temporary_places.push(declaration);
                Ok(place)
            }
            CheckedStructuralValueKind::Case(construction) => {
                let source = validation::scalar_case_constructor(
                    &self.checked.typed,
                    construction.expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "structural case lost its authored constructor",
                ))?;
                let place = place_id(allocate_dense(self.next_place)?);
                let slot = crate::scalar_computations::cases::reserve(
                    self.checked,
                    construction.expression,
                    &source,
                    self.structural_types,
                    place,
                )?;
                let fields =
                    crate::scalar_computations::cases::fields(self.checked, &construction)?
                        .to_vec();
                let field_start = self.values.len();
                for (ordinal, field) in fields.iter().enumerate() {
                    let field_ordinal = u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("structural field ordinal overflow")
                    })?;
                    let value = self.scalar(
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression: construction.expression,
                            field_ordinal,
                        },
                        field.value,
                        field_start,
                    )?;
                    self.values.push(value);
                }
                let completed = slot.construction(&self.values[field_start..])?;
                crate::scalar_computations::cases::emit(
                    &completed,
                    &self.values[field_start..],
                    self.next_value,
                    self.operations,
                )?;
                let declaration =
                    crate::scalar_computations::cases::declarations(&self.operations.operations)
                        .find(|declaration| declaration.id == place)
                        .ok_or(LoweringError::Unsupported(
                            "structural case result declaration missing",
                        ))?;
                self.temporary_places.push(declaration);
                self.values.truncate(field_start);
                Ok(place)
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                let arms = self
                    .checked
                    .facts
                    .values
                    .structural_values
                    .dispatch_arms
                    .span(arms)
                    .ok_or(LoweringError::Unsupported(
                        "structural dispatch arm span is stale",
                    ))?
                    .to_vec();
                let source_count = self.values.len();
                let subject = self.scalar(
                    CheckedScalarExpressionRole::StructuralValueSubject {
                        expression: node.expression,
                    },
                    subject,
                    source_count,
                )?;
                self.values.push(subject);
                let join = block_id(allocate_dense(self.next_block)?);
                let joined_values = self.values[..source_count]
                    .iter()
                    .map(|value| {
                        Ok(ValueDeclaration {
                            id: value_id(allocate_dense(self.next_value)?),
                            ..*value
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let place = place_id(allocate_dense(self.next_place)?);
                self.temporary_places.push(StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: join,
                        position: 0,
                    },
                });
                for (position, arm) in arms.iter().enumerate() {
                    let mut fallback = None;
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        let pattern = self.scalar(
                            CheckedScalarExpressionRole::StructuralValuePattern {
                                source_arm: arm.source_arm,
                            },
                            pattern,
                            source_count,
                        )?;
                        let subject = self.values[source_count];
                        if pattern.value_type() != subject.value_type()
                            || arm.equality_use.is_valid()
                        {
                            return unsupported(
                                "structural dispatch comparison requires matching builtin scalar operands",
                            );
                        }
                        // Replay requires coverage. A final value pattern is
                        // therefore the last literal Boolean alternative, not
                        // permission to default to an arbitrary integer arm.
                        if position + 1 < arms.len() {
                            let kind = if subject.scalar_type == ScalarType::Boolean {
                                OperationKind::BooleanEqual {
                                    left: subject.id,
                                    right: pattern.id,
                                }
                            } else if matches!(subject.scalar_type, ScalarType::Integer { .. }) {
                                LoweredIntegerComparisonKind::Equal
                                    .operation(subject.id, pattern.id)
                            } else {
                                return unsupported(
                                    "structural dispatch needs selected floating comparison lowering",
                                );
                            };
                            let condition = value_id(allocate_dense(self.next_value)?);
                            let id = self.operations.allocate();
                            self.operations.push(Operation {
                                id,
                                result: OperationResult::Scalar(ValueDeclaration {
                                    id: condition,
                                    scalar_type: ScalarType::Boolean,
                                    qualifications: Default::default(),
                                }),
                                kind,
                            });
                            let selected = block_id(allocate_dense(self.next_block)?);
                            let next = block_id(allocate_dense(self.next_block)?);
                            let when_true = self.edge(selected, Vec::new(), Vec::new())?;
                            let when_false = self.edge(next, Vec::new(), Vec::new())?;
                            self.finish(Terminator::Conditional {
                                condition,
                                when_true,
                                when_false,
                            });
                            fallback = Some((next, self.values.clone()));
                            self.start(selected);
                        }
                    }
                    self.values.truncate(source_count);
                    let selected = self.value(arm.value)?;
                    let edge = self.edge(
                        join,
                        self.values.iter().map(|value| value.id).collect(),
                        vec![StructuralArgument {
                            place: selected,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        }],
                    )?;
                    self.finish(Terminator::Jump {
                        edge: edge.edge,
                        target: edge.target,
                        arguments: edge.arguments,
                        structural_arguments: edge.structural_arguments,
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    });
                    if let Some((next, values)) = fallback {
                        self.start(next);
                        *self.values = values;
                    }
                }
                self.start(join);
                *self.values = joined_values.clone();
                self.evaluation.parameters = joined_values;
                self.evaluation.block_structural_parameters =
                    vec![StructuralParameterDeclaration {
                        place,
                        position: 0,
                        is_self: false,
                        structural_type: self.structural_type,
                        multiplicity: self.multiplicity,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    }];
                Ok(place)
            }
        }
    }

    fn scalar(
        &mut self,
        role: CheckedScalarExpressionRole,
        value: checked_trees::CheckedScalarComputationHandle,
        source_count: usize,
    ) -> Result<ValueDeclaration, LoweringError> {
        let value = self.evaluation.source_value(
            self.checked,
            self.machine,
            self.state,
            self.statement,
            role,
            &checked_trees::CheckedCallScalarArgument::Computation(value),
            source_count,
            self.values,
            self.next_value,
            self.next_block,
            self.next_edge,
            self.operations,
            self.calls,
        )?;
        Ok(value)
    }

    fn edge(
        &mut self,
        target: BlockId,
        arguments: Vec<ValueId>,
        structural_arguments: Vec<StructuralArgument>,
    ) -> Result<SuccessorEdge, LoweringError> {
        Ok(SuccessorEdge {
            edge: edge_id(allocate_dense(self.next_edge)?),
            target,
            arguments,
            structural_arguments,
            trivial_affine_discards: Vec::new(),
        })
    }

    fn finish(&mut self, terminator: Terminator) {
        self.evaluation.blocks.push(Block {
            id: self.evaluation.current,
            parameters: std::mem::take(&mut self.evaluation.parameters),
            structural_parameters: std::mem::take(&mut self.evaluation.block_structural_parameters),
            operations: self.operations[self.evaluation.operation_start..].to_vec(),
            terminator,
        });
    }

    fn start(&mut self, block: BlockId) {
        self.evaluation.current = block;
        self.evaluation.operation_start = self.operations.len();
        self.operations.byte_lengths.clear();
    }
}
