use super::*;

#[cfg(test)]
mod x86_import_plan_tests {
    use super::*;
    use omega_target_operations::{
        RuntimeStorageRegion, TargetInstructionOperand, TargetInstructionOperandKind,
    };

    fn operand(kind: TargetInstructionOperandKind) -> TargetInstructionOperand {
        TargetInstructionOperand { kind }
    }

    #[test]
    fn general_import_plan_carries_register_stack_and_result_placements() {
        let operands = std::iter::once(operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        ))
        .chain((0..6).map(|value| operand(TargetInstructionOperandKind::ImmediateInteger(value))))
        .collect::<Vec<_>>();

        let plan = normalized_win64_import_plan(&operands, true).expect("Microsoft x64 plan");

        assert_eq!(
            plan.parameters[0].locations,
            [ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
        assert_eq!(
            plan.parameters[4].locations,
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }]
        );
        assert_eq!(
            plan.parameters[5].locations,
            [ValueLocation::Stack {
                stack_byte_offset: 40,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }]
        );
        assert_eq!(
            normalized_win64_result_register(&plan, true).expect("result placement"),
            Some(MachineRegister::X86Rax)
        );
        let bytes = encode_win64_import_call(&operands, true, false)
            .expect("the general encoder must consume the evaluated placements");
        assert!(
            bytes.windows(2).any(|window| window == [0x49, 0xbb]),
            "the result base must use plan-clobbered r11"
        );
        assert!(!bytes.windows(2).any(|window| window == [0x49, 0xbf]));
    }

    #[test]
    fn win64_indirect_aggregate_arguments_use_aligned_copies_and_positional_pointers() {
        let operands = vec![
            operand(TargetInstructionOperandKind::ImmediateInteger(1)),
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(2)),
            operand(TargetInstructionOperandKind::ImmediateInteger(3)),
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 96,
                byte_count: 16,
                alignment: 8,
            }),
        ];
        let plan = normalized_win64_import_plan(&operands, false)
            .expect("Microsoft x64 aggregate argument plan");
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                copy_stack_byte_offset: Some(48),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Stack {
                    stack_byte_offset: 32,
                    ..
                },
                copy_stack_byte_offset: Some(80),
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, false, false)
            .expect("Microsoft x64 aggregate argument call");
        assert_eq!(
            bytes.len(),
            win64_import_call_width(&operands, false, false)
        );
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 104]);
        assert!(
            bytes
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x94, 0x24, 48, 0, 0, 0]),
            "the second positional argument must point RDX at its aligned copy"
        );
        assert!(
            bytes.windows(16).any(|window| window
                == [
                    0x48, 0x8d, 0x84, 0x24, 80, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 32, 0, 0, 0,
                ]),
            "the fifth positional argument must store its copy pointer above shadow space"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, false, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(4), None]
        );
    }

    #[test]
    fn win64_odd_width_record_uses_an_indirect_copy_without_breaking_stack_alignment() {
        let operands = [operand(
            TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 7,
                byte_count: 3,
                alignment: 1,
            },
        )];
        let plan = normalized_win64_import_plan(&operands, false)
            .expect("odd-width Microsoft x64 aggregate plan");
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                copy_stack_byte_offset: Some(32),
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, false, false)
            .expect("odd-width Microsoft x64 aggregate call");
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        assert_eq!(
            bytes.len(),
            win64_import_call_width(&operands, false, false)
        );
        assert!(
            bytes
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x8c, 0x24, 32, 0, 0, 0]),
            "RCX must point at the three-byte caller copy"
        );
    }

    #[test]
    fn win64_direct_aggregate_arguments_use_positional_registers_and_stack_slots() {
        let aggregate = |byte_offset, byte_count| {
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count,
                alignment: byte_count,
            })
        };
        let operands = [
            aggregate(0, 1),
            aggregate(8, 2),
            aggregate(16, 4),
            aggregate(24, 8),
            aggregate(32, 4),
        ];
        let plan = normalized_win64_import_plan(&operands, false)
            .expect("direct Microsoft x64 aggregate plan");
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                byte_size: 1,
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                byte_size: 4,
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, false, false)
            .expect("direct Microsoft x64 aggregate call");
        assert_eq!(
            bytes.len(),
            win64_import_call_width(&operands, false, false)
        );
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        for load in [
            &[0x41, 0x8a, 0x8b, 0, 0, 0, 0][..],
            &[0x66, 0x41, 0x8b, 0x93, 8, 0, 0, 0],
            &[0x45, 0x8b, 0x83, 16, 0, 0, 0],
            &[0x4d, 0x8b, 0x8b, 24, 0, 0, 0],
        ] {
            assert!(
                bytes.windows(load.len()).any(|window| window == load),
                "missing direct aggregate register load {load:02x?}"
            );
        }
        assert!(
            bytes
                .windows(14)
                .any(|window| window
                    == [0x41, 0x8b, 0x83, 32, 0, 0, 0, 0x89, 0x84, 0x24, 32, 0, 0, 0,]),
            "the fifth direct record must occupy the low bytes of stack slot 32"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, false, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2), Some(3), Some(4), None]
        );
    }

    #[test]
    fn win64_direct_aggregate_results_spill_rax_at_the_record_width() {
        for (byte_count, store) in [
            (1, &[0x41, 0x88, 0x83][..]),
            (2, &[0x66, 0x41, 0x89, 0x83][..]),
            (4, &[0x41, 0x89, 0x83][..]),
            (8, &[0x49, 0x89, 0x83][..]),
        ] {
            let operands = [operand(
                TargetInstructionOperandKind::RuntimeSmallAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 24,
                    byte_count,
                    alignment: byte_count,
                },
            )];
            let plan = normalized_win64_import_plan(&operands, true)
                .expect("direct Microsoft x64 aggregate result plan");
            assert_eq!(
                normalized_win64_result_register(&plan, true).expect("result register"),
                Some(MachineRegister::X86Rax)
            );

            let bytes = encode_win64_import_call(&operands, true, false)
                .expect("direct Microsoft x64 aggregate result call");
            assert_eq!(bytes.len(), win64_import_call_width(&operands, true, false));
            let store_start = bytes.len() - store.len() - 4;
            assert_eq!(&bytes[store_start..store_start + store.len()], store);
            assert_eq!(&bytes[bytes.len() - 4..], &24u32.to_le_bytes());
            assert_eq!(
                win64_import_call_relocation_sites(&operands, true, false)
                    .iter()
                    .map(|site| site.operand_index)
                    .collect::<Vec<_>>(),
                [None, Some(0)]
            );
        }
    }

    #[test]
    fn win64_indirect_aggregate_result_uses_hidden_rcx_and_shifts_arguments() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
        ];
        let plan = normalized_win64_import_plan(&operands, true)
            .expect("indirect Microsoft x64 aggregate result plan");
        assert!(plan.result.as_ref().is_some_and(win64_result_is_indirect));
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, true, false)
            .expect("indirect Microsoft x64 aggregate result call");
        assert_eq!(bytes.len(), win64_import_call_width(&operands, true, false));
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        assert_eq!(&bytes[4..6], &[0x49, 0xbb]);
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 64, 0, 0, 0],
            "RCX must address the caller-owned result record"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x93, 8, 0, 0, 0],
            "the first declared argument must shift to RDX"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, true, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), None]
        );
    }

    #[test]
    fn win64_scalar_floats_use_positional_xmm_registers_stack_and_xmm0_result() {
        let float = |byte_offset, byte_count| {
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count,
            })
        };
        let integer = |byte_offset| {
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 8,
            })
        };
        let operands = [
            float(0, 8),
            integer(8),
            float(16, 4),
            integer(24),
            float(32, 8),
            float(40, 4),
        ];
        let plan = normalized_win64_import_plan(&operands, true)
            .expect("Microsoft x64 scalar-float import plan");
        assert!(matches!(
            plan.result.as_ref().unwrap().locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(1),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[3].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(3),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, true, false)
            .expect("Microsoft x64 scalar-float import call");
        assert_eq!(bytes.len(), win64_import_call_width(&operands, true, false));
        for instruction in [
            &[0xf3, 0x41, 0x0f, 0x10, 0x8b, 16, 0, 0, 0][..],
            &[0xf2, 0x41, 0x0f, 0x10, 0x9b, 32, 0, 0, 0],
            &[0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0],
        ] {
            assert!(
                bytes
                    .windows(instruction.len())
                    .any(|window| window == instruction),
                "missing float instruction {instruction:02x?}"
            );
        }
        assert!(
            bytes
                .windows(14)
                .any(|window| window
                    == [0x41, 0x8b, 0x83, 40, 0, 0, 0, 0x89, 0x84, 0x24, 32, 0, 0, 0]),
            "the fifth-position f32 must occupy the low four bytes of stack slot 32"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, true, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(4), Some(5), None, Some(0)]
        );
    }

    #[test]
    fn win64_encoder_rejects_scratch_above_the_plan_clobber_ceiling() {
        let mut plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("baseline Microsoft x64 plan");
        plan.ordinary_clobbers = omega_calling_conventions::RegisterSet::new(
            plan.ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::X86R11),
        );

        let error =
            validate_win64_encoder_plan(&plan).expect_err("missing volatile scratch must reject");
        assert!(error.message.contains("X86R11"));
        assert!(error.message.contains("ordinary-clobber ceiling"));
    }

    #[test]
    fn compatibility_host_encoder_rejects_a_sysv_target_policy() {
        let key = HostOperationKey::new(HostCapability::Clock, HostOperation::TickCount);
        let operands = [operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        )];

        let error = encode_host_call_sequence_no_plan(CallingPolicy::SystemVAMD64, key, &operands)
            .expect_err("the Win64 compatibility encoder must not silently choose its ABI");

        assert!(error.message.contains("not SystemVAMD64"));
    }

    #[test]
    fn authored_import_consumes_the_supplied_plan_and_matching_relocation_walk() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(8, 8)),
        };
        let mut plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("baseline SysV plan");
        plan.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::X86Rcx,
            value_byte_offset: 0,
            byte_size: 8,
        }];
        omega_calling_conventions::validate_call_plan(&plan, &signature)
            .expect("source-selected nondefault placement remains structurally valid");

        let bytes = encode_authored_import_call_sequence(&plan, &operands)
            .expect("source-selected authored import");
        assert!(
            bytes.windows(2).any(|window| window == [0x48, 0xb9]),
            "the authored parameter placement must select rcx"
        );
        assert!(
            !bytes.windows(2).any(|window| window == [0x48, 0xbf]),
            "the target-derived SysV rdi placement must not replace the authored plan"
        );

        let sites = authored_import_relocation_sites(&plan, &operands);
        let call = sites
            .iter()
            .find(|site| site.kind == X86_64RelocationSiteKind::Relative32)
            .expect("call relocation");
        assert_eq!(bytes[call.byte_offset - 1], 0xe8);
        assert_eq!(
            sites
                .iter()
                .filter(|site| site.kind == X86_64RelocationSiteKind::Absolute64)
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0)]
        );
    }

    #[test]
    fn authored_sysv_small_aggregates_use_planned_registers_and_results() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 16,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 16,
                alignment: 8,
            }),
        ];
        let key = HostOperationKey::new(HostCapability::Unknown, HostOperation::Unknown);
        let layout = sysv_import_layout(&operands, true).expect("SysV aggregate import layout");

        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 8]);
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 32, 0, 0, 0]),
            "tag must load into planned rdi"
        );
        assert!(
            layout
                .bytes
                .windows(14)
                .any(|window| window
                    == [0x49, 0x8b, 0xb3, 40, 0, 0, 0, 0x49, 0x8b, 0x93, 48, 0, 0, 0]),
            "aggregate fragments must load into planned rsi/rdx"
        );
        assert!(
            layout.bytes.windows(14).any(
                |window| window == [0x49, 0x89, 0x83, 0, 0, 0, 0, 0x49, 0x89, 0x93, 8, 0, 0, 0]
            ),
            "result fragments must store from planned rax/rdx"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), None, Some(0)]
        );
        assert_eq!(
            encode_host_call_sequence_no_plan(CallingPolicy::SystemVAMD64, key, &operands)
                .expect("routed SysV authored import"),
            layout.bytes
        );
    }

    #[test]
    fn authored_sysv_scalar_floats_use_the_independent_xmm_bank_and_result() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
        ];
        let layout = sysv_import_layout(&operands, true).expect("SysV scalar-float import");

        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0]),
            "first float must load into xmm0 independently of rdi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x8b, 32, 0, 0, 0]),
            "second float must load into xmm1 independently of rsi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0]),
            "the float result must spill from planned xmm0"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(4), None, Some(0)]
        );
    }

    #[test]
    fn authored_sysv_ninth_scalar_float_moves_to_the_stack() {
        let mut operands = vec![operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        )];
        operands.extend((0..9).map(|index| {
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16 + index * 8,
                byte_count: 8,
            })
        }));
        operands.push(operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 96,
                byte_count: 8,
            },
        ));

        let layout = sysv_import_layout(&operands, true).expect("SysV stack-float import");
        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 8]);
        assert!(
            layout.bytes.windows(15).any(|window| window
                == [
                    0x49, 0x8b, 0x83, 80, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 0, 0, 0, 0,
                ]),
            "the ninth float's bits must occupy outgoing stack offset zero: {:02x?}",
            layout.bytes
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 96, 0, 0, 0]),
            "the independent integer bank must still start at rdi"
        );
    }

    #[test]
    fn authored_sysv_register_exhausted_aggregate_rolls_wholly_to_stack() {
        let mut operands = vec![operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        )];
        operands.extend((0..5).map(|index| {
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32 + index * 8,
                byte_count: 8,
            })
        }));
        operands.push(operand(
            TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 80,
                byte_count: 16,
                alignment: 8,
            },
        ));
        operands.push(operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 96,
                byte_count: 8,
            },
        ));

        let layout = sysv_import_layout(&operands, true).expect("SysV rollback import layout");
        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 24]);
        assert!(
            layout.bytes.windows(30).any(|window| window
                == [
                    0x49, 0x8b, 0x83, 80, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 0, 0, 0, 0, 0x49, 0x8b,
                    0x83, 88, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 8, 0, 0, 0,
                ]),
            "the complete aggregate must occupy outgoing stack offsets 0 and 8"
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x4d, 0x8b, 0x8b, 96, 0, 0, 0]),
            "the trailing scalar must retain the rolled-back r9 register"
        );
    }

    #[test]
    fn sysv_vtable_field_marshals_wire_arguments_and_small_result() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 16,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 48,
                byte_count: 16,
                alignment: 8,
            }),
        ];
        let layout = sysv_field_call_layout_for_plan(
            &operands,
            24,
            true,
            true,
            HostCallPlan::CompatibilityOracle,
        )
        .expect("SysV vtable field call");

        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 32, 0, 0, 0]),
            "receiver must load into planned rdi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0x48, 0x8b, 0x87, 24, 0, 0, 0, 0xff, 0xd0]),
            "dispatch must read the field from the receiver and call rax"
        );
        assert!(
            layout.bytes.windows(14).any(
                |window| window == [0x49, 0x89, 0x83, 0, 0, 0, 0, 0x49, 0x89, 0x93, 8, 0, 0, 0]
            ),
            "small result must spill from planned rax/rdx fragments"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(0)]
        );
    }

    #[test]
    fn sysv_table_function_excludes_dispatch_table_from_wire_signature() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 8,
            }),
        ];
        let layout = sysv_field_call_layout_for_plan(
            &operands,
            40,
            true,
            false,
            HostCallPlan::CompatibilityOracle,
        )
        .expect("SysV table-function call");

        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0]),
            "first wire float must use xmm0"
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 24, 0, 0, 0]),
            "first wire integer must use rdi, proving the table consumed no slot"
        );
        assert!(
            layout.bytes.windows(16).any(|window| window
                == [
                    0x49, 0x8b, 0x83, 8, 0, 0, 0, 0x48, 0x8b, 0x80, 40, 0, 0, 0, 0xff, 0xd0,
                ]),
            "dispatch must load the table slot, then the function field"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0]),
            "float result must spill from xmm0"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(2), Some(3), Some(1), Some(0)]
        );
    }

    #[test]
    fn authored_sysv_memory_class_uses_stack_and_hidden_result_pointer() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 8,
            }),
        ];
        let layout = sysv_import_layout(&operands, true).expect("SysV MEMORY-class import");

        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 24]);
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8d, 0xbb, 0, 0, 0, 0]),
            "hidden result destination must materialize in rdi"
        );
        for stack_offset in [0u8, 8, 16] {
            assert!(
                layout
                    .bytes
                    .windows(8)
                    .any(|window| window == [0x48, 0x89, 0x84, 0x24, stack_offset, 0, 0, 0]),
                "large argument fragment must occupy stack offset {stack_offset}"
            );
        }
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xb3, 64, 0, 0, 0]),
            "declared scalar must shift to rsi behind the hidden result pointer"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2), None]
        );
    }

    #[test]
    fn authored_sysv_two_f64_record_uses_xmm_fragments_and_result() {
        let operands = [
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    member_byte_count: 8,
                    members: 2,
                },
            ),
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 16,
                    member_byte_count: 8,
                    members: 2,
                },
            ),
        ];
        let layout = sysv_import_layout(&operands, true).expect("SysV two-f64 record import");

        assert!(
            layout.bytes.windows(18).any(|window| window
                == [
                    0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0, 0xf2, 0x41, 0x0f, 0x10, 0x8b, 24, 0,
                    0, 0,
                ]),
            "argument members must load into xmm0/xmm1"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0])
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x8b, 8, 0, 0, 0])
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), None, Some(0)]
        );
    }

    #[test]
    fn authored_sysv_three_f32_record_packs_by_eightbyte() {
        let aggregate = || {
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 16,
                    member_byte_count: 4,
                    members: 3,
                },
            )
        };
        let operands = [aggregate(), aggregate()];
        let layout = sysv_import_layout(&operands, true).expect("SysV three-f32 record import");

        assert!(layout.bytes.windows(18).any(|window| window
            == [
                0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0, 0xf3, 0x41, 0x0f, 0x10, 0x8b, 24, 0, 0,
                0,
            ]));
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 16, 0, 0, 0])
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf3, 0x41, 0x0f, 0x11, 0x8b, 24, 0, 0, 0])
        );
    }

    #[test]
    fn authored_sysv_mixed_record_uses_rax_and_xmm0_result() {
        let aggregate = |byte_offset| {
            operand(TargetInstructionOperandKind::RuntimeSystemVAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 16,
                alignment: 8,
                sse_eightbytes: 0b10,
            })
        };
        let operands = [aggregate(0), aggregate(16)];
        let layout = sysv_import_layout(&operands, true).expect("SysV mixed aggregate import");

        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 16, 0, 0, 0]),
            "INTEGER argument eightbyte must load into rdi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x83, 24, 0, 0, 0]),
            "SSE argument eightbyte must load into xmm0"
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x89, 0x83, 0, 0, 0, 0]),
            "INTEGER result eightbyte must store from rax"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 8, 0, 0, 0]),
            "SSE result eightbyte must store from xmm0"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), None, Some(0)]
        );
    }

    #[test]
    fn authored_sysv_nonhomogeneous_sse_record_uses_two_xmm_fragments() {
        let aggregate = |byte_offset| {
            operand(TargetInstructionOperandKind::RuntimeSystemVAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 16,
                alignment: 8,
                sse_eightbytes: 0b11,
            })
        };
        let operands = [aggregate(0), aggregate(16)];
        let layout =
            sysv_import_layout(&operands, true).expect("SysV non-homogeneous SSE aggregate");

        assert!(layout.bytes.windows(18).any(|window| window
            == [
                0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0, 0xf2, 0x41, 0x0f, 0x10, 0x8b, 24, 0, 0,
                0,
            ]));
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0])
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x8b, 8, 0, 0, 0])
        );
    }

    #[test]
    fn sysv_vtable_large_result_shifts_receiver_to_rsi() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 8,
            }),
        ];
        let layout = sysv_field_call_layout_for_plan(
            &operands,
            24,
            true,
            true,
            HostCallPlan::CompatibilityOracle,
        )
        .expect("SysV sret vtable call");

        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0x48, 0x8b, 0x86, 24, 0, 0, 0, 0xff, 0xd0]),
            "receiver dispatch must use planned rsi behind hidden rdi"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2)]
        );
    }

    #[test]
    fn authored_sysv_encoder_rejects_scratch_above_the_plan_clobber_ceiling() {
        let mut plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(16, 8)],
                result: Some(ValueShape::integer(16, 8)),
            },
        )
        .expect("baseline SysV aggregate plan");
        plan.ordinary_clobbers = omega_calling_conventions::RegisterSet::new(
            plan.ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::X86R11),
        );

        let error = validate_sysv_import_plan(&plan)
            .expect_err("missing volatile staging scratch must reject");
        assert!(error.message.contains("X86R11"));
        assert!(error.message.contains("ordinary-clobber ceiling"));
    }

    #[test]
    fn non_boundary_constant_results_remain_policy_independent() {
        let key = HostOperationKey::new(
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond,
        );
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(
                1_000_000_000,
            )),
        ];

        encode_host_call_sequence_no_plan(CallingPolicy::SystemVAMD64, key, &operands)
            .expect("constant materialization does not apply a calling policy");
        assert_eq!(
            constant_host_result_clobbers().as_slice(),
            &[MachineRegister::X86Rax, MachineRegister::X86R15]
        );
    }

    #[test]
    fn simple_kernel32_calls_keep_their_exact_bytes_and_relocations() {
        let get_std = HostOperationKey::new(HostCapability::Stdout, HostOperation::GetStdHandle);
        let get_std_operands = [operand(TargetInstructionOperandKind::ImmediateInteger(-11))];
        let get_std_plan = evaluate_normalized_win64_plan(&CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result: Some(ValueShape::integer(8, 8)),
        })
        .expect("GetStdHandle native plan");
        validate_normalized_win64_get_std_handle_plan(HostCallPlan::Authoritative(&get_std_plan))
            .expect("retained GetStdHandle plan");
        let bytes = encode_host_call_sequence_no_plan(
            CallingPolicy::MicrosoftX64,
            get_std,
            &get_std_operands,
        )
        .expect("plan-driven GetStdHandle");
        assert_eq!(
            bytes,
            encode_host_call_sequence_with_plan(
                CallingPolicy::MicrosoftX64,
                get_std,
                &get_std_operands,
                &get_std_plan,
            )
            .expect("retained GetStdHandle plan drives the fixed encoder")
        );
        assert_eq!(
            bytes,
            [
                0x48, 0x83, 0xec, 0x28, 0xb9, 0xf5, 0xff, 0xff, 0xff, 0xe8, 0, 0, 0, 0, 0x48, 0x83,
                0xc4, 0x28,
            ]
        );
        assert_eq!(
            host_call_relocation_sites(get_std, &get_std_operands),
            [X86_64RelocationSite {
                operand_index: None,
                byte_offset: 10,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            }]
        );

        let exit = HostOperationKey::new(HostCapability::Process, HostOperation::ExitProcess);
        let dword_literal_operands = [operand(TargetInstructionOperandKind::ImmediateInteger(70))];
        let dword_plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result: None,
            },
        )
        .expect("DWORD literal plan");
        assert_eq!(
            encode_host_call_sequence_with_plan(
                CallingPolicy::MicrosoftX64,
                exit,
                &dword_literal_operands,
                &dword_plan,
            )
            .expect("the selected DWORD plan types its contextual literal"),
            encode_host_call_sequence_no_plan(
                CallingPolicy::MicrosoftX64,
                exit,
                &dword_literal_operands,
            )
            .expect("compatibility literal encoding"),
        );
        let exit_operands = [operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 4,
            },
        )];
        encode_host_call_sequence_no_plan(CallingPolicy::MicrosoftX64, exit, &exit_operands)
            .expect("plan-driven ExitProcess");
        let sites = host_call_relocation_sites(exit, &exit_operands);
        assert_eq!(sites[0].byte_offset, 6, "runtime region-base imm64");
        assert_eq!(sites[1].byte_offset, 22, "call rel32");
    }

    #[test]
    fn time_out_parameter_plans_model_the_actual_native_signatures() {
        let qpc = normalized_win64_out_param_plan(
            HostOperation::MonotonicTicks,
            HostCallPlan::CompatibilityOracle,
        )
        .expect("QueryPerformanceCounter plan");
        assert_eq!(
            qpc.parameters[0].locations,
            [ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
        assert_eq!(
            normalized_win64_result_register(&qpc, true).expect("QPC native BOOL result"),
            Some(MachineRegister::X86Rax)
        );

        let filetime = normalized_win64_out_param_plan(
            HostOperation::WallClockRaw,
            HostCallPlan::CompatibilityOracle,
        )
        .expect("GetSystemTimePreciseAsFileTime plan");
        assert!(
            filetime.result.is_none(),
            "FILETIME native call returns void"
        );

        let wrong = evaluate_normalized_win64_plan(&CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(8, 8)),
        })
        .expect("unrelated Win64 plan");
        assert!(
            normalized_win64_out_param_plan(
                HostOperation::MonotonicTicks,
                HostCallPlan::Authoritative(&wrong),
            )
            .is_err(),
            "a retained semantic-result plan must not replace QPC's native out-pointer plan"
        );
    }

    #[test]
    fn file_io_plan_models_registers_stack_argument_and_native_result() {
        let plan = normalized_win64_file_io_plan(HostCallPlan::CompatibilityOracle)
            .expect("ReadFile/WriteFile plan");
        let expected_registers = [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
        ];
        for (index, expected) in expected_registers.into_iter().enumerate() {
            assert_eq!(
                win64_argument_location(&plan.parameters[index], index)
                    .expect("file-I/O register placement"),
                Win64ArgumentLocation::Register(expected)
            );
        }
        assert_eq!(
            win64_argument_location(&plan.parameters[4], 4).expect("OVERLAPPED stack placement"),
            Win64ArgumentLocation::Stack(32)
        );
        assert_eq!(
            normalized_win64_result_register(&plan, true).expect("native BOOL result"),
            Some(MachineRegister::X86Rax)
        );
        assert_eq!(
            win64_composite_reserve(48).expect("outgoing area plus temporary"),
            56
        );
        assert_eq!(
            normalized_win64_file_io_layout(HostCallPlan::Authoritative(&plan))
                .expect("retained native file plan"),
            normalized_win64_file_io_layout(HostCallPlan::CompatibilityOracle)
                .expect("compatibility file plan")
        );
        let wrong = evaluate_normalized_win64_plan(&CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 3],
            result: Some(ValueShape::integer(8, 8)),
        })
        .expect("unrelated three-word plan");
        assert!(
            normalized_win64_file_io_layout(HostCallPlan::Authoritative(&wrong)).is_err(),
            "a retained outer adapter plan must not replace ReadFile/WriteFile's native plan"
        );
    }
}
#[cfg(test)]
mod call_encoding_tests {
    use super::append_call_register;

