/*
We define the canonical AArch64 physical register model
(`aarch64_physical_register_model`), operation and call constraint keys, and the
validated register-constraint catalog (`aarch64_register_constraint_catalog`)
across AAPCS64, Darwin, Linux system calls, scalar/aggregate calls, and
floating-point transport.
*/

mod call_keys {
    use register_model::{
        PreservationConvention, RegisterConstraintKey, ValidatedPhysicalRegisterModel,
    };
    use target::{Architecture, NativeTarget, ObjectFormat};

    pub fn aarch64_preservation_convention_for_target(
        model: &ValidatedPhysicalRegisterModel,
        target: NativeTarget,
    ) -> Option<&PreservationConvention> {
        if target.architecture != Architecture::Aarch64 {
            return None;
        }
        let name = match target.object_format {
            ObjectFormat::Elf => "aapcs64",
            ObjectFormat::MachO => "darwin-aapcs64",
            ObjectFormat::Coff => return None,
        };
        model.model().convention_named(name)
    }

    pub const AARCH64_AAPCS64_CALL: RegisterConstraintKey = RegisterConstraintKey::call(0);

    pub const AARCH64_DARWIN_CALL: RegisterConstraintKey = RegisterConstraintKey::call(1);

    pub fn aarch64_darwin_register_call_keys() -> Vec<RegisterConstraintKey> {
        (11..=19).map(RegisterConstraintKey::call).collect()
    }

    pub fn aarch64_register_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        let first = if darwin { 1020 } else { 1000 };
        (first..first + 18)
            .map(RegisterConstraintKey::call)
            .collect()
    }

    pub fn aarch64_register_aggregate_return_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        let first = if darwin { 12 } else { 10 };
        (first..first + 2).map(RegisterConstraintKey::ret).collect()
    }

    pub fn aarch64_aapcs64_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
        (700..=708).map(RegisterConstraintKey::call).collect()
    }

    pub fn aarch64_darwin_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
        (720..=728).map(RegisterConstraintKey::call).collect()
    }

    pub fn aarch64_aapcs64_register_call_keys() -> Vec<RegisterConstraintKey> {
        [3, 4, 2, 5, 6, 7, 8, 9, 10]
            .into_iter()
            .map(RegisterConstraintKey::call)
            .collect()
    }

    pub fn aarch64_register_unit_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        if darwin {
            aarch64_darwin_register_unit_call_keys()
        } else {
            aarch64_aapcs64_register_unit_call_keys()
        }
    }

    pub fn aarch64_aapcs64_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
        vec![RegisterConstraintKey::call(3000)]
    }

    pub fn aarch64_darwin_normalized_foreign_call_keys() -> Vec<RegisterConstraintKey> {
        vec![RegisterConstraintKey::call(3300)]
    }

    pub(crate) const AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64: RegisterConstraintKey =
        RegisterConstraintKey::call(2);

    pub const AARCH64_AAPCS64_RETURN: RegisterConstraintKey = RegisterConstraintKey::ret(0);

    pub const AARCH64_DARWIN_RETURN: RegisterConstraintKey = RegisterConstraintKey::ret(1);

    pub const AARCH64_AAPCS64_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey::ret(2);

    pub const AARCH64_DARWIN_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey::ret(3);
}
mod constraint_catalog {
    use crate::register_model::physical_model::{FLOAT64, GPR64};
    use crate::register_model::{
        AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
        AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE,
        AARCH64_ADDRESS_OFFSET, AARCH64_BITS_TO_FLOAT32, AARCH64_BITS_TO_FLOAT64,
        AARCH64_BYTE_LITERAL_IMAGE_ADDRESS, AARCH64_COMPARE_BYTE_SEQUENCES, AARCH64_COMPARE_I64,
        AARCH64_COMPARE_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
        AARCH64_COPY_BYTES, AARCH64_COPY_I64, AARCH64_DARWIN_CALL,
        AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32, AARCH64_DARWIN_HOSTED_READ_BYTE,
        AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
        AARCH64_DIVIDE_I64, AARCH64_DIVIDE_U64, AARCH64_FLOAT32_TO_BITS, AARCH64_FLOAT64_TO_BITS,
        AARCH64_FRAME_ADDRESS, AARCH64_HOSTED_EXIT_PROCESS_I32, AARCH64_HOSTED_READ_BYTE,
        AARCH64_HOSTED_WRITE_BYTE_I32, AARCH64_INLINE_ASSEMBLY_DEFAULT, AARCH64_JUMP,
        AARCH64_LINUX_SYSTEM_CALL, AARCH64_LOAD_ENCODED_SCALAR, AARCH64_LOAD_PACKED, AARCH64_LOAD8,
        AARCH64_LOAD8_INDEXED, AARCH64_LOAD16, AARCH64_LOAD32, AARCH64_LOAD64,
        AARCH64_MATERIALIZE_BOOLEAN, AARCH64_MATERIALIZE_I64, AARCH64_MULTIPLY_I64,
        AARCH64_REMAINDER_I64, AARCH64_REMAINDER_U64, AARCH64_REQUIRED_REGISTER_CONSTRAINTS,
        AARCH64_SATURATING_ADD_CLAMPED, AARCH64_SATURATING_ADD_U64,
        AARCH64_SATURATING_DIVIDE_SIGNED, AARCH64_SATURATING_MULTIPLY_CLAMPED,
        AARCH64_SATURATING_MULTIPLY_U64, AARCH64_SATURATING_SUBTRACT_CLAMPED,
        AARCH64_SATURATING_SUBTRACT_UNSIGNED, AARCH64_SCAN_BYTE_SEQUENCE, AARCH64_SHIFT_I64,
        AARCH64_STORE, AARCH64_STORE_ENCODED_SCALAR, AARCH64_STORE_PACKED, AARCH64_STORE64,
        AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE, AARCH64_TRAPPING_BINARY,
        AARCH64_TRAPPING_CONVERT, AARCH64_TRAPPING_FIXED_PAIR, AARCH64_TRAPPING_SHIFT,
        AARCH64_ZERO_BYTES, aarch64_aapcs64_normalized_foreign_call_keys,
        aarch64_aapcs64_register_call_keys, aarch64_aapcs64_register_unit_call_keys,
        aarch64_darwin_normalized_foreign_call_keys, aarch64_darwin_register_call_keys,
        aarch64_darwin_register_unit_call_keys, aarch64_register_aggregate_call_keys,
        aarch64_register_aggregate_return_keys, canonical_aarch64_physical_register_model_identity,
        validated_aarch64_physical_register_model,
    };
    use crate::register_model::{AARCH64_ACCESSOR_READ, AARCH64_ACCESSOR_WRITE};
    use crate::register_model::{float_scalar_calls, indirect_results, mixed_calls};
    use crate::{
        aarch64_aapcs64_mixed_unit_call_keys, aarch64_darwin_mixed_unit_call_keys,
        aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys,
        aarch64_indirect_aggregate_call_keys, aarch64_mixed_aggregate_call_keys,
    };
    use register_model::complement;
    use register_model::{
        PreparedRegisterConstraintCatalog, PreservationConvention, RegisterClassId,
        RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintId,
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
        RegisterOperandConstraint, ValidatedPhysicalRegisterModel, sorted_units,
        validate_register_constraint_catalog,
    };
    use target::Architecture;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Aarch64RegisterConstraintCatalogValidationError {
        PhysicalModelArchitectureMismatch,
        NonCanonicalPhysicalModel,
        Structural(RegisterConstraintCatalogValidationError),
        TargetSemantics(RegisterConstraintKey),
    }

