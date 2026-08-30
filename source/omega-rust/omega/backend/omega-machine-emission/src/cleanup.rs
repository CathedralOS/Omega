use omega_assigned_target_operations::{
    AssignedBooleanControl, AssignedConditionalBooleanArm, AssignedFunction, AssignedOperation,
    AssignedUnitOperation,
};
use omega_calling_conventions::ValueLocation;
use omega_machine_code::{
    Aarch64ReturnLinkEvidence, InternalCallRelocation, InternalUnitCallRecord, MachineCodeFunction,
    ScalarCallStackEvidence, ScalarCleanupPreservationEvidence, ScalarConditionalBranchEvidence,
    ScalarConditionalCondition, ScalarControlAffineCleanupRecord, ScalarControlFlowEvidence,
    SemanticCodeAttribution, SemanticCodeSite, StackAdjustmentPair,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;

use super::{
    EmissionError, aarch64_unit_stack_access, append_aarch64_instructions,
    collect_scalar_stack_evidence, emit_aarch64_adjust_sp, emit_aarch64_boolean_condition_value,
    emit_aarch64_boolean_control, emit_aarch64_condition_load, emit_aarch64_unit_call,
    emit_function, emit_x86_64_adjust_sp, emit_x86_64_boolean_condition_value,
    emit_x86_64_boolean_control, emit_x86_64_parameter_return, emit_x86_64_stack_load,
    emit_x86_64_stack_store, emit_x86_64_unit_call, linear_boolean_expression,
};

pub(super) fn emit_scalar_return_with_cleanup(
    function: &AssignedFunction,
    scalar: &AssignedOperation,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    call_plan: &omega_calling_conventions::CallPlan,
    structural_parameters: &[omega_target_operations::TargetStructuralParameter],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    psi_edge: psi_core::EdgeId,
    psi: psi_terminal::TerminalPsiIdentity,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<MachineCodeFunction, EmissionError> {
    if cleanup_actions.is_empty()
        || cleanup_actions.len() != structural_parameters.len()
        || call_plan.parameters.len() < structural_parameters.len()
        || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
            .iter()
            .zip(structural_parameters)
            .any(|(placement, parameter)| placement != &parameter.placement)
        || structural_parameters
            .iter()
            .rev()
            .zip(cleanup_actions)
            .any(|(parameter, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != parameter.place
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place != parameter.place
                        || cleanup.structural_type != parameter.structural_type
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
        || scalar_operation_edge(scalar) != Some(psi_edge)
    {
        return Err(EmissionError::UnsupportedScalarCleanup);
    }
    let mut emitted = emit_function(
        &AssignedFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: function.provenance.clone(),
            operation: scalar.clone(),
        },
        psi,
        target,
        functions,
    )?;
    let scalar_control_flow = emitted
        .scalar_stack
        .as_ref()
        .map(|stack| stack.control_flow.clone())
        .ok_or(EmissionError::UnsupportedScalarCleanup)?;
    if emitted.unit_stack.is_some()
        || emitted.unit_affine_cleanup.is_some()
        || emitted.scalar_affine_cleanup.is_some()
        || !emitted.internal_calls.is_empty()
        || !emitted.internal_unit_calls.is_empty()
        || emitted.structural_return.is_some()
        || !matches!(
            &scalar_control_flow,
            ScalarControlFlowEvidence::Linear
                | ScalarControlFlowEvidence::BooleanSharedConvergence { .. }
        )
    {
        return Err(EmissionError::UnsupportedScalarCleanup);
    }
    match target.architecture {
        Architecture::X86_64 if emitted.bytes.pop() == Some(0xc3) => {}
        Architecture::Aarch64
            if emitted.bytes.len() >= 4
                && emitted.bytes.split_off(emitted.bytes.len() - 4)
                    == 0xd65f_03c0_u32.to_le_bytes() => {}
        _ => return Err(EmissionError::UnsupportedScalarCleanup),
    }
    let cleanup_offset = emitted.bytes.len();
    let mut internal_unit_calls = Vec::new();
    let (frame_allocation_byte_count, result_store_offset, aarch64_link_store_offset) =
        match target.architecture {
            Architecture::X86_64 => {
                // Keep the ABI result out of every cleanup callee's caller-clobbered
                // register set. A 16-byte lifetime frame also preserves the entry
                // stack residue assumed by the existing Unit-call emitter.
                emit_x86_64_adjust_sp(&mut emitted.bytes, 16, false);
                let result_store_offset = emitted.bytes.len();
                emit_x86_64_stack_store(&mut emitted.bytes, 0, 0);
                (
                    result_store_offset - cleanup_offset,
                    result_store_offset,
                    None,
                )
            }
            Architecture::Aarch64 => {
                let mut instructions = Vec::new();
                emit_aarch64_adjust_sp(&mut instructions, 16, false)?;
                let result_store_offset = emitted.bytes.len() + instructions.len() * 4;
                instructions.push(aarch64_unit_stack_access(0xf900_0000, 0, 0, 8)?);
                let link_store_offset = emitted.bytes.len() + instructions.len() * 4;
                instructions.push(aarch64_unit_stack_access(0xf900_0000, 30, 8, 8)?);
                append_aarch64_instructions(&mut emitted.bytes, instructions);
                (4, result_store_offset, Some(link_store_offset))
            }
        };
    for (ordinal, action) in cleanup_actions.iter().enumerate() {
        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            continue;
        };
        if !executable_nominal_cleanup(cleanup, functions)? {
            continue;
        }
        let action_ordinal =
            u32::try_from(ordinal).map_err(|_| EmissionError::UnsupportedScalarCleanup)?;
        let owner = CallSiteOwner::CleanupAction {
            edge: psi_edge,
            action_ordinal,
        };
        let code_offset = emitted.bytes.len();
        match target.architecture {
            Architecture::X86_64 => {
                emit_x86_64_unit_call(
                    &mut emitted.bytes,
                    owner,
                    cleanup.cleanup_machine,
                    &[],
                    target,
                    &[],
                    &mut emitted.internal_calls,
                )?;
            }
            Architecture::Aarch64 => {
                emit_aarch64_unit_call(
                    &mut emitted.bytes,
                    owner,
                    cleanup.cleanup_machine,
                    &[],
                    &[],
                    &mut emitted.internal_calls,
                )?;
            }
        }
        let relocation = emitted
            .internal_calls
            .last_mut()
            .expect("cleanup call emission retains its relocation");
        let unit_stack = relocation
            .unit_stack
            .take()
            .expect("cleanup call starts from the Unit call emitter");
        relocation.scalar_stack = Some(ScalarCallStackEvidence {
            outbound: unit_stack.outbound,
            // The composed scalar-cleanup function keeps LR in its lifetime
            // frame, so no cleanup call owns a separate AArch64 link slot.
            aarch64_return_link: None,
        });
        internal_unit_calls.push(InternalUnitCallRecord {
            owner,
            target: cleanup.cleanup_machine,
            result: None,
            structural_result: None,
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            operation_ordinal: 0,
            code_offset,
            byte_count: emitted.bytes.len() - code_offset,
        });
    }
    let (
        result_load_offset,
        aarch64_link_load_offset,
        frame_release_offset,
        frame_release_byte_count,
    ) = match target.architecture {
        Architecture::X86_64 => {
            let result_load_offset = emitted.bytes.len();
            emit_x86_64_stack_load(&mut emitted.bytes, 0, 0);
            let release_offset = emitted.bytes.len();
            emit_x86_64_adjust_sp(&mut emitted.bytes, 16, true);
            let release_byte_count = emitted.bytes.len() - release_offset;
            emitted.bytes.push(0xc3);
            (result_load_offset, None, release_offset, release_byte_count)
        }
        Architecture::Aarch64 => {
            let mut instructions = Vec::new();
            let result_load_offset = emitted.bytes.len();
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 0, 0, 8)?);
            let link_load_offset = emitted.bytes.len() + instructions.len() * 4;
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 30, 8, 8)?);
            let release_offset = emitted.bytes.len() + instructions.len() * 4;
            emit_aarch64_adjust_sp(&mut instructions, 16, true)?;
            append_aarch64_instructions(&mut emitted.bytes, instructions);
            emitted
                .bytes
                .extend_from_slice(&0xd65f_03c0_u32.to_le_bytes());
            (
                result_load_offset,
                Some(link_load_offset),
                release_offset,
                4,
            )
        }
    };
    let parameter_records = structural_parameters
        .iter()
        .map(|parameter| omega_machine_code::UnitParameterRecord {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            shape: parameter.shape,
        })
        .collect::<Vec<_>>();
    let parameter_homes = structural_parameters
        .iter()
        .map(|parameter| omega_machine_code::UnitParameterHomeRecord {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: 0,
            indirect: matches!(
                parameter.placement.locations.as_slice(),
                [ValueLocation::Indirect { .. }]
            ),
        })
        .collect::<Vec<_>>();
    let cleanup = omega_machine_code::UnitAffineCleanupRecord {
        psi_edge,
        structural_types: structural_types.to_vec(),
        locals: Vec::new(),
        actions: cleanup_actions.to_vec(),
        code_offset: cleanup_offset,
        byte_count: emitted.bytes.len() - cleanup_offset,
    };
    emitted.unit_stack = None;
    let cleanup_preservation = ScalarCleanupPreservationEvidence {
        frame: StackAdjustmentPair {
            byte_size: 16,
            allocation_offset: cleanup_offset,
            allocation_byte_count: frame_allocation_byte_count,
            release_offset: frame_release_offset,
            release_byte_count: frame_release_byte_count,
        },
        result_byte_offset: 0,
        result_store_offset,
        result_load_offset,
        aarch64_return_link: match (aarch64_link_store_offset, aarch64_link_load_offset) {
            (Some(store_offset), Some(load_offset)) => Some(Aarch64ReturnLinkEvidence {
                frame_byte_offset: 8,
                store_offset,
                load_offset,
            }),
            (None, None) => None,
            _ => unreachable!("AArch64 cleanup link save and restore are paired"),
        },
    };
    emitted.scalar_stack = Some(collect_scalar_stack_evidence(
        target.architecture,
        &emitted.bytes,
        scalar_control_flow,
        Some(cleanup_preservation),
    )?);
    emitted.internal_unit_calls = internal_unit_calls;
    emitted.scalar_affine_cleanup = Some(cleanup.clone());
    emitted.scalar_structural_parameters = parameter_records;
    emitted.scalar_structural_parameter_homes = parameter_homes;
    emitted
        .semantic_code_attribution
        .push(SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(psi_edge),
            operation_ordinal: 0,
            code_offset: cleanup.code_offset,
            byte_count: cleanup.byte_count,
        });
    Ok(emitted)
}

