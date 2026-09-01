use super::shared::*;

pub(super) fn receipt(
    plan: &SelectedInstructionPlan,
    legalized: &ValidatedLegalizedOperations,
) -> SelectedInstructionValidationReceipt {
    let function_count = plan.functions.len() + plan.structural_unit_functions.len();
    let block_count = plan
        .functions
        .iter()
        .map(|function| function.blocks.len())
        .sum::<usize>()
        + plan.structural_unit_functions.len();
    let virtual_register_count = plan
        .functions
        .iter()
        .map(|function| function.virtual_registers.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len() + 1)
        .sum::<usize>()
        + plan
            .structural_unit_functions
            .iter()
            .map(|function| 1 + usize::from(function.call.is_some()))
            .sum::<usize>();
    SelectedInstructionValidationReceipt {
        identity: selected_instruction_plan_identity(plan),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        optimization_unit: legalized.receipt().optimization_unit(),
        fuel_schedule: legalized.receipt().fuel_schedule(),
        function_count,
        block_count,
        virtual_register_count,
        instruction_count,
    }
}

pub fn selected_instruction_plan_identity(
    plan: &SelectedInstructionPlan,
) -> SelectedInstructionPlanIdentity {
    selected_instruction_plan_identity_with_schema(
        plan,
        b"omega.terminal-selected-instructions.v12\0",
        false,
    )
}

#[doc(hidden)]
pub fn selected_instruction_plan_identity_v11_legacy(
    plan: &SelectedInstructionPlan,
) -> SelectedInstructionPlanIdentity {
    selected_instruction_plan_identity_with_schema(
        plan,
        b"omega.terminal-selected-instructions.v11\0",
        true,
    )
}

fn selected_instruction_plan_identity_with_schema(
    plan: &SelectedInstructionPlan,
    domain: &[u8],
    legacy_legalized_identity: bool,
) -> SelectedInstructionPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
        encode_len(&mut bytes, function.virtual_registers.len());
        for register in &function.virtual_registers {
            bytes.extend_from_slice(&register.id.0.to_le_bytes());
            encode_scalar_type(&mut bytes, register.scalar_type);
            bytes.extend_from_slice(&register.class.0.to_le_bytes());
            match register.origin {
                VirtualRegisterOrigin::EntryParameter {
                    source_value,
                    parameter_index,
                } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&source_value.get().to_le_bytes());
                    bytes.extend_from_slice(&(parameter_index as u64).to_le_bytes());
                }
                VirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value,
                } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&source_value.get().to_le_bytes());
                }
                VirtualRegisterOrigin::LegalizationTemporary {
                    instruction,
                    temporary,
                    source_value,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&temporary.0.to_le_bytes());
                    bytes.extend_from_slice(&source_value.get().to_le_bytes());
                }
            }
            encode_definition_site(&mut bytes, register.definition_site);
            encode_option_u16(&mut bytes, register.entry_fixed_view.map(|view| view.0));
        }
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.id.0.to_le_bytes());
            bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_instruction(&mut bytes, instruction);
            }
            match &block.terminator {
                SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero,
                    when_zero,
                } => {
                    bytes.push(0);
                    encode_instruction(&mut bytes, instruction);
                    encode_successor(&mut bytes, when_nonzero);
                    encode_successor(&mut bytes, when_zero);
                }
                SelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                } => {
                    bytes.push(1);
                    encode_instruction(&mut bytes, instruction);
                    bytes.extend_from_slice(&psi_return_edge.get().to_le_bytes());
                }
            }
        }
    }
    bytes.extend_from_slice(
        &selected_structural_legalized_identity(plan, legacy_legalized_identity).bytes(),
    );
    encode_len(&mut bytes, plan.structural_unit_functions.len());
    for function in &plan.structural_unit_functions {
        encode_selected_structural_unit_function(&mut bytes, function);
    }
    SelectedInstructionPlanIdentity::from_canonical_bytes(&bytes)
}