    impl std::fmt::Display for Aarch64RegisterConstraintCatalogValidationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "invalid AArch64 register constraint catalog: {self:?}"
            )
        }
    }

    impl std::error::Error for Aarch64RegisterConstraintCatalogValidationError {}

    pub fn aarch64_register_constraint_catalog(
        model: &ValidatedPhysicalRegisterModel,
    ) -> RegisterConstraintCatalog {
        let physical = model.model();
        assert_eq!(physical.architecture, Architecture::Aarch64);

        let view = |name: &str| {
            physical
                .view_named(name)
                .unwrap_or_else(|| panic!("validated AArch64 model must define {name}"))
        };
        let merged_units = |base: Vec<register_model::RegisterUnitId>, names: &[&str]| {
            let mut units = base;
            for name in names {
                units.extend(view(name).units.iter().copied());
            }
            units.sort_unstable();
            units.dedup();
            units
        };
        let fixed = |operand: u16, access: RegisterOperandAccess, name: &str| {
            let view = view(name);
            RegisterOperandConstraint {
                operand,
                access,
                class: view.class,
                fixed_view: Some(view.id),
                tied_to: None,
                early_clobber: false,
            }
        };
        let allocatable = |operand: u16, access: RegisterOperandAccess, class: RegisterClassId| {
            RegisterOperandConstraint {
                operand,
                access,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            }
        };
        let convention = |name: &str| {
            physical
                .convention_named(name)
                .unwrap_or_else(|| panic!("validated AArch64 model must define {name}"))
        };
        let call_operands = || {
            ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
                .into_iter()
                .enumerate()
                .map(|(operand, name)| {
                    fixed(
                        u16::try_from(operand).expect("operand index fits u16"),
                        RegisterOperandAccess::Use,
                        name,
                    )
                })
                .chain([fixed(8, RegisterOperandAccess::Def, "x0")])
                .collect::<Vec<_>>()
        };
        let use_then_early_clobber = |operands: u16, inputs: u16| {
            (0..operands)
                .map(|operand| {
                    let mut row = allocatable(
                        operand,
                        if operand < inputs {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        },
                        GPR64,
                    );
                    row.early_clobber = operand >= inputs;
                    row
                })
                .collect::<Vec<_>>()
        };

        let use_use_def = || {
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ]
        };
        let use_def = || {
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ]
        };
        let early_def = |operand: u16| {
            let mut row = allocatable(operand, RegisterOperandAccess::Def, GPR64);
            row.early_clobber = true;
            row
        };

        let sp_units = view("sp").units.clone();
        let pc_units = view("pc").units.clone();
        let link_units = view("x30").units.clone();
        let x0_units = view("x0").units.clone();
        let call_uses = sorted_units(sp_units.iter().copied().chain(pc_units.iter().copied()));
        let call_defs = sorted_units(link_units.iter().copied().chain(pc_units.iter().copied()));
        let call_clobbers = |convention: &PreservationConvention| {
            convention
                .caller_saved
                .iter()
                .copied()
                .filter(|unit| !x0_units.contains(unit) && !call_defs.contains(unit))
                .collect::<Vec<_>>()
        };
        let return_uses = sorted_units(sp_units.iter().copied().chain(link_units.iter().copied()));
        let all_units = physical
            .units
            .iter()
            .map(|unit| unit.id)
            .collect::<Vec<_>>();
        let common_fixed = sorted_units(
            sp_units
                .iter()
                .copied()
                .chain(view("xzr").units.iter().copied())
                .chain(view("fpcr").units.iter().copied())
                .chain(view("fpsr").units.iter().copied())
                .chain(pc_units.iter().copied()),
        );
        let aapcs = convention("aapcs64");
        let darwin = convention("darwin-aapcs64");
        let instruction_row =
            |id: RegisterConstraintId,
             key: RegisterConstraintKey,
             operands: Vec<RegisterOperandConstraint>| {
                RegisterInstructionConstraint {
                    id,
                    key,
                    operands,
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                }
            };
        let nzcv_defining_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_defs: view("nzcv").units.clone(),
            ..instruction_row(id, key, operands)
        };
        let nzcv_clobbering_row = |id, key, operands| RegisterInstructionConstraint {
            clobbers: view("nzcv").units.clone(),
            ..instruction_row(id, key, operands)
        };
        let return_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_uses: return_uses.clone(),
            implicit_defs: pc_units.clone(),
            ..instruction_row(id, key, operands)
        };
        let jump_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_uses: pc_units.clone(),
            implicit_defs: pc_units.clone(),
            ..instruction_row(id, key, operands)
        };
        let sp_use_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_uses: view("sp").units.clone(),
            ..instruction_row(id, key, operands)
        };
        let call_row = |id, key, operands, convention: &PreservationConvention| {
            RegisterInstructionConstraint {
                implicit_uses: call_uses.clone(),
                implicit_defs: call_defs.clone(),
                clobbers: call_clobbers(convention),
                ..instruction_row(id, key, operands)
            }
        };

        let mut constraints = vec![
            instruction_row(
                RegisterConstraintId(0),
                crate::AARCH64_MEMORY_FENCE,
                Vec::new(),
            ),
            instruction_row(
                RegisterConstraintId(0),
                crate::AARCH64_ATOMIC_FENCE,
                Vec::new(),
            ),
            jump_row(RegisterConstraintId(0), crate::AARCH64_CRASH, Vec::new()),
            call_row(
                RegisterConstraintId(0),
                AARCH64_AAPCS64_CALL,
                call_operands(),
                aapcs,
            ),
            call_row(
                RegisterConstraintId(1),
                AARCH64_DARWIN_CALL,
                call_operands(),
                darwin,
            ),
            call_row(
                RegisterConstraintId(17),
                AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64,
                vec![
                    fixed(0, RegisterOperandAccess::Use, "x0"),
                    fixed(1, RegisterOperandAccess::Use, "x1"),
                    fixed(2, RegisterOperandAccess::Def, "x0"),
                ],
                aapcs,
            ),
            return_row(
                RegisterConstraintId(2),
                AARCH64_AAPCS64_RETURN,
                vec![fixed(0, RegisterOperandAccess::Use, "x0")],
            ),
            return_row(
                RegisterConstraintId(3),
                AARCH64_DARWIN_RETURN,
                vec![fixed(0, RegisterOperandAccess::Use, "x0")],
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(4),
                key: AARCH64_LINUX_SYSTEM_CALL,
                operands: ["x8", "x0", "x1", "x2", "x3", "x4", "x5"]
                    .into_iter()
                    .enumerate()
                    .map(|(operand, name)| {
                        fixed(
                            u16::try_from(operand).expect("operand index fits u16"),
                            if operand == 1 {
                                RegisterOperandAccess::UseDef
                            } else {
                                RegisterOperandAccess::Use
                            },
                            name,
                        )
                    })
                    .collect(),
                implicit_uses: pc_units.clone(),
                implicit_defs: pc_units.clone(),
                clobbers: view("nzcv").units.clone(),
            },
            RegisterInstructionConstraint {
                id: RegisterConstraintId(5),
                key: AARCH64_INLINE_ASSEMBLY_DEFAULT,
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: complement(&all_units, &common_fixed, &[]),
            },
            instruction_row(
                RegisterConstraintId(6),
                AARCH64_MATERIALIZE_I64,
                vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            ),
            instruction_row(
                RegisterConstraintId(6),
                AARCH64_BYTE_LITERAL_IMAGE_ADDRESS,
                vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(6),
                key: AARCH64_MATERIALIZE_BOOLEAN,
                operands: vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
                implicit_uses: view("nzcv").units.clone(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
            },
            instruction_row(RegisterConstraintId(7), AARCH64_COPY_I64, use_def()),
            nzcv_defining_row(
                RegisterConstraintId(8),
                AARCH64_COMPARE_I64_ZERO,
                vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(9),
                key: AARCH64_CONDITIONAL_BRANCH,
                operands: Vec::new(),
                implicit_uses: sorted_units(
                    view("nzcv")
                        .units
                        .iter()
                        .copied()
                        .chain(view("pc").units.iter().copied()),
                ),
                implicit_defs: view("pc").units.clone(),
                clobbers: Vec::new(),
            },
            instruction_row(RegisterConstraintId(10), AARCH64_ADD_I64, use_use_def()),
            instruction_row(
                RegisterConstraintId(11),
                AARCH64_ADD_I64_IMMEDIATE,
                use_def(),
            ),
            instruction_row(
                RegisterConstraintId(12),
                AARCH64_SUBTRACT_I64,
                use_use_def(),
            ),
            instruction_row(
                RegisterConstraintId(13),
                AARCH64_SUBTRACT_I64_IMMEDIATE,
                use_def(),
            ),
            nzcv_defining_row(
                RegisterConstraintId(16),
                AARCH64_COMPARE_I64,
                vec![
                    allocatable(0, RegisterOperandAccess::Use, GPR64),
                    allocatable(1, RegisterOperandAccess::Use, GPR64),
                ],
            ),
            nzcv_defining_row(
                RegisterConstraintId(18),
                AARCH64_COMPARE_I64_IMMEDIATE,
                vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            ),
            return_row(
                RegisterConstraintId(14),
                AARCH64_AAPCS64_RETURN_UNIT,
                Vec::new(),
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(15),
                key: AARCH64_DARWIN_RETURN_UNIT,
                operands: Vec::new(),
                implicit_uses: return_uses,
                implicit_defs: pc_units.clone(),
                clobbers: Vec::new(),
            },
            jump_row(RegisterConstraintId(0), AARCH64_JUMP, Vec::new()),
        ];

        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_LOAD64,
            use_def(),
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_LOAD8_INDEXED,
            use_use_def(),
        ));
        for (key, result_access) in [
            (AARCH64_STORE, RegisterOperandAccess::Use),
            (AARCH64_ADDRESS_OFFSET, RegisterOperandAccess::Def),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    allocatable(0, RegisterOperandAccess::Use, GPR64),
                    allocatable(1, result_access, GPR64),
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
            });
        }
        let mut store_address = allocatable(1, RegisterOperandAccess::Def, GPR64);
        store_address.early_clobber = true;
        constraints.push(sp_use_row(
            RegisterConstraintId(0),
            AARCH64_STORE64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                store_address,
            ],
        ));
        constraints.push(sp_use_row(
            RegisterConstraintId(0),
            AARCH64_FRAME_ADDRESS,
            vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
        ));
        let scalar_call = constraints
            .iter()
            .find(|row| row.key == AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64)
            .expect("pair call row")
            .clone();
        for (arity, key) in aarch64_aapcs64_register_call_keys().into_iter().enumerate() {
            if key == AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64 {
                continue;
            }
            let mut call = scalar_call.clone();
            call.key = key;
            call.operands = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
                .into_iter()
                .take(arity)
                .enumerate()
                .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
                .chain([fixed(arity as u16, RegisterOperandAccess::Def, "x0")])
                .collect();
            constraints.push(call);
        }

        let abi_call = constraints
            .iter()
            .find(|row| row.key == AARCH64_DARWIN_CALL)
            .expect("canonical ABI call row")
            .clone();
        for darwin in [false, true] {
            for (index, key) in aarch64_register_aggregate_call_keys(darwin)
                .into_iter()
                .enumerate()
            {
                let arity = index % 9;
                let result_count = index / 9 + 1;
                let mut call = if darwin {
                    abi_call.clone()
                } else {
                    scalar_call.clone()
                };
                call.key = key;
                call.operands = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
                    .into_iter()
                    .take(arity)
                    .enumerate()
                    .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
                    .chain(["x0", "x1"].into_iter().take(result_count).enumerate().map(
                        |(index, name)| {
                            fixed((arity + index) as u16, RegisterOperandAccess::Def, name)
                        },
                    ))
                    .collect();
                for name in ["x0", "x1"].into_iter().take(result_count) {
                    call.clobbers
                        .retain(|unit| !view(name).write_units.contains(unit));
                }
                constraints.push(call);
            }
            let source_key = if darwin {
                AARCH64_DARWIN_RETURN
            } else {
                AARCH64_AAPCS64_RETURN
            };
            let returned = constraints
                .iter()
                .find(|row| row.key == source_key)
                .expect("canonical return row")
                .clone();
            for (index, key) in aarch64_register_aggregate_return_keys(darwin)
                .into_iter()
                .enumerate()
            {
                let mut row = returned.clone();
                row.key = key;
                row.operands = ["x0", "x1"]
                    .into_iter()
                    .take(index + 1)
                    .enumerate()
                    .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
                    .collect();
                constraints.push(row);
            }
        }
        for (arity, key) in aarch64_darwin_register_call_keys().into_iter().enumerate() {
            let mut call = abi_call.clone();
            call.key = key;
            call.operands = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
                .into_iter()
                .take(arity)
                .enumerate()
                .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
                .chain([fixed(arity as u16, RegisterOperandAccess::Def, "x0")])
                .collect();
            constraints.push(call);
        }

        for (key, syscall_register) in [
            (AARCH64_HOSTED_READ_BYTE, "x8"),
            (AARCH64_DARWIN_HOSTED_READ_BYTE, "x16"),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: Vec::new(),
                implicit_uses: merged_units(view("sp").units.clone(), &["pc"]),
                implicit_defs: Vec::new(),
                clobbers: merged_units(
                    Vec::new(),
                    &["x0", "x1", "x2", syscall_register, "x9", "nzcv"],
                ),
            });
        }
        for (key, syscall_register) in [
            (AARCH64_HOSTED_WRITE_BYTE_I32, "x8"),
            (AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, "x16"),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
                implicit_uses: merged_units(view("sp").units.clone(), &["pc"]),
                implicit_defs: Vec::new(),
                clobbers: merged_units(Vec::new(), &["x0", "x1", "x2", syscall_register, "nzcv"]),
            });
        }
        for (key, syscall_register) in [
            (AARCH64_HOSTED_EXIT_PROCESS_I32, "x8"),
            (AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32, "x16"),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
                implicit_uses: view("pc").units.clone(),
                implicit_defs: Vec::new(),
                clobbers: merged_units(Vec::new(), &["x0", syscall_register, "nzcv"]),
            });
        }
        for (scalar_key, unit_key) in aarch64_aapcs64_register_call_keys()
            .into_iter()
            .zip(aarch64_aapcs64_register_unit_call_keys())
            .chain(
                aarch64_darwin_register_call_keys()
                    .into_iter()
                    .zip(aarch64_darwin_register_unit_call_keys()),
            )
        {
            let mut call = constraints
                .iter()
                .find(|row| row.key == scalar_key)
                .expect("canonical scalar call row")
                .clone();
            call.key = unit_key;
            call.operands.pop();
            call.clobbers = sorted_units(
                call.clobbers
                    .into_iter()
                    .chain(view("x0").units.iter().copied()),
            );
            constraints.push(call);
        }
        for (key, source_class, destination_class) in [
            (AARCH64_LOAD8, GPR64, GPR64),
            (AARCH64_LOAD16, GPR64, GPR64),
            (AARCH64_LOAD32, GPR64, GPR64),
            (AARCH64_FLOAT32_TO_BITS, FLOAT64, GPR64),
            (AARCH64_FLOAT64_TO_BITS, FLOAT64, GPR64),
            (AARCH64_BITS_TO_FLOAT32, GPR64, FLOAT64),
            (AARCH64_BITS_TO_FLOAT64, GPR64, FLOAT64),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    allocatable(0, RegisterOperandAccess::Use, source_class),
                    allocatable(1, RegisterOperandAccess::Def, destination_class),
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
            });
        }
        constraints.push(nzcv_clobbering_row(
            RegisterConstraintId(0),
            AARCH64_COPY_BYTES,
            use_then_early_clobber(5, 3),
        ));
        constraints.push(nzcv_clobbering_row(
            RegisterConstraintId(0),
            AARCH64_ZERO_BYTES,
            use_then_early_clobber(3, 2),
        ));
        for (key, input_count) in [
            (AARCH64_LOAD_ENCODED_SCALAR, 2),
            (AARCH64_STORE_ENCODED_SCALAR, 3),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: use_then_early_clobber(6, input_count),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: view("nzcv").units.clone(),
            });
        }
        constraints.push(nzcv_clobbering_row(
            RegisterConstraintId(0),
            AARCH64_SCAN_BYTE_SEQUENCE,
            use_then_early_clobber(5, 2),
        ));
        constraints.push(nzcv_clobbering_row(
            RegisterConstraintId(0),
            AARCH64_COMPARE_BYTE_SEQUENCES,
            use_then_early_clobber(8, 4),
        ));
        for (key, load) in [(AARCH64_LOAD_PACKED, true), (AARCH64_STORE_PACKED, false)] {
            let mut result = allocatable(
                1,
                if load {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                },
                GPR64,
            );
            result.early_clobber = load;
            let scratch = early_def(2);
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    allocatable(0, RegisterOperandAccess::Use, GPR64),
                    result,
                    scratch,
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
            });
        }
        constraints.push(nzcv_defining_row(
            RegisterConstraintId(0),
            AARCH64_SATURATING_SUBTRACT_UNSIGNED,
            use_use_def(),
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_REMAINDER_I64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                early_def(2),
            ],
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_DIVIDE_I64,
            use_use_def(),
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_REMAINDER_U64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                early_def(2),
            ],
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_DIVIDE_U64,
            use_use_def(),
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_MULTIPLY_I64,
            use_use_def(),
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            AARCH64_SHIFT_I64,
            use_use_def(),
        ));
        constraints.push(nzcv_defining_row(
            RegisterConstraintId(0),
            AARCH64_SATURATING_ADD_U64,
            use_use_def(),
        ));
        for key in [
            AARCH64_SATURATING_ADD_CLAMPED,
            AARCH64_SATURATING_SUBTRACT_CLAMPED,
            AARCH64_SATURATING_DIVIDE_SIGNED,
            AARCH64_SATURATING_MULTIPLY_CLAMPED,
            AARCH64_SATURATING_MULTIPLY_U64,
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    allocatable(0, RegisterOperandAccess::Use, GPR64),
                    allocatable(1, RegisterOperandAccess::Use, GPR64),
                    early_def(2),
                    early_def(3),
                ],
                implicit_uses: Vec::new(),
                implicit_defs: view("nzcv").units.clone(),
                clobbers: Vec::new(),
            });
        }
        for (key, sources) in [
            (AARCH64_TRAPPING_BINARY, 2),
            (AARCH64_TRAPPING_FIXED_PAIR, 2),
            (AARCH64_TRAPPING_SHIFT, 2),
            (AARCH64_TRAPPING_CONVERT, 1),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: (0..sources)
                    .map(|operand| allocatable(operand, RegisterOperandAccess::Use, GPR64))
                    .chain((sources..sources + 2).map(early_def))
                    .collect(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: view("nzcv").units.clone(),
            });
        }
        mixed_calls::append_constraints(&mut constraints, model);
        for (key, inputs, outputs, writes_flags) in [
            (crate::AARCH64_ATOMIC_LOAD, 1, 1, false),
            (crate::AARCH64_ATOMIC_STORE, 2, 0, false),
            (crate::AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE, 3, 2, true),
        ] {
            let mut operands = (0..inputs)
                .map(|operand| allocatable(operand, RegisterOperandAccess::Use, GPR64))
                .collect::<Vec<_>>();
            operands.extend((inputs..inputs + outputs).map(early_def));
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: if writes_flags {
                    view("nzcv").units.clone()
                } else {
                    Vec::new()
                },
            });
        }
        for (key, names, comparison) in [
            (
                crate::AARCH64_ATOMIC_UPDATE,
                &["x9", "x10", "x11"][..],
                false,
            ),
            (
                crate::AARCH64_ATOMIC_COMPARE_EXCHANGE,
                &["x9", "x10", "x11", "x12"][..],
                true,
            ),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: names
                    .iter()
                    .enumerate()
                    .map(|(ordinal, name)| {
                        fixed(
                            ordinal as u16,
                            if ordinal + 1 == names.len() {
                                RegisterOperandAccess::Def
                            } else {
                                RegisterOperandAccess::Use
                            },
                            name,
                        )
                    })
                    .collect(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: sorted_units(
                    ["x16", "x17"]
                        .into_iter()
                        .chain(comparison.then_some("nzcv"))
                        .flat_map(|name| view(name).units.iter().copied()),
                ),
            });
        }
        mixed_calls::append_normalized_foreign_constraints(&mut constraints, model);
        super::system_calls::append_constraints(&mut constraints, model);
        float_scalar_calls::append_constraints(&mut constraints, model);
        indirect_results::append_constraints(&mut constraints, model);
        for (key, inputs) in [
            (crate::AARCH64_IEEE_FLOAT_ARITHMETIC, 2),
            (crate::AARCH64_IEEE_FLOAT_UNARY, 1),
            (crate::AARCH64_IEEE_FLOAT_TERNARY, 3),
        ] {
            let mut operands = (0..inputs)
                .map(|index| allocatable(index, RegisterOperandAccess::Use, GPR64))
                .collect::<Vec<_>>();
            operands.push(allocatable(inputs, RegisterOperandAccess::Def, GPR64));
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: sorted_units(view("fpcr").units.iter().copied()),
                implicit_defs: Vec::new(),
                clobbers: sorted_units(
                    ["q16", "q17", "q18"]
                        .into_iter()
                        .flat_map(|name| view(name).units.iter().copied()),
                ),
            });
        }
        for (key, inputs) in [
            (crate::AARCH64_FLOAT_RESULT_TRAPPING, 1),
            (crate::AARCH64_FLOAT_RESULT_SATURATING, 2),
        ] {
            let scratch: &[&str] = if inputs == 1 {
                &["q16", "nzcv"]
            } else {
                &["q16", "q17", "nzcv"]
            };
            let mut operands = (0..inputs)
                .map(|position| allocatable(position, RegisterOperandAccess::Use, GPR64))
                .collect::<Vec<_>>();
            operands.push(allocatable(inputs, RegisterOperandAccess::Def, GPR64));
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: sorted_units(
                    scratch
                        .iter()
                        .flat_map(|name| view(name).units.iter().copied()),
                ),
            });
        }
        for (key, restore) in [
            (crate::AARCH64_SAVE_FLOATING_CONTROL, false),
            (crate::AARCH64_RESTORE_FLOATING_CONTROL, true),
        ] {
            let mut implicit_uses = view("sp").units.clone();
            if !restore {
                implicit_uses.extend(view("fpcr").units.iter().copied());
            }
            implicit_uses.sort_unstable();
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: Vec::new(),
                implicit_uses,
                implicit_defs: if restore {
                    view("fpcr").units.clone()
                } else {
                    Vec::new()
                },
                clobbers: {
                    let mut units = view("x9").units.clone();
                    if !restore {
                        units.extend(view("x16").units.iter().copied());
                    }
                    units.sort_unstable();
                    units
                },
            });
        }
        for (key, template) in [
            (AARCH64_ACCESSOR_READ, AARCH64_LOAD32),
            (AARCH64_ACCESSOR_WRITE, AARCH64_STORE),
        ] {
            let mut row = constraints
                .iter()
                .find(|row| row.key == template)
                .expect("canonical scalar transfer")
                .clone();
            row.key = key;
            constraints.push(row);
        }
        for (key, access, scratch) in [
            (
                crate::AARCH64_SYSTEM_REGISTER_READ,
                RegisterOperandAccess::Def,
                &["x9", "x15"][..],
            ),
            (
                crate::AARCH64_SYSTEM_REGISTER_WRITE,
                RegisterOperandAccess::Use,
                &["x9", "x10", "x11", "x15"][..],
            ),
        ] {
            let mut clobbers = scratch
                .iter()
                .flat_map(|name| view(name).units.iter().copied())
                .collect::<Vec<_>>();
            clobbers.sort_unstable();
            clobbers.dedup();
            constraints.push(RegisterInstructionConstraint {
                clobbers,
                ..instruction_row(
                    RegisterConstraintId(0),
                    key,
                    vec![allocatable(0, access, GPR64)],
                )
            });
        }
        constraints.sort_by_key(|constraint| constraint.key);
        for (id, constraint) in constraints.iter_mut().enumerate() {
            constraint.id =
                RegisterConstraintId(u16::try_from(id).expect("constraint roster fits u16"));
        }
        RegisterConstraintCatalog {
            architecture: Architecture::Aarch64,
            required: {
                let mut required = AARCH64_REQUIRED_REGISTER_CONSTRAINTS.to_vec();
                required.extend(aarch64_aapcs64_mixed_unit_call_keys());
                required.extend(aarch64_darwin_mixed_unit_call_keys());
                for darwin in [false, true] {
                    required.extend(aarch64_float_scalar_call_keys(darwin));
                    required.extend(aarch64_float_scalar_return_keys(darwin));
                    required.extend(aarch64_indirect_aggregate_call_keys(darwin));
                    required.extend(aarch64_register_aggregate_call_keys(darwin));
                    required.extend(aarch64_mixed_aggregate_call_keys(darwin));
                    required.extend(aarch64_register_aggregate_return_keys(darwin));
                }
                required.extend(aarch64_aapcs64_normalized_foreign_call_keys());
                required.extend(aarch64_darwin_normalized_foreign_call_keys());
                required.extend(super::system_calls::aarch64_linux_system_call_keys());
                required.sort_unstable();
                required
            },
            constraints,
        }
    }

    fn check_aarch64_catalog_model(
        model: &ValidatedPhysicalRegisterModel,
    ) -> Result<(), Aarch64RegisterConstraintCatalogValidationError> {
        if model.model().architecture != Architecture::Aarch64 {
            return Err(
                Aarch64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch,
            );
        }
        if model.identity() != canonical_aarch64_physical_register_model_identity() {
            return Err(Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel);
        }
        Ok(())
    }

    pub fn prepare_aarch64_register_constraint_catalog(
        model: &ValidatedPhysicalRegisterModel,
    ) -> Result<PreparedRegisterConstraintCatalog, Aarch64RegisterConstraintCatalogValidationError>
    {
        check_aarch64_catalog_model(model)?;
        Ok(canonical_aarch64_register_constraint_catalog().clone())
    }

    fn canonical_aarch64_register_constraint_catalog() -> &'static PreparedRegisterConstraintCatalog
    {
        static CANONICAL: std::sync::OnceLock<PreparedRegisterConstraintCatalog> =
            std::sync::OnceLock::new();
        CANONICAL.get_or_init(|| {
            register_model::produce_register_constraint_catalog(
                validated_aarch64_physical_register_model(),
                aarch64_register_constraint_catalog,
            )
        })
    }

    pub fn validate_aarch64_register_constraint_catalog(
        catalog: RegisterConstraintCatalog,
        model: &ValidatedPhysicalRegisterModel,
    ) -> Result<PreparedRegisterConstraintCatalog, Aarch64RegisterConstraintCatalogValidationError>
    {
        check_aarch64_catalog_model(model)?;
        let validated = validate_register_constraint_catalog(catalog, model)
            .map_err(Aarch64RegisterConstraintCatalogValidationError::Structural)?;
        let canonical = aarch64_register_constraint_catalog_for(model);
        let actual_rows = &validated.catalog().constraints;
        for key in canonical.required.iter().copied() {
            let Ok(actual_position) = actual_rows.binary_search_by_key(&key, |row| row.key) else {
                return Err(Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(key));
            };
            let expected_position = canonical
                .constraints
                .binary_search_by_key(&key, |row| row.key)
                .expect("target-owned inventory and rows are closed together");
            if actual_rows[actual_position] != canonical.constraints[expected_position] {
                return Err(Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(key));
            }
        }
        if let Some(unexpected) = validated
            .catalog()
            .constraints
            .iter()
            .find(|constraint| canonical.required.binary_search(&constraint.key).is_err())
        {
            return Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(unexpected.key),
            );
        }
        Ok(validated)
    }

    pub(crate) fn aarch64_register_constraint_catalog_for(
        model: &ValidatedPhysicalRegisterModel,
    ) -> std::borrow::Cow<'static, RegisterConstraintCatalog> {
        if model.identity() == canonical_aarch64_physical_register_model_identity() {
            std::borrow::Cow::Borrowed(canonical_aarch64_register_constraint_catalog().catalog())
        } else {
            std::borrow::Cow::Owned(aarch64_register_constraint_catalog(model))
        }
    }
}
mod float_scalar_calls {
    use super::{
        AARCH64_AAPCS64_RETURN, AARCH64_DARWIN_RETURN, aarch64_mixed_unit_call_keys,
        aarch64_register_unit_call_keys,
    };
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
        RegisterOperandConstraint, ValidatedPhysicalRegisterModel, find_register_constraint,
    };
    pub fn aarch64_float_scalar_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        let count = input_keys(darwin).len() + aarch64_mixed_unit_call_keys(darwin).len();
        (0..count)
            .map(|ordinal| {
                RegisterConstraintKey::call((if darwin { 1800 } else { 1600 }) + ordinal as u32)
            })
            .collect()
    }

    pub fn aarch64_float_scalar_return_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        vec![RegisterConstraintKey::ret(if darwin { 1601 } else { 1600 })]
    }

    fn input_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        aarch64_register_unit_call_keys(darwin)
            .into_iter()
            .chain(aarch64_mixed_unit_call_keys(darwin))
            .collect()
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        let Some(result) = model.model().view_named("d0") else {
            return;
        };
        for darwin in [false, true] {
            let inputs = input_keys(darwin).into_iter().map(|key| (key, "d0")).chain(
                aarch64_mixed_unit_call_keys(darwin)
                    .into_iter()
                    .map(|key| (key, "x0")),
            );
            for ((input, result_name), key) in inputs.zip(aarch64_float_scalar_call_keys(darwin)) {
                let Some(result) = model.model().view_named(result_name) else {
                    return;
                };
                let Some(mut row) = find_register_constraint(constraints, input).cloned() else {
                    return;
                };
                row.key = key;
                row.operands.push(RegisterOperandConstraint {
                    operand: row.operands.len() as u16,
                    access: RegisterOperandAccess::Def,
                    class: result.class,
                    fixed_view: Some(result.id),
                    tied_to: None,
                    early_clobber: false,
                });
                row.clobbers
                    .retain(|unit| !result.write_units.contains(unit));
                constraints.push(row);
            }
            let base = if darwin {
                AARCH64_DARWIN_RETURN
            } else {
                AARCH64_AAPCS64_RETURN
            };
            let Some(mut row) = find_register_constraint(constraints, base).cloned() else {
                return;
            };
            row.key = aarch64_float_scalar_return_keys(darwin)[0];
            row.operands = vec![RegisterOperandConstraint {
                operand: 0,
                access: RegisterOperandAccess::Use,
                class: result.class,
                fixed_view: Some(result.id),
                tied_to: None,
                early_clobber: false,
            }];
            constraints.push(row);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{
            aarch64_aapcs64_register_unit_call_keys, aarch64_physical_register_model,
            aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
        };
        use super::{aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys};

        #[test]
        fn floating_return_uses_ordinary_return_bytes_and_exact_home() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .unwrap();
            let home = physical.model().view_named("d0").unwrap().id;
            let wrong = physical.model().view_named("d1").unwrap().id;
            let kind = selected_instructions::SelectedInstructionKind::ReturnScalar;
            let alternative = selected_instructions::MachineAlternativeKey {
                family: selected_instructions::MachineAlternativeFamily::ReturnScalar,
                variant: 0,
            };
            let encoded =
                crate::encode_aarch64_selected_form(&physical, kind, alternative, &[home])
                    .expect("floating return");
            crate::validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[home],
                encoded.bytes(),
            )
            .unwrap();
            assert!(
                crate::validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    alternative,
                    &[wrong],
                    encoded.bytes()
                )
                .is_err()
            );
        }

        #[test]
        fn scalar_float_roster_is_complete_and_binds_result_home_and_clobbers() {
            let physical =
                register_model::validate_physical_register_model(aarch64_physical_register_model())
                    .expect("physical model");
            let catalog = aarch64_register_constraint_catalog(&physical);
            validate_aarch64_register_constraint_catalog(catalog.clone(), &physical)
                .expect("complete catalog");
            let mut keys = Vec::new();
            for darwin in [false, true] {
                keys.extend(aarch64_float_scalar_call_keys(darwin));
                keys.extend(aarch64_float_scalar_return_keys(darwin));
            }
            let count = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), count, "ABI key ranges do not overlap");
            for key in keys {
                assert_eq!(
                    catalog
                        .constraints
                        .iter()
                        .filter(|row| row.key == key)
                        .count(),
                    1
                );
            }
            let result = physical.model().view_named("d0").unwrap();
            let call_key = aarch64_float_scalar_call_keys(false)[0];
            let row = catalog.constraint(call_key).unwrap();
            assert_eq!(row.operands.last().unwrap().fixed_view, Some(result.id));
            assert!(
                result
                    .write_units
                    .iter()
                    .all(|unit| !row.clobbers.contains(unit))
            );
            let input = catalog
                .constraint(aarch64_aapcs64_register_unit_call_keys()[0])
                .unwrap();
            assert!(
                input
                    .clobbers
                    .iter()
                    .all(|unit| row.clobbers.contains(unit) || result.write_units.contains(unit)),
                "every caller-save effect remains either a Def write or a clobber"
            );
            let mut wrong_home = catalog.clone();
            let row =
                register_model::find_register_constraint_mut(&mut wrong_home.constraints, call_key)
                    .unwrap();
            row.operands.last_mut().unwrap().fixed_view =
                Some(physical.model().view_named("d1").unwrap().id);
            assert!(validate_aarch64_register_constraint_catalog(wrong_home, &physical).is_err());
            let mut wrong_clobbers = catalog;
            register_model::find_register_constraint_mut(&mut wrong_clobbers.constraints, call_key)
                .unwrap()
                .clobbers
                .clear();
            assert!(
                validate_aarch64_register_constraint_catalog(wrong_clobbers, &physical).is_err()
            );
        }
    }
}
#[cfg(test)]
mod float_transport_tests {
    use super::{
        AARCH64_FLOAT32_TO_BITS, aarch64_aapcs64_mixed_unit_call_keys,
        aarch64_physical_register_model, aarch64_register_constraint_catalog,
        validate_aarch64_register_constraint_catalog,
    };
    use crate::register_model::physical_model::GPR64;
    use register_model::validate_physical_register_model;
    #[test]
    fn transport_and_mixed_call_constraints_reject_forged_classes_and_clobbers() {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&physical);
        let mut wrong_transport = catalog.clone();
        let transport = register_model::find_register_constraint_mut(
            &mut wrong_transport.constraints,
            AARCH64_FLOAT32_TO_BITS,
        )
        .unwrap();
        transport.operands[0].class = GPR64;
        assert!(validate_aarch64_register_constraint_catalog(wrong_transport, &physical).is_err());
        let mut wrong_call = catalog;
        let call_key = aarch64_aapcs64_mixed_unit_call_keys()[0];
        let call =
            register_model::find_register_constraint_mut(&mut wrong_call.constraints, call_key)
                .unwrap();
        assert!(call.clobbers.pop().is_some());
        assert!(validate_aarch64_register_constraint_catalog(wrong_call, &physical).is_err());
    }
}
mod indirect_results {
    use super::{aarch64_mixed_unit_call_keys, aarch64_register_unit_call_keys};
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
        RegisterOperandConstraint, ValidatedPhysicalRegisterModel, find_register_constraint,
    };
    pub fn aarch64_indirect_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        let first = if darwin { 2300 } else { 2200 };
        (first..first + 81)
            .map(RegisterConstraintKey::call)
            .collect()
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        let Some(pointer) = model.model().view_named("x8") else {
            return;
        };
        for darwin in [false, true] {
            let inputs = aarch64_register_unit_call_keys(darwin);
            let mixed = aarch64_mixed_unit_call_keys(darwin);
            for (input, key) in inputs
                .into_iter()
                .chain(mixed)
                .zip(aarch64_indirect_aggregate_call_keys(darwin))
            {
                let Some(mut row) = find_register_constraint(constraints, input).cloned() else {
                    return;
                };
                row.key = key;
                row.operands.push(RegisterOperandConstraint {
                    operand: row.operands.len() as u16,
                    access: RegisterOperandAccess::Use,
                    class: pointer.class,
                    fixed_view: Some(pointer.id),
                    tied_to: None,
                    early_clobber: false,
                });
                constraints.push(row);
            }
        }
    }
}
mod mixed_calls {
    use super::{
        aarch64_aapcs64_register_unit_call_keys, aarch64_darwin_register_unit_call_keys,
        aarch64_register_unit_call_keys,
    };
    use register_model::call_constraint_keys;
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
        RegisterOperandConstraint, ValidatedPhysicalRegisterModel, find_register_constraint,
    };
    pub fn aarch64_aapcs64_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
        call_constraint_keys(800..872)
    }

    pub fn aarch64_darwin_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
        call_constraint_keys(872..944)
    }

    pub fn aarch64_mixed_unit_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        if darwin {
            aarch64_darwin_mixed_unit_call_keys()
        } else {
            aarch64_aapcs64_mixed_unit_call_keys()
        }
    }

    pub fn aarch64_mixed_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
        let first = if darwin { 1300 } else { 1100 };
        call_constraint_keys(first..first + 144)
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        for darwin in [false, true] {
            let base_key = aarch64_register_unit_call_keys(darwin)[0];
            let keys = aarch64_mixed_unit_call_keys(darwin);
            let Some(base) = find_register_constraint(constraints, base_key).cloned() else {
                continue;
            };
            let mut keys = keys.into_iter();
            for general_count in 0..=8 {
                for float_count in 1..=8 {
                    let mut row = base.clone();
                    let Some(key) = keys.next() else { return };
                    row.key = key;
                    let Some(operands) = (0..general_count)
                        .map(|position| format!("x{position}"))
                        .chain((0..float_count).map(|position| format!("d{position}")))
                        .enumerate()
                        .map(|(operand, name)| {
                            let view = model.model().view_named(&name)?;
                            Some(RegisterOperandConstraint {
                                operand: operand as u16,
                                access: RegisterOperandAccess::Use,
                                class: view.class,
                                fixed_view: Some(view.id),
                                tied_to: None,
                                early_clobber: false,
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                    else {
                        return;
                    };
                    row.operands = operands;
                    constraints.push(row);
                }
            }
        }
        for darwin in [false, true] {
            let unit_keys = aarch64_mixed_unit_call_keys(darwin);
            for (ordinal, key) in aarch64_mixed_aggregate_call_keys(darwin)
                .into_iter()
                .enumerate()
            {
                let Some(mut row) = constraints
                    .iter()
                    .find(|row| row.key == unit_keys[ordinal % 72])
                    .cloned()
                else {
                    return;
                };
                row.key = key;
                let result_count = ordinal / 72 + 1;
                for name in ["x0", "x1"].into_iter().take(result_count) {
                    let Some(view) = model.model().view_named(name) else {
                        return;
                    };
                    row.operands.push(RegisterOperandConstraint {
                        operand: row.operands.len() as u16,
                        access: RegisterOperandAccess::Def,
                        class: view.class,
                        fixed_view: Some(view.id),
                        tied_to: None,
                        early_clobber: false,
                    });
                    row.clobbers.retain(|unit| !view.write_units.contains(unit));
                }
                constraints.push(row);
            }
        }
    }

    pub(super) fn append_normalized_foreign_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        _model: &ValidatedPhysicalRegisterModel,
    ) {
        for (base_key, keys) in [
            (
                aarch64_aapcs64_register_unit_call_keys()[0],
                super::aarch64_aapcs64_normalized_foreign_call_keys(),
            ),
            (
                aarch64_darwin_register_unit_call_keys()[0],
                super::aarch64_darwin_normalized_foreign_call_keys(),
            ),
        ] {
            let Some(mut row) = find_register_constraint(constraints, base_key).cloned() else {
                return;
            };
            let [key] = keys.as_slice() else {
                return;
            };
            row.key = *key;
            constraints.push(row);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{
            aarch64_physical_register_model, aarch64_register_aggregate_call_keys,
            aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
        };
        use super::{
            RegisterOperandAccess, aarch64_aapcs64_mixed_unit_call_keys,
            aarch64_darwin_mixed_unit_call_keys, aarch64_mixed_aggregate_call_keys,
        };
        use register_model::validate_physical_register_model;

        #[test]
        fn mixed_aggregate_rows_preserve_banks_and_define_only_result_fragments() {
            let model =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let catalog = aarch64_register_constraint_catalog(&model);
            for darwin in [false, true] {
                let unit_keys = if darwin {
                    aarch64_darwin_mixed_unit_call_keys()
                } else {
                    aarch64_aapcs64_mixed_unit_call_keys()
                };
                let old_keys = aarch64_register_aggregate_call_keys(darwin);
                for (ordinal, key) in aarch64_mixed_aggregate_call_keys(darwin)
                    .into_iter()
                    .enumerate()
                {
                    assert!(!old_keys.contains(&key));
                    let fragments = ordinal / 72 + 1;
                    let general_count = ordinal % 72 / 8;
                    let float_count = ordinal % 8 + 1;
                    let arity = general_count + float_count;
                    let row = catalog.constraint(key).unwrap();
                    let unit = catalog.constraint(unit_keys[ordinal % 72]).unwrap();
                    assert_eq!(row.operands.len(), arity + fragments);
                    let names = (0..general_count)
                        .map(|position| format!("x{position}"))
                        .chain((0..float_count).map(|position| format!("d{position}")))
                        .chain((0..fragments).map(|position| format!("x{position}")));
                    for (position, (operand, name)) in row.operands.iter().zip(names).enumerate() {
                        let view = model.model().view_named(&name).unwrap();
                        assert_eq!(operand.operand, position as u16);
                        assert_eq!(operand.fixed_view, Some(view.id));
                        assert_eq!(operand.class, view.class);
                        assert_eq!(
                            operand.access,
                            if position < arity {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                        );
                        assert_eq!(operand.tied_to, None);
                        assert!(!operand.early_clobber);
                    }
                    assert_eq!(row.implicit_uses, unit.implicit_uses);
                    assert_eq!(row.implicit_defs, unit.implicit_defs);
                    let mut clobbers = unit.clobbers.clone();
                    for name in ["x0", "x1"].into_iter().take(fragments) {
                        let view = model.model().view_named(name).unwrap();
                        clobbers.retain(|unit| !view.write_units.contains(unit));
                    }
                    assert_eq!(row.clobbers, clobbers);
                }
            }
            validate_aarch64_register_constraint_catalog(catalog, &model).unwrap();
        }

        #[test]
        fn mixed_aggregate_rows_reject_result_bank_access_and_clobber_mutations() {
            let model =
                validate_physical_register_model(aarch64_physical_register_model()).unwrap();
            let catalog = aarch64_register_constraint_catalog(&model);
            for darwin in [false, true] {
                let key = aarch64_mixed_aggregate_call_keys(darwin)[143];
                for mutation in 0..4 {
                    let mut changed = catalog.clone();
                    let row =
                        register_model::find_register_constraint_mut(&mut changed.constraints, key)
                            .unwrap();
                    match mutation {
                        0 => row.operands.last_mut().unwrap().access = RegisterOperandAccess::Use,
                        1 => {
                            let view = model.model().view_named("d1").unwrap();
                            let operand = row.operands.last_mut().unwrap();
                            operand.fixed_view = Some(view.id);
                            operand.class = view.class;
                        }
                        2 => row
                            .clobbers
                            .extend(&model.model().view_named("x1").unwrap().write_units),
                        3 => row.clobbers.retain(|unit| {
                            !model
                                .model()
                                .view_named("x2")
                                .unwrap()
                                .write_units
                                .contains(unit)
                        }),
                        _ => unreachable!(),
                    }
                    assert!(validate_aarch64_register_constraint_catalog(changed, &model).is_err());
                }
            }
        }
    }
}
mod operation_keys {
    use crate::register_model::{
        AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
        AARCH64_AAPCS64_RETURN_UNIT, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
        AARCH64_DARWIN_RETURN_UNIT,
    };
    use register_model::RegisterConstraintKey;

    pub const AARCH64_MEMORY_FENCE: RegisterConstraintKey = RegisterConstraintKey::instruction(801);

    pub const AARCH64_ATOMIC_FENCE: RegisterConstraintKey = RegisterConstraintKey::instruction(800);
    pub const AARCH64_ATOMIC_LOAD: RegisterConstraintKey = RegisterConstraintKey::instruction(781);
    pub const AARCH64_ATOMIC_STORE: RegisterConstraintKey = RegisterConstraintKey::instruction(782);
    pub const AARCH64_ATOMIC_UPDATE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(783);
    pub const AARCH64_ATOMIC_COMPARE_EXCHANGE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(784);

    pub const AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(810);

    pub const AARCH64_CRASH: RegisterConstraintKey = RegisterConstraintKey::instruction(780);

    pub const AARCH64_SAVE_FLOATING_CONTROL: RegisterConstraintKey =
        RegisterConstraintKey::instruction(753);
    pub const AARCH64_RESTORE_FLOATING_CONTROL: RegisterConstraintKey =
        RegisterConstraintKey::instruction(754);

    pub const AARCH64_IEEE_FLOAT_ARITHMETIC: RegisterConstraintKey =
        RegisterConstraintKey::instruction(785);

    pub const AARCH64_IEEE_FLOAT_UNARY: RegisterConstraintKey =
        RegisterConstraintKey::instruction(786);

    pub const AARCH64_IEEE_FLOAT_TERNARY: RegisterConstraintKey =
        RegisterConstraintKey::instruction(787);

    pub const AARCH64_FLOAT_RESULT_TRAPPING: RegisterConstraintKey =
        RegisterConstraintKey::instruction(788);
    pub const AARCH64_FLOAT_RESULT_SATURATING: RegisterConstraintKey =
        RegisterConstraintKey::instruction(789);

    pub const AARCH64_COPY_BYTES: RegisterConstraintKey = RegisterConstraintKey::instruction(752);

    pub const AARCH64_LOAD_ENCODED_SCALAR: RegisterConstraintKey =
        RegisterConstraintKey::instruction(797);
    pub const AARCH64_STORE_ENCODED_SCALAR: RegisterConstraintKey =
        RegisterConstraintKey::instruction(798);

    pub const AARCH64_SCAN_BYTE_SEQUENCE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(794);

    pub const AARCH64_COMPARE_BYTE_SEQUENCES: RegisterConstraintKey =
        RegisterConstraintKey::instruction(795);

    pub const AARCH64_ZERO_BYTES: RegisterConstraintKey = RegisterConstraintKey::instruction(796);

    pub const AARCH64_LOAD8: RegisterConstraintKey = RegisterConstraintKey::instruction(730);

    pub const AARCH64_LOAD_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(750);

    pub const AARCH64_STORE_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(751);

    pub const AARCH64_LOAD16: RegisterConstraintKey = RegisterConstraintKey::instruction(731);

    pub const AARCH64_LOAD32: RegisterConstraintKey = RegisterConstraintKey::instruction(714);

    pub const AARCH64_FLOAT32_TO_BITS: RegisterConstraintKey =
        RegisterConstraintKey::instruction(710);

    pub const AARCH64_FLOAT64_TO_BITS: RegisterConstraintKey =
        RegisterConstraintKey::instruction(711);

    pub const AARCH64_BITS_TO_FLOAT32: RegisterConstraintKey =
        RegisterConstraintKey::instruction(712);

    pub const AARCH64_BITS_TO_FLOAT64: RegisterConstraintKey =
        RegisterConstraintKey::instruction(713);

    pub(crate) const AARCH64_LINUX_SYSTEM_CALL: RegisterConstraintKey =
        RegisterConstraintKey::system_call(0);

    pub(crate) const AARCH64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey =
        RegisterConstraintKey::inline_assembly(0);

    pub const AARCH64_MATERIALIZE_BOOLEAN: RegisterConstraintKey =
        RegisterConstraintKey::instruction(732);

    pub const AARCH64_MATERIALIZE_I64: RegisterConstraintKey =
        RegisterConstraintKey::instruction(0);

    pub const AARCH64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(1);

    pub const AARCH64_COMPARE_I64_ZERO: RegisterConstraintKey =
        RegisterConstraintKey::instruction(2);

    pub const AARCH64_CONDITIONAL_BRANCH: RegisterConstraintKey =
        RegisterConstraintKey::instruction(3);

    pub const AARCH64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(4);

    pub const AARCH64_ADD_I64_IMMEDIATE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(5);

    pub const AARCH64_SATURATING_SUBTRACT_UNSIGNED: RegisterConstraintKey =
        RegisterConstraintKey::instruction(54);

    pub const AARCH64_SATURATING_ADD_U64: RegisterConstraintKey =
        RegisterConstraintKey::instruction(55);

    pub const AARCH64_DIVIDE_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(56);

    pub const AARCH64_REMAINDER_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(57);

    pub const AARCH64_SATURATING_ADD_CLAMPED: RegisterConstraintKey =
        RegisterConstraintKey::instruction(58);

    pub const AARCH64_SATURATING_SUBTRACT_CLAMPED: RegisterConstraintKey =
        RegisterConstraintKey::instruction(59);

    pub const AARCH64_SATURATING_DIVIDE_SIGNED: RegisterConstraintKey =
        RegisterConstraintKey::instruction(60);

    pub const AARCH64_SATURATING_MULTIPLY_CLAMPED: RegisterConstraintKey =
        RegisterConstraintKey::instruction(65);

    pub const AARCH64_SATURATING_MULTIPLY_U64: RegisterConstraintKey =
        RegisterConstraintKey::instruction(66);

    pub const AARCH64_TRAPPING_BINARY: RegisterConstraintKey =
        RegisterConstraintKey::instruction(67);

    pub const AARCH64_TRAPPING_FIXED_PAIR: RegisterConstraintKey =
        RegisterConstraintKey::instruction(68);

    pub const AARCH64_TRAPPING_SHIFT: RegisterConstraintKey =
        RegisterConstraintKey::instruction(69);

    pub const AARCH64_TRAPPING_CONVERT: RegisterConstraintKey =
        RegisterConstraintKey::instruction(70);

    pub const AARCH64_MULTIPLY_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(61);

    pub const AARCH64_DIVIDE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(62);

    pub const AARCH64_REMAINDER_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(63);

    pub const AARCH64_SHIFT_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(64);

    pub const AARCH64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(6);

    pub const AARCH64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(7);

    pub const AARCH64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(8);

    pub const AARCH64_COMPARE_I64_IMMEDIATE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(18);

    pub const AARCH64_JUMP: RegisterConstraintKey = RegisterConstraintKey::instruction(9);

    pub const AARCH64_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey =
        RegisterConstraintKey::instruction(715);

    pub const AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey =
        RegisterConstraintKey::instruction(716);

    pub const AARCH64_DARWIN_HOSTED_READ_BYTE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(718);

    pub const AARCH64_HOSTED_READ_BYTE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(717);

    pub const AARCH64_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey =
        RegisterConstraintKey::instruction(14);

    pub const AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey =
        RegisterConstraintKey::instruction(17);

    pub const AARCH64_LOAD64: RegisterConstraintKey = RegisterConstraintKey::instruction(10);

    pub const AARCH64_LOAD8_INDEXED: RegisterConstraintKey = RegisterConstraintKey::instruction(11);

    pub const AARCH64_STORE: RegisterConstraintKey = RegisterConstraintKey::instruction(15);

    pub const AARCH64_ADDRESS_OFFSET: RegisterConstraintKey =
        RegisterConstraintKey::instruction(16);

    pub const AARCH64_STORE64: RegisterConstraintKey = RegisterConstraintKey::instruction(12);

    pub const AARCH64_FRAME_ADDRESS: RegisterConstraintKey = RegisterConstraintKey::instruction(13);

    pub const AARCH64_BYTE_LITERAL_IMAGE_ADDRESS: RegisterConstraintKey =
        RegisterConstraintKey::instruction(791);

    pub(crate) const AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 120] = [
        AARCH64_AAPCS64_CALL,
        AARCH64_DARWIN_CALL,
        AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64,
        RegisterConstraintKey::call(3),
        RegisterConstraintKey::call(4),
        RegisterConstraintKey::call(5),
        RegisterConstraintKey::call(6),
        RegisterConstraintKey::call(7),
        RegisterConstraintKey::call(8),
        RegisterConstraintKey::call(9),
        RegisterConstraintKey::call(10),
        RegisterConstraintKey::call(11),
        RegisterConstraintKey::call(12),
        RegisterConstraintKey::call(13),
        RegisterConstraintKey::call(14),
        RegisterConstraintKey::call(15),
        RegisterConstraintKey::call(16),
        RegisterConstraintKey::call(17),
        RegisterConstraintKey::call(18),
        RegisterConstraintKey::call(19),
        RegisterConstraintKey::call(700),
        RegisterConstraintKey::call(701),
        RegisterConstraintKey::call(702),
        RegisterConstraintKey::call(703),
        RegisterConstraintKey::call(704),
        RegisterConstraintKey::call(705),
        RegisterConstraintKey::call(706),
        RegisterConstraintKey::call(707),
        RegisterConstraintKey::call(708),
        RegisterConstraintKey::call(720),
        RegisterConstraintKey::call(721),
        RegisterConstraintKey::call(722),
        RegisterConstraintKey::call(723),
        RegisterConstraintKey::call(724),
        RegisterConstraintKey::call(725),
        RegisterConstraintKey::call(726),
        RegisterConstraintKey::call(727),
        RegisterConstraintKey::call(728),
        AARCH64_AAPCS64_RETURN,
        AARCH64_DARWIN_RETURN,
        AARCH64_AAPCS64_RETURN_UNIT,
        AARCH64_DARWIN_RETURN_UNIT,
        AARCH64_LINUX_SYSTEM_CALL,
        AARCH64_INLINE_ASSEMBLY_DEFAULT,
        AARCH64_MATERIALIZE_I64,
        AARCH64_COPY_I64,
        AARCH64_COMPARE_I64_ZERO,
        AARCH64_CONDITIONAL_BRANCH,
        AARCH64_ADD_I64,
        AARCH64_ADD_I64_IMMEDIATE,
        AARCH64_SUBTRACT_I64,
        AARCH64_SUBTRACT_I64_IMMEDIATE,
        AARCH64_COMPARE_I64,
        AARCH64_JUMP,
        AARCH64_LOAD64,
        AARCH64_LOAD8_INDEXED,
        AARCH64_STORE64,
        AARCH64_FRAME_ADDRESS,
        AARCH64_HOSTED_WRITE_BYTE_I32,
        AARCH64_STORE,
        AARCH64_ADDRESS_OFFSET,
        AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32,
        AARCH64_COMPARE_I64_IMMEDIATE,
        AARCH64_SATURATING_SUBTRACT_UNSIGNED,
        AARCH64_SATURATING_ADD_U64,
        AARCH64_DIVIDE_U64,
        AARCH64_REMAINDER_I64,
        AARCH64_SATURATING_ADD_CLAMPED,
        AARCH64_SATURATING_SUBTRACT_CLAMPED,
        AARCH64_SATURATING_DIVIDE_SIGNED,
        AARCH64_MULTIPLY_I64,
        AARCH64_DIVIDE_I64,
        AARCH64_REMAINDER_U64,
        AARCH64_SHIFT_I64,
        AARCH64_SATURATING_MULTIPLY_CLAMPED,
        AARCH64_SATURATING_MULTIPLY_U64,
        AARCH64_TRAPPING_BINARY,
        AARCH64_TRAPPING_FIXED_PAIR,
        AARCH64_TRAPPING_SHIFT,
        AARCH64_TRAPPING_CONVERT,
        AARCH64_FLOAT32_TO_BITS,
        AARCH64_FLOAT64_TO_BITS,
        AARCH64_BITS_TO_FLOAT32,
        AARCH64_BITS_TO_FLOAT64,
        AARCH64_LOAD32,
        AARCH64_HOSTED_EXIT_PROCESS_I32,
        AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32,
        AARCH64_HOSTED_READ_BYTE,
        AARCH64_DARWIN_HOSTED_READ_BYTE,
        AARCH64_LOAD8,
        AARCH64_LOAD16,
        AARCH64_MATERIALIZE_BOOLEAN,
        AARCH64_LOAD_PACKED,
        AARCH64_STORE_PACKED,
        AARCH64_COPY_BYTES,
        AARCH64_SAVE_FLOATING_CONTROL,
        AARCH64_RESTORE_FLOATING_CONTROL,
        AARCH64_CRASH,
        AARCH64_ATOMIC_LOAD,
        AARCH64_ATOMIC_STORE,
        AARCH64_ATOMIC_UPDATE,
        AARCH64_ATOMIC_COMPARE_EXCHANGE,
        AARCH64_IEEE_FLOAT_ARITHMETIC,
        AARCH64_IEEE_FLOAT_UNARY,
        AARCH64_IEEE_FLOAT_TERNARY,
        AARCH64_FLOAT_RESULT_TRAPPING,
        AARCH64_FLOAT_RESULT_SATURATING,
        AARCH64_BYTE_LITERAL_IMAGE_ADDRESS,
        AARCH64_SCAN_BYTE_SEQUENCE,
        AARCH64_COMPARE_BYTE_SEQUENCES,
        AARCH64_ZERO_BYTES,
        AARCH64_LOAD_ENCODED_SCALAR,
        AARCH64_STORE_ENCODED_SCALAR,
        AARCH64_ATOMIC_FENCE,
        AARCH64_MEMORY_FENCE,
        AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE,
        AARCH64_ACCESSOR_READ,
        AARCH64_ACCESSOR_WRITE,
        AARCH64_SYSTEM_REGISTER_READ,
        AARCH64_SYSTEM_REGISTER_WRITE,
    ];

    pub const AARCH64_SYSTEM_REGISTER_READ: RegisterConstraintKey =
        RegisterConstraintKey::instruction(902);

    pub const AARCH64_SYSTEM_REGISTER_WRITE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(903);

    pub const AARCH64_ACCESSOR_READ: RegisterConstraintKey =
        RegisterConstraintKey::instruction(900);

    pub const AARCH64_ACCESSOR_WRITE: RegisterConstraintKey =
        RegisterConstraintKey::instruction(901);
}
mod physical_model {
    use calling_conventions::MachineRegister;
    use register_model::{
        PhysicalModelBuilder, PhysicalRegisterModel, PhysicalRegisterModelIdentity,
        PreservationConvention, RegisterClassId, RegisterUnitId, RegisterUnitKind, RegisterViewId,
        RegisterWriteSemantics, ReservationReason, ValidatedPhysicalRegisterModel, complement,
        reservation_overlay, sorted_units, validate_physical_register_model, view_ids,
    };
    use std::collections::BTreeMap;
    use target::Architecture;

    pub(crate) const GPR64: RegisterClassId = RegisterClassId(0);

    const GPR32: RegisterClassId = RegisterClassId(1);

    const VECTOR128: RegisterClassId = RegisterClassId(2);

    pub(crate) const FLOAT64: RegisterClassId = RegisterClassId(3);

    const FLOAT32: RegisterClassId = RegisterClassId(4);

    const FLAGS: RegisterClassId = RegisterClassId(5);

    const STACK64: RegisterClassId = RegisterClassId(6);

    const STACK32: RegisterClassId = RegisterClassId(7);

    const ZERO: RegisterClassId = RegisterClassId(8);

    const FLOAT_CONTROL: RegisterClassId = RegisterClassId(9);

    const INSTRUCTION_POINTER: RegisterClassId = RegisterClassId(10);

    pub fn aarch64_fixed_register_view(
        model: &ValidatedPhysicalRegisterModel,
        register: MachineRegister,
    ) -> Option<RegisterViewId> {
        if model.identity() != canonical_aarch64_physical_register_model_identity() {
            return None;
        }
        let name = match register {
            MachineRegister::Aarch64X(index @ 0..=30) => format!("x{index}"),
            MachineRegister::Aarch64V(index @ 0..=31) => format!("d{index}"),
            _ => return None,
        };
        model.model().view_named(&name).map(|view| view.id)
    }

    pub(crate) fn aarch64_physical_register_model_ref() -> &'static PhysicalRegisterModel {
        static MODEL: std::sync::OnceLock<PhysicalRegisterModel> = std::sync::OnceLock::new();
        MODEL.get_or_init(build_aarch64_physical_register_model)
    }

    pub fn aarch64_physical_register_model() -> PhysicalRegisterModel {
        aarch64_physical_register_model_ref().clone()
    }

    fn build_aarch64_physical_register_model() -> PhysicalRegisterModel {
        let mut builder = PhysicalModelBuilder::with_classes(&[
            "aarch64.gpr64",
            "aarch64.gpr32",
            "aarch64.vector128",
            "aarch64.float64",
            "aarch64.float32",
            "aarch64.flags",
            "aarch64.stack64",
            "aarch64.stack32",
            "aarch64.zero",
            "aarch64.float-control",
            "aarch64.instruction-pointer",
        ]);
        let mut named_units = BTreeMap::<String, RegisterUnitId>::new();
        let mut named_views = BTreeMap::<String, RegisterViewId>::new();
        for index in 0..31 {
            let x = format!("x{index}");
            let w = format!("w{index}");
            let unit = builder.unit(format!("{x}.storage"), 64, RegisterUnitKind::IntegerLane);
            let x_view = builder.view(
                x.clone(),
                GPR64,
                vec![unit],
                vec![unit],
                64,
                RegisterWriteSemantics::ExactView,
                true,
            );
            let w_view = builder.view(
                w.clone(),
                GPR32,
                vec![unit],
                vec![unit],
                32,
                RegisterWriteSemantics::ZeroExtendsWithinUnit,
                true,
            );
            named_units.insert(x.clone(), unit);
            named_views.extend([(x, x_view), (w, w_view)]);
        }
        let sp_unit = builder.unit("sp.storage".into(), 64, RegisterUnitKind::StackPointer);
        let sp_view = builder.view(
            "sp".into(),
            STACK64,
            vec![sp_unit],
            vec![sp_unit],
            64,
            RegisterWriteSemantics::ExactView,
            false,
        );
        let wsp_view = builder.view(
            "wsp".into(),
            STACK32,
            vec![sp_unit],
            vec![sp_unit],
            32,
            RegisterWriteSemantics::ZeroExtendsWithinUnit,
            false,
        );
        named_views.extend([("sp".into(), sp_view), ("wsp".into(), wsp_view)]);

        let zero_unit = builder.unit("zero.storage".into(), 64, RegisterUnitKind::Zero);
        let xzr_view = builder.view(
            "xzr".into(),
            ZERO,
            vec![zero_unit],
            vec![zero_unit],
            64,
            RegisterWriteSemantics::Discards,
            false,
        );
        let wzr_view = builder.view(
            "wzr".into(),
            ZERO,
            vec![zero_unit],
            vec![zero_unit],
            32,
            RegisterWriteSemantics::Discards,
            false,
        );
        named_views.extend([("xzr".into(), xzr_view), ("wzr".into(), wzr_view)]);

        let mut vector_units = Vec::new();
        for index in 0..32 {
            let low = builder.unit(
                format!("v{index}.bits0_63"),
                64,
                RegisterUnitKind::VectorLane,
            );
            let high = builder.unit(
                format!("v{index}.bits64_127"),
                64,
                RegisterUnitKind::VectorLane,
            );
            let q = format!("q{index}");
            let d = format!("d{index}");
            let s = format!("s{index}");
            let q_view = builder.view(
                q.clone(),
                VECTOR128,
                vec![low, high],
                vec![low, high],
                128,
                RegisterWriteSemantics::ExactView,
                true,
            );
            let d_view = builder.view(
                d.clone(),
                FLOAT64,
                vec![low],
                vec![low, high],
                64,
                RegisterWriteSemantics::InstructionDefined,
                true,
            );
            let s_view = builder.view(
                s.clone(),
                FLOAT32,
                vec![low],
                vec![low],
                32,
                RegisterWriteSemantics::InstructionDefined,
                true,
            );
            named_views.extend([(q, q_view), (d, d_view), (s, s_view)]);
            vector_units.push((low, high));
        }
        let nzcv_unit = builder.unit("nzcv.storage".into(), 4, RegisterUnitKind::Flags);
        let nzcv_view = builder.view(
            "nzcv".into(),
            FLAGS,
            vec![nzcv_unit],
            vec![nzcv_unit],
            4,
            RegisterWriteSemantics::InstructionDefined,
            false,
        );
        named_views.insert("nzcv".into(), nzcv_view);
        let fpcr_unit = builder.unit("fpcr.storage".into(), 32, RegisterUnitKind::FloatingControl);
        let fpcr_view = builder.view(
            "fpcr".into(),
            FLOAT_CONTROL,
            vec![fpcr_unit],
            vec![fpcr_unit],
            32,
            RegisterWriteSemantics::InstructionDefined,
            false,
        );
        named_views.insert("fpcr".into(), fpcr_view);
        let fpsr_unit = builder.unit("fpsr.storage".into(), 32, RegisterUnitKind::FloatingControl);
        let fpsr_view = builder.view(
            "fpsr".into(),
            FLOAT_CONTROL,
            vec![fpsr_unit],
            vec![fpsr_unit],
            32,
            RegisterWriteSemantics::InstructionDefined,
            false,
        );
        named_views.insert("fpsr".into(), fpsr_view);
        let pc_unit = builder.unit(
            "pc.storage".into(),
            64,
            RegisterUnitKind::InstructionPointer,
        );
        let pc_view = builder.view(
            "pc".into(),
            INSTRUCTION_POINTER,
            vec![pc_unit],
            vec![pc_unit],
            64,
            RegisterWriteSemantics::InstructionDefined,
            false,
        );
        named_views.insert("pc".into(), pc_view);

        let all_units = builder.unit_ids();
        let common_fixed = sorted_units([sp_unit, zero_unit, fpcr_unit, fpsr_unit, pc_unit]);
        let aapcs_callee = sorted_units(
            (19..=29)
                .map(|index| named_units[&format!("x{index}")])
                .chain(vector_units[8..16].iter().map(|(low, _)| *low)),
        );
        let aapcs_caller = complement(&all_units, &common_fixed, &aapcs_callee);
        let x18 = named_units["x18"];
        let darwin_fixed = sorted_units(common_fixed.iter().copied().chain([x18]));
        let darwin_caller = complement(&all_units, &darwin_fixed, &aapcs_callee);
        let conventions = vec![
            PreservationConvention {
                name: "aapcs64".into(),
                argument_views: view_ids(
                    &named_views,
                    &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
                ),
                result_views: view_ids(&named_views, &["x0"]),
                caller_saved: aapcs_caller.clone(),
                callee_saved: aapcs_callee.clone(),
                fixed: common_fixed.clone(),
                stack_alignment: 16,
                red_zone_bytes: 0,
            },
            PreservationConvention {
                name: "darwin-aapcs64".into(),
                argument_views: view_ids(
                    &named_views,
                    &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
                ),
                result_views: view_ids(&named_views, &["x0"]),
                caller_saved: darwin_caller,
                callee_saved: aapcs_callee,
                fixed: darwin_fixed,
                stack_alignment: 16,
                red_zone_bytes: 0,
            },
        ];
        macro_rules! r {
            ($a:expr => $b:expr => $c:expr) => {
                reservation_overlay($a, $b, $c)
            };
        }
        let reservations = vec![
            r!("aarch64.stack-pointer" => ReservationReason::StackPointer => vec![sp_unit]),
            r!("aarch64.zero-register" => ReservationReason::Architectural => vec![zero_unit]),
            r!("aarch64.instruction-pointer" => ReservationReason::Architectural => vec![pc_unit]),
            r!("aarch64.frame-pointer" => ReservationReason::FramePointer => vec![named_units["x29"]]),
            r!("darwin.aarch64.platform" => ReservationReason::Platform => vec![x18]),
            r!("omega.aarch64.dispatch" => ReservationReason::Dispatch => vec![named_units["x27"]]),
            r!("omega.aarch64.metering" => ReservationReason::Metering => vec![named_units["x28"]]),
        ];
        builder.finish(Architecture::Aarch64, conventions, reservations)
    }

    pub fn validated_aarch64_physical_register_model() -> &'static ValidatedPhysicalRegisterModel {
        static CANONICAL: std::sync::OnceLock<ValidatedPhysicalRegisterModel> =
            std::sync::OnceLock::new();
        CANONICAL.get_or_init(|| {
            validate_physical_register_model(aarch64_physical_register_model()).unwrap_or_else(
                |error| panic!("canonical aarch64 physical register model must validate: {error}"),
            )
        })
    }

    pub(crate) fn canonical_aarch64_physical_register_model_identity()
    -> PhysicalRegisterModelIdentity {
        validated_aarch64_physical_register_model().identity()
    }
}
mod system_calls {
    use register_model::{
        RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
        RegisterOperandAccess, RegisterOperandConstraint, ValidatedPhysicalRegisterModel,
    };

