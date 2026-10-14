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
            if !self.places.iter().any(|(candidate, _)| *candidate == value) {
                self.places
                    .push((value, place_id(allocate_dense(next_place)?)));
            }
            let node = checked.facts.values.structural_values.nodes.get(value);
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
                        let Some(range) = computations::subslices::range(argument) else {
                            continue;
                        };
                        if !self
                            .ranges
                            .iter()
                            .any(|slot| slot.expression == range.expression)
                        {
                            self.ranges.push(computations::subslices::reserve(
                                identities,
                                checked,
                                state,
                                planned_call.coordinate.statement_index,
                                argument,
                                types,
                                next_place,
                            )?);
                        }
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
                    let prepared = super::super::unit_operations::prepare(
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