    #[test]
    fn low_registers_emit_ff_d0_through_ff_d7_without_rex() {
        // `FF /2` register-direct: ModRM = 0xD0 | rm, no REX for rax..rdi.
        // rax=D0 rcx=D1 rdx=D2 rbx=D3 rsp=D4 rbp=D5 rsi=D6 rdi=D7.
        for reg in 0u8..8 {
            let mut bytes = Vec::new();
            append_call_register(&mut bytes, reg);
            assert_eq!(
                bytes,
                vec![0xff, 0xd0 + reg],
                "call r{reg} must be FF {:02X} with no REX",
                0xd0 + reg
            );
        }
    }

    #[test]
    fn extended_registers_take_a_rex_b_prefix() {
        // r8..r15 need REX.B (0x41); ModRM low 3 bits wrap (r8 -> D0, r11 -> D3).
        for reg in 8u8..16 {
            let mut bytes = Vec::new();
            append_call_register(&mut bytes, reg);
            assert_eq!(
                bytes,
                vec![0x41, 0xff, 0xd0 | (reg & 0x7)],
                "call r{reg} must be 41 FF {:02X}",
                0xd0 | (reg & 0x7)
            );
        }
    }

    #[test]
    fn canonical_targets_are_exact() {
        // Spot-check the registers the first-boot path actually uses.
        let mut rax = Vec::new();
        append_call_register(&mut rax, 0);
        assert_eq!(rax, vec![0xff, 0xd0], "call rax");

        let mut r11 = Vec::new();
        append_call_register(&mut r11, 11);
        assert_eq!(r11, vec![0x41, 0xff, 0xd3], "call r11");
    }
}
#[cfg(test)]
mod vtable_call_encoding_tests {
    use super::{
        X86_64RelocationSiteKind, encode_win64_table_function_call, encode_win64_vtable_call,
        encode_win64_vtable_call_at_offset, win64_table_function_call_relocation_sites,
        win64_table_function_call_width, win64_vtable_call_relocation_sites,
        win64_vtable_call_width,
    };
    use omega_target_operations::{InstructionOperandLike, RuntimeStorageRegion};

