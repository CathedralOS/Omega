//! Selected operator applications only an ordinary body admits: the
//! structural-operand and structural-result operator realizations and the
//! selected IEEE FMA. Every checked call shape a composed state shares emits
//! through `operation_frame` instead.

use super::super::parameters::{lower_structural_arguments, validate_transfer_shape};
use super::MachineEmission;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::unit::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, LoweringError, Operation,
    OperationKind, OperationResult, ScalarType, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, emit_direct_expression, lookup_machine_id,
    lookup_type_id, lower_checked_scalar_expression, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};

impl MachineEmission<'_> {
    pub(super) fn selected_operator_structural_scalar_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            coordinate,
            result,
            realization_machine,
            realization_state,
            scalar_arguments,
            structural_arguments,
            ..
        } = operation
        else {
            unreachable!("dispatched selected_operator_structural_scalar_call")
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.scalar_result_values.len())
        {
            return unsupported(
                "selected structural Unit operator result binding ordinal drifted from source order",
            );
        }
        let realizations = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .filter(|target| {
                target.machine == *realization_machine && target.state == *realization_state
            })
            .collect::<Vec<_>>();
        let [target] = realizations.as_slice() else {
            return unsupported("selected structural Unit operator target is absent or ambiguous");
        };
        validate_transfer_shape(
            structural_arguments,
            &[],
            self.parameters,
            &[],
            &[],
            &target.structural_parameters,
            self.type_ids,
            self.structural_types,
            &[],
            &self.primitive_local_places,
            None,
        )?;
        if scalar_arguments.len() != target.scalar_parameters.len() {
            return unsupported(
                "selected structural Unit operator scalar argument count disagrees with its realization",
            );
        }
        let source_types = self
            .scalar_result_values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let scalar_arguments = scalar_arguments
            .iter()
            .zip(&target.scalar_parameters)
            .map(|(argument, target)| {
                let argument = lower_checked_scalar_expression(argument)?;
                if direct_expression_contains_short_circuit(&argument) {
                    return unsupported(
                        "selected structural Unit operator arguments do not yet admit short-circuit control",
                    );
                }
                let target_type = terminal_scalar_type(target.primitive_type)?;
                if argument.scalar_type() != target_type {
                    return unsupported(
                        "selected structural Unit operator scalar argument type disagrees with its realization",
                    );
                }
                validate_direct_parameter_types(&argument, &source_types)?;
                Ok(emit_direct_expression(
                    &argument,
                    &self.scalar_result_values,
                    &mut self.next_value_identity,
                    &mut self.operations,
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let arguments = lower_structural_arguments(
            structural_arguments,
            self.parameters,
            &[],
            &[],
            &[],
            &self.primitive_local_places,
        )?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(self.next_value_identity),
            scalar_type: terminal_scalar_type(result.primitive_type)?,
        };
        self.next_value_identity =
            self.next_value_identity
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "selected structural scalar result identity space is exhausted",
                ))?;
        let operation_id = self.operations.allocate();
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural scalar statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural scalar call ordinal exceeds usize",
                    )
                })?,
            },
            None,
            operation_id,
            *realization_machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id,
            result: OperationResult::Scalar(value),
            kind: OperationKind::CallStructuralScalar {
                callee: lookup_machine_id(self.machine_ids, *realization_machine)?,
                arguments: scalar_arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        self.scalar_result_values.push(value);
        Ok(None)
    }

    pub(super) fn selected_operator_structural_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
            coordinate,
            result,
            realization_machine,
            realization_state,
            scalar_arguments,
            structural_arguments,
            discard_result_on_return,
            ..
        } = operation
        else {
            unreachable!("dispatched selected_operator_structural_call")
        };
        let realizations = checked
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines
            .iter()
            .filter(|target| {
                target.machine == *realization_machine && target.state == *realization_state
            })
            .collect::<Vec<_>>();
        let [target] = realizations.as_slice() else {
            return unsupported(
                "selected structural-result Unit operator target is absent or ambiguous",
            );
        };
        validate_transfer_shape(
            structural_arguments,
            &[],
            self.parameters,
            &[],
            &[],
            std::slice::from_ref(&target.structural_parameter),
            self.type_ids,
            self.structural_types,
            &[],
            &self.primitive_local_places,
            None,
        )?;
        if scalar_arguments.len() != target.scalar_parameters.len() {
            return unsupported(
                "selected structural-result scalar argument count disagrees with its realization",
            );
        }
        let source_types = self
            .scalar_result_values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let scalar_arguments = scalar_arguments
            .iter()
            .zip(&target.scalar_parameters)
            .map(|(argument, target)| {
                let argument = lower_checked_scalar_expression(argument)?;
                if direct_expression_contains_short_circuit(&argument) {
                    return unsupported(
                        "selected structural-result arguments do not admit short-circuit control",
                    );
                }
                let target_type = terminal_scalar_type(target.primitive_type)?;
                if argument.scalar_type() != target_type {
                    return unsupported(
                        "selected structural-result scalar argument type disagrees with its realization",
                    );
                }
                validate_direct_parameter_types(&argument, &source_types)?;
                Ok(emit_direct_expression(
                    &argument,
                    &self.scalar_result_values,
                    &mut self.next_value_identity,
                    &mut self.operations,
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let arguments = lower_structural_arguments(
            structural_arguments,
            self.parameters,
            &[],
            &[],
            &[],
            &self.primitive_local_places,
        )?;
        let operation_id = self.operations.allocate();
        let result_place = place_id(allocate_dense(&mut self.next_place)?);
        let result_type = lookup_type_id(self.type_ids, &result.type_identity)?;
        let result_declaration = StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id,
                structural_type: result_type,
            },
        };
        self.operations.record_source_call(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural-result statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected structural-result call ordinal exceeds usize",
                    )
                })?,
            },
            None,
            operation_id,
            *realization_machine,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation_id,
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: result_place,
                structural_type: result_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::CallStructuralWithScalarArguments {
                callee: lookup_machine_id(self.machine_ids, *realization_machine)?,
                arguments: scalar_arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: arguments,
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        self.structural_result_places
            .push((result_declaration, *discard_result_on_return));
        Ok(None)
    }

    pub(super) fn selected_ieee_float_fused_multiply_add(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
            coordinate,
            result,
            requirement_operator,
            provider_plan_report_fingerprint,
            provider_plan_commitment,
            format,
            operands,
        } = operation
        else {
            unreachable!("dispatched selected_ieee_float_fused_multiply_add")
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.scalar_result_values.len())
        {
            return unsupported(
                "selected IEEE FMA result binding ordinal drifted from source order",
            );
        }
        let result_type = terminal_scalar_type(result.primitive_type)?;
        if result_type != ScalarType::IeeeFloat(*format) {
            return unsupported("selected IEEE FMA result type disagrees with its exact format");
        }
        let [left, right, addend] = operands.as_slice() else {
            return unsupported("selected IEEE FMA must retain exactly three operands");
        };
        let source_types = self
            .scalar_result_values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>();
        let mut lower_operand = |operand: &CheckedScalarExpression| {
            let operand = lower_checked_scalar_expression(operand)?;
            if direct_expression_contains_short_circuit(&operand) {
                return unsupported(
                    "selected IEEE FMA operands do not admit short-circuit control",
                );
            }
            if operand.scalar_type() != result_type {
                return unsupported("selected IEEE FMA operand type disagrees with its result");
            }
            validate_direct_parameter_types(&operand, &source_types)?;
            Ok(emit_direct_expression(
                &operand,
                &self.scalar_result_values,
                &mut self.next_value_identity,
                &mut self.operations,
            ))
        };
        let left = lower_operand(left)?;
        let right = lower_operand(right)?;
        let addend = lower_operand(addend)?;
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(self.next_value_identity),
            scalar_type: result_type,
        };
        self.next_value_identity =
            self.next_value_identity
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "selected IEEE FMA result value identity space is exhausted",
                ))?;
        let operation = self.operations.allocate();
        self.operations.record_selected_ieee_float_fma(
            SourceCallCoordinate {
                state: plan.state,
                statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                    LoweringError::Unsupported(
                        "selected IEEE FMA statement coordinate exceeds usize",
                    )
                })?,
                call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                    LoweringError::Unsupported("selected IEEE FMA call ordinal exceeds usize")
                })?,
            },
            operation,
            *requirement_operator,
            *provider_plan_report_fingerprint,
            *provider_plan_commitment,
            *format,
        )?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: terminal_psi::OperationResult::Scalar(value),
            kind: OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
            },
        });
        self.scalar_result_values.push(value);
        Ok(None)
    }
}
