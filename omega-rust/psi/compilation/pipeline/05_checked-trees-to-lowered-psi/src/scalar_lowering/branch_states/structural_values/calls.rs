//! Consume the common formal-order schedule in an expression continuation.
//! Checked result ordinals join intermediate owners to their consumers; scalar
//! argument positions remain live across intervening structural producers.
use super::*;
use crate::emission::operation_emission::buffer::StructuralBinding;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::unit::attached_unit::argument_schedule;

impl Storage {
    pub(super) fn reserve_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        root: checked_trees::CheckedStructuralValueHandle,
        types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
    ) -> Result<(), LoweringError> {
        for value in crate::expression_preparation::computation_graph::structural_dependencies(
            checked, root,
        )?
        .values
        {
            let node = checked.facts.values.structural_values.nodes.get(value);
            if !self.places.iter().any(|(candidate, _)| *candidate == value) {
                let range_source = match &node.kind {
                    CheckedStructuralValueKind::BorrowedSliceView { source, .. }
                    | CheckedStructuralValueKind::MutableSliceView { source, .. }
                        if computations::subslices::range(source).is_some() =>
                    {
                        Some(source)
                    }
                    _ => None,
                };
                let place = if let Some(source) = range_source {
                    let declaration = carrier_declaration(identities, checked, types, node.type_reference)?;
                    self.reserve_range(identities, checked, node.state, node.statement_index, source, declaration.id, types, next_place)?
                } else if let CheckedStructuralValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes }, ..
                }) = &node.kind {
                    let declaration = carrier_declaration(identities, checked, types, node.type_reference)?;
                    let literal = crate::emission::structural_byte_sequence_store::ByteLiteral::reserve(identities, bytes, declaration.id, types, next_place)?;
                    let place = literal.place;
                    self.literals.push((value, literal));
                    place
                } else {
                    place_id(allocate_dense(next_place)?)
                };
                self.places.push((value, place));
            }
            if matches!(
                node.kind,
                checked_trees::CheckedStructuralValueKind::Call { .. }
            ) && !node.qualification_decisions.is_empty()
                && !self
                    .producers
                    .iter()
                    .any(|(candidate, _)| *candidate == value)
            {
                self.producers
                    .push((value, place_id(allocate_dense(next_place)?)));
            }
        }
        self.reserve_calls(
            identities,
            checked,
            checked
                .facts
                .values
                .structural_values
                .calls_for_root(root)
                .unwrap_or(&[]),
            types,
            next_place,
        )
    }

    /// A range occurrence has one completed descriptor home, whether it is a
    /// constructor leaf or a direct call operand. Reserving its backing alone
    /// loses the narrowing operation when the value schedule later consumes it.
    fn reserve_range(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        statement: u32,
        argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
        structural_type: StructuralTypeId,
        types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
    ) -> Result<PlaceId, LoweringError> {
        let range = computations::subslices::range(argument).ok_or(LoweringError::Unsupported(
            "structural range reservation lost its source",
        ))?;
        if let Some(slot) = self
            .ranges
            .iter()
            .find(|slot| slot.expression == range.expression)
        {
            if slot.structural_type != structural_type {
                return unsupported("structural range occurrence has conflicting result carriers");
            }
            return Ok(slot.place);
        }
        let slot = computations::subslices::reserve(
            identities,
            checked,
            state,
            statement,
            argument,
            structural_type,
            types,
            next_place,
        )?;
        let place = slot.place;
        self.ranges.push(slot);
        Ok(place)
    }

    fn reserve_calls(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        calls: &[checked_trees::CheckedStructuralValueCall],
        types: &[StructuralTypeDeclaration],
        next_place: &mut u64,
    ) -> Result<(), LoweringError> {
        for call in calls {
            let state = checked
                .facts
                .values
                .structural_values
                .nodes
                .get(call.value)
                .state;
            for (index, operation) in call
                .operands()
                .iter()
                .chain(std::iter::once(call.operation()))
                .enumerate()
            {
                if let Some(planned_call) = operation.call() {
                    for argument in &planned_call.structural_arguments {
                        if computations::subslices::range(argument).is_none() {
                            continue;
                        }
                        let declaration =
                            crate::terminal_identities::structural_declaration_by_identity(
                                types,
                                identities,
                                checked,
                                argument.type_identity,
                                "computed range has no structural type namespace",
                            )?;
                        self.reserve_range(
                            identities,
                            checked,
                            state,
                            planned_call.coordinate.statement_index,
                            argument,
                            declaration.id,
                            types,
                            next_place,
                        )?;
                    }
                }
                let result =
                    operation
                        .structural_result_binding()
                        .ok_or(LoweringError::Unsupported(
                            "structural operand preparation has no result binding",
                        ))?;
                let place = if index == call.operands().iter().count() {
                    self.producer_place(call.value)?
                } else if let CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                        value,
                        calls,
                        ..
                    },
                ) = operation
                {
                    self.reserve_value(identities, checked, *value, types, next_place)?;
                    self.reserve_calls(identities, checked, calls, types, next_place)?;
                    self.place(*value)?
                } else if let Some((_, place)) = self
                    .results
                    .iter()
                    .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
                {
                    *place
                } else {
                    place_id(allocate_dense(next_place)?)
                };
                if let Some((_, existing)) = self
                    .results
                    .iter()
                    .find(|(ordinal, _)| *ordinal == result.binding_ordinal)
                {
                    if *existing != place {
                        return unsupported(
                            "structural result ordinal has conflicting reserved homes",
                        );
                    }
                } else {
                    self.results.push((result.binding_ordinal, place));
                }
            }
        }
        Ok(())
    }

    fn result_place(&self, ordinal: u32) -> Result<PlaceId, LoweringError> {
        self.results
            .iter()
            .find(|(candidate, _)| *candidate == ordinal)
            .map(|(_, place)| *place)
            .ok_or(LoweringError::Unsupported(
                "structural operand result has no reserved home",
            ))
    }

    fn producer_place(
        &self,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Result<PlaceId, LoweringError> {
        self.producers
            .iter()
            .find(|(candidate, _)| *candidate == value)
            .map(|(_, place)| *place)
            .map_or_else(|| self.place(value), Ok)
    }
}