    pub fn aarch64_linux_system_call_keys() -> Vec<RegisterConstraintKey> {
        (100..114).map(RegisterConstraintKey::system_call).collect()
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        let physical = model.model();
        let view = |name| {
            physical
                .view_named(name)
                .expect("canonical kernel register")
        };
        let arguments = ["x0", "x1", "x2", "x3", "x4", "x5"];
        let result = view("x0");
        let clobbers = ["x0", "x1", "x2", "x3", "x4", "x5", "x8", "nzcv"]
            .into_iter()
            .flat_map(|name| view(name).units.iter().copied())
            .collect::<Vec<_>>();
        for (position, key) in aarch64_linux_system_call_keys().into_iter().enumerate() {
            let argument_count = position / 2;
            let returns_value = position % 2 != 0;
            let operands = arguments[..argument_count]
                .iter()
                .map(|name| (view(name), RegisterOperandAccess::Use))
                .chain(returns_value.then_some((result, RegisterOperandAccess::Def)))
                .enumerate()
                .map(|(operand, (register, access))| RegisterOperandConstraint {
                    operand: operand as u16,
                    access,
                    class: register.class,
                    fixed_view: Some(register.id),
                    tied_to: None,
                    early_clobber: false,
                })
                .collect();
            let mut clobbers = clobbers
                .iter()
                .copied()
                .filter(|unit| !returns_value || !result.units.contains(unit))
                .collect::<Vec<_>>();
            clobbers.sort_unstable();
            clobbers.dedup();
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: view("pc").units.clone(),
                implicit_defs: view("pc").units.clone(),
                clobbers,
            });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn system_call_rows_retain_exact_arguments_results_and_clobbers() {
            let physical = crate::validated_aarch64_physical_register_model();
            let catalog = crate::aarch64_register_constraint_catalog(physical);
            let validated =
                crate::validate_aarch64_register_constraint_catalog(catalog.clone(), physical)
                    .expect("canonical syscall constraints");
            for (position, key) in aarch64_linux_system_call_keys().into_iter().enumerate() {
                let argument_count = position / 2;
                let returns_value = position % 2 != 0;
                let row = validated.catalog().constraint(key).unwrap();
                assert_eq!(
                    row.operands.len(),
                    argument_count + usize::from(returns_value)
                );
                let expected = ["x0", "x1", "x2", "x3", "x4", "x5"];
                for (operand, name) in row.operands.iter().zip(&expected[..argument_count]) {
                    assert_eq!(operand.access, RegisterOperandAccess::Use);
                    assert_eq!(
                        operand.fixed_view,
                        Some(physical.model().view_named(name).unwrap().id)
                    );
                }
                if returns_value {
                    let result = row.operands.last().unwrap();
                    assert_eq!(result.access, RegisterOperandAccess::Def);
                    assert_eq!(
                        result.fixed_view,
                        Some(physical.model().view_named("x0").unwrap().id)
                    );
                }
                let mut written = row.clobbers.clone();
                if returns_value {
                    written.extend_from_slice(&physical.model().view_named("x0").unwrap().units);
                }
                written.sort_unstable();
                written.dedup();
                let mut expected_written = ["x0", "x1", "x2", "x3", "x4", "x5", "x8", "nzcv"]
                    .into_iter()
                    .flat_map(|name| {
                        physical
                            .model()
                            .view_named(name)
                            .unwrap()
                            .units
                            .iter()
                            .copied()
                    })
                    .collect::<Vec<_>>();
                expected_written.sort_unstable();
                expected_written.dedup();
                assert_eq!(written, expected_written);
                assert_eq!(
                    row.implicit_uses,
                    physical.model().view_named("pc").unwrap().units
                );
                assert_eq!(row.implicit_defs, row.implicit_uses);
            }

            let key = aarch64_linux_system_call_keys()[8];
            let mut wrong_argument = catalog.clone();
            let row =
                register_model::find_register_constraint_mut(&mut wrong_argument.constraints, key)
                    .unwrap();
            row.operands[3].fixed_view = Some(physical.model().view_named("x4").unwrap().id);
            assert!(
                crate::validate_aarch64_register_constraint_catalog(wrong_argument, physical)
                    .is_err()
            );

            let mut missing_clobbers = catalog;
            let row = register_model::find_register_constraint_mut(
                &mut missing_clobbers.constraints,
                key,
            )
            .unwrap();
            row.clobbers.clear();
            assert!(
                crate::validate_aarch64_register_constraint_catalog(missing_clobbers, physical)
                    .is_err()
            );
        }
    }
}
pub use system_calls::aarch64_linux_system_call_keys;
#[cfg(test)]
mod tests {
    use super::{
        AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
        AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO,
        AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_I64, AARCH64_LINUX_SYSTEM_CALL,
        AARCH64_MATERIALIZE_I64, AARCH64_REQUIRED_REGISTER_CONSTRAINTS, AARCH64_SUBTRACT_I64,
        Aarch64RegisterConstraintCatalogValidationError, aarch64_aapcs64_mixed_unit_call_keys,
        aarch64_aapcs64_normalized_foreign_call_keys, aarch64_darwin_mixed_unit_call_keys,
        aarch64_darwin_normalized_foreign_call_keys, aarch64_fixed_register_view,
        aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys,
        aarch64_indirect_aggregate_call_keys, aarch64_mixed_aggregate_call_keys,
        aarch64_physical_register_model, aarch64_preservation_convention_for_target,
        aarch64_register_aggregate_call_keys, aarch64_register_aggregate_return_keys,
        aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
    };
    use crate::register_model::physical_model::GPR64;
    use calling_conventions::MachineRegister;
    use register_model::test_support::{
        AggregateFragmentRoster, ForeignTemplateRoster, assert_aggregate_fragments_are_definitions,
        assert_templates_reject_changed_effects, assert_templates_retain_call_effects, row,
        row_mut,
    };
    use register_model::{
        RegisterConstraintCatalogValidationError, RegisterModelValidationError,
        validate_physical_register_model,
    };
    use register_model::{
        RegisterConstraintId, RegisterOperandAccess, RegisterUnit, RegisterUnitId, RegisterUnitKind,
    };
    use target::NativeTarget;