    /// A minimal operand: either a runtime scalar (RCX = this from a field) or
    /// a runtime storage address (RDX = &text field). Everything else None.
    enum Op {
        Scalar {
            region: RuntimeStorageRegion,
            offset: usize,
            size: usize,
        },
        Float {
            region: RuntimeStorageRegion,
            offset: usize,
            size: usize,
        },
        Address {
            region: RuntimeStorageRegion,
            offset: usize,
        },
        Aggregate {
            region: RuntimeStorageRegion,
            offset: usize,
            size: usize,
            alignment: usize,
        },
    }
    impl InstructionOperandLike for Op {
        fn data_address(&self) -> Option<omega_target_operations::TargetDataObjectHandle> {
            None
        }
        fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_string_is_bounded_buffer(&self) -> bool {
            false
        }
        fn runtime_pointee_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_pointee_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_scalar_integer(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
            match self {
                Op::Scalar {
                    region,
                    offset,
                    size,
                } => Some((*region, *offset, *size)),
                _ => None,
            }
        }
        fn runtime_scalar_float(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
            match self {
                Op::Float {
                    region,
                    offset,
                    size,
                } => Some((*region, *offset, *size)),
                _ => None,
            }
        }
        fn runtime_large_aggregate(&self) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
            match self {
                Op::Aggregate {
                    region,
                    offset,
                    size,
                    alignment,
                } => Some((*region, *offset, *size, *alignment)),
                _ => None,
            }
        }
        fn runtime_storage_address(&self) -> Option<(RuntimeStorageRegion, usize)> {
            match self {
                Op::Address { region, offset } => Some((*region, *offset)),
                _ => None,
            }
        }
        fn immediate_integer(&self) -> Option<i64> {
            None
        }
        fn byte_length(&self) -> Option<usize> {
            None
        }
    }

    #[test]
    fn output_string_marshals_this_and_text_then_calls_through_slot_1() {
        // output_string(this: addr@machine+0, text: &field@machine+8) -> VtableSlot(1).
        let operands = vec![
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Address {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
            },
        ];
        let bytes = encode_win64_vtable_call(&operands, 1).expect("encode");
        assert_eq!(
            bytes.len(),
            win64_vtable_call_width(&operands, 1, false),
            "width matches"
        );

        // 2 register args -> reserve = 32 (padded to 40); sub rsp, 40 (imm8).
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        // arg 0 (this -> RCX): mov r11,imm64 (10) then mov rcx,[r11+0].
        assert_eq!(bytes[4], 0x49, "mov r11,imm64 opcode #0");
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8b, 0x8b, 0, 0, 0, 0],
            "rcx = [r11+0]"
        );
        // arg 1 (text -> RDX lea): mov r11,imm64 then lea rdx,[r11+8].
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8d, 0x93, 8, 0, 0, 0],
            "lea rdx, [r11+8]"
        );
        // the vtable read + indirect call, then restore.
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x81, 8, 0, 0, 0],
            "mov rax, [rcx+8] (slot 1)"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
    }

    #[test]
    fn vtable_call_with_result_skips_the_result_operand_and_stores_rax() {
        // let status = protocol.method(text): operands = [result, this, text];
        // the result place must NOT marshal as an argument (the old encoder
        // put it in RCX and dispatched through it -- the M2 #UD at 0xB0000).
        let operands = vec![
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Address {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
            },
        ];
        let bytes = encode_win64_vtable_call_at_offset(&operands, 8, true).expect("encode");
        assert_eq!(
            bytes.len(),
            win64_vtable_call_width(&operands, 8, true),
            "width matches"
        );

        // Args marshal exactly as the no-result shape: this -> RCX, text -> RDX.
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8b, 0x8b, 0, 0, 0, 0],
            "rcx = [r11+0] (this)"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8d, 0x93, 8, 0, 0, 0],
            "lea rdx, [r11+8]"
        );
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x81, 8, 0, 0, 0],
            "mov rax, [rcx+8]"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
        // The result store tail: mov r11,imm64 (relocated) + mov [r11+16], rax.
        assert_eq!(
            &bytes[51..53],
            &[0x49, 0xbb],
            "mov r11, imm64 (result base)"
        );
        assert_eq!(
            &bytes[61..68],
            &[0x49, 0x89, 0x83, 16, 0, 0, 0],
            "mov [r11+16], rax"
        );
        assert_eq!(bytes.len(), 68);
    }

    #[test]
    fn table_function_call_keeps_the_table_off_the_wire() {
        // let status = boot_services.get_memory_map(&arg): operands =
        // [result@16, table@0, &arg@8]. EFI table services take NO This: the
        // declared argument after the table lands in RCX, and the table is
        // read only to load the fn-ptr field (+56 here).
        let operands = vec![
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Address {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
            },
        ];
        let bytes = encode_win64_table_function_call(&operands, 56, true).expect("encode");
        assert_eq!(
            bytes.len(),
            win64_table_function_call_width(&operands, 56, true),
            "width matches"
        );

        // One register arg -> reserve 40; the FIRST DECLARED ARG (not the
        // table) lands in RCX.
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 8, 0, 0, 0],
            "lea rcx, [r11+8] (arg)"
        );
        // The table pointer loads for dispatch only: mov r11,imm64 (relocated
        // to the table's region base) + mov rax,[r11+0], then the fn-ptr read.
        assert_eq!(&bytes[21..23], &[0x49, 0xbb], "mov r11, imm64 (table base)");
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x83, 0, 0, 0, 0],
            "rax = [r11+0] (table)"
        );
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x80, 56, 0, 0, 0],
            "rax = [rax+56] (fn ptr)"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
        // Result store tail.
        assert_eq!(
            &bytes[51..53],
            &[0x49, 0xbb],
            "mov r11, imm64 (result base)"
        );
        assert_eq!(
            &bytes[61..68],
            &[0x49, 0x89, 0x83, 16, 0, 0, 0],
            "mov [r11+16], rax"
        );
        assert_eq!(bytes.len(), 68);

        // Relocation sites: the arg lea (operand 2) at 4+2, the table load
        // (operand 1) at 21+2, the result store (operand 0) at 51+2 -- all
        // Absolute64 region bases.
        let sites = win64_table_function_call_relocation_sites(&operands, true);
        let offsets: Vec<(Option<usize>, usize)> = sites
            .iter()
            .map(|site| (site.operand_index, site.byte_offset))
            .collect();
        assert_eq!(offsets, vec![(Some(2), 6), (Some(1), 23), (Some(0), 53)]);
        assert!(
            sites
                .iter()
                .all(|site| matches!(site.kind, X86_64RelocationSiteKind::Absolute64))
        );
    }

    #[test]
    fn indirect_calls_share_win64_aggregate_caller_copy_layouts() {
        let receiver = || Op::Scalar {
            region: RuntimeStorageRegion::Machine,
            offset: 0,
            size: 8,
        };
        let aggregate = || Op::Aggregate {
            region: RuntimeStorageRegion::Machine,
            offset: 16,
            size: 24,
            alignment: 8,
        };

        let vtable_operands = vec![receiver(), aggregate()];
        let vtable = encode_win64_vtable_call_at_offset(&vtable_operands, 8, false)
            .expect("Win64 vtable aggregate call");
        assert_eq!(
            vtable.len(),
            win64_vtable_call_width(&vtable_operands, 8, false)
        );
        assert_eq!(&vtable[..4], &[0x48, 0x83, 0xec, 56]);
        assert!(
            vtable
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x94, 0x24, 32, 0, 0, 0]),
            "the record following the receiver must point RDX at its copy"
        );
        assert_eq!(
            win64_vtable_call_relocation_sites(&vtable_operands, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );

        let table_operands = vec![receiver(), aggregate()];
        let table = encode_win64_table_function_call(&table_operands, 56, false)
            .expect("Win64 service-table aggregate call");
        assert_eq!(
            table.len(),
            win64_table_function_call_width(&table_operands, 56, false)
        );
        assert_eq!(&table[..4], &[0x48, 0x83, 0xec, 56]);
        assert!(
            table
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x8c, 0x24, 32, 0, 0, 0]),
            "the first declared service argument must point RCX at its copy"
        );
        assert_eq!(
            win64_table_function_call_relocation_sites(&table_operands, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(0)]
        );
    }

    #[test]
    fn indirect_vtable_result_shifts_the_receiver_and_has_no_store_tail() {
        let operands = vec![
            Op::Aggregate {
                region: RuntimeStorageRegion::Machine,
                offset: 32,
                size: 24,
                alignment: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
        ];
        let bytes = encode_win64_vtable_call_at_offset(&operands, 8, true)
            .expect("Win64 vtable indirect result call");
        assert_eq!(bytes.len(), win64_vtable_call_width(&operands, 8, true));
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 32, 0, 0, 0],
            "hidden RCX must address the result record"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x93, 0, 0, 0, 0],
            "the receiver must shift to RDX"
        );
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x82, 8, 0, 0, 0],
            "dispatch must read through the shifted receiver"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0]);
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40]);
        assert_eq!(bytes.len(), 51, "the callee writes the result in place");
        assert_eq!(
            win64_vtable_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| (site.operand_index, site.byte_offset))
                .collect::<Vec<_>>(),
            [(Some(0), 6), (Some(1), 23)]
        );
    }

    #[test]
    fn indirect_table_function_result_shifts_declared_arguments_only() {
        let operands = vec![
            Op::Aggregate {
                region: RuntimeStorageRegion::Machine,
                offset: 32,
                size: 24,
                alignment: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
                size: 8,
            },
        ];
        let bytes = encode_win64_table_function_call(&operands, 56, true)
            .expect("Win64 service-table indirect result call");
        assert_eq!(
            bytes.len(),
            win64_table_function_call_width(&operands, 56, true)
        );
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 32, 0, 0, 0],
            "hidden RCX must address the result record"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x93, 8, 0, 0, 0],
            "the first declared service argument must shift to RDX"
        );
        assert_eq!(
            &bytes[48..55],
            &[0x49, 0x8b, 0x83, 0, 0, 0, 0],
            "the table remains dispatch-only"
        );
        assert_eq!(
            win64_table_function_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| (site.operand_index, site.byte_offset))
                .collect::<Vec<_>>(),
            [(Some(0), 6), (Some(2), 23), (Some(1), 40)]
        );
    }

    #[test]
    fn vtable_float_argument_and_result_use_their_positional_xmm_registers() {
        let operands = vec![
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
                size: 8,
            },
        ];
        let bytes = encode_win64_vtable_call_at_offset(&operands, 8, true)
            .expect("Win64 vtable float call");
        assert_eq!(bytes.len(), win64_vtable_call_width(&operands, 8, true));
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x8b, 8, 0, 0, 0]),
            "the second positional argument must load into XMM1"
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 16, 0, 0, 0]),
            "the result must spill from XMM0"
        );
        assert_eq!(
            win64_vtable_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(0)]
        );
    }

    #[test]
    fn table_function_float_layout_excludes_the_dispatch_table() {
        let operands = vec![
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 4,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
                size: 4,
            },
        ];
        let bytes = encode_win64_table_function_call(&operands, 56, true)
            .expect("Win64 service-table float call");
        assert_eq!(
            bytes.len(),
            win64_table_function_call_width(&operands, 56, true)
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf3, 0x41, 0x0f, 0x10, 0x83, 8, 0, 0, 0]),
            "the first declared service argument must use XMM0"
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf3, 0x41, 0x0f, 0x11, 0x83, 16, 0, 0, 0]),
            "the service result must spill from XMM0"
        );
        assert_eq!(
            win64_table_function_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(2), Some(1), Some(0)]
        );
    }
}
#[cfg(test)]
mod byte_io_width_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    const PARAMETERS: [MachineRegister; 3] = [
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdx,
    ];

    #[test]
    fn byte_op_widths_match_emission() {
        for (target_offset, payload_offset) in [(0usize, 4usize), (8, 4), (48, 4)] {
            let import = encode_runtime_byte_read_import(target_offset, payload_offset).unwrap();
            assert_eq!(import.len(), runtime_byte_read_import_width());
            let syscall = encode_runtime_byte_read_syscall(
                target_offset,
                payload_offset,
                0,
                &PARAMETERS,
                MachineRegister::X86Rax,
                MachineRegister::X86Rax,
                0,
            )
            .unwrap();
            assert_eq!(syscall.len(), runtime_byte_read_syscall_width());
        }
        for source_offset in [0usize, 8, 48] {
            let import = encode_runtime_byte_write_import(source_offset).unwrap();
            assert_eq!(import.len(), runtime_byte_write_import_width());
            let syscall = encode_runtime_byte_write_syscall(
                source_offset,
                1,
                &PARAMETERS,
                MachineRegister::X86Rax,
                MachineRegister::X86Rax,
                0,
            )
            .unwrap();
            assert_eq!(syscall.len(), runtime_byte_write_syscall_width());
        }
    }

    #[test]
    fn composite_syscalls_reject_registers_the_encoder_cannot_realize() {
        let noncanonical_parameters = [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdx,
        ];
        let diagnostic = encode_runtime_byte_write_syscall(
            0,
            1,
            &noncanonical_parameters,
            MachineRegister::X86Rax,
            MachineRegister::X86Rax,
            0,
        )
        .unwrap_err();

        assert!(
            diagnostic
                .message
                .contains("cannot realize normalized plan")
        );
    }
}
