use super::*;
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    catalogs: &mut composed_control::ComposedCatalogs,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    source_custody::validate(checked, machine, state, operation)?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, value, .. } = operation
    else {
        return unsupported("structural value producer missing");
    };
    let structural_type = lookup_type_id(&catalogs.type_ids, &result.type_identity)?;
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
        catalogs,
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
        .catalogs
        .temporary_places
        .iter()
        .position(|item| item.id == place)
        .ok_or(LoweringError::Unsupported(
            "structural value place was not declared",
        ))?;
    let declaration = emission.catalogs.temporary_places.remove(declaration);
    emission.catalogs.result_places.push(declaration);
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
    Ok(())
}

struct Emission<'a, 'b> {
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    catalogs: &'b mut composed_control::ComposedCatalogs,
    evaluation: &'b mut argument_evaluation::Evaluation,
    values: &'b mut Vec<ValueDeclaration>,
    next_value: &'b mut u64,
    next_block: &'b mut u64,
    next_edge: &'b mut u64,
    operations: &'b mut OperationBuffer,
}

impl Emission<'_, '_> {
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
            CheckedStructuralValueKind::Case {
                data_symbol,
                case_symbol,
            } => {
                let data = self
                    .checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == data_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "constructed sum declaration missing",
                    ))?;
                let variant = self
                    .checked
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Variant(variant)
                            if variant.symbol == case_symbol =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "constructed case declaration missing",
                    ))?;
                let identity = variant
                    .identity
                    .map(|identity| format!("#{identity}"))
                    .unwrap_or_else(|| variant.name.as_str().to_owned());
                let shape = &self
                    .catalogs
                    .structural_types
                    .iter()
                    .find(|item| item.id == self.structural_type)
                    .ok_or(LoweringError::Unsupported(
                        "constructed structural type missing",
                    ))?
                    .shape;
                let StructuralTypeShape::Sum { cases } = shape else {
                    return unsupported("case construction needs a nominal sum");
                };
                let case = cases
                    .iter()
                    .find(|case| case.identity == identity && case.fields.is_empty())
                    .ok_or(LoweringError::Unsupported(
                        "fresh case requires its exact empty payload",
                    ))?
                    .id;
                let operation = self.operations.allocate();
                let place = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                self.catalogs
                    .temporary_places
                    .push(StructuralPlaceDeclaration {
                        id: place,
                        kind: StructuralPlaceKind::OperationResult {
                            producer: operation,
                            structural_type: self.structural_type,
                        },
                    });
                self.operations.push(Operation {
                    id: operation,
                    result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        place,
                        structural_type: self.structural_type,
                        multiplicity: self.multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishScalarCase {
                        result_case: case,
                        fields: Vec::new(),
                    },
                });
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
                let place = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                self.catalogs
                    .temporary_places
                    .push(StructuralPlaceDeclaration {
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
        let mut calls = self.catalogs.scalar_calls.emission_context();
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
            &mut calls,
        )?;
        self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
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
