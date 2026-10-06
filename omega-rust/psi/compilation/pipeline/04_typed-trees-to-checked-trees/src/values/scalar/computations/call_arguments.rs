use super::{Builder, CheckedScalarExpressionRole, ExpressionHandle, SymbolHandle};
use crate::values::scalar::call_lowering::call_is_boundary;

impl Builder<'_, '_> {
    pub(super) fn record_call_arguments(
        &mut self,
        statement: u32,
        call_ordinal: u32,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
    ) {
        let Some(parameters) = crate::semantic::calls::call_target_parameters(self.program, target)
        else {
            return;
        };
        let explicit_parameters = parameters.iter();
        if parameters.len() != arguments.len() {
            return;
        }
        let boundary = call_is_boundary(self.program, target);
        let mut scalar_ordinal = 0u32;
        for (argument, parameter) in arguments.iter().zip(explicit_parameters) {
            // An erased parameter position has no scalar ordinal: the callee
            // plan (`execution::unit::calls`) skips it the same way.
            match crate::execution::terminal_unit::strips_erased_parameter(parameter) {
                Some(true) => continue,
                Some(false) => {}
                None => return,
            }
            let Some(primitive_type) = self
                .program
                .primitive_type_reference(parameter.type_reference)
            else {
                // The shared value owner retains construction and copying at
                // their exact argument occurrence. Lending admission remains
                // separate; no source-shape roster decides what a value is.
                if !parameter.is_const {
                    self.retain_structural_argument(*argument, parameter.type_reference);
                }
                continue;
            };
            if parameter.is_const
                || (parameter.is_mutable
                    && crate::values::mutable_scalar_parameter_type(self.program, parameter)
                        .is_none())
            {
                return;
            }
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal,
                    argument_ordinal: scalar_ordinal,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal,
                    argument_ordinal: scalar_ordinal,
                }
            };
            self.record_root(self.pure, statement, role, *argument, primitive_type);
            let Some(next) = scalar_ordinal.checked_add(1) else {
                return;
            };
            scalar_ordinal = next;
        }
    }
}