impl Preparation<'_> {
    pub(super) fn prepare_calls(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        producer: &checked_trees::CheckedStructuralValueCall,
    ) -> Result<PlaceId, LoweringError> {
        let (machine, _) = crate::declarations::state_declaration(self.checked, self.state)?;
        let caller = super::super::unit_operations::Caller::published(
            self.checked,
            machine.symbol,
            self.state,
        )?;
        let steps = argument_schedule::operand_preparation(
            self.checked,
            machine.symbol,
            self.state,
            producer.operands(),
            producer.operation(),
        )?;
        let operations = producer
            .operands()
            .iter()
            .chain(std::iter::once(producer.operation()))
            .collect::<Vec<_>>();
        let mut positions = vec![Vec::new(); operations.len()];
        let mut structural_arguments = vec![Vec::new(); operations.len()];
        let mut selector_positions = vec![Vec::<(usize, usize, usize)>::new(); operations.len()];
        let mut results: Vec<(u32, StructuralBinding)> = Vec::new();
        // Each completed producer enters the namespace used by subsequent
        // actuals. A side list of result ordinals alone loses expression-backed
        // storage when the next argument borrows that producer's descriptor.
        let mut bindings = self.bindings.clone();
        for step in steps {
            match step {
                argument_schedule::Step::Subslice { operation, ordinal } => {
                    let call = operations[operation]
                        .call()
                        .ok_or(LoweringError::Unsupported(
                            "range operand schedule lost its call",
                        ))?;
                    let argument = call.structural_arguments.get(ordinal).ok_or(
                        LoweringError::Unsupported("range operand schedule lost its argument"),
                    )?;
                    let range = computations::subslices::range(argument).ok_or(
                        LoweringError::Unsupported("range operand schedule lost its source"),
                    )?;
                    let slot = self
                        .reserved
                        .ranges
                        .iter()
                        .find(|slot| slot.expression == range.expression)
                        .ok_or(LoweringError::Unsupported(
                            "range operand has no reserved descriptor",
                        ))?;
                    let access =
                        crate::expression_preparation::bindings::structural_paths::terminal_access(
                            argument.access,
                        );
                    let completed = crate::emission::operation_emission::calls::PreparedStructuralArgument::from_static(
                        StructuralArgument::whole(slot.place, access),
                        Some(slot.structural_type),
                    )?;
                    structural_arguments[operation].push((ordinal, completed));
                    self.steps.push(Step::Range {
                        argument: argument.clone(),
                        slot: slot.clone(),
                        bindings: bindings.clone(),
                        statement: call.coordinate.statement_index,
                        prefix: self.prefix.clone(),
                    });
                }
                argument_schedule::Step::Selector {
                    operation,
                    argument,
                    segment,
                } => {
                    let call = operations[operation]
                        .call()
                        .ok_or(LoweringError::Unsupported(
                            "selector schedule lost its call",
                        ))?;
                    let selector = call
                        .structural_arguments
                        .get(argument)
                        .and_then(|argument| argument.call_projection().get(segment))
                        .ok_or(LoweringError::Unsupported(
                            "selector schedule lost its path",
                        ))?;
                    let result_type = match selector {
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                            let source = self
                                .checked
                                .facts
                                .values
                                .scalar_computations
                                .roots
                                .iter()
                                .map(|(_, source)| source)
                                .find(|source| {
                                    source.root == *root
                                        && source.state == self.state
                                        && source.statement_ordinal
                                            == call.coordinate.statement_index
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "call selector lost its source computation",
                                ))?;
                            let result_type = computations::computation_value_type(
                                identities,
                                self.checked,
                                self.qualifications,
                                *root,
                                &bindings,
                                &self.prefix,
                            )?;
                            self.steps.push(Step::Scalar {
                                role: source.role,
                                result_type,
                                prefix: self.prefix.clone(),
                            });
                            result_type
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                            let value = match index {
                                checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                                    bindings.runtime_index_field(identities, field)?
                                }
                                checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                                    bindings.runtime_index_storage(*symbol)?
                                }
                                _ => {
                                    return unsupported(
                                        "scheduled selector has no evaluated operand",
                                    );
                                }
                            };
                            let result_type = value.value_type(&self.prefix)?;
                            self.steps.push(Step::Snapshot {
                                value,
                                prefix: self.prefix.clone(),
                            });
                            result_type
                        }
                        _ => return unsupported("scheduled selector has no evaluated operand"),
                    };
                    if !matches!(result_type.scalar_type, semantic_vocabulary::ScalarType::Integer(integer) if !integer.is_address())
                    {
                        return unsupported("call selector is not an integer operand");
                    }
                    selector_positions[operation].push((argument, segment, self.prefix.len()));
                    self.prefix.push(result_type);
                }
                argument_schedule::Step::Argument { operation, ordinal } => {
                    let call = operations[operation]
                        .call()
                        .ok_or(LoweringError::Unsupported(
                            "argument schedule lost its call",
                        ))?;
                    let argument =
                        call.scalar_arguments
                            .get(ordinal)
                            .ok_or(LoweringError::Unsupported(
                                "argument schedule lost its scalar actual",
                            ))?;
                    if positions[operation].len() != ordinal {
                        return unsupported("argument schedule reordered scalar operands");
                    }
                    let value_type = match argument {
                        checked_trees::CheckedCallScalarArgument::Pure(expression) => bindings
                            .expression(
                                identities,
                                &self.checked.facts.proven_scalar_indexes,
                                expression,
                            )?
                            .value_type(&self.prefix)?,
                        checked_trees::CheckedCallScalarArgument::Computation(root) => {
                            computations::computation_value_type(
                                identities,
                                self.checked,
                                self.qualifications,
                                *root,
                                &bindings,
                                &self.prefix,
                            )?
                        }
                    };
                    positions[operation].push(self.prefix.len());
                    self.steps.push(Step::Argument {
                        coordinate: call.coordinate,
                        ordinal,
                        argument: argument.clone(),
                        prefix: self.prefix.clone(),
                    });
                    self.prefix.push(value_type);
                }
                argument_schedule::Step::Call(operation) => {
                    let call = operations[operation]
                        .call()
                        .ok_or(LoweringError::Unsupported(
                            "argument schedule lost its call",
                        ))?;
                    let binding = call.structural_result().ok_or(LoweringError::Unsupported(
                        "structural operand call lost its result",
                    ))?;
                    if positions[operation].len() != call.scalar_arguments.len() {
                        return unsupported("structural call has incomplete scalar preparation");
                    }
                    let result = self.result(identities, binding)?;
                    let arguments = positions[operation]
                        .iter()
                        .map(|position| LoweredDirectExpression::Parameter {
                            position: *position,
                            scalar_type: self.prefix[*position].scalar_type,
                        })
                        .collect();
                    let mut prepared = super::super::unit_operations::prepare(
                        identities,
                        self.checked,
                        &caller,
                        operations[operation],
                        &bindings,
                        arguments,
                        self.types,
                        Some(result.clone()),
                        &results,
                        &structural_arguments[operation],
                    )?;
                    // Paths consume the snapshots at their authored schedule
                    // positions; later arguments cannot make them read storage again.
                    for (ordinal, argument) in call.structural_arguments.iter().enumerate() {
                        let saved = selector_positions[operation]
                            .iter()
                            .filter(|(argument, _, _)| *argument == ordinal)
                            .collect::<Vec<_>>();
                        let expected = argument.call_selector_segments().collect::<Vec<_>>();
                        if saved.len() != expected.len()
                            || saved
                                .iter()
                                .zip(&expected)
                                .any(|((_, segment, _), (expected, _))| segment != expected)
                        {
                            return unsupported("call selector snapshots disagree with its path");
                        }
                        prepared.call.structural_arguments[ordinal].selector_positions =
                            saved.iter().map(|(_, _, position)| *position).collect();
                    }
                    self.steps.push(Step::Call {
                        prepared,
                        prefix: self.prefix.clone(),
                    });
                    if results
                        .iter()
                        .any(|(ordinal, _)| *ordinal == binding.binding_ordinal)
                    {
                        return unsupported("structural operand result is established twice");
                    }
                    results.push((binding.binding_ordinal, result.into()));
                }
                argument_schedule::Step::Produce(operation) => {
                    let CheckedUnitEffectOperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                            result,
                            value,
                            calls,
                            ..
                        },
                    ) = operations[operation]
                    else {
                        return unsupported(
                            "scalar expression constructor requires structural value custody",
                        );
                    };
                    let node = self
                        .checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(*value);
                    let expression = node.expression;
                    // The producer includes contextual coercion. The authored
                    // expression's type may still describe its input carrier.
                    let reference = node.type_reference;
                    if !reference.is_valid() {
                        return unsupported("structural operand construction lost its type");
                    }
                    let mut prepared = prepare_value(
                        identities,
                        self.checked,
                        self.qualifications,
                        self.state,
                        facts::PlaceRoot::Expression(expression),
                        result.statement_index,
                        *value,
                        calls,
                        reference,
                        &bindings,
                        &self.prefix,
                        self.types,
                        self.reserved,
                    )?;
                    self.reserved.result_place(result.binding_ordinal)?;
                    for home in &mut prepared.storage {
                        if home.root == facts::PlaceRoot::Expression(expression) {
                            home.result_binding_ordinal = Some(result.binding_ordinal);
                        }
                    }
                    bindings.retain_completed_homes(&prepared.storage)?;
                    self.consumed.extend(prepared.consumed.iter().copied());
                    self.steps.push(Step::Value {
                        prepared,
                        prefix: self.prefix.clone(),
                    });
                    if results
                        .iter()
                        .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
                    {
                        return unsupported("structural operand result is established twice");
                    }
                    results.push((
                        result.binding_ordinal,
                        self.result(identities, result)?.into(),
                    ));
                }
                _ => {
                    return unsupported(
                        "structural expression schedule requires unsupported operand storage",
                    );
                }
            }
        }
        self.reserved.producer_place(producer.value)
    }

    fn result(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        binding: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    ) -> Result<super::super::super::StructuralOperationResult, LoweringError> {
        let declaration = crate::terminal_identities::structural_declaration_by_identity(
            self.types,
            identities,
            self.checked,
            binding.type_identity,
            "structural operand result lost its carrier",
        )?;
        let multiplicity = match binding.multiplicity {
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Linear => StructuralMultiplicity::Linear,
        };
        Ok(super::super::super::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: self.reserved.result_place(binding.binding_ordinal)?,
            structural_type: declaration.id,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        })
    }
}
