//! Encode already settled physical adapter instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterEncodingError;
use AdapterEncodingError as Error;
use calling_conventions::{MachineRegister, gpr_code};
use post_allocation_machine::adapters::{
    AdapterInstructions, AdapterOperation, Scalar32Operation, X86AdapterTemplate, X86Condition,
    X86DescriptorTable,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use target::Architecture;

fn template_bytes(template: X86AdapterTemplate) -> &'static [u8] {
    match template {
        X86AdapterTemplate::AlignStack => &[0x48, 0x83, 0xe4, 0xf0],
        X86AdapterTemplate::MaskAndTestIst => &[0x83, 0xe0, 7, 0x85, 0xc0],
        X86AdapterTemplate::ShiftHighGateAddress => &[0x48, 0xc1, 0xe0, 32, 0x48, 0x89, 0xc1],
        X86AdapterTemplate::MergeMiddleGateAddress => &[0x48, 0xc1, 0xe0, 16, 0x48, 0x09, 0xc1],
        X86AdapterTemplate::MergeLowGateAddress => &[0x48, 0x09, 0xc8],
        X86AdapterTemplate::ReadTaskSelector => &[0x0f, 0x00, 0xc8, 0x0f, 0xb7, 0xc0],
        X86AdapterTemplate::TestSelectorAlignment => &[0xa9, 7, 0, 0, 0],
        X86AdapterTemplate::TestSelectorNonzero => &[0x85, 0xc0],
        X86AdapterTemplate::SelectorLimit => &[0x48, 0x89, 0xc1, 0x83, 0xc0, 15],
        X86AdapterTemplate::CompareDescriptorLimit => &[0x39, 0xd0],
        X86AdapterTemplate::AddDescriptorBase => &[0x48, 0x01, 0xca],
        X86AdapterTemplate::HighDescriptorBase => &[0x25, 0, 0, 0, 0xff, 0x48, 0x89, 0xc1],
        X86AdapterTemplate::MergeDescriptorBase => &[0x48, 0x09, 0xc1],
        X86AdapterTemplate::FinishDescriptorBase => {
            &[0x48, 0xc1, 0xe0, 32, 0x48, 0x09, 0xc8, 0x48, 0x89, 0xc2]
        }
        X86AdapterTemplate::LoadIstTop => &[0x48, 0x8b, 0x54, 0xca, 28],
        X86AdapterTemplate::LoadIstRoot => &[0x48, 0x8b, 0x52, 0xc8],
        X86AdapterTemplate::CompareActiveRoot => &[
            0x48,
            0x83,
            0xf8,
            runtime_abi::static_interrupts::root::ACTIVE as u8,
        ],
        X86AdapterTemplate::CompareProvider => &[0x48, 0x39, 0xc8],
    }
}
pub fn encode_instructions(instructions: &AdapterInstructions) -> Result<(Vec<u8>, usize), Error> {
    let model = register_model::validate_physical_register_model(match instructions.architecture {
        Architecture::X86_64 => isa_x86_64::x86_64_physical_register_model(),
        Architecture::Aarch64 => isa_aarch64::aarch64_physical_register_model(),
    })
    .map_err(|_| AdapterEncodingError)?;
    let mut bytes = Vec::new();
    let mut call_field = None;
    for operation in &instructions.operations {
        if matches!(operation, AdapterOperation::Call) {
            let field =
                bytes.len() + usize::from(instructions.architecture == Architecture::X86_64);
            if call_field.replace(field).is_some() {
                return Err(AdapterEncodingError);
            }
        }
        encode_operation(instructions.architecture, &model, operation, &mut bytes)?;
    }
    Ok((bytes, call_field.ok_or(AdapterEncodingError)?))
}
fn view(
    model: &ValidatedPhysicalRegisterModel,
    register: MachineRegister,
    size: u8,
) -> Result<RegisterViewId, Error> {
    let name = match register {
        MachineRegister::Aarch64X(number) if number < 31 => format!("x{number}"),
        MachineRegister::Aarch64V(number) if number < 32 => {
            format!("{}{number}", if size == 16 { "q" } else { "d" })
        }
        MachineRegister::X86Xmm(number) if number < 16 => format!("xmm{number}"),
        register => [
            "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ]
        .get(usize::from(gpr_code(register).ok_or(AdapterEncodingError)?))
        .ok_or(AdapterEncodingError)?
        .to_string(),
    };
    model
        .model()
        .view_named(&name)
        .map(|view| view.id)
        .ok_or(AdapterEncodingError)
}

pub(crate) fn encode_operation(
    architecture: Architecture,
    model: &ValidatedPhysicalRegisterModel,
    operation: &AdapterOperation,
    bytes: &mut Vec<u8>,
) -> Result<(), Error> {
    use AdapterOperation as Op;
    let encoded = match *operation {
        Op::Frame {
            bytes: frame,
            interval,
            touches,
            enter,
        } => {
            let frame = match architecture {
                Architecture::X86_64 => isa_x86_64::encode_system_v_amd64_frame_protocol(
                    model,
                    u64::from(frame),
                    isa_x86_64::X86_64StackProbe {
                        interval_bytes: interval,
                        touches,
                    },
                    &[],
                )
                .map_err(|_| AdapterEncodingError)?,
                Architecture::Aarch64 => isa_aarch64::encode_aapcs64_frame_protocol(
                    model,
                    u64::from(frame),
                    isa_aarch64::Aarch64StackProbe {
                        interval_bytes: interval,
                        touches,
                    },
                    &[],
                )
                .map_err(|_| AdapterEncodingError)?,
            };
            if enter {
                frame.prologue
            } else {
                frame.epilogue
            }
        }
        Op::FrameMemory {
            register,
            offset,
            size,
            load,
            scratch,
        } => {
            let register = view(model, register, size)?;
            match architecture {
                Architecture::X86_64 => {
                    isa_x86_64::encode_x86_64_frame_memory(model, register, offset, size, load)
                        .map_err(|_| AdapterEncodingError)?
                }
                Architecture::Aarch64 => isa_aarch64::encode_aarch64_frame_memory(
                    model,
                    register,
                    offset,
                    size,
                    load,
                    view(model, scratch, 8)?,
                )
                .map_err(|_| AdapterEncodingError)?,
            }
        }
        Op::PointerMemory {
            register,
            base,
            offset,
            size,
            load,
        } => {
            let (register, base) = (view(model, register, size)?, view(model, base, 8)?);
            match architecture {
                Architecture::X86_64 => isa_x86_64::encode_x86_64_pointer_memory(
                    model, register, base, offset, size, load,
                )
                .map_err(|_| AdapterEncodingError)?,
                Architecture::Aarch64 => isa_aarch64::encode_aarch64_pointer_memory(
                    model, register, base, offset, size, load,
                )
                .map_err(|_| AdapterEncodingError)?,
            }
        }
        Op::FrameAddress { register, offset } => match architecture {
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_frame_address(model, view(model, register, 8)?, offset)
                    .map_err(|_| AdapterEncodingError)?
            }
            Architecture::Aarch64 => {
                isa_aarch64::encode_aarch64_frame_address(model, view(model, register, 8)?, offset)
                    .map_err(|_| AdapterEncodingError)?
            }
        },
        Op::AlignedFrameAddress {
            register,
            offset,
            alignment,
            member,
        } => match architecture {
            Architecture::X86_64 => isa_x86_64::encode_x86_64_aligned_frame_address(
                model,
                view(model, register, 8)?,
                offset,
                alignment,
                member,
            )
            .map_err(|_| AdapterEncodingError)?,
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_aligned_frame_address(
                model,
                view(model, register, 8)?,
                offset,
                alignment,
                member,
            )
            .map_err(|_| AdapterEncodingError)?,
        },
        Op::Constant { register, value } => match architecture {
            Architecture::X86_64 => {
                isa_x86_64::encode_x86_64_integer_constant(model, view(model, register, 8)?, value)
                    .map_err(|_| AdapterEncodingError)?
            }
            Architecture::Aarch64 => isa_aarch64::encode_aarch64_integer_constant(
                model,
                view(model, register, 8)?,
                value,
            )
            .map_err(|_| AdapterEncodingError)?,
        },
        Op::SignExtend { register, bits } => {
            use selected_instructions::{
                MachineAlternativeFamily as Family, MachineAlternativeKey,
                SelectedInstructionKind as Kind,
            };
            let (kind, family) = match bits {
                8 => (Kind::SignExtendI8, Family::SignExtendI8),
                16 => (Kind::SignExtendI16, Family::SignExtendI16),
                _ => return Err(AdapterEncodingError),
            };
            let register = view(model, register, 8)?;
            let operands = [register, register];
            let key = MachineAlternativeKey { family, variant: 0 };
            match architecture {
                Architecture::X86_64 => {
                    isa_x86_64::encode_x86_64_selected_form(model, kind, key, &operands)
                        .map_err(|_| AdapterEncodingError)?
                        .bytes()
                        .to_vec()
                }
                Architecture::Aarch64 => {
                    isa_aarch64::encode_aarch64_selected_form(model, kind, key, &operands)
                        .map_err(|_| AdapterEncodingError)?
                        .bytes()
                        .to_vec()
                }
            }
        }
        Op::Call => match architecture {
            Architecture::X86_64 => isa_x86_64::ORDINARY_DIRECT_CALL_BYTES.to_vec(),
            Architecture::Aarch64 => isa_aarch64::ORDINARY_DIRECT_CALL_BYTES.to_vec(),
        },
        Op::Return => match architecture {
            Architecture::X86_64 => isa_x86_64::ORDINARY_RETURN_BYTES.to_vec(),
            Architecture::Aarch64 => isa_aarch64::ORDINARY_RETURN_BYTES.to_vec(),
        },
        _ if architecture != Architecture::X86_64 => return Err(AdapterEncodingError),
        Op::InterruptReturn => isa_x86_64::INTERRUPT_RETURN_BYTES.to_vec(),
        Op::ClearDirection => isa_x86_64::CLEAR_DIRECTION_FLAG_BYTES.to_vec(),
        Op::X86PushPop { register, pop } => {
            if register >= 16 {
                return Err(AdapterEncodingError);
            }
            let mut encoded = Vec::new();
            if register >= 8 {
                encoded.push(0x41);
            }
            encoded.push((if pop { 0x58 } else { 0x50 }) + (register & 7));
            encoded
        }
        Op::X86AdjustStack { amount, subtract } => {
            vec![0x48, 0x83, if subtract { 0xec } else { 0xc4 }, amount]
        }
        Op::X86RequireEqual { register, value } => {
            let code = gpr_code(register).ok_or(AdapterEncodingError)?;
            vec![
                0x48 | (code >> 3),
                0x83,
                0xf8 | (code & 7),
                value,
                0x74,
                0x02,
                0x0f,
                0x0b,
            ]
        }
        Op::X86IncrementChecked { register } => {
            let code = gpr_code(register).ok_or(AdapterEncodingError)?;
            vec![
                0x48 | (code >> 3),
                0x83,
                0xc0 | (code & 7),
                1,
                0x73,
                0x02,
                0x0f,
                0x0b,
            ]
        }
        Op::X86DescriptorStore { offset, table } => {
            let modrm = match table {
                X86DescriptorTable::Global => 0x84,
                X86DescriptorTable::Interrupt => 0x8c,
            };
            let mut encoded = vec![0x0f, 0x01, modrm, 0x24];
            encoded.extend_from_slice(&offset.to_le_bytes());
            encoded
        }
        Op::X86CompareEax { value } => {
            let mut encoded = vec![0x3d];
            encoded.extend_from_slice(&value.to_le_bytes());
            encoded
        }
        Op::X86Require(condition) => {
            let jump = match condition {
                X86Condition::Equal => 0x74,
                X86Condition::NotEqual => 0x75,
                X86Condition::UnsignedLessOrEqual => 0x76,
            };
            let mut encoded = vec![jump, 4];
            encoded.extend_from_slice(&isa_x86_64::PRIVILEGED_STOP_BYTES);
            encoded
        }
        Op::X86Stop => isa_x86_64::PRIVILEGED_STOP_BYTES.to_vec(),
        Op::X86Fixed(template) => template_bytes(template).to_vec(),
        Op::X86Scalar32(operation) => encode_scalar32(operation)?,
    };
    bytes.extend_from_slice(&encoded);
    Ok(())
}

