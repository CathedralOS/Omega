//! Consume the common formal-order schedule in an expression continuation.
//! Checked result ordinals join intermediate owners to their consumers; scalar
//! argument positions remain live across intervening structural producers.
use super::*;
use crate::emission::operation_emission::buffer::StructuralBinding;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::unit::attached_unit::argument_schedule;
use crate::unit::attached_unit::bodies::{UnitBody, UnitPlans};

impl Storage {
    pub(super) fn reserve_value(
        &mut self,
        checked: &CheckedTrees,
        root: checked_trees::CheckedStructuralValueHandle,
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
        }
        self.reserve_calls(
            checked,
            checked
                .facts
                .values
                .structural_values
                .calls_for_root(root)
                .unwrap_or(&[]),
            next_place,
        )
    }

    fn reserve_calls(
        &mut self,
        checked: &CheckedTrees,
        calls: &[checked_trees::CheckedStructuralValueCall],
        next_place: &mut u64,
    ) -> Result<(), LoweringError> {
        for call in calls {
            for (index, operation) in call
                .operands()
                .iter()
                .chain(std::iter::once(call.operation()))
                .enumerate()
            {
                let result =
                    operation
                        .structural_result_binding()
                        .ok_or(LoweringError::Unsupported(
                            "structural operand preparation has no result binding",
                        ))?;
                let place = if index == call.operands().iter().count() {
                    self.place(call.value)?
                } else if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value,
                    calls,
                    ..
                } = operation
                {
                    self.reserve_value(checked, *value, next_place)?;
                    self.reserve_calls(checked, calls, next_place)?;
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
}

impl Preparation<'_> {
    pub(super) fn prepare_calls(
        &mut self,
        producer: &checked_trees::CheckedStructuralValueCall,
    ) -> Result<PlaceId, LoweringError> {
        let (machine, source) = crate::declarations::state_declaration(self.checked, self.state)?;
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
        let mut results: Vec<(u32, StructuralBinding)> = Vec::new();
        for step in steps {
            match step {
                argument_schedule::Step::Argument { operation, ordinal } => {
                    let call = operations[operation]
                        .call()
                        .ok_or(LoweringError::Unsupported(
                            "argument schedule lost its call",
                        ))?;
                    let target = UnitBody::find(
                        UnitPlans::published(&self.checked.facts.flow.terminal_unit_effects),
                        call.target.machine,
                    )?
                    .entry()?;
                    let parameter =
                        target
                            .scalar_parameters
                            .get(ordinal)
                            .ok_or(LoweringError::Unsupported(
                                "argument schedule lost its scalar formal",
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
                    positions[operation].push(self.prefix.len());
                    self.steps.push(Step::Argument {
                        coordinate: call.coordinate,
                        ordinal,
                        argument: argument.clone(),
                        prefix: self.prefix.clone(),
                    });
                    self.prefix
                        .push(super::super::terminal_scalar_type(parameter.primitive_type)?.into());
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
                    let result = self.result(binding)?;
                    let mut prepared = super::super::unit_operations::prepare(
                        self.checked,
                        machine.symbol,
                        &caller,
                        operations[operation],
                        self.bindings,
                        &self.prefix,
                        self.types,
                        Some(result.clone()),
                        &results,
                    )?;
                    prepared.call.arguments = positions[operation]
                        .iter()
                        .zip(&prepared.argument_types)
                        .map(
                            |(position, scalar_type)| LoweredDirectExpression::Parameter {
                                position: *position,
                                scalar_type: scalar_type.scalar_type,
                            },
                        )
                        .collect();
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
                argument_schedule::Step::Constructor(operation) => {
                    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        result,
                        value,
                        calls,
                        ..
                    } = operations[operation]
                    else {
                        return unsupported(
                            "scalar expression constructor requires structural value custody",
                        );
                    };
                    let expression = self
                        .checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(*value)
                        .expression;
                    let reference = validation::expression_result_type_reference(
                        &self.checked.typed,
                        machine,
                        source,
                        expression,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "structural operand construction lost its type",
                    ))?;
                    let prepared = prepare_value(
                        self.checked,
                        self.qualifications,
                        self.state,
                        facts::PlaceRoot::Expression(expression),
                        result.statement_index,
                        *value,
                        calls,
                        reference,
                        self.bindings,
                        &self.prefix,
                        self.types,
                        self.reserved,
                    )?;
                    self.reserved.result_place(result.binding_ordinal)?;
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
                    results.push((result.binding_ordinal, self.result(result)?.into()));
                }
                _ => {
                    return unsupported(
                        "structural expression schedule requires unsupported operand storage",
                    );
                }
            }
        }
        self.reserved.place(producer.value)
    }

    fn result(
        &self,
        binding: &checked_trees::CheckedUnitStructuralResultBindingPlan,
    ) -> Result<super::super::super::StructuralOperationResult, LoweringError> {
        let declaration = self
            .types
            .iter()
            .find(|declaration| declaration.identity == binding.type_identity)
            .ok_or(LoweringError::Unsupported(
                "structural operand result lost its carrier",
            ))?;
        let multiplicity = match binding.multiplicity {
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Linear => {
                return unsupported("scalar structural value cannot acquire linear claims");
            }
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
