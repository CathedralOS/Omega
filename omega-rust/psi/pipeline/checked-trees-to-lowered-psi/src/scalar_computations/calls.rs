//! One authored formal stream interleaves scalar operands and array construction.
//! An array's leaf staging is retired at its constructor, leaving earlier scalar
//! actuals intact. CPS attaches this stream only to the selected expression path.

use super::*;
use arena::HandleSpan;
use checked_trees::CheckedScalarComputationStructuralArgument;

enum Operand {
    Scalar(Argument),
    Array {
        slot: arrays::Slot,
        leaves: Vec<Argument>,
    },
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call(
        &mut self,
        source_call: Handle<checked_trees::FlowCallFact>,
        target_machine: symbols::SymbolHandle,
        target_state: symbols::SymbolHandle,
        call_ordinal: u32,
        arguments: HandleSpan<Computation>,
        structural: HandleSpan<CheckedScalarComputationStructuralArgument>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let control = &self.checked.facts.flow.control;
        if !control.calls.is_valid(source_call) {
            return unsupported("scalar computation lost its exact checked invocation");
        }
        let source = control.calls.get(source_call);
        let state = control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == self.machine && state.state_symbol == site.state)
            .ok_or(LoweringError::Unsupported(
                "scalar computation invocation state is absent",
            ))?;
        if !control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .any(|call| std::ptr::eq(call, source))
            || source.statement_index != site.statement as usize
            || source.call_ordinal != call_ordinal as usize
            || source.target_symbol != target_state
        {
            return unsupported("scalar computation invocation coordinate disagrees");
        }
        let plans = &self.checked.facts.values.scalar_computations;
        let structural =
            plans
                .structural_arguments
                .span(structural)
                .ok_or(LoweringError::Unsupported(
                    "computed structural arguments have a stale span",
                ))?;
        let structural_arguments = structural_arguments::lower(
            self.checked,
            target_machine,
            target_state,
            structural,
            site.bindings,
            &self.arrays,
        )?;
        let scalars = plans
            .operands
            .span(arguments)
            .ok_or(LoweringError::Unsupported(
                "scalar computation call has an invalid argument span",
            ))?;
        let callee = crate::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            self.checked,
            target_machine,
        )?;
        let (_, source_state) =
            crate::scalar_source_custody::authored_state(self.checked, target_state)?;
        let parameters = callee.structural_parameters();
        let mut scalar_ordinal = 0usize;
        let mut structural_ordinal = 0usize;
        let mut operands = Vec::new();
        for position in 0..self.checked.state_parameters(source_state).len() {
            if let Some(parameter) = parameters.get(structural_ordinal)
                && parameter.position as usize == position
            {
                let argument =
                    structural
                        .get(structural_ordinal)
                        .ok_or(LoweringError::Unsupported(
                            "computed call is missing its structural actual",
                        ))?;
                if let CheckedScalarComputationStructuralArgument::Array {
                    expression,
                    elements,
                    ..
                } = argument
                {
                    let slot = self
                        .arrays
                        .iter()
                        .find(|slot| slot.expression == *expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed array has no reserved structural result",
                        ))?
                        .clone();
                    let leaves = plans
                        .operands
                        .span(*elements)
                        .ok_or(LoweringError::Unsupported(
                            "computed array elements have a stale span",
                        ))?
                        .iter()
                        .copied()
                        .map(Argument::Computation)
                        .collect();
                    operands.push(Operand::Array { slot, leaves });
                }
                structural_ordinal += 1;
            } else {
                let scalar = scalars
                    .get(scalar_ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "computed call has no scalar actual at its formal position",
                    ))?;
                operands.push(Operand::Scalar(Argument::Computation(*scalar)));
                scalar_ordinal += 1;
            }
        }
        if scalar_ordinal != scalars.len() || structural_ordinal != structural.len() {
            return unsupported("computed call formal roster is incomplete");
        }
        let mut prefixes = vec![input_types.to_vec()];
        for operand in &operands {
            let mut prefix = prefixes
                .last()
                .ok_or(LoweringError::Unsupported(
                    "computed call lost its input prefix",
                ))?
                .clone();
            if let Operand::Scalar(argument) = operand {
                prefix.push(self.argument_type(argument, site, input_types)?);
            }
            prefixes.push(prefix);
        }
        let call_types = prefixes.last().ok_or(LoweringError::Unsupported(
            "computed call lost its completed prefix",
        ))?;
        let call = lower_scalar_call(
            self.checked,
            self.qualifications,
            self.machine,
            site.state,
            site.statement,
            target_machine,
            target_state,
            call_ordinal,
            result_type,
            call_types,
            parameters_for_actuals(call_types, input_types.len()),
            structural_arguments,
            ScalarCallCrashScope::Arguments,
        )?;
        if self.calls.contains(&call.source_coordinate) {
            return unsupported("scalar computation repeats a call occurrence");
        }
        self.calls.push(call.source_coordinate);
        let mut continuation = self.binding(
            call_types,
            input_types.len(),
            target,
            LoweredScalarBinding::DirectCall(call),
        );
        for (ordinal, operand) in operands.iter().enumerate().rev() {
            let prefix = &prefixes[ordinal];
            continuation = match operand {
                Operand::Scalar(argument) => {
                    self.argument(argument, prefix, continuation, site, active)?
                }
                Operand::Array { slot, leaves } => {
                    let mut leaf_types = prefix.clone();
                    for leaf in leaves {
                        leaf_types.push(self.argument_type(leaf, site, input_types)?);
                    }
                    let constructor = self.push(LoweredScalarBranchState {
                        parameter_types: leaf_types.clone(),
                        bindings: Vec::new(),
                        structural_effects: vec![LoweredScalarEffect::EstablishScalarArray(
                            LoweredScalarArrayConstruction {
                                place: slot.place,
                                structural_type: slot.structural_type,
                                elements: parameters_for_actuals(&leaf_types, prefix.len()),
                            },
                        )],
                        terminator: LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            target: continuation,
                            arguments: super::parameters(prefix),
                            structural_arguments: Vec::new(),
                        },
                    });
                    self.sequence(leaves, prefix, constructor, site, active)?
                }
            };
        }
        Ok(continuation)
    }
}

fn parameters_for_actuals(
    types: &[QualifiedScalarType],
    prefix: usize,
) -> Vec<LoweredDirectExpression> {
    super::parameters(types).into_iter().skip(prefix).collect()
}
