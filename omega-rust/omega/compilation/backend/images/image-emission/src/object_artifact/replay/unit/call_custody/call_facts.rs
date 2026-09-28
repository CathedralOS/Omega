//! What one internal Unit call custody check establishes before its roster
//! and arguments are checked: the inputs it reads, the call site's span, the
//! stack facts of a call with arguments, the callee ABI it is checked
//! against, the expected call plan, and the projection facts of its
//! structural arguments.

use super::{fixed_integer_abi_shape, unit_scalar_shape};
use crate::{ObjectError, ObjectScalarCallStack, ObjectUnitCallStack, ObjectUnitStack};
use machine_code::{MachineCodeFunction, SemanticCodeAttribution, StructuralReturnRecord};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget};
use target_operations::{CallSiteOwner, MixedStructuralScalarFunctionAbi, TerminalPsiProvenance};

/// Everything the check reads: the function and evidence the call lives in,
/// the callee's retained parameter facts, and the custody record under
/// validation.
#[derive(Clone, Copy)]
pub(super) struct CallInputs<'a> {
    pub(super) target: NativeTarget,
    pub(super) function: &'a MachineCodeFunction,
    pub(super) machine: MachineId,
    pub(super) provenance: &'a TerminalPsiProvenance,
    pub(super) function_bytes: &'a [u8],
    pub(super) attribution: &'a [SemanticCodeAttribution],
    pub(super) relocations: &'a [machine_code::InternalCallRelocation],
    pub(super) internal_unit_calls: &'a [machine_code::InternalUnitCallRecord],
    pub(super) parameter_homes: &'a [machine_code::UnitParameterHomeRecord],
    pub(super) validated_function_stack: Option<&'a ObjectUnitStack>,
    pub(super) validated_call_stack: Option<&'a ObjectUnitCallStack>,
    pub(super) validated_scalar_call_stack: Option<&'a ObjectScalarCallStack>,
    pub(super) callee_parameter_abi: Option<&'a machine_code::ParameterFunctionAbiRecord>,
    pub(super) callee_unit_parameters: &'a [machine_code::UnitParameterRecord],
    pub(super) callee_mixed_abi: Option<&'a MixedStructuralScalarFunctionAbi>,
    pub(super) callee_structural_return: Option<&'a StructuralReturnRecord>,
    pub(super) custody: &'a machine_code::InternalUnitCallRecord,
    pub(super) affine_cleanup: Option<&'a machine_code::UnitAffineCleanupRecord>,
    pub(super) fully_consumed_affine_parameter: bool,
}

/// The call site's span: the relocation that encodes the call, the end of the
/// call's bytes and of the relocation, and the linkage bytes the target's
/// call instruction pushes.
#[derive(Clone, Copy)]
pub(super) struct CallSpan<'a> {
    pub(super) relocation: &'a machine_code::InternalCallRelocation,
    pub(super) end: usize,
    pub(super) relocation_end: usize,
    pub(super) linkage_bytes: u32,
}

/// The stack facts of a call with arguments: the validated function stack,
/// the call-stack bytes every argument must record (the validated call
/// stack's transient bytes less the target's linkage bytes), and the owning
/// operation with its position in the function's provenance.
#[derive(Clone, Copy)]
pub(super) struct StackFacts<'a> {
    pub(super) validated_function_stack: &'a ObjectUnitStack,
    pub(super) expected_call_stack_bytes: u32,
    pub(super) operation: semantic_vocabulary::OperationId,
    pub(super) operation_position: usize,
}