    #[test]
    fn aggregate_result_fragments_are_definitions_not_unknown_clobbers() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&model);
        assert_aggregate_fragments_are_definitions(
            &model,
            &catalog,
            &AggregateFragmentRoster {
                keys: [false, true]
                    .into_iter()
                    .flat_map(aarch64_register_aggregate_call_keys)
                    .collect(),
                keys_per_group: 9,
                second_view: "x1",
                scratch_view: "x2",
            },
            |catalog, model| validate_aarch64_register_constraint_catalog(catalog, model).is_ok(),
        );
    }

    #[test]
    fn fixed_machine_register_views_are_target_owned() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        assert_eq!(
            aarch64_fixed_register_view(&model, MachineRegister::Aarch64X(0)),
            model.model().view_named("x0").map(|view| view.id)
        );
        assert_eq!(
            aarch64_fixed_register_view(&model, MachineRegister::X86Rdi),
            None
        );
    }

    #[test]
    fn preservation_convention_is_selected_by_exact_target_policy() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        assert_eq!(
            aarch64_preservation_convention_for_target(&model, NativeTarget::linux_arm64())
                .unwrap()
                .name,
            "aapcs64"
        );
        assert_eq!(
            aarch64_preservation_convention_for_target(&model, NativeTarget::macos_arm64())
                .unwrap()
                .name,
            "darwin-aapcs64"
        );
        assert!(
            aarch64_preservation_convention_for_target(&model, NativeTarget::windows_x64())
                .is_none()
        );
    }

    #[test]
    fn model_validates_without_collapsing_stack_and_zero_registers() {
        let validated =
            validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let model = validated.model();
        let view = |name| model.view_named(name).unwrap().id;
        assert!(model.aliases(view("x0"), view("w0")));
        assert!(model.aliases(view("q0"), view("d0")));
        assert!(!model.aliases(view("sp"), view("xzr")));
        let aapcs = model.convention_named("aapcs64").unwrap();
        let d8 = model.view_named("d8").unwrap();
        let q8 = model.view_named("q8").unwrap();
        assert!(aapcs.callee_saved.contains(&d8.units[0]));
        assert!(!aapcs.callee_saved.contains(&q8.units[1]));
        assert!(!model.view_named("sp").unwrap().allocatable);
        assert!(!model.view_named("xzr").unwrap().allocatable);
    }

    #[test]
    fn register_constraint_catalog_closes_the_required_aarch64_inventory() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let validated = validate_aarch64_register_constraint_catalog(
            aarch64_register_constraint_catalog(&model),
            &model,
        )
        .unwrap();
        let catalog = validated.catalog();
        assert!(
            AARCH64_REQUIRED_REGISTER_CONSTRAINTS
                .iter()
                .all(|key| catalog.required.contains(key))
        );
        assert_eq!(
            catalog.required.len(),
            AARCH64_REQUIRED_REGISTER_CONSTRAINTS.len()
                + aarch64_indirect_aggregate_call_keys(false).len()
                + aarch64_indirect_aggregate_call_keys(true).len()
                + aarch64_float_scalar_call_keys(false).len()
                + aarch64_float_scalar_call_keys(true).len()
                + aarch64_float_scalar_return_keys(false).len()
                + aarch64_float_scalar_return_keys(true).len()
                + aarch64_aapcs64_mixed_unit_call_keys().len()
                + aarch64_darwin_mixed_unit_call_keys().len()
                + aarch64_register_aggregate_call_keys(false).len()
                + aarch64_register_aggregate_call_keys(true).len()
                + aarch64_mixed_aggregate_call_keys(false).len()
                + aarch64_mixed_aggregate_call_keys(true).len()
                + aarch64_register_aggregate_return_keys(false).len()
                + aarch64_register_aggregate_return_keys(true).len()
                + aarch64_aapcs64_normalized_foreign_call_keys().len()
                + aarch64_darwin_normalized_foreign_call_keys().len()
                + crate::aarch64_linux_system_call_keys().len()
        );

        let call = row(catalog, AARCH64_AAPCS64_CALL);
        assert_eq!(call.key, AARCH64_AAPCS64_CALL);
        assert_eq!(call.operands.len(), 9);
        assert_eq!(call.operands[8].access, RegisterOperandAccess::Def);
        for state in ["x30", "pc"] {
            assert!(
                model
                    .model()
                    .view_named(state)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| call.implicit_defs.contains(unit))
            );
        }

        let scalar_call = row(catalog, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64);
        assert_eq!(scalar_call.key, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64);
        assert_eq!(scalar_call.operands.len(), 3);
        assert_eq!(
            scalar_call.operands[0].fixed_view,
            Some(model.model().view_named("x0").unwrap().id)
        );
        assert_eq!(
            scalar_call.operands[1].fixed_view,
            Some(model.model().view_named("x1").unwrap().id)
        );
        assert_eq!(
            scalar_call.operands[2].fixed_view,
            Some(model.model().view_named("x0").unwrap().id)
        );

        let returned = row(catalog, AARCH64_AAPCS64_RETURN);
        assert!(
            model
                .model()
                .view_named("x30")
                .unwrap()
                .units
                .iter()
                .all(|unit| returned.implicit_uses.contains(unit))
        );
        assert!(
            model
                .model()
                .view_named("pc")
                .unwrap()
                .units
                .iter()
                .all(|unit| returned.implicit_defs.contains(unit))
        );

        let syscall = row(catalog, AARCH64_LINUX_SYSTEM_CALL);
        assert_eq!(syscall.key, AARCH64_LINUX_SYSTEM_CALL);
        assert_eq!(syscall.operands[1].access, RegisterOperandAccess::UseDef);
        assert_eq!(
            syscall.operands[0].fixed_view,
            Some(model.model().view_named("x8").unwrap().id)
        );
        assert!(
            model
                .model()
                .view_named("nzcv")
                .unwrap()
                .units
                .iter()
                .all(|unit| syscall.clobbers.contains(unit))
        );

        let materialize = row(catalog, AARCH64_MATERIALIZE_I64);
        assert_eq!(materialize.key, AARCH64_MATERIALIZE_I64);
        assert_eq!(materialize.operands.len(), 1);
        assert_eq!(materialize.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(materialize.operands[0].class, GPR64);

        let copy = row(catalog, AARCH64_COPY_I64);
        assert_eq!(copy.key, AARCH64_COPY_I64);
        assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(copy.operands[1].access, RegisterOperandAccess::Def);

        let compare = row(catalog, AARCH64_COMPARE_I64_ZERO);
        assert_eq!(compare.key, AARCH64_COMPARE_I64_ZERO);
        assert_eq!(compare.operands[0].class, GPR64);
        assert_eq!(
            compare.implicit_defs,
            model.model().view_named("nzcv").unwrap().units
        );

        let branch = row(catalog, AARCH64_CONDITIONAL_BRANCH);
        assert_eq!(branch.key, AARCH64_CONDITIONAL_BRANCH);
        for state in ["nzcv", "pc"] {
            assert!(
                model
                    .model()
                    .view_named(state)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| branch.implicit_uses.contains(unit))
            );
        }

        let add = row(catalog, AARCH64_ADD_I64);
        assert_eq!(add.key, AARCH64_ADD_I64);
        assert_eq!(add.operands.len(), 3);
        assert_eq!(add.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(add.operands[1].access, RegisterOperandAccess::Use);
        assert_eq!(add.operands[2].access, RegisterOperandAccess::Def);
        assert!(add.operands.iter().all(|operand| operand.class == GPR64));
        assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
        assert!(add.implicit_uses.is_empty());
        assert!(add.implicit_defs.is_empty());
        assert!(add.clobbers.is_empty());

        let add_immediate = row(catalog, AARCH64_ADD_I64_IMMEDIATE);
        assert_eq!(add_immediate.key, AARCH64_ADD_I64_IMMEDIATE);
        assert_eq!(add_immediate.operands.len(), 2);
        assert_eq!(add_immediate.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(add_immediate.operands[1].access, RegisterOperandAccess::Def);
        assert!(
            add_immediate
                .operands
                .iter()
                .all(|operand| operand.class == GPR64
                    && operand.fixed_view.is_none()
                    && operand.tied_to.is_none()
                    && !operand.early_clobber)
        );
        assert!(add_immediate.implicit_uses.is_empty());
        assert!(add_immediate.implicit_defs.is_empty());
        assert!(add_immediate.clobbers.is_empty());

        let subtract = row(catalog, AARCH64_SUBTRACT_I64);
        assert_eq!(subtract.key, AARCH64_SUBTRACT_I64);
        assert_eq!(subtract.operands.len(), 3);
        assert_eq!(subtract.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(subtract.operands[1].access, RegisterOperandAccess::Use);
        assert_eq!(subtract.operands[2].access, RegisterOperandAccess::Def);
        assert!(
            subtract
                .operands
                .iter()
                .all(|operand| operand.class == GPR64
                    && operand.fixed_view.is_none()
                    && operand.tied_to.is_none()
                    && !operand.early_clobber)
        );
        assert!(subtract.implicit_uses.is_empty());
        assert!(subtract.implicit_defs.is_empty());
        assert!(subtract.clobbers.is_empty());
        assert_eq!(
            branch.implicit_defs,
            model.model().view_named("pc").unwrap().units
        );
    }

    #[test]
    fn every_missing_required_aarch64_constraint_rejects() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        for (position, expected) in aarch64_register_constraint_catalog(&model)
            .required
            .into_iter()
            .enumerate()
        {
            let mut catalog = aarch64_register_constraint_catalog(&model);
            catalog.constraints.remove(position);
            for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
                constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
            }
            assert_eq!(
                validate_aarch64_register_constraint_catalog(catalog, &model),
                Err(Aarch64RegisterConstraintCatalogValidationError::Structural(
                    RegisterConstraintCatalogValidationError::MissingRequiredConstraint(expected)
                ))
            );
        }
    }

    #[test]
    fn aarch64_target_semantics_reject_class_compatible_corruption() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let mut wrong_syscall_register = aarch64_register_constraint_catalog(&model);
        row_mut(&mut wrong_syscall_register, AARCH64_LINUX_SYSTEM_CALL).operands[5].fixed_view =
            Some(model.model().view_named("x3").unwrap().id);
        assert_eq!(
            validate_aarch64_register_constraint_catalog(wrong_syscall_register, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_LINUX_SYSTEM_CALL
                )
            )
        );

        let mut missing_nzcv = aarch64_register_constraint_catalog(&model);
        row_mut(&mut missing_nzcv, AARCH64_LINUX_SYSTEM_CALL)
            .clobbers
            .clear();
        assert_eq!(
            validate_aarch64_register_constraint_catalog(missing_nzcv, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_LINUX_SYSTEM_CALL
                )
            )
        );

        let mut missing_link_state = aarch64_register_constraint_catalog(&model);
        let x30 = model.model().view_named("x30").unwrap().units[0];
        row_mut(&mut missing_link_state, AARCH64_AAPCS64_CALL)
            .implicit_defs
            .retain(|unit| *unit != x30);
        assert_eq!(
            validate_aarch64_register_constraint_catalog(missing_link_state, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_AAPCS64_CALL
                )
            )
        );
    }

    #[test]
    fn aarch64_compare_rejects_one_field_missing_flags_definition() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let mut catalog = aarch64_register_constraint_catalog(&model);
        row_mut(&mut catalog, AARCH64_COMPARE_I64_ZERO)
            .implicit_defs
            .clear();
        assert_eq!(
            validate_aarch64_register_constraint_catalog(catalog, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_COMPARE_I64_ZERO,
                )
            )
        );

        let mut immediate = aarch64_register_constraint_catalog(&model);
        row_mut(&mut immediate, AARCH64_ADD_I64_IMMEDIATE).operands[0].access =
            RegisterOperandAccess::Def;
        assert_eq!(
            validate_aarch64_register_constraint_catalog(immediate, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_ADD_I64_IMMEDIATE,
                )
            )
        );
    }

    #[test]
    fn aarch64_add_rejects_one_field_operand_role_change() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let mut catalog = aarch64_register_constraint_catalog(&model);
        row_mut(&mut catalog, AARCH64_ADD_I64).operands[1].access = RegisterOperandAccess::Def;
        assert_eq!(
            validate_aarch64_register_constraint_catalog(catalog, &model),
            Err(Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(AARCH64_ADD_I64,))
        );

        let mut subtract = aarch64_register_constraint_catalog(&model);
        row_mut(&mut subtract, AARCH64_SUBTRACT_I64).operands[1].access =
            RegisterOperandAccess::Def;
        assert_eq!(
            validate_aarch64_register_constraint_catalog(subtract, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_SUBTRACT_I64,
                )
            )
        );

        let mut flag_clobber = aarch64_register_constraint_catalog(&model);
        row_mut(&mut flag_clobber, AARCH64_SUBTRACT_I64).clobbers =
            model.model().view_named("nzcv").unwrap().units.clone();
        assert_eq!(
            validate_aarch64_register_constraint_catalog(flag_clobber, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_SUBTRACT_I64,
                )
            )
        );
    }

    #[test]
    fn aarch64_catalog_validation_rejects_same_architecture_forged_physical_model() {
        let canonical =
            validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&canonical);
        let mut forged = aarch64_physical_register_model();
        forged.views[0].name = "forged.x0".into();
        let forged = validate_physical_register_model(forged).unwrap();
        assert_eq!(
            aarch64_fixed_register_view(&forged, MachineRegister::Aarch64X(0)),
            None
        );
        assert_eq!(
            validate_aarch64_register_constraint_catalog(catalog, &forged),
            Err(Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel)
        );
    }

    #[test]
    fn unknown_and_omitted_units_reject() {
        let mut unknown = aarch64_physical_register_model();
        unknown.views[0].units[0] = RegisterUnitId(u16::MAX);
        assert_eq!(
            validate_physical_register_model(unknown),
            Err(RegisterModelValidationError::UnknownUnit(RegisterUnitId(
                u16::MAX
            )))
        );

        let mut omitted = aarch64_physical_register_model();
        let omitted_id = RegisterUnitId(u16::try_from(omitted.units.len()).unwrap());
        omitted.units.push(RegisterUnit {
            id: omitted_id,
            name: "omitted.storage".into(),
            bits: 1,
            kind: RegisterUnitKind::Flags,
        });
        assert_eq!(
            validate_physical_register_model(omitted),
            Err(RegisterModelValidationError::UnitNotCovered(omitted_id))
        );
    }

    fn aarch64_foreign_template_roster() -> ForeignTemplateRoster {
        ForeignTemplateRoster {
            templates: vec![
                (
                    super::aarch64_aapcs64_register_unit_call_keys()[0],
                    super::aarch64_aapcs64_normalized_foreign_call_keys(),
                ),
                (
                    super::aarch64_darwin_register_unit_call_keys()[0],
                    super::aarch64_darwin_normalized_foreign_call_keys(),
                ),
            ],
            signature_views: ["x0", "d0"],
            effect_view: "x2",
        }
    }

    #[test]
    fn normalized_foreign_templates_retain_complete_target_call_effects() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&model);
        assert_templates_retain_call_effects(&catalog, &aarch64_foreign_template_roster());
    }

    #[test]
    fn normalized_foreign_templates_reject_signature_rows_and_changed_effects() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&model);
        assert_templates_reject_changed_effects(
            &model,
            &catalog,
            &aarch64_foreign_template_roster(),
            |catalog, model| validate_aarch64_register_constraint_catalog(catalog, model).is_ok(),
        );
    }
}
pub(crate) use call_keys::AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64;
pub use call_keys::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_DARWIN_CALL,
    AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
    aarch64_aapcs64_normalized_foreign_call_keys, aarch64_aapcs64_register_call_keys,
    aarch64_aapcs64_register_unit_call_keys, aarch64_darwin_normalized_foreign_call_keys,
    aarch64_darwin_register_call_keys, aarch64_darwin_register_unit_call_keys,
    aarch64_preservation_convention_for_target, aarch64_register_aggregate_call_keys,
    aarch64_register_aggregate_return_keys, aarch64_register_unit_call_keys,
};
pub(crate) use constraint_catalog::aarch64_register_constraint_catalog_for;
pub use constraint_catalog::{
    Aarch64RegisterConstraintCatalogValidationError, aarch64_register_constraint_catalog,
    prepare_aarch64_register_constraint_catalog, validate_aarch64_register_constraint_catalog,
};
pub use float_scalar_calls::{aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys};
pub use indirect_results::aarch64_indirect_aggregate_call_keys;
pub use mixed_calls::{
    aarch64_aapcs64_mixed_unit_call_keys, aarch64_darwin_mixed_unit_call_keys,
    aarch64_mixed_aggregate_call_keys, aarch64_mixed_unit_call_keys,
};
pub use operation_keys::{
    AARCH64_ACCESSOR_READ, AARCH64_ACCESSOR_WRITE, AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE,
    AARCH64_ADDRESS_OFFSET, AARCH64_ATOMIC_COMPARE_EXCHANGE, AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE,
    AARCH64_ATOMIC_FENCE, AARCH64_ATOMIC_LOAD, AARCH64_ATOMIC_STORE, AARCH64_ATOMIC_UPDATE,
    AARCH64_BITS_TO_FLOAT32, AARCH64_BITS_TO_FLOAT64, AARCH64_BYTE_LITERAL_IMAGE_ADDRESS,
    AARCH64_COMPARE_BYTE_SEQUENCES, AARCH64_COMPARE_I64, AARCH64_COMPARE_I64_IMMEDIATE,
    AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH, AARCH64_COPY_BYTES, AARCH64_COPY_I64,
    AARCH64_CRASH, AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32, AARCH64_DARWIN_HOSTED_READ_BYTE,
    AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, AARCH64_DIVIDE_I64, AARCH64_DIVIDE_U64,
    AARCH64_FLOAT_RESULT_SATURATING, AARCH64_FLOAT_RESULT_TRAPPING, AARCH64_FLOAT32_TO_BITS,
    AARCH64_FLOAT64_TO_BITS, AARCH64_FRAME_ADDRESS, AARCH64_HOSTED_EXIT_PROCESS_I32,
    AARCH64_HOSTED_READ_BYTE, AARCH64_HOSTED_WRITE_BYTE_I32, AARCH64_IEEE_FLOAT_ARITHMETIC,
    AARCH64_IEEE_FLOAT_TERNARY, AARCH64_IEEE_FLOAT_UNARY, AARCH64_JUMP,
    AARCH64_LOAD_ENCODED_SCALAR, AARCH64_LOAD_PACKED, AARCH64_LOAD8, AARCH64_LOAD8_INDEXED,
    AARCH64_LOAD16, AARCH64_LOAD32, AARCH64_LOAD64, AARCH64_MATERIALIZE_BOOLEAN,
    AARCH64_MATERIALIZE_I64, AARCH64_MEMORY_FENCE, AARCH64_MULTIPLY_I64, AARCH64_REMAINDER_I64,
    AARCH64_REMAINDER_U64, AARCH64_RESTORE_FLOATING_CONTROL, AARCH64_SATURATING_ADD_CLAMPED,
    AARCH64_SATURATING_ADD_U64, AARCH64_SATURATING_DIVIDE_SIGNED,
    AARCH64_SATURATING_MULTIPLY_CLAMPED, AARCH64_SATURATING_MULTIPLY_U64,
    AARCH64_SATURATING_SUBTRACT_CLAMPED, AARCH64_SATURATING_SUBTRACT_UNSIGNED,
    AARCH64_SAVE_FLOATING_CONTROL, AARCH64_SCAN_BYTE_SEQUENCE, AARCH64_SHIFT_I64, AARCH64_STORE,
    AARCH64_STORE_ENCODED_SCALAR, AARCH64_STORE_PACKED, AARCH64_STORE64, AARCH64_SUBTRACT_I64,
    AARCH64_SUBTRACT_I64_IMMEDIATE, AARCH64_SYSTEM_REGISTER_READ, AARCH64_SYSTEM_REGISTER_WRITE,
    AARCH64_TRAPPING_BINARY, AARCH64_TRAPPING_CONVERT, AARCH64_TRAPPING_FIXED_PAIR,
    AARCH64_TRAPPING_SHIFT, AARCH64_ZERO_BYTES,
};
pub(crate) use operation_keys::{
    AARCH64_INLINE_ASSEMBLY_DEFAULT, AARCH64_LINUX_SYSTEM_CALL,
    AARCH64_REQUIRED_REGISTER_CONSTRAINTS,
};
pub use physical_model::{
    aarch64_fixed_register_view, aarch64_physical_register_model,
    validated_aarch64_physical_register_model,
};
pub(crate) use physical_model::{
    aarch64_physical_register_model_ref, canonical_aarch64_physical_register_model_identity,
};
