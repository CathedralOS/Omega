use super::*;
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind};

pub(super) type StructuralCallEmitter<'a> = dyn FnMut(
        &CheckedUnitEffectOperationPlan,
        Option<&[ValueDeclaration]>,
        &mut CallEmissionContext<'_>,
        &mut OperationBuffer,
        &mut u64,
    ) -> Result<StructuralPlaceDeclaration, LoweringError>
    + 'a;

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
    call_emitter: &mut StructuralCallEmitter<'_>,
    calls: &mut CallEmissionContext<'_>,
    evaluation: &mut argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    source_custody::validate(checked, machine, state, operation)?;
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls: operand_calls,
        ..
    } = operation
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
    let sources = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, result.statement_index)
        .map(|(_, receipt)| {
            checked
                .facts
                .flow
                .ownership
                .selection_sources
                .span(receipt.sources)
                .ok_or(LoweringError::Unsupported(
                    "owned selection source span is stale",
                ))?
                .iter()
                .rev()
                .map(|source| {
                    evaluation
                        .structural_locals
                        .iter()
                        .find(|(symbol, _)| *symbol == source.symbol)
                        .map(|(_, argument)| argument.clone())
                        .ok_or(LoweringError::Unsupported(
                            "owned selection source has no established physical local",
                        ))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if !sources.is_empty() && multiplicity != StructuralMultiplicity::Affine {
        return unsupported("owned selection requires uniform affine root cleanup");
    }
    let owners = if sources.is_empty() {
        Vec::new()
    } else {
        let owners = prepare_owners(
            checked,
            machine,
            state,
            result.statement_index,
            structural_types,
            evaluation,
            operations,
        )?;
        let candidate_positions = owners
            .iter()
            .enumerate()
            .filter_map(|(position, owner)| {
                sources
                    .iter()
                    .any(|source| source.place == owner.value.place)
                    .then_some(position)
            })
            .collect::<Vec<_>>();
        if candidate_positions.len() != sources.len() {
            return unsupported("owned selection source is absent from the live emitted frontier");
        }
        if candidate_positions
            .windows(2)
            .any(|positions| positions[1] != positions[0] + 1)
        {
            return unsupported(
                "interleaved selection sources require path-dependent residual establishment-order correspondence",
            );
        }
        owners
    };
    let mut emission = Emission {
        checked,
        machine,
        state,
        statement: result.statement_index,
        structural_type,
        multiplicity,
        structural_types,
        type_ids,
        next_place,
        temporary_places,
        calls,
        operand_calls,
        call_emitter,
        evaluation,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        sources,
        owners,
    };
    let place = emission.value(*value, None)?;
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
    let mut local_symbol = symbols::SymbolHandle::invalid();
    if let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(
            crate::scalar_source_custody::authored_state(checked, state)?
                .1
                .statement_nodes,
        )
        .get(result.statement_index as usize)
    {
        local_symbol = local.symbol;
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
    if multiplicity == StructuralMultiplicity::Affine {
        emission.evaluation.structural_value_owners.push(
            argument_evaluation::StructuralValueOwner {
                symbol: local_symbol,
                statement: result.statement_index,
                value: terminal_psi::StructuralOperationResult {
                    place,
                    structural_type,
                    multiplicity,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                },
            },
        );
    }
    Ok(declaration)
}

fn prepare_owners(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    structural_types: &[StructuralTypeDeclaration],
    evaluation: &argument_evaluation::Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<argument_evaluation::StructuralValueOwner>, LoweringError> {
    let available = |symbol: symbols::SymbolHandle| -> Result<bool, LoweringError> {
        if !symbol.is_valid() {
            return Ok(true);
        }
        for (_, receipt) in checked.facts.flow.ownership.owned_selections.iter() {
            if receipt.machine == machine
                && receipt.state == state
                && receipt.statement_ordinal < statement
                && checked
                    .facts
                    .flow
                    .ownership
                    .selection_sources
                    .span_or_empty(receipt.sources)
                    .iter()
                    .any(|source| source.symbol == symbol)
            {
                return Ok(false);
            }
        }
        for (_, event) in checked.facts.flow.ownership.permissions.iter() {
            if event.machine_symbol != machine
                || event.state_symbol != state
                || event.root != facts::PlaceRoot::Symbol(symbol)
                || event.access != language_semantics::PermissionAccess::Owned
            {
                continue;
            }
            let ordinal = match event.source {
                language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call {
                    statement_index, ..
                } => statement_index,
                _ => continue,
            };
            if ordinal >= statement as usize
                || event.kind == language_semantics::PermissionEventKind::Establish
            {
                continue;
            }
            if !event.segments.is_empty() {
                return unsupported(
                    "selection pass-through of partially moved roots requires retained residual frontier correspondence",
                );
            }
            return Ok(false);
        }
        Ok(true)
    };
    let mut owners = Vec::new();
    for owner in &evaluation.structural_value_owners {
        if available(owner.symbol)? {
            owners.push(owner.clone());
        }
    }
    let statements = checked.statement_table.statements(
        crate::scalar_source_custody::authored_state(checked, state)?
            .1
            .statement_nodes,
    );
    for (symbol, argument) in &evaluation.structural_locals {
        if owners.iter().any(|owner| owner.symbol == *symbol) || !available(*symbol)? {
            continue;
        }
        let (ordinal, local) = statements
            .iter()
            .enumerate()
            .find_map(|(ordinal, source)| match source {
                checked_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == *symbol =>
                {
                    Some((ordinal, local))
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "selection survivor has no authored local",
            ))?;
        if checked.type_multiplicity(local.type_reference) != Multiplicity::Affine {
            continue;
        }
        if ordinal >= statement as usize
            || !argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                &checked.typed,
                local.type_reference,
            )
        {
            return unsupported("selection survivor requires whole plain-affine custody");
        }
        let value = operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
            .ok_or(LoweringError::Unsupported(
                "selection survivor requires an exact emitted operation-result frontier",
            ))?;
        if value.multiplicity != StructuralMultiplicity::Affine
            || !value.claims.is_empty()
            || !value.qualifications.is_empty()
            || !value.projected_qualifications.is_empty()
            || !structural_types
                .iter()
                .any(|declaration| declaration.id == value.structural_type)
        {
            return unsupported("selection survivor requires claim-free plain-affine transport");
        }
        owners.push(argument_evaluation::StructuralValueOwner {
            symbol: *symbol,
            statement: u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("selection survivor statement exceeds u32")
            })?,
            value: value.clone(),
        });
    }
    owners.sort_by_key(|owner| owner.statement);
    for source in statements.iter().take(statement as usize) {
        if let checked_trees::statement::StatementNode::LocalData(local) = source
            && checked.type_multiplicity(local.type_reference) == Multiplicity::Affine
            && available(local.symbol)?
            && !owners.iter().any(|owner| owner.symbol == local.symbol)
        {
            return unsupported(
                "selection transport requires a typed frontier entry for every live owned local",
            );
        }
    }
    Ok(owners)
}

pub(super) struct Emission<'a, 'b, 'calls> {
    pub(super) checked: &'a CheckedTrees,
    pub(super) machine: symbols::SymbolHandle,
    pub(super) state: symbols::SymbolHandle,
    pub(super) statement: u32,
    pub(super) structural_type: StructuralTypeId,
    pub(super) multiplicity: StructuralMultiplicity,
    pub(super) structural_types: &'a [StructuralTypeDeclaration],
    pub(super) type_ids: &'a [(String, StructuralTypeId)],
    pub(super) next_place: &'b mut u64,
    pub(super) temporary_places: &'b mut Vec<StructuralPlaceDeclaration>,
    pub(super) calls: &'b mut CallEmissionContext<'calls>,
    pub(super) operand_calls: &'a [checked_trees::CheckedStructuralValueCall],
    pub(super) call_emitter: &'b mut StructuralCallEmitter<'a>,
    pub(super) evaluation: &'b mut argument_evaluation::Evaluation,
    pub(super) values: &'b mut Vec<ValueDeclaration>,
    pub(super) next_value: &'b mut u64,
    pub(super) next_block: &'b mut u64,
    pub(super) next_edge: &'b mut u64,
    pub(super) operations: &'b mut OperationBuffer,
    sources: Vec<StructuralArgument>,
    owners: Vec<argument_evaluation::StructuralValueOwner>,
}

pub(super) struct ValueContinuation {
    block: BlockId,
    parameters: Vec<ValueDeclaration>,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    place: PlaceId,
    remaining_owners: Vec<argument_evaluation::StructuralValueOwner>,
    pass_through: Vec<(PlaceId, PlaceId)>,
    residuals: Vec<PlaceId>,
}

impl Emission<'_, '_, '_> {
    pub(super) fn value(
        &mut self,
        value: CheckedStructuralValueHandle,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
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
                if !self.sources.is_empty() {
                    return unsupported("selected ownership mixes fresh and existing obligations");
                }
                let place = self.record(value)?;
                if let Some(continuation) = continuation {
                    self.complete_value(place, continuation)?;
                }
                Ok(place)
            }
            CheckedStructuralValueKind::Call { .. } => {
                let mut matching = self.operand_calls.iter().filter(|call| call.value == value);
                let call = matching.next().ok_or(LoweringError::Unsupported(
                    "structural operand call missing",
                ))?;
                if matching.next().is_some() {
                    return unsupported("structural operand call is duplicated");
                }
                let evaluated = self.evaluation.arguments(
                    self.checked,
                    self.machine,
                    self.state,
                    call.operation(),
                    self.values,
                    self.next_value,
                    self.next_block,
                    self.next_edge,
                    self.operations,
                    self.calls,
                )?;
                let declaration = (self.call_emitter)(
                    call.operation(),
                    evaluated.as_deref(),
                    self.calls,
                    self.operations,
                    self.next_place,
                )?;
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = declaration.kind
                else {
                    return unsupported("structural operand call has no result place");
                };
                if structural_type != self.structural_type {
                    return unsupported("structural operand call changed its returned type");
                }
                if let Some(continuation) = continuation {
                    self.complete_value(declaration.id, continuation)?;
                }
                Ok(declaration.id)
            }
            CheckedStructuralValueKind::Place(argument) => {
                if self.sources.is_empty()
                    && continuation.is_none()
                    && self.structural_types.iter().any(|declaration| {
                        declaration.id == self.structural_type
                            && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                    })
                {
                    // Source replay distinguishes a whole record child from a
                    // selected sum leaf, whose receipt and continuation remain mandatory.
                    return crate::scalar_bindings::ScalarBindings::new(self.values.len())
                        .with_structural_parameters(&self.evaluation.structural_parameters)
                        .with_structural_locals(&self.evaluation.structural_locals)
                        .owned_argument(&argument)
                        .map(|binding| binding.place);
                }
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } = argument.source
                else {
                    return unsupported("owned selection requires an established structural local");
                };
                if !argument.path.is_empty()
                    || argument.access != checked_trees::CheckedStructuralAccess::Owned
                {
                    return unsupported("owned selection requires whole owned sources");
                }
                let selected = self
                    .evaluation
                    .structural_locals
                    .iter()
                    .find(|(source, _)| *source == symbol)
                    .map(|(_, argument)| argument.place)
                    .ok_or(LoweringError::Unsupported(
                        "owned selection local place missing",
                    ))?;
                if !self.sources.iter().any(|source| source.place == selected) {
                    return unsupported("owned selection place is absent from its receipt sources");
                }
                let continuation = continuation.ok_or(LoweringError::Unsupported(
                    "direct owned place requires a structural continuation",
                ))?;
                self.complete_value(selected, continuation)?;
                Ok(continuation.place)
            }
            CheckedStructuralValueKind::Case(construction) => {
                if !self.sources.is_empty() {
                    return unsupported(
                        "mixed fresh and existing ownership requires a join carrying unequal residual counts",
                    );
                }
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
                if let Some(continuation) = continuation {
                    self.complete_value(place, continuation)?;
                }
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
                let owned_continuation;
                let is_root = continuation.is_none();
                let continuation = if let Some(continuation) = continuation {
                    continuation
                } else {
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
                    let mut structural_parameters = Vec::new();
                    let first_candidate = self.owners.iter().position(|owner| {
                        self.sources
                            .iter()
                            .any(|source| source.place == owner.value.place)
                    });
                    let mut remaining_owners = self
                        .owners
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| Some(*position) != first_candidate)
                        .map(|(_, owner)| owner.clone())
                        .collect::<Vec<_>>();
                    let mut pass_through = Vec::new();
                    let mut residuals = Vec::new();
                    for position in 0..remaining_owners.len() + 1 {
                        let (structural_type, multiplicity) = remaining_owners
                            .get(position)
                            .map_or((self.structural_type, self.multiplicity), |owner| {
                                (owner.value.structural_type, owner.value.multiplicity)
                            });
                        let owner = remaining_owners.get_mut(position);
                        let position = u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported(
                                "owned selection parameter count exceeds u32",
                            )
                        })?;
                        let place = place_id(allocate_dense(self.next_place)?);
                        if let Some(owner) = owner {
                            if self
                                .sources
                                .iter()
                                .any(|source| source.place == owner.value.place)
                            {
                                owner.symbol = symbols::SymbolHandle::invalid();
                                residuals.push(place);
                            } else {
                                pass_through.push((owner.value.place, place));
                            }
                            owner.value.place = place;
                        }
                        self.temporary_places.push(StructuralPlaceDeclaration {
                            id: place,
                            kind: StructuralPlaceKind::BlockParameter {
                                block: join,
                                position,
                            },
                        });
                        structural_parameters.push(StructuralParameterDeclaration {
                            place,
                            position,
                            is_self: false,
                            structural_type,
                            multiplicity,
                            access: StructuralAccess::Owned,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        });
                    }
                    let place = structural_parameters
                        .last()
                        .ok_or(LoweringError::Unsupported(
                            "structural continuation has no result",
                        ))?
                        .place;
                    owned_continuation = ValueContinuation {
                        block: join,
                        parameters: joined_values,
                        structural_parameters,
                        place,
                        remaining_owners,
                        pass_through,
                        residuals,
                    };
                    &owned_continuation
                };
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
                    self.value(arm.value, Some(continuation))?;
                    if let Some((next, values)) = fallback {
                        self.start(next);
                        *self.values = values;
                    }
                }
                if is_root {
                    self.start(continuation.block);
                    *self.values = continuation.parameters.clone();
                    self.evaluation.parameters = continuation.parameters.clone();
                    self.evaluation.block_structural_parameters =
                        continuation.structural_parameters.clone();
                    if !self.sources.is_empty() {
                        self.evaluation.selection_cleanups.push(
                            argument_evaluation::SelectionCleanup {
                                selected: continuation.place,
                                sources: self
                                    .sources
                                    .iter()
                                    .rev()
                                    .map(|source| source.place)
                                    .collect(),
                                remaining: continuation.residuals.iter().rev().copied().collect(),
                                pass_through: continuation.pass_through.clone(),
                                next_operation: self.operations.next_identity,
                            },
                        );
                        self.evaluation.structural_value_owners =
                            continuation.remaining_owners.clone();
                        self.evaluation.structural_locals.retain(|(_, argument)| {
                            !self
                                .sources
                                .iter()
                                .any(|source| source.place == argument.place)
                        });
                        for (_, argument) in &mut self.evaluation.structural_locals {
                            if let Some((_, target)) = continuation
                                .pass_through
                                .iter()
                                .find(|(source, _)| *source == argument.place)
                            {
                                argument.place = *target;
                            }
                        }
                        self.rebind_local_cases()?;
                    }
                }
                Ok(continuation.place)
            }
        }
    }

    fn complete_value(
        &mut self,
        selected: PlaceId,
        continuation: &ValueContinuation,
    ) -> Result<(), LoweringError> {
        let mut structural_arguments = self
            .owners
            .iter()
            .filter(|owner| owner.value.place != selected)
            .map(|owner| StructuralArgument {
                place: owner.value.place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            })
            .collect::<Vec<_>>();
        structural_arguments.push(StructuralArgument {
            place: selected,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        });
        if structural_arguments.len() != continuation.structural_parameters.len() {
            return unsupported("structural value has unequal residual ownership at its join");
        }
        let edge = self.edge(
            continuation.block,
            self.values.iter().map(|value| value.id).collect(),
            structural_arguments,
        )?;
        self.finish(Terminator::Jump {
            edge: edge.edge,
            target: edge.target,
            arguments: edge.arguments,
            structural_arguments: edge.structural_arguments,
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        });
        Ok(())
    }

    fn rebind_local_cases(&mut self) -> Result<(), LoweringError> {
        let statements = self.checked.statement_table.statements(
            crate::scalar_source_custody::authored_state(self.checked, self.state)?
                .1
                .statement_nodes,
        );
        let mut cases = Vec::new();
        for (symbol, argument) in &self.evaluation.structural_locals {
            let Some(local) = statements.iter().find_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == *symbol =>
                {
                    Some(local)
                }
                _ => None,
            }) else {
                continue;
            };
            let identity = self.checked.normalized_type_identity(local.type_reference);
            if self.structural_types.iter().any(|declaration| {
                declaration.identity == identity.as_str()
                    && matches!(declaration.shape, StructuralTypeShape::Sum { .. })
            }) {
                cases.push(
                    crate::scalar_bindings::structural_cases::LocalCaseBinding::new(
                        self.checked,
                        *symbol,
                        local.type_reference,
                        argument.place,
                        self.structural_types,
                    )?,
                );
            }
        }
        self.evaluation.local_cases = cases;
        Ok(())
    }

    pub(super) fn scalar(
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