/// Which callee ABI the call is checked against; at most one is retained.
#[derive(Clone, Copy)]
pub(super) enum CalleeAbi<'a> {
    /// A scalar parameter ABI whose Unit parameters are borrowed.
    Parameter(&'a machine_code::ParameterFunctionAbiRecord),
    /// A mixed structural/scalar ABI with a scalar result.
    Mixed(&'a MixedStructuralScalarFunctionAbi),
    /// A structural return that also takes scalar parameters or claim-free
    /// affine identity custody.
    MixedStructuralReturn(&'a StructuralReturnRecord),
    /// No retained ABI: the plan follows the arguments' own shapes.
    Untyped,
}

/// The projection facts of the call's structural arguments: which arguments
/// project a path, which are claim transfers, the exact projected affine
/// result the call settles into, and the parameter home a projected
/// argument reads.
pub(super) struct ProjectionFacts<'a> {
    pub(super) projected_argument_indexes: std::collections::BTreeSet<usize>,
    pub(super) transferred_argument_indexes: std::collections::BTreeSet<usize>,
    pub(super) projected_result: Option<&'a machine_code::InternalStructuralCallResult>,
    pub(super) projected_home: Option<&'a machine_code::UnitParameterHomeRecord>,
}

impl<'a> CallInputs<'a> {
    /// The call is well-formed enough to check: it belongs to `function` and
    /// its bytes, is authored, keeps every parameter home and argument source
    /// on the stack, and returns at most one of a scalar or structural result
    /// that no argument also names.
    pub(super) fn validate_call_shape(&self) -> Result<(), ObjectError> {
        let CallInputs {
            function,
            machine,
            function_bytes,
            parameter_homes,
            custody,
            ..
        } = *self;
        let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
        if function.machine != machine
            || function.bytes.as_slice() != function_bytes
            || !matches!(
                custody.source,
                machine_code::InternalUnitCallSource::Authored
            )
            || parameter_homes
                .iter()
                .any(|home| home.location.stack_byte_offset().is_none())
            || custody
                .arguments
                .iter()
                .any(|argument| argument.source_location.stack_byte_offset().is_none())
        {
            return Err(invalid());
        }
        if custody.result.is_some() && custody.structural_result.is_some() {
            return Err(invalid());
        }
        if custody.structural_result.as_ref().is_some_and(|result| {
            custody
                .arguments
                .iter()
                .any(|argument| argument.place == result.operation_result.place)
        }) {
            return Err(invalid());
        }
        Ok(())
    }

    /// The call site's span: the relocation encoding this call, exactly one
    /// of a Unit or scalar call stack, and the ends of the call's bytes and of
    /// the relocation.
    pub(super) fn call_span(&self) -> Result<CallSpan<'a>, ObjectError> {
        let CallInputs {
            target,
            relocations,
            validated_call_stack,
            validated_scalar_call_stack,
            custody,
            affine_cleanup,
            machine,
            ..
        } = *self;
        let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
        let Some(relocation) = relocations.iter().find(|relocation| {
            relocation.owner == custody.owner
                && relocation.target == custody.target
                && (relocation.unit_stack.is_some()
                    || (affine_cleanup.is_some()
                        && matches!(relocation.owner, CallSiteOwner::CleanupAction { .. })
                        && relocation.scalar_stack.is_some()))
        }) else {
            return Err(invalid());
        };
        if validated_call_stack.is_none() == validated_scalar_call_stack.is_none() {
            return Err(invalid());
        }
        let end = custody
            .code_offset
            .checked_add(custody.byte_count)
            .ok_or_else(invalid)?;
        let relocation_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
        let linkage_bytes = match target.architecture {
            Architecture::X86_64 => 8,
            Architecture::Aarch64 => 0,
        };
        Ok(CallSpan {
            relocation,
            end,
            relocation_end,
            linkage_bytes,
        })
    }

    /// Whether the call carries no scalar or structural arguments and no
    /// claim transfers.
    pub(super) fn is_argument_free(&self) -> bool {
        let custody = self.custody;
        custody.scalar_arguments.is_empty()
            && custody.arguments.is_empty()
            && custody.claim_transfers.is_empty()
    }

    /// The stack facts of a call with arguments: the validated function and
    /// call stacks, the call-stack bytes every argument must record, and the
    /// owning operation with its position in the function's provenance.
    pub(super) fn stack_facts(&self, span: &CallSpan<'_>) -> Result<StackFacts<'a>, ObjectError> {
        let CallInputs {
            provenance,
            validated_function_stack,
            validated_call_stack,
            custody,
            machine,
            ..
        } = *self;
        let CallSpan { linkage_bytes, .. } = *span;
        let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
        let validated_function_stack = validated_function_stack.ok_or_else(invalid)?;
        let validated_call_stack = validated_call_stack.ok_or_else(invalid)?;
        let expected_call_stack_bytes = validated_call_stack
            .transient_bytes
            .checked_sub(linkage_bytes)
            .ok_or_else(invalid)?;
        let CallSiteOwner::Operation(operation) = custody.owner else {
            return Err(invalid());
        };
        let operation_position = provenance
            .operations
            .iter()
            .position(|candidate| *candidate == operation)
            .ok_or_else(invalid)?;
        Ok(StackFacts {
            validated_function_stack,
            expected_call_stack_bytes,
            operation,
            operation_position,
        })
    }

    /// Which callee ABI the call is checked against: a structural return only
    /// counts when it also takes scalar parameters or claim-free affine
    /// identity custody, and at most one ABI may be retained.
    pub(super) fn callee_abi(&self) -> Result<CalleeAbi<'a>, ObjectError> {
        let CallInputs {
            callee_parameter_abi,
            callee_mixed_abi,
            callee_structural_return,
            machine,
            ..
        } = *self;
        let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
        let callee_mixed_structural_return = callee_structural_return.filter(|returned| {
        !returned.scalar_parameters.is_empty()
            || crate::object_artifact::replay::structural::return_record::has_claim_free_affine_identity_custody(returned)
    });
        if usize::from(callee_parameter_abi.is_some())
            + usize::from(callee_mixed_abi.is_some())
            + usize::from(callee_mixed_structural_return.is_some())
            > 1
        {
            return Err(invalid());
        }
        Ok(if let Some(abi) = callee_parameter_abi {
            CalleeAbi::Parameter(abi)
        } else if let Some(abi) = callee_mixed_abi {
            CalleeAbi::Mixed(abi)
        } else if let Some(returned) = callee_mixed_structural_return {
            CalleeAbi::MixedStructuralReturn(returned)
        } else {
            CalleeAbi::Untyped
        })
    }

    /// The call plan the target's native policy assigns to the callee's
    /// signature: its parameters from the retained ABI (or the arguments' own
    /// shapes), its result from the call's scalar or structural result.
    pub(super) fn expected_callee_plan(
        &self,
        callee_abi: CalleeAbi<'_>,
    ) -> Result<calling_conventions::CallPlan, ObjectError> {
        let CallInputs {
            target,
            callee_unit_parameters,
            callee_structural_return,
            custody,
            machine,
            ..
        } = *self;
        let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
        let expected_plan = calling_conventions::evaluate_call_plan(
            calling_conventions::CallingPolicy::native_for_target(target),
            &calling_conventions::CallSignature {
                parameters: match callee_abi {
                    CalleeAbi::Parameter(abi) => abi
                        .parameters
                        .iter()
                        .map(|parameter| {
                            unit_scalar_shape(parameter.scalar_type).ok_or_else(invalid)
                        })
                        .chain(
                            callee_unit_parameters
                                .iter()
                                .map(|parameter| Ok(parameter.shape)),
                        )
                        .collect::<Result<Vec<_>, _>>()?,
                    CalleeAbi::Mixed(abi) => abi
                        .scalar_parameters
                        .iter()
                        .map(|parameter| {
                            fixed_integer_abi_shape(parameter.scalar_type).ok_or_else(invalid)
                        })
                        .chain(
                            abi.structural_parameters
                                .iter()
                                .map(|parameter| Ok(parameter.shape)),
                        )
                        .collect::<Result<Vec<_>, _>>()?,
                    CalleeAbi::MixedStructuralReturn(returned) => returned
                        .scalar_parameters
                        .iter()
                        .map(|parameter| {
                            fixed_integer_abi_shape(parameter.scalar_type).ok_or_else(invalid)
                        })
                        .chain(
                            returned
                                .parameter_placements
                                .iter()
                                .map(|placement| Ok(placement.shape)),
                        )
                        .collect::<Result<Vec<_>, _>>()?,
                    CalleeAbi::Untyped => custody
                        .arguments
                        .iter()
                        .map(|argument| argument.shape)
                        .collect(),
                },
                result: if let Some(result) = custody.result {
                    let bytes = match result {
                        semantic_vocabulary::ScalarType::Boolean => 1,
                        semantic_vocabulary::ScalarType::Integer(integer) => {
                            integer.bits().div_ceil(8)
                        }
                        semantic_vocabulary::ScalarType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary32,
                        ) => 4,
                        semantic_vocabulary::ScalarType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary64,
                        ) => 8,
                    };
                    Some(match result {
                        semantic_vocabulary::ScalarType::IeeeFloat(_) => {
                            calling_conventions::ValueShape::float(bytes)
                        }
                        _ => calling_conventions::ValueShape::integer(
                            bytes,
                            bytes.next_power_of_two().min(8),
                        ),
                    })
                } else if custody.structural_result.is_some() {
                    callee_structural_return.map(|returned| returned.shape)
                } else {
                    None
                },
            },
        )
        .map_err(|_| invalid())?;
        Ok(expected_plan)
    }

    /// The projection facts of the call's structural arguments: which
    /// arguments project a path, which are claim transfers, the exact
    /// projected affine result the call settles into, and the single parameter
    /// home a projected argument reads (whose frame the function must reserve
    /// exactly when the function has no Unit continuations).
    pub(super) fn projection_facts(
        &self,
        stacks: &StackFacts<'a>,
    ) -> Result<ProjectionFacts<'a>, ObjectError> {
        let CallInputs {
            target,
            function,
            internal_unit_calls,
            parameter_homes,
            custody,
            affine_cleanup,
            machine,
            ..
        } = *self;
        let StackFacts {
            validated_function_stack,
            ..
        } = *stacks;
        let invalid = || ObjectError::InvalidInternalUnitCallEvidence(machine);
        let projected_argument_indexes = custody
            .arguments
            .iter()
            .enumerate()
            .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
            .collect::<std::collections::BTreeSet<_>>();
        let transferred_argument_indexes = custody
            .claim_transfers
            .iter()
            .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
            .collect::<std::collections::BTreeSet<_>>();
        let projected_result = crate::object_artifact::replay::structural::affine_projected_calls::exact_projected_affine_result(
        parameter_homes,
        internal_unit_calls,
        affine_cleanup,
    )
    .or_else(|| {
        let disposed = crate::object_artifact::replay::unit::continuations::completed_roots(
            parameter_homes,
            internal_unit_calls,
            &function.unit_continuations,
            function.unit_affine_cleanup.as_ref(),
        )?;
        crate::object_artifact::replay::unit::continuations::result_for_call(internal_unit_calls, &disposed, custody)
    });
        if custody
            .structural_result
            .as_ref()
            .is_some_and(|result| result.result_home.is_some())
            && projected_result.is_none()
        {
            return Err(invalid());
        }
        let projected_home = if projected_argument_indexes.is_empty()
            || projected_result.is_some_and(|result| {
                custody
                    .arguments
                    .iter()
                    .all(|argument| argument.place == result.operation_result.place)
            }) {
            None
        } else if !function.unit_continuations.is_empty() {
            let [argument] = custody.arguments.as_slice() else {
                return Err(invalid());
            };
            Some(
                parameter_homes
                    .iter()
                    .find(|home| home.place == argument.place)
                    .ok_or_else(invalid)?,
            )
        } else {
            let [home] = parameter_homes else {
                return Err(invalid());
            };
            if home.location.stack_byte_offset() != Some(0)
                || home.indirect
                    != matches!(
                        home.source.locations.as_slice(),
                        [calling_conventions::ValueLocation::Indirect { .. }]
                    )
            {
                return Err(invalid());
            }
            let caller_scalar_shapes = function.parameter_abi.as_ref().map_or_else(
                || Ok(Vec::new()),
                |abi| {
                    abi.parameters
                        .iter()
                        .map(|parameter| {
                            unit_scalar_shape(parameter.scalar_type).ok_or_else(invalid)
                        })
                        .collect::<Result<Vec<_>, _>>()
                },
            )?;
            let expected_caller_plan = calling_conventions::evaluate_call_plan(
                calling_conventions::CallingPolicy::native_for_target(target),
                &calling_conventions::CallSignature {
                    parameters: caller_scalar_shapes
                        .into_iter()
                        .chain(std::iter::once(home.shape))
                        .collect(),
                    result: None,
                },
            )
            .map_err(|_| invalid())?;
            if function
                .parameter_abi
                .as_ref()
                .is_some_and(|abi| abi.call_plan != expected_caller_plan)
                || expected_caller_plan.parameters.last() != Some(&home.source)
            {
                return Err(invalid());
            }
            let stored_bytes = if home.indirect {
                8
            } else {
                u32::from(home.shape.byte_size)
            };
            let expected_frame_bytes = match target.architecture {
                Architecture::X86_64 => stored_bytes.next_multiple_of(16),
                Architecture::Aarch64 => stored_bytes
                    .next_multiple_of(8)
                    .checked_add(8)
                    .map(|bytes| bytes.next_multiple_of(16))
                    .ok_or_else(invalid)?,
            };
            if validated_function_stack.frame_bytes != expected_frame_bytes {
                return Err(invalid());
            }
            Some(home)
        };
        Ok(ProjectionFacts {
            projected_argument_indexes,
            transferred_argument_indexes,
            projected_result,
            projected_home,
        })
    }
}