struct BooleanControlCleanupEmission {
    bytes: Vec<u8>,
    internal_calls: Vec<InternalCallRelocation>,
    internal_unit_calls: Vec<InternalUnitCallRecord>,
    cleanups: Vec<ScalarControlAffineCleanupRecord>,
    branches: Vec<ScalarConditionalBranchEvidence>,
}

impl BooleanControlCleanupEmission {
    fn append(&mut self, mut child: Self) -> Result<(), EmissionError> {
        let base = self.bytes.len();
        shift_boolean_control_cleanup_emission(&mut child, base)?;
        self.bytes.append(&mut child.bytes);
        self.internal_calls.append(&mut child.internal_calls);
        self.internal_unit_calls
            .append(&mut child.internal_unit_calls);
        self.cleanups.append(&mut child.cleanups);
        self.branches.append(&mut child.branches);
        Ok(())
    }
}

fn shift_boolean_control_cleanup_emission(
    emission: &mut BooleanControlCleanupEmission,
    base: usize,
) -> Result<(), EmissionError> {
    let shift = |offset: &mut usize| -> Result<(), EmissionError> {
        *offset = offset
            .checked_add(base)
            .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
        Ok(())
    };
    for call in &mut emission.internal_calls {
        shift(&mut call.offset)?;
        if let Some(stack) = &mut call.scalar_stack {
            if let Some(outbound) = &mut stack.outbound {
                shift(&mut outbound.allocation_offset)?;
                shift(&mut outbound.release_offset)?;
            }
            if let Some(link) = &mut stack.aarch64_return_link {
                shift(&mut link.store_offset)?;
                shift(&mut link.load_offset)?;
            }
        }
    }
    for call in &mut emission.internal_unit_calls {
        shift(&mut call.code_offset)?;
    }
    for leaf in &mut emission.cleanups {
        shift(&mut leaf.cleanup.code_offset)?;
        shift(&mut leaf.preservation.frame.allocation_offset)?;
        shift(&mut leaf.preservation.frame.release_offset)?;
        shift(&mut leaf.preservation.result_store_offset)?;
        shift(&mut leaf.preservation.result_load_offset)?;
        if let Some(link) = &mut leaf.preservation.aarch64_return_link {
            shift(&mut link.store_offset)?;
            shift(&mut link.load_offset)?;
        }
    }
    for branch in &mut emission.branches {
        shift(&mut branch.branch_offset)?;
        shift(&mut branch.false_arm_offset)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_boolean_control_with_cleanup(
    function: &AssignedFunction,
    control: &AssignedBooleanControl,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    call_plan: &omega_calling_conventions::CallPlan,
    structural_parameters: &[omega_target_operations::TargetStructuralParameter],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<MachineCodeFunction, EmissionError> {
    if cleanup_actions.is_empty()
        || cleanup_actions.len() != structural_parameters.len()
        || call_plan.parameters.len() < structural_parameters.len()
        || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
            .iter()
            .zip(structural_parameters)
            .any(|(placement, parameter)| placement != &parameter.placement)
        || structural_parameters
            .iter()
            .rev()
            .zip(cleanup_actions)
            .any(|(parameter, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != parameter.place
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place != parameter.place
                        || cleanup.structural_type != parameter.structural_type
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(EmissionError::UnsupportedScalarCleanup);
    }

    let mut leaf_ordinal = 0_usize;
    let emitted = emit_boolean_control_cleanup_tree(
        control,
        structural_types,
        cleanup_actions,
        target,
        functions,
        &mut leaf_ordinal,
    )?;
    if leaf_ordinal < 2
        || emitted.cleanups.len() != leaf_ordinal
        || emitted.branches.len().checked_add(1) != Some(leaf_ordinal)
    {
        return Err(EmissionError::UnsupportedScalarCleanup);
    }
    let mut edges = emitted
        .cleanups
        .iter()
        .map(|leaf| leaf.cleanup.psi_edge)
        .collect::<Vec<_>>();
    edges.sort_unstable();
    edges.dedup();
    if edges.len() != leaf_ordinal {
        return Err(EmissionError::UnsupportedScalarCleanup);
    }
    let control_flow = ScalarControlFlowEvidence::ConditionalTree {
        decisions: emitted.branches,
        crash_leaves: vec![false; leaf_ordinal],
        branches: Vec::new(),
    };
    let scalar_stack = Some(collect_scalar_stack_evidence(
        target.architecture,
        &emitted.bytes,
        control_flow,
        None,
    )?);
    let parameter_records = structural_parameters
        .iter()
        .map(|parameter| omega_machine_code::UnitParameterRecord {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            shape: parameter.shape,
        })
        .collect::<Vec<_>>();
    let parameter_homes = structural_parameters
        .iter()
        .map(|parameter| omega_machine_code::UnitParameterHomeRecord {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            shape: parameter.shape,
            source: parameter.placement.clone(),
            byte_offset: 0,
            indirect: matches!(
                parameter.placement.locations.as_slice(),
                [ValueLocation::Indirect { .. }]
            ),
        })
        .collect::<Vec<_>>();
    let semantic_code_attribution = emitted
        .cleanups
        .iter()
        .enumerate()
        .map(|(operation_ordinal, leaf)| SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(leaf.cleanup.psi_edge),
            operation_ordinal,
            code_offset: leaf.cleanup.code_offset,
            byte_count: leaf.cleanup.byte_count,
        })
        .collect();
    Ok(MachineCodeFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        bytes: emitted.bytes,
        x86_scalar_fma: Vec::new(),
        unit_stack: None,
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack,
        internal_calls: emitted.internal_calls,
        foreign_calls: Vec::new(),
        internal_unit_calls: emitted.internal_unit_calls,
        unit_affine_cleanup: None,
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: emitted.cleanups,
        scalar_structural_parameters: parameter_records,
        scalar_structural_parameter_homes: parameter_homes,
        ranked_u32_countdown: None,
        semantic_code_attribution,
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_boolean_control_cleanup_tree(
    control: &AssignedBooleanControl,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    target: NativeTarget,
    functions: &[AssignedFunction],
    leaf_ordinal: &mut usize,
) -> Result<BooleanControlCleanupEmission, EmissionError> {
    match control {
        AssignedBooleanControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => {
            let (prefix, condition, aarch64_condition_register) = match target.architecture {
                Architecture::X86_64 => {
                    let mut bytes = emit_x86_64_parameter_return(
                        *condition_source,
                        false,
                        *condition_location,
                    )?;
                    if bytes.pop() != Some(0xc3) {
                        return Err(EmissionError::ConditionalBranchEncodingInvalid);
                    }
                    bytes.extend_from_slice(&[0x85, 0xc0]);
                    (bytes, ScalarConditionalCondition::Parameter, None)
                }
                Architecture::Aarch64 => {
                    let (bytes, condition_register) =
                        emit_aarch64_condition_load(*condition_source, *condition_location)?;
                    (
                        bytes,
                        ScalarConditionalCondition::Parameter,
                        Some(condition_register),
                    )
                }
            };
            emit_boolean_cleanup_conditional(
                prefix,
                condition,
                when_true,
                when_false,
                structural_types,
                cleanup_actions,
                target,
                functions,
                leaf_ordinal,
                aarch64_condition_register,
            )
        }
        AssignedBooleanControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } if linear_boolean_expression(condition) => {
            let mut condition_calls = Vec::new();
            let prefix = match target.architecture {
                Architecture::X86_64 => emit_x86_64_boolean_condition_value(
                    condition_frame,
                    condition,
                    Some((&mut condition_calls, target)),
                    None,
                )?,
                Architecture::Aarch64 => emit_aarch64_boolean_condition_value(
                    condition_frame,
                    condition,
                    Some((&mut condition_calls, target)),
                    None,
                )?,
            };
            if !condition_calls.is_empty() {
                return Err(EmissionError::UnsupportedScalarCleanup);
            }
            emit_boolean_cleanup_conditional(
                prefix,
                ScalarConditionalCondition::Expression,
                when_true,
                when_false,
                structural_types,
                cleanup_actions,
                target,
                functions,
                leaf_ordinal,
                None,
            )
        }
        AssignedBooleanControl::ReturnExpression { expression, .. }
            if !linear_boolean_expression(expression) =>
        {
            Err(EmissionError::UnsupportedScalarCleanup)
        }
        AssignedBooleanControl::ReturnImmediate {
            psi_return_edge, ..
        }
        | AssignedBooleanControl::ReturnParameter {
            psi_return_edge, ..
        }
        | AssignedBooleanControl::ReturnNotParameter {
            psi_return_edge, ..
        }
        | AssignedBooleanControl::ReturnExpression {
            psi_return_edge, ..
        } => {
            let fragment = match target.architecture {
                Architecture::X86_64 => emit_x86_64_boolean_control(control, target)?,
                Architecture::Aarch64 => emit_aarch64_boolean_control(control, target)?,
            };
            if !fragment.internal_calls.is_empty() || fragment.conditional.is_some() {
                return Err(EmissionError::UnsupportedScalarCleanup);
            }
            let ordinal = *leaf_ordinal;
            *leaf_ordinal = leaf_ordinal
                .checked_add(1)
                .ok_or(EmissionError::UnsupportedScalarCleanup)?;
            emit_boolean_cleanup_leaf(
                fragment.bytes,
                *psi_return_edge,
                structural_types,
                cleanup_actions,
                target,
                functions,
                ordinal,
            )
        }
        _ => Err(EmissionError::UnsupportedScalarCleanup),
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_boolean_cleanup_conditional(
    mut prefix: Vec<u8>,
    condition: ScalarConditionalCondition,
    when_true: &AssignedConditionalBooleanArm,
    when_false: &AssignedConditionalBooleanArm,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    target: NativeTarget,
    functions: &[AssignedFunction],
    leaf_ordinal: &mut usize,
    aarch64_condition_register: Option<u8>,
) -> Result<BooleanControlCleanupEmission, EmissionError> {
    let true_emission = emit_boolean_control_cleanup_tree(
        &when_true.control,
        structural_types,
        cleanup_actions,
        target,
        functions,
        leaf_ordinal,
    )?;
    let false_emission = emit_boolean_control_cleanup_tree(
        &when_false.control,
        structural_types,
        cleanup_actions,
        target,
        functions,
        leaf_ordinal,
    )?;
    let branch_offset = prefix.len();
    let branch_byte_count = match target.architecture {
        Architecture::X86_64 => {
            let displacement = i32::try_from(true_emission.bytes.len())
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            prefix.extend_from_slice(&[0x0f, 0x84]);
            prefix.extend_from_slice(&displacement.to_le_bytes());
            6
        }
        Architecture::Aarch64 => {
            let branch_words = true_emission
                .bytes
                .len()
                .checked_div(4)
                .and_then(|words| words.checked_add(1))
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            if branch_words > 0x3ffff {
                return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
            }
            let branch = match (condition, aarch64_condition_register) {
                (ScalarConditionalCondition::Parameter, Some(register)) => {
                    0x3400_0000_u32 | ((branch_words as u32) << 5) | u32::from(register)
                }
                (ScalarConditionalCondition::Expression, None) => {
                    0x5400_0000_u32 | ((branch_words as u32) << 5)
                }
                _ => return Err(EmissionError::ConditionalBranchEncodingInvalid),
            };
            prefix.extend_from_slice(&branch.to_le_bytes());
            4
        }
    };
    let false_arm_offset = prefix
        .len()
        .checked_add(true_emission.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut emission = BooleanControlCleanupEmission {
        bytes: prefix,
        internal_calls: Vec::new(),
        internal_unit_calls: Vec::new(),
        cleanups: Vec::new(),
        branches: vec![ScalarConditionalBranchEvidence {
            condition,
            branch_offset,
            branch_byte_count,
            false_arm_offset,
        }],
    };
    emission.append(true_emission)?;
    emission.append(false_emission)?;
    Ok(emission)
}

#[allow(clippy::too_many_arguments)]
fn emit_boolean_cleanup_leaf(
    mut bytes: Vec<u8>,
    psi_edge: psi_core::EdgeId,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
    target: NativeTarget,
    functions: &[AssignedFunction],
    leaf_ordinal: usize,
) -> Result<BooleanControlCleanupEmission, EmissionError> {
    match target.architecture {
        Architecture::X86_64 if bytes.pop() == Some(0xc3) => {}
        Architecture::Aarch64
            if bytes.len() >= 4
                && bytes.split_off(bytes.len() - 4) == 0xd65f_03c0_u32.to_le_bytes() => {}
        _ => return Err(EmissionError::UnsupportedScalarCleanup),
    }
    let cleanup_offset = bytes.len();
    let mut internal_calls = Vec::new();
    let mut internal_unit_calls = Vec::new();
    let (frame_allocation_byte_count, result_store_offset, aarch64_link_store_offset) =
        match target.architecture {
            Architecture::X86_64 => {
                emit_x86_64_adjust_sp(&mut bytes, 16, false);
                let result_store_offset = bytes.len();
                emit_x86_64_stack_store(&mut bytes, 0, 0);
                (
                    result_store_offset - cleanup_offset,
                    result_store_offset,
                    None,
                )
            }
            Architecture::Aarch64 => {
                let mut instructions = Vec::new();
                emit_aarch64_adjust_sp(&mut instructions, 16, false)?;
                let result_store_offset = bytes.len() + instructions.len() * 4;
                instructions.push(aarch64_unit_stack_access(0xf900_0000, 0, 0, 8)?);
                let link_store_offset = bytes.len() + instructions.len() * 4;
                instructions.push(aarch64_unit_stack_access(0xf900_0000, 30, 8, 8)?);
                append_aarch64_instructions(&mut bytes, instructions);
                (4, result_store_offset, Some(link_store_offset))
            }
        };
    for (ordinal, action) in cleanup_actions.iter().enumerate() {
        let psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
            continue;
        };
        if !executable_nominal_cleanup(cleanup, functions)? {
            continue;
        }
        let action_ordinal =
            u32::try_from(ordinal).map_err(|_| EmissionError::UnsupportedScalarCleanup)?;
        let owner = CallSiteOwner::CleanupAction {
            edge: psi_edge,
            action_ordinal,
        };
        let code_offset = bytes.len();
        match target.architecture {
            Architecture::X86_64 => {
                emit_x86_64_unit_call(
                    &mut bytes,
                    owner,
                    cleanup.cleanup_machine,
                    &[],
                    target,
                    &[],
                    &mut internal_calls,
                )?;
            }
            Architecture::Aarch64 => {
                emit_aarch64_unit_call(
                    &mut bytes,
                    owner,
                    cleanup.cleanup_machine,
                    &[],
                    &[],
                    &mut internal_calls,
                )?;
            }
        }
        let relocation = internal_calls
            .last_mut()
            .expect("cleanup call emission retains its relocation");
        let unit_stack = relocation
            .unit_stack
            .take()
            .expect("cleanup call starts from the Unit call emitter");
        relocation.scalar_stack = Some(ScalarCallStackEvidence {
            outbound: unit_stack.outbound,
            aarch64_return_link: None,
        });
        internal_unit_calls.push(InternalUnitCallRecord {
            owner,
            target: cleanup.cleanup_machine,
            result: None,
            structural_result: None,
            arguments: Vec::new(),
            claim_transfers: Vec::new(),
            operation_ordinal: leaf_ordinal,
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }
    let (
        result_load_offset,
        aarch64_link_load_offset,
        frame_release_offset,
        frame_release_byte_count,
    ) = match target.architecture {
        Architecture::X86_64 => {
            let result_load_offset = bytes.len();
            emit_x86_64_stack_load(&mut bytes, 0, 0);
            let release_offset = bytes.len();
            emit_x86_64_adjust_sp(&mut bytes, 16, true);
            let release_byte_count = bytes.len() - release_offset;
            bytes.push(0xc3);
            (result_load_offset, None, release_offset, release_byte_count)
        }
        Architecture::Aarch64 => {
            let mut instructions = Vec::new();
            let result_load_offset = bytes.len();
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 0, 0, 8)?);
            let link_load_offset = bytes.len() + instructions.len() * 4;
            instructions.push(aarch64_unit_stack_access(0xf940_0000, 30, 8, 8)?);
            let release_offset = bytes.len() + instructions.len() * 4;
            emit_aarch64_adjust_sp(&mut instructions, 16, true)?;
            append_aarch64_instructions(&mut bytes, instructions);
            bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes());
            (
                result_load_offset,
                Some(link_load_offset),
                release_offset,
                4,
            )
        }
    };
    let cleanup = omega_machine_code::UnitAffineCleanupRecord {
        psi_edge,
        structural_types: structural_types.to_vec(),
        locals: Vec::new(),
        actions: cleanup_actions.to_vec(),
        code_offset: cleanup_offset,
        byte_count: bytes.len() - cleanup_offset,
    };
    let preservation = ScalarCleanupPreservationEvidence {
        frame: StackAdjustmentPair {
            byte_size: 16,
            allocation_offset: cleanup_offset,
            allocation_byte_count: frame_allocation_byte_count,
            release_offset: frame_release_offset,
            release_byte_count: frame_release_byte_count,
        },
        result_byte_offset: 0,
        result_store_offset,
        result_load_offset,
        aarch64_return_link: match (aarch64_link_store_offset, aarch64_link_load_offset) {
            (Some(store_offset), Some(load_offset)) => Some(Aarch64ReturnLinkEvidence {
                frame_byte_offset: 8,
                store_offset,
                load_offset,
            }),
            (None, None) => None,
            _ => unreachable!("AArch64 cleanup link save and restore are paired"),
        },
    };
    Ok(BooleanControlCleanupEmission {
        bytes,
        internal_calls,
        internal_unit_calls,
        cleanups: vec![ScalarControlAffineCleanupRecord {
            cleanup,
            preservation,
        }],
        branches: Vec::new(),
    })
}

fn scalar_operation_edge(operation: &AssignedOperation) -> Option<psi_core::EdgeId> {
    match operation {
        AssignedOperation::ReturnIntegerImmediate { psi_edge, .. }
        | AssignedOperation::ReturnBooleanImmediate { psi_edge, .. }
        | AssignedOperation::ReturnIntegerParameter { psi_edge, .. }
        | AssignedOperation::ReturnBooleanParameter { psi_edge, .. }
        | AssignedOperation::ReturnBooleanNotParameter { psi_edge, .. }
        | AssignedOperation::ReturnBooleanSharedConvergence { psi_edge, .. }
        | AssignedOperation::ReturnBooleanExpression { psi_edge, .. }
        | AssignedOperation::ReturnIntegerExpression { psi_edge, .. } => Some(*psi_edge),
        _ => None,
    }
}
pub(super) fn stack_adjustment_pair(
    byte_size: u32,
    allocation: Option<(usize, usize)>,
    release: Option<(usize, usize)>,
) -> Option<StackAdjustmentPair> {
    match (byte_size, allocation, release) {
        (0, None, None) => None,
        (
            byte_size,
            Some((allocation_offset, allocation_byte_count)),
            Some((release_offset, release_byte_count)),
        ) => Some(StackAdjustmentPair {
            byte_size,
            allocation_offset,
            allocation_byte_count,
            release_offset,
            release_byte_count,
        }),
        _ => unreachable!("nonzero stack adjustment must retain both encoded operations"),
    }
}

pub(super) fn executable_nominal_cleanup(
    cleanup: &psi_terminal::NominalAffineCleanup,
    functions: &[AssignedFunction],
) -> Result<bool, EmissionError> {
    let invalid = || EmissionError::InvalidNominalCleanupTarget(cleanup.cleanup_machine);
    if cleanup.cleanup_receiver.is_some() || !cleanup.requirement_obligations.is_empty() {
        return Err(invalid());
    }
    let cleanup_function = functions
        .iter()
        .find(|function| function.machine == cleanup.cleanup_machine)
        .ok_or_else(invalid)?;
    let AssignedOperation::UnitBody(cleanup_body) = &cleanup_function.operation else {
        return Err(invalid());
    };
    if cleanup_function.attachment != Some(cleanup.structural_type)
        || !cleanup_body.parameters.is_empty()
    {
        return Err(invalid());
    }
    let Some((cleanup_return, helper_calls)) = cleanup_body.operations.split_last() else {
        return Err(invalid());
    };
    if !matches!(cleanup_return,
            AssignedUnitOperation::Return {
                cleanup_actions,
                ..
            } if cleanup_actions.is_empty())
    {
        return Err(invalid());
    }
    let helper_sites = helper_calls
        .iter()
        .map(|operation| match operation {
            AssignedUnitOperation::Call {
                psi_operation,
                callee,
                copies,
                claim_transfers,
                ..
            } if copies.is_empty() && claim_transfers.is_empty() => Ok((*psi_operation, *callee)),
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if helper_sites
        .iter()
        .map(|(operation, _)| *operation)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != helper_sites.len()
        || helper_sites
            .iter()
            .map(|(_, callee)| *callee)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != helper_sites.len()
    {
        return Err(invalid());
    }
    for (_, callee) in &helper_sites {
        let helper = functions
            .iter()
            .find(|function| function.machine == *callee)
            .ok_or_else(invalid)?;
        let AssignedOperation::UnitBody(helper_body) = &helper.operation else {
            return Err(invalid());
        };
        if helper.machine == cleanup.cleanup_machine
            || helper.attachment.is_none()
            || !helper_body.parameters.is_empty()
            || !matches!(
                helper_body.operations.as_slice(),
                [AssignedUnitOperation::Return {
                    cleanup_actions,
                    ..
                }] if cleanup_actions.is_empty()
            )
        {
            return Err(invalid());
        }
    }
    Ok(!helper_sites.is_empty())
}

pub(super) fn exact_partial_cleanup_partition(
    declarations: &[psi_terminal::StructuralTypeDeclaration],
    root_type: psi_core::StructuralTypeId,
    moved: &[(
        &[psi_terminal::StructuralPathSegment],
        psi_core::StructuralTypeId,
    )],
    residuals: &[&psi_terminal::StructuralAffineDiscard],
) -> bool {
    if declarations.is_empty() || moved.is_empty() || residuals.is_empty() {
        return false;
    }
    let mut by_id = std::collections::BTreeMap::new();
    let mut identities = std::collections::BTreeSet::new();
    for declaration in declarations {
        if declaration.identity.is_empty()
            || !identities.insert(declaration.identity.as_str())
            || by_id.insert(declaration.id, declaration).is_some()
        {
            return false;
        }
    }
    if declarations.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return false;
    }
    let mut expected = Vec::new();
    append_expected_partial_residuals(root_type, &[], moved, &by_id, &mut expected).is_some()
        && residuals.len() == expected.len()
        && residuals
            .iter()
            .zip(expected)
            .all(|(actual, (path, structural_type))| {
                actual.path == path && actual.structural_type == structural_type
            })
}

fn append_expected_partial_residuals(
    structural_type: psi_core::StructuralTypeId,
    prefix: &[psi_terminal::StructuralPathSegment],
    moved: &[(
        &[psi_terminal::StructuralPathSegment],
        psi_core::StructuralTypeId,
    )],
    declarations: &std::collections::BTreeMap<
        psi_core::StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
    output: &mut Vec<(
        Vec<psi_terminal::StructuralPathSegment>,
        psi_core::StructuralTypeId,
    )>,
) -> Option<()> {
    if prefix.is_empty()
        && moved.iter().all(|(path, _)| {
            matches!(
                path,
                [
                    psi_terminal::StructuralPathSegment::FixedIndex(_),
                    psi_terminal::StructuralPathSegment::FixedIndex(_)
                ]
            )
        })
    {
        let psi_terminal::StructuralTypeShape::FixedArray { element, length: 2 } =
            declarations.get(&structural_type)?.shape
        else {
            return None;
        };
        let psi_terminal::StructuralTypeShape::FixedArray {
            element: leaf,
            length: inner_length @ (3 | 4 | 5),
        } = declarations.get(&element)?.shape
        else {
            return None;
        };
        if moved.len() != 2
            || moved.iter().any(|(_, moved_type)| *moved_type != leaf)
            || !matches!(
                declarations
                    .get(&leaf)
                    .map(|declaration| &declaration.shape),
                Some(psi_terminal::StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_by_outer = moved
            .iter()
            .filter_map(|(path, _)| match path {
                [
                    psi_terminal::StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                    psi_terminal::StructuralPathSegment::FixedIndex(inner),
                ] if *inner < inner_length => Some((*outer, *inner)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        if moved_by_outer.len() != 2 {
            return None;
        }
        for outer in (0_u64..2).rev() {
            let moved_inner = *moved_by_outer.get(&outer)?;
            for inner in (0_u64..inner_length).rev() {
                if inner != moved_inner {
                    output.push((
                        vec![
                            psi_terminal::StructuralPathSegment::FixedIndex(outer),
                            psi_terminal::StructuralPathSegment::FixedIndex(inner),
                        ],
                        leaf,
                    ));
                }
            }
        }
        return Some(());
    }
    if prefix.is_empty()
        && moved
            .iter()
            .all(|(path, _)| matches!(path, [psi_terminal::StructuralPathSegment::FixedIndex(_)]))
    {
        let declaration = declarations.get(&structural_type)?;
        let psi_terminal::StructuralTypeShape::FixedArray { element, length } = declaration.shape
        else {
            return None;
        };
        if !matches!((length, moved.len()), (2, 1) | (3, 1 | 2) | (4, 2))
            || moved.iter().any(|(_, moved_type)| *moved_type != element)
            || !matches!(
                declarations
                    .get(&element)
                    .map(|declaration| &declaration.shape),
                Some(psi_terminal::StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_indexes = moved
            .iter()
            .filter_map(|(path, _)| match path {
                [psi_terminal::StructuralPathSegment::FixedIndex(index)] if *index < length => {
                    Some(*index)
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        if moved_indexes.len() != moved.len() {
            return None;
        }
        let residuals = (0..length)
            .rev()
            .filter(|index| !moved_indexes.contains(index))
            .collect::<Vec<_>>();
        if residuals.is_empty() {
            return None;
        }
        output.extend(residuals.into_iter().map(|residual| {
            (
                vec![psi_terminal::StructuralPathSegment::FixedIndex(residual)],
                element,
            )
        }));
        return Some(());
    }
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &declarations.get(&structural_type)?.shape
    else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(
                    field.field_type,
                    psi_terminal::StructuralFieldType::Structural(_)
                        | psi_terminal::StructuralFieldType::Scalar(_)
                        | psi_terminal::StructuralFieldType::IeeeFloat(_)
                        | psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
        })
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        let matching = moved
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(psi_terminal::StructuralPathSegment::Field(identity))
                    if identity == &field.identity)
            })
            .copied()
            .collect::<Vec<_>>();
        matched += matching.len();
        let mut field_path = prefix.to_vec();
        field_path.push(psi_terminal::StructuralPathSegment::Field(
            field.identity.clone(),
        ));
        let psi_terminal::StructuralFieldType::Structural(field_type) = field.field_type else {
            if !matching.is_empty() {
                return None;
            }
            continue;
        };
        if matching.is_empty() {
            output.push((field_path, field_type));
        } else if matching.iter().any(|(path, _)| path.len() == 1) {
            if matching.len() != 1 || matching[0].1 != field_type {
                return None;
            }
        } else {
            let nested = matching
                .iter()
                .map(|(path, leaf)| (&path[1..], *leaf))
                .collect::<Vec<_>>();
            append_expected_partial_residuals(
                field_type,
                &field_path,
                &nested,
                declarations,
                output,
            )?;
        }
    }
    (matched == moved.len()).then_some(())
}