fn encode_scalar32(operation: Scalar32Operation) -> Result<Vec<u8>, Error> {
    use Scalar32Operation as S;
    let register = |register: MachineRegister| {
        gpr_code(register)
            .filter(|code| *code < 8)
            .ok_or(AdapterEncodingError)
    };
    let immediate = |mut bytes: Vec<u8>, value: u32| {
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    };
    Ok(match operation {
        S::Move {
            destination,
            source,
        } => vec![0x89, 0xc0 | register(source)? << 3 | register(destination)?],
        S::Or {
            destination,
            source,
        } => vec![0x09, 0xc0 | register(source)? << 3 | register(destination)?],
        S::And { register: r, value } => {
            let r = register(r)?;
            immediate(
                if r == 0 {
                    vec![0x25]
                } else {
                    vec![0x81, 0xe0 | r]
                },
                value,
            )
        }
        S::Compare { register: r, value } => immediate(vec![0x81, 0xf8 | register(r)?], value),
        S::Test { register: r, value } => immediate(vec![0xf7, 0xc0 | register(r)?], value),
        S::ShiftLeft { register: r, bits } if bits < 32 => vec![0xc1, 0xe0 | register(r)?, bits],
        S::ShiftLeft { .. } => return Err(AdapterEncodingError),
        S::ByteSwap { register: r } => vec![0x0f, 0xc8 | register(r)?],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use AdapterOperation as Op;
    use MachineRegister as R;

    fn check(architecture: Architecture, operations: Vec<Op>) -> Vec<u8> {
        let mut program = AdapterInstructions {
            architecture,
            operations,
        };
        program.push(Op::Call);
        program.push(Op::Return);
        let effects = program.effects();
        let (bytes, call) = super::encode_instructions(&program).unwrap();
        let decoded = match architecture {
            Architecture::X86_64 => {
                isa_x86_64::decode_x86_64_generated_entry(&bytes, Some(call as u32))
            }
            Architecture::Aarch64 => {
                isa_aarch64::decode_aarch64_generated_entry(&bytes, Some(call as u32))
            }
        }
        .unwrap();
        assert_eq!(effects, decoded.effects, "{:?}", program.operations);
        bytes
    }

    #[test]
    fn physical_adapter_effects_agree_with_independent_target_decoders() {
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            let (register, scratch, vector) = match architecture {
                Architecture::X86_64 => (R::X86Rax, R::X86R11, R::X86Xmm(13)),
                Architecture::Aarch64 => (R::Aarch64X(0), R::Aarch64X(17), R::Aarch64V(29)),
            };
            for frame in [0, 16, 8192] {
                for enter in [false, true] {
                    check(
                        architecture,
                        vec![Op::Frame {
                            bytes: frame,
                            interval: 4096,
                            touches: if frame > 4096 { frame / 4096 } else { 0 },
                            enter,
                        }],
                    );
                }
            }
            for (register, size) in [(register, 1), (register, 2), (register, 8), (vector, 16)] {
                for offset in [0, 16, 65536] {
                    for load in [false, true] {
                        check(
                            architecture,
                            vec![Op::FrameMemory {
                                register,
                                offset,
                                size,
                                load,
                                scratch,
                            }],
                        );
                        check(
                            architecture,
                            vec![Op::PointerMemory {
                                register,
                                base: scratch,
                                offset,
                                size,
                                load,
                            }],
                        );
                    }
                }
            }
            for offset in [0, 16, 4096, 4097, 0x100_0000] {
                check(architecture, vec![Op::FrameAddress { register, offset }]);
            }
            for value in [
                0,
                0xffff,
                u64::MAX,
                0xffff_ffff_1234_ffff,
                0x1234_5678_9012_3456,
            ] {
                check(architecture, vec![Op::Constant { register, value }]);
            }
            for member in [0, 8, 65536] {
                check(
                    architecture,
                    vec![Op::AlignedFrameAddress {
                        register,
                        offset: 16,
                        alignment: 64,
                        member,
                    }],
                );
            }
        }
    }

    #[test]
    fn integer_adapter_checks_require_an_x86_general_purpose_register() {
        for register in [R::X86Xmm(0), R::Aarch64X(0), R::Aarch64V(0)] {
            for operation in [
                Op::X86RequireEqual { register, value: 1 },
                Op::X86IncrementChecked { register },
            ] {
                let program = AdapterInstructions {
                    architecture: Architecture::X86_64,
                    operations: vec![operation, Op::Call, Op::Return],
                };
                assert_eq!(encode_instructions(&program), Err(AdapterEncodingError));
            }
        }
    }

    #[test]
    fn installed_adapter_preserves_the_descriptor_table_and_guard_predicate() {
        for (operation, expected) in [
            (
                Op::X86DescriptorStore {
                    offset: 16,
                    table: X86DescriptorTable::Global,
                },
                &[0x0f, 0x01, 0x84, 0x24, 16, 0, 0, 0][..],
            ),
            (
                Op::X86DescriptorStore {
                    offset: 16,
                    table: X86DescriptorTable::Interrupt,
                },
                &[0x0f, 0x01, 0x8c, 0x24, 16, 0, 0, 0],
            ),
            (Op::X86Require(X86Condition::Equal), &[0x74, 4]),
            (Op::X86Require(X86Condition::NotEqual), &[0x75, 4]),
            (
                Op::X86Require(X86Condition::UnsignedLessOrEqual),
                &[0x76, 4],
            ),
        ] {
            let bytes = check(Architecture::X86_64, vec![operation]);
            assert_eq!(&bytes[..expected.len()], expected);
        }
    }

    #[test]
    fn installed_adapter_templates_declare_every_register_and_cpu_state_use() {
        use X86AdapterTemplate as T;
        for template in [
            T::AlignStack,
            T::MaskAndTestIst,
            T::ShiftHighGateAddress,
            T::MergeMiddleGateAddress,
            T::MergeLowGateAddress,
            T::ReadTaskSelector,
            T::TestSelectorAlignment,
            T::TestSelectorNonzero,
            T::SelectorLimit,
            T::CompareDescriptorLimit,
            T::AddDescriptorBase,
            T::HighDescriptorBase,
            T::MergeDescriptorBase,
            T::FinishDescriptorBase,
            T::LoadIstTop,
            T::LoadIstRoot,
            T::CompareActiveRoot,
            T::CompareProvider,
        ] {
            check(Architecture::X86_64, vec![Op::X86Fixed(template)]);
        }
        for operation in [
            Op::ClearDirection,
            Op::InterruptReturn,
            Op::X86PushPop {
                register: 12,
                pop: false,
            },
            Op::X86PushPop {
                register: 3,
                pop: true,
            },
            Op::X86AdjustStack {
                amount: 16,
                subtract: true,
            },
            Op::X86RequireEqual {
                register: R::X86R11,
                value: 1,
            },
            Op::X86IncrementChecked {
                register: R::X86R10,
            },
            Op::X86CompareEax { value: 4095 },
            Op::X86Stop,
        ] {
            check(Architecture::X86_64, vec![operation]);
        }
    }
}