fn selected_structural_legalized_identity(
    plan: &SelectedInstructionPlan,
    legacy: bool,
) -> LegalizedOperationPlanIdentity {
    let legalized = LegalizedOperationPlan {
        psi: plan.psi,
        optimization_unit: omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
            b"omega.selected-structural-legalized-fingerprint.v1",
        ),
        fuel_schedule: plan.fuel_schedule,
        target: plan.target,
        entry: plan.entry,
        functions: Vec::new(),
        unit_functions: Vec::new(),
        structural_unit_functions: plan
            .structural_unit_functions
            .iter()
            .map(|function| SourceStructuralUnitFunction {
                machine: function.machine,
                attachment: function.attachment,
                provenance: function.provenance.clone(),
                recipe: if !function.boundary_settlements.is_empty() {
                    omega_legalized_operations::StructuralUnitLegalizationRecipe::ClaimCompletionSettlementsThenReturnUnitV1
                } else {
                    match function.call.as_ref().map(|call| &call.source) {
                        Some(omega_legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit) => {
                            omega_legalized_operations::StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1
                        }
                        Some(omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider { .. }) => {
                            omega_legalized_operations::StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1
                        }
                        None => omega_legalized_operations::StructuralUnitLegalizationRecipe::ReturnUnitV1,
                    }
                },
                structural_types: function.structural_types.clone(),
                call_plan: function.abi.call_plan.clone(),
                parameters: function
                    .abi
                    .parameters
                    .iter()
                    .map(|parameter| LegalizedCallUnitParameter {
                        semantic: parameter.semantic.clone(),
                        target: parameter.target.clone(),
                    })
                    .collect(),
                structural_places: function.structural_places.clone(),
                entry_claims: function.entry_claims.clone(),
                published_service_ceiling: function.published_service_ceiling.clone(),
                entry_block: function.source_entry_block,
                boundary_settlements: function.boundary_settlements.clone(),
                call: function.call.as_ref().map(|call| LegalizedCallUnit {
                    source: call.source.clone(),
                    operation: call.operation,
                    callee: call.callee,
                    arguments: call
                        .arguments
                        .iter()
                        .map(|argument| LegalizedCallUnitArgument {
                            semantic: argument.semantic.clone(),
                            target: argument.target.clone(),
                        })
                        .collect(),
                    claim_transfers: call.claim_transfers.clone(),
                    requirement_obligations: call.requirement_obligations.clone(),
                    crash_continuations: call.crash_continuations.clone(),
                    fuel: call.provenance.fuel.clone(),
                    effect: call.effect,
                    ownership: call.ownership.clone(),
                }),
                return_edge: function.terminator.psi_return_edge,
                return_fuel: function.terminator.instruction.provenance.fuel.clone(),
                return_effect: function.terminator.effect,
                return_ownership: function.terminator.ownership.clone(),
            })
            .collect(),
        projected_structural_call_returns: Vec::new(),
    };
    if legacy {
        omega_legalized_operations::legalized_operation_plan_identity_v9_legacy(&legalized)
    } else {
        legalized_operation_plan_identity(&legalized)
    }
}

fn encode_selected_structural_unit_function(
    bytes: &mut Vec<u8>,
    function: &SelectedStructuralUnitFunction,
) {
    bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
    bytes.push(match function.abi.recipe {
        SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1 => 1,
    });
    encode_structural_layout(bytes, function.abi.layout);
    match &function.call {
        None => bytes.push(0),
        Some(call) => {
            bytes.push(1);
            bytes.extend_from_slice(&call.id.0.to_le_bytes());
            encode_structural_layout(bytes, call.layout);
            encode_constraint_key(bytes, call.constraint);
            encode_u16s(bytes, call.implicit_uses.iter().map(|unit| unit.0));
            encode_u16s(bytes, call.implicit_defs.iter().map(|unit| unit.0));
            encode_u16s(bytes, call.clobbers.iter().map(|unit| unit.0));
            encode_selected_provenance(bytes, &call.provenance);
        }
    }
    encode_instruction(bytes, &function.terminator.instruction);
}