// Call operands have one evaluator regardless of whether the eventual invocation
// enters a checked body or a boundary. The exact formal roster determines the
// scalar/structural lanes; the producer retains authored order and loan custody.
impl Builder<'_, '_> {
    pub(super) fn call_operands(
        &mut self,
        source_call: arena::Handle<checked_trees::FlowCallFact>,
        arguments: &[ExpressionHandle],
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<(
        arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
        arena::HandleSpan<checked_trees::CheckedScalarComputationStructuralArgument>,
        arena::HandleSpan<checked_trees::CheckedScalarComputationTemporaryDiscard>,
    )> {
        use typed_trees::types::TypeReferenceNode;
        if arguments.len() != parameters.len()
            || parameters.iter().any(|parameter| {
                parameter.is_const
                    || (parameter.is_mutable
                        && self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                        && crate::values::mutable_scalar_parameter_type(self.program, parameter)
                            .is_none())
            })
        {
            return None;
        }
        let mut computed_arguments = Vec::with_capacity(arguments.len());
        let mut structural_arguments = Vec::new();
        let mut temporary_discards = Vec::new();
        for (argument, parameter) in arguments.iter().zip(parameters) {
            // An erased parameter position consumes its authored
            // argument as proof material only: no scalar operand and
            // no structural argument, matching the callee's stripped
            // signature plan (`execution::unit::calls::signatures`).
            if crate::execution::terminal_unit::strips_erased_parameter(parameter)? {
                continue;
            }
            if let Some(primitive_type) = self
                .program
                .primitive_type_reference(parameter.type_reference)
            {
                let root = self.expression(*argument, primitive_type)?;
                self.plans.nodes.get_mut(root).authored_root = *argument;
                computed_arguments.push(root);
            } else {
                let state = crate::semantic::calls::find_state_in_machine(
                    self.program,
                    self.machine,
                    self.state,
                )?;
                let mut selectors = Vec::new();
                if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    *argument,
                ) {
                    for segment in place.segments {
                        if let facts::PlaceSegment::Index { expression } = segment {
                            let reference = crate::flow::expression_type_reference_in_state(
                                self.program,
                                self.state,
                                self.statement_index,
                                expression,
                            )?;
                            let primitive = self.program.primitive_type_reference(reference)?;
                            let value = self.expression(expression, primitive)?;
                            self.plans.nodes.get_mut(value).authored_root = expression;
                            selectors.push((
                                expression,
                                checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(
                                    value,
                                ),
                            ));
                        }
                    }
                }
                let argument_plan = crate::execution::terminal_unit::structural_computation_argument(
                            self.program,
                            self.borrow,
                            self.machine,
                            state,
                            self.flow.control.calls.get(source_call),
                            *argument,
                            parameter,
                    Some(&|expression| selectors.iter().find_map(|(source, value)| (*source == expression).then(|| value.clone()))),
                        )
                        .or_else(|| {
                            let argument_ordinal = structural_arguments.len();
                            crate::execution::terminal_unit::calls::subslice_computation_argument(
                                self.program, self.operators, self.pure, self.plans, self.borrow,
                                self.machine, state, self.flow.control.calls.get(source_call),
                                *argument, parameter, u32::try_from(argument_ordinal).ok()?,
                            )
                        })
                        .or_else(|| {
                            // A `"literal"` actual loans a byte sequence the
                            // callee reads as a borrowed view; places alone
                            // cannot name it, so the call argument reuses the
                            // statement route's literal carrier.
                            crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                                self.program,
                                parameter.type_reference,
                                *argument,
                            )
                        })
                        .or_else(|| {
                            if parameter.is_mutable {
                                return None;
                            }
                            let value = self.retain_structural_argument(*argument, parameter.type_reference)?;
                            let expression = self.structural_values.nodes.get(value).expression;
                            let (access, referent) = match self.program.type_reference_table.type_reference(parameter.type_reference) {
                                TypeReferenceNode::Reference { access: language_semantics::ReferenceAccess::Shared, referee, .. } =>
                                    (checked_trees::CheckedStructuralAccess::SharedBorrow, *referee),
                                TypeReferenceNode::Reference { .. } => return None,
                                _ => (checked_trees::CheckedStructuralAccess::Owned, parameter.type_reference),
                            };
                            if access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                                // Retention only supplies storage. The permission pass owns
                                // the loan's exact consumer and dying continuation; carry its
                                // settlement into the scalar call instead of inferring a drop
                                // later from the reference formal or constructor spelling.
                                crate::execution::terminal_unit::calls::validate_shared_temporary(
                                    self.program, self.flow, self.machine, self.state,
                                    self.flow.control.calls.get(source_call), expression,
                                )?;
                                match self.program.type_multiplicity(referent) {
                                    language_semantics::Multiplicity::Affine => {
                                        temporary_discards.push(checked_trees::CheckedScalarComputationTemporaryDiscard {
                                            argument_ordinal: u32::try_from(structural_arguments.len()).ok()?,
                                        });
                                    }
                                    language_semantics::Multiplicity::Unrestricted => {}
                                    _ => return None,
                                }
                            }
                            // The retained root still owns the complete declared
                            // type matched to this formal. The operand names its
                            // storage carrier; qualifications are not type rows.
                            let contract = validation::structural_result_contract(self.program, referent).ok()?;
                            Some(checked_trees::CheckedUnitStructuralArgumentPlan {
                                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                    expression,
                                },
                                path: Vec::new(),
                                type_identity: self.program
                                    .normalized_type_identity(contract.carrier)
                                    .into_string(),
                                access,
                            })
                        });
                structural_arguments.push(
                    checked_trees::CheckedScalarComputationStructuralArgument::Place(
                        argument_plan?,
                    ),
                );
            }
        }
        let arguments = self.plans.operands.insert_many(computed_arguments);
        let structural_arguments = self
            .plans
            .structural_arguments
            .insert_many(structural_arguments);
        temporary_discards.reverse();
        let temporary_discards = self
            .plans
            .temporary_discards
            .insert_many(temporary_discards);

        Some((arguments, structural_arguments, temporary_discards))
    }
}