fn encode_structural_layout(
    bytes: &mut Vec<u8>,
    layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
) {
    bytes.extend_from_slice(&layout.shadow_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.outgoing_frame_byte_count.to_le_bytes());
    bytes.extend_from_slice(&layout.pre_call_stack_alignment.to_le_bytes());
    for binding in layout.bindings {
        bytes.extend_from_slice(&(binding.parameter_index as u64).to_le_bytes());
        encode_machine_register(bytes, binding.pointer);
        bytes.extend_from_slice(&binding.copy_stack_byte_offset.to_le_bytes());
        bytes.extend_from_slice(&binding.byte_count.to_le_bytes());
        bytes.extend_from_slice(&binding.alignment.to_le_bytes());
    }
}

fn encode_selected_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_ids(
        bytes,
        provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(bytes, provenance.values.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.edges.iter().map(|edge| edge.get()));
    encode_ids(
        bytes,
        provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &provenance.fuel);
}

fn encode_machine_register(bytes: &mut Vec<u8>, register: MachineRegister) {
    let (tag, payload) = match register {
        MachineRegister::X86Rax => (0, 0),
        MachineRegister::X86Rcx => (1, 0),
        MachineRegister::X86Rdx => (2, 0),
        MachineRegister::X86Rbx => (3, 0),
        MachineRegister::X86Rsp => (4, 0),
        MachineRegister::X86Rbp => (5, 0),
        MachineRegister::X86Rsi => (6, 0),
        MachineRegister::X86Rdi => (7, 0),
        MachineRegister::X86R8 => (8, 0),
        MachineRegister::X86R9 => (9, 0),
        MachineRegister::X86R10 => (10, 0),
        MachineRegister::X86R11 => (11, 0),
        MachineRegister::X86R12 => (12, 0),
        MachineRegister::X86R13 => (13, 0),
        MachineRegister::X86R14 => (14, 0),
        MachineRegister::X86R15 => (15, 0),
        MachineRegister::X86Xmm(index) => (16, index),
        MachineRegister::Aarch64X(index) => (17, index),
        MachineRegister::Aarch64V(index) => (18, index),
    };
    bytes.push(tag);
    bytes.push(payload);
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    bytes.extend_from_slice(&instruction.id.0.to_le_bytes());
    bytes.push(match instruction.kind {
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::ConditionalBranchNonZero => 2,
        SelectedInstructionKind::ReturnI64 => 3,
        SelectedInstructionKind::CopyI64 => 4,
        SelectedInstructionKind::ExactAddI64 { .. } => 5,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
    });
    match instruction.kind {
        SelectedInstructionKind::MaterializeI64 { value } => match value {
            psi_core::IntegerValue::Signed(value) => {
                bytes.push(0);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            psi_core::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        },
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            match immediate {
                psi_core::IntegerValue::Signed(value) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                psi_core::IntegerValue::Unsigned(value) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ReturnI64
        | SelectedInstructionKind::ReturnUnit => {}
    }
    encode_constraint_key(bytes, instruction.constraint);
    encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(match operand.access {
            RegisterOperandAccess::Use => 0,
            RegisterOperandAccess::Def => 1,
            RegisterOperandAccess::UseDef => 2,
        });
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_ids(
        bytes,
        instruction
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .values
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        instruction.provenance.edges.iter().map(|edge| edge.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &instruction.provenance.fuel);
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.block.0.to_le_bytes());
    bytes.extend_from_slice(&successor.source_target.get().to_le_bytes());
    encode_len(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.scalar_type);
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    bytes.extend_from_slice(&(fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: omega_target::NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::X86_64 => 0,
        omega_target::Architecture::Aarch64 => 1,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        omega_register_model::RegisterConstraintFamily::Call => 0,
        omega_register_model::RegisterConstraintFamily::Return => 1,
        omega_register_model::RegisterConstraintFamily::SystemCall => 2,
        omega_register_model::RegisterConstraintFamily::InlineAssembly => 3,
        omega_register_model::RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                psi_core::IntegerCarrier::Fixed => 0,
                psi_core::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                psi_core::IeeeFloatFormat::Binary32 => 0,
                psi_core::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn encode_option_id(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
