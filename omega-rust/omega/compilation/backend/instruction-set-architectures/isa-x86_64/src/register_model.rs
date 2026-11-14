/*
We define the canonical x86-64 physical register model
(`x86_64_physical_register_model`), operation and call constraint keys, and the
validated register-constraint catalog (`x86_64_register_constraint_catalog`)
across System V AMD64, Microsoft x64, Linux system calls, scalar/aggregate
calls, packed memory operations, and privileged machine instructions.
*/

pub(super) fn fixed_operand(
    operand: u16,
    access: RegisterOperandAccess,
    view: &RegisterView,
) -> RegisterOperandConstraint {
    RegisterOperandConstraint {
        operand,
        access,
        class: view.class,
        fixed_view: Some(view.id),
        tied_to: None,
        early_clobber: false,
    }
}

pub(super) fn allocatable_operand(
    operand: u16,
    access: RegisterOperandAccess,
    class: RegisterClassId,
) -> RegisterOperandConstraint {
    RegisterOperandConstraint {
        operand,
        access,
        class,
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    }
}

pub(super) fn operand_lane(count: u16, inputs: u16) -> Vec<RegisterOperandConstraint> {
    (0..count)
        .map(|operand| {
            let written = operand >= inputs;
            let row = allocatable_operand(
                operand,
                if written {
                    RegisterOperandAccess::Def
                } else {
                    RegisterOperandAccess::Use
                },
                physical_model::GPR64,
            );
            if written { early_clobber(row) } else { row }
        })
        .collect()
}

pub(super) fn early_clobber(operand: RegisterOperandConstraint) -> RegisterOperandConstraint {
    RegisterOperandConstraint {
        early_clobber: true,
        ..operand
    }
}

pub(crate) fn view_of(name: &str) -> &'static RegisterView {
    physical_model::x86_64_physical_register_model_ref()
        .view_named(name)
        .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
}

pub(crate) fn units(name: &str) -> Vec<RegisterUnitId> {
    view_of(name).units.clone()
}

pub(crate) fn units_of(names: &[&str]) -> Vec<RegisterUnitId> {
    sorted_units(
        names
            .iter()
            .flat_map(|name| view_of(name).units.iter().copied()),
    )
}

use register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterOperandConstraint, RegisterUnitId,
    RegisterView, sorted_units,
};

mod call_keys {
    use register_model::{
        PreservationConvention, RegisterConstraintKey as Key, ValidatedPhysicalRegisterModel,
    };
    use target::{Architecture, NativeTarget, ObjectFormat};

    pub const X86_64_MICROSOFT_CALL_UNIT: Key = Key::call(700);

    pub fn x86_64_preservation_convention_for_target(
        model: &ValidatedPhysicalRegisterModel,
        target: NativeTarget,
    ) -> Option<&PreservationConvention> {
        if target.architecture != Architecture::X86_64 {
            return None;
        }
        let name = match target.object_format {
            ObjectFormat::Elf | ObjectFormat::MachO => "system-v-amd64",
            ObjectFormat::Coff => "microsoft-x64",
        };
        model.model().convention_named(name)
    }

    pub const X86_64_SYSTEM_V_CALL: Key = Key::call(0);

    pub const X86_64_MICROSOFT_CALL: Key = Key::call(1);

    pub fn x86_64_microsoft_register_call_keys() -> Vec<Key> {
        (10..=14).map(Key::call).collect()
    }

    pub fn x86_64_system_v_register_unit_call_keys() -> Vec<Key> {
        (0..=6).map(|arity| Key::call(740 + arity)).collect()
    }

    pub fn x86_64_microsoft_register_unit_call_keys() -> Vec<Key> {
        (0..=4)
            .map(|arity| Key::call(if arity == 2 { 700 } else { 720 + arity }))
            .collect()
    }

    pub fn x86_64_system_v_register_call_keys() -> Vec<Key> {
        [4, 5, 3, 6, 7, 8, 9].into_iter().map(Key::call).collect()
    }

    pub fn x86_64_system_v_aggregate_call_keys() -> Vec<Key> {
        (1000..1014).map(Key::call).collect()
    }

    pub fn x86_64_system_v_aggregate_return_keys() -> Vec<Key> {
        (10..12).map(Key::ret).collect()
    }

    pub fn x86_64_microsoft_aggregate_call_keys() -> Vec<Key> {
        (1020..1025).map(Key::call).collect()
    }

    pub fn x86_64_microsoft_aggregate_return_keys() -> Vec<Key> {
        vec![Key::ret(12)]
    }

    pub fn x86_64_system_v_normalized_foreign_call_keys() -> Vec<Key> {
        vec![Key::call(3000)]
    }

    pub fn x86_64_microsoft_normalized_foreign_call_keys() -> Vec<Key> {
        vec![Key::call(3300)]
    }

    pub(crate) const X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64: Key = Key::call(3);

    pub const X86_64_SYSTEM_V_RETURN: Key = Key::ret(0);

    pub const X86_64_MICROSOFT_RETURN: Key = Key::ret(1);

    pub const X86_64_SYSTEM_V_RETURN_UNIT: Key = Key::ret(2);

    pub const X86_64_MICROSOFT_RETURN_UNIT: Key = Key::ret(3);
}
mod constraint_catalog {
    use crate::register_model::physical_model::{GPR64, VECTOR128};
    use crate::register_model::{X86_64_ACCESSOR_READ, X86_64_ACCESSOR_WRITE};
    use crate::register_model::{
        X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_ADDRESS_OFFSET,
        X86_64_ATOMIC_BITWISE_UPDATE, X86_64_ATOMIC_COMPARE_EXCHANGE, X86_64_ATOMIC_LOAD,
        X86_64_ATOMIC_STORE, X86_64_ATOMIC_UPDATE, X86_64_BITS_TO_FLOAT32, X86_64_BITS_TO_FLOAT64,
        X86_64_BYTE_LITERAL_IMAGE_ADDRESS, X86_64_COMPARE_BYTE_SEQUENCES, X86_64_COMPARE_I64,
        X86_64_COMPARE_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
        X86_64_COPY_BYTES, X86_64_COPY_I64, X86_64_DIVIDE_I64, X86_64_DIVIDE_U64,
        X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS, X86_64_FRAME_ADDRESS,
        X86_64_HOSTED_EXIT_PROCESS_I32, X86_64_HOSTED_READ_BYTE, X86_64_HOSTED_WRITE_BYTE_I32,
        X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_JUMP, X86_64_LINUX_SYSTEM_CALL,
        X86_64_LOAD_ENCODED_SCALAR, X86_64_LOAD8, X86_64_LOAD8_INDEXED, X86_64_LOAD16,
        X86_64_LOAD32, X86_64_LOAD64, X86_64_MATERIALIZE_BOOLEAN, X86_64_MATERIALIZE_I64,
        X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT,
        X86_64_MULTIPLY_I64, X86_64_REMAINDER_I64, X86_64_REMAINDER_U64,
        X86_64_SATURATING_ADD_CLAMPED, X86_64_SATURATING_ADD_U64, X86_64_SATURATING_DIVIDE_SIGNED,
        X86_64_SATURATING_MULTIPLY_CLAMPED, X86_64_SATURATING_MULTIPLY_U64,
        X86_64_SATURATING_SUBTRACT_CLAMPED, X86_64_SATURATING_SUBTRACT_UNSIGNED,
        X86_64_SCAN_BYTE_SEQUENCE, X86_64_SHIFT_I64, X86_64_STORE, X86_64_STORE_ENCODED_SCALAR,
        X86_64_STORE64, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_CALL,
        X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64_TRAPPING_BINARY, X86_64_TRAPPING_CONVERT, X86_64_TRAPPING_FIXED_PAIR,
        X86_64_TRAPPING_SHIFT, X86_64_ZERO_BYTES,
        canonical_x86_64_physical_register_model_identity,
        validated_x86_64_physical_register_model, x86_64_microsoft_aggregate_call_keys,
        x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_register_call_keys,
        x86_64_microsoft_register_unit_call_keys, x86_64_required_register_constraints,
        x86_64_system_v_aggregate_call_keys, x86_64_system_v_aggregate_return_keys,
        x86_64_system_v_register_call_keys, x86_64_system_v_register_unit_call_keys,
    };
    use crate::register_model::{
        float_scalar_calls, indirect_results, mixed_aggregate_calls, mixed_calls, packed_memory,
    };
    use register_model::complement;
    use register_model::{
        PreparedRegisterConstraintCatalog, PreservationConvention, RegisterClassId,
        RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintId,
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess as Access,
        RegisterOperandConstraint, RegisterUnitId, ValidatedPhysicalRegisterModel, sorted_units,
        validate_register_constraint_catalog,
    };
    use target::Architecture;

    pub fn x86_64_register_constraint_catalog(
        model: &ValidatedPhysicalRegisterModel,
    ) -> RegisterConstraintCatalog {
        let physical = model.model();
        assert_eq!(physical.architecture, Architecture::X86_64);

        let view = |name: &str| {
            physical
                .view_named(name)
                .unwrap_or_else(|| panic!("validated x86-64 model must define {name}"))
        };
        let merged_units = |base: Vec<RegisterUnitId>, names: &[&str]| {
            sorted_units(
                base.into_iter().chain(
                    names
                        .iter()
                        .flat_map(|name| view(name).units.iter().copied()),
                ),
            )
        };
        let fixed = |operand: u16, access: Access, name: &str| {
            super::fixed_operand(operand, access, view(name))
        };
        let allocatable = |operand: u16, access: Access, class: RegisterClassId| {
            super::allocatable_operand(operand, access, class)
        };
        let convention = |name: &str| {
            physical
                .convention_named(name)
                .unwrap_or_else(|| panic!("validated x86-64 model must define {name}"))
        };

        let rsp_units = view("rsp").units.clone();
        let rip_units = view("rip").units.clone();
        let rax_units = view("rax").units.clone();
        let control_defs = sorted_units(rsp_units.iter().copied().chain(rip_units.iter().copied()));
        let call_clobbers = |convention: &PreservationConvention| {
            convention
                .caller_saved
                .iter()
                .copied()
                .filter(|unit| !rax_units.contains(unit) && !control_defs.contains(unit))
                .collect::<Vec<_>>()
        };

        let sysv = convention("system-v-amd64");
        let microsoft = convention("microsoft-x64");
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
        let condition_clobbering_row = |id, key, operands| RegisterInstructionConstraint {
            clobbers: view("condition_flags").units.clone(),
            ..instruction_row(id, key, operands)
        };
        let condition_defining_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_defs: view("condition_flags").units.clone(),
            ..instruction_row(id, key, operands)
        };
        let return_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_uses: rsp_units.clone(),
            implicit_defs: control_defs.clone(),
            ..instruction_row(id, key, operands)
        };
        let jump_row = |id, key, operands| RegisterInstructionConstraint {
            implicit_uses: rip_units.clone(),
            implicit_defs: rip_units.clone(),
            ..instruction_row(id, key, operands)
        };
        let all_units = physical
            .units
            .iter()
            .map(|unit| unit.id)
            .collect::<Vec<_>>();
        let fixed_machine_state =
            sorted_units(rsp_units.iter().copied().chain(rip_units.iter().copied()));
        let syscall_clobbers = sorted_units(
            view("rcx")
                .units
                .iter()
                .copied()
                .chain(view("r11").units.iter().copied())
                .chain(view("rflags").units.iter().copied()),
        );

        let mut port_write = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_PORT_WRITE,
            Vec::new(),
        );
        port_write.clobbers = sorted_units(
            ["rax", "rdx", "r10", "r11"]
                .into_iter()
                .flat_map(|name| view(name).units.iter().copied()),
        );
        let register_access_rows = [
            (crate::X86_64_PORT_REGISTER_READ, true, false),
            (crate::X86_64_PORT_REGISTER_WRITE, false, false),
            (crate::X86_64_MODEL_SPECIFIC_READ, true, true),
            (crate::X86_64_MODEL_SPECIFIC_WRITE, false, true),
        ]
        .map(|(key, read, model_specific)| {
            let mut row = instruction_row(
                RegisterConstraintId(0),
                key,
                vec![
                    allocatable(0, Access::Use, view("rax").class),
                    allocatable(
                        1,
                        if read { Access::Def } else { Access::Use },
                        view("rax").class,
                    ),
                ],
            );
            let mut scratch = vec!["rax", "rdx", "r10"];
            if !read || model_specific {
                scratch.push("r11");
            }
            if model_specific {
                scratch.push("rcx");
            }
            row.clobbers = sorted_units(
                scratch
                    .into_iter()
                    .flat_map(|name| view(name).units.iter().copied()),
            );
            row
        });
        let mut flags_snapshot = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_FLAGS_SNAPSHOT,
            vec![allocatable(0, Access::Def, GPR64)],
        );
        flags_snapshot.implicit_uses = sorted_units(
            ["rflags", "rsp"]
                .into_iter()
                .flat_map(|name| view(name).units.iter().copied()),
        );
        flags_snapshot.clobbers = sorted_units(
            ["r10", "r15"]
                .into_iter()
                .flat_map(|name| view(name).units.iter().copied()),
        );
        let mut flags_restore = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_FLAGS_RESTORE,
            vec![allocatable(0, Access::Use, GPR64)],
        );
        flags_restore.implicit_uses = sorted_units(view("rsp").units.iter().copied());
        flags_restore.implicit_defs = sorted_units(view("rflags").units.iter().copied());
        flags_restore.clobbers = flags_snapshot.clobbers.clone();
        let mut global_descriptor_table = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_GLOBAL_DESCRIPTOR_TABLE,
            vec![allocatable(0, Access::Use, GPR64)],
        );
        global_descriptor_table.clobbers = sorted_units(view("r10").units.iter().copied());
        let mut task_register_activate = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_TASK_REGISTER_ACTIVATE,
            vec![
                fixed(0, Access::Use, "rdi"),
                fixed(1, Access::Use, "rsi"),
                fixed(2, Access::Use, "rdx"),
                fixed(3, Access::Use, "rcx"),
                fixed(4, Access::Use, "r8"),
                fixed(5, Access::Def, "rax"),
            ],
        );
        task_register_activate.implicit_uses = sorted_units(
            ["rsp", "control_flags"]
                .into_iter()
                .flat_map(|name| view(name).units.iter().copied()),
        );
        task_register_activate.clobbers = sorted_units(
            ["r9", "r10", "r11", "condition_flags"]
                .into_iter()
                .flat_map(|name| view(name).units.iter().copied()),
        );
        let installed_row = |key, operands, scratch: &[&str], flags_stack: bool| {
            let mut row = instruction_row(RegisterConstraintId(0), key, operands);
            if flags_stack {
                row.implicit_uses = sorted_units(
                    ["rsp", "control_flags"]
                        .into_iter()
                        .flat_map(|name| view(name).units.iter().copied()),
                );
            }
            row.clobbers = sorted_units(
                scratch
                    .iter()
                    .flat_map(|name| view(name).units.iter().copied()),
            );
            row
        };
        let installed_interrupt_cpu_check = installed_row(
            crate::X86_64_INSTALLED_INTERRUPT_CPU_CHECK,
            vec![
                fixed(0, Access::Use, "rdi"),
                fixed(1, Access::Use, "rsi"),
                fixed(2, Access::Use, "rcx"),
                fixed(3, Access::Def, "rax"),
            ],
            &["r9", "r10", "r11", "condition_flags"],
            true,
        );
        let installed_interrupt_task_commit = installed_row(
            crate::X86_64_INSTALLED_INTERRUPT_TASK_COMMIT,
            vec![
                fixed(0, Access::Use, "rdi"),
                fixed(1, Access::Use, "rsi"),
                fixed(2, Access::Use, "rcx"),
            ],
            &[
                "rax",
                "rdx",
                "r8",
                "r9",
                "r10",
                "r11",
                "condition_flags",
                "control_flags",
            ],
            true,
        );
        let installed_interrupt_table_publication = installed_row(
            crate::X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION,
            vec![fixed(0, Access::Use, "rdi"), fixed(1, Access::Use, "rcx")],
            &["rax"],
            false,
        );
        let task_register_read = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_TASK_REGISTER_READ,
            vec![allocatable(0, Access::Def, GPR64)],
        );
        let segment_reload = |key, scratch: &[&str]| {
            let mut row = instruction_row(
                RegisterConstraintId(0),
                key,
                vec![
                    fixed(0, Access::Use, "rdi"),
                    fixed(1, Access::Use, "rdx"),
                    fixed(2, Access::Use, "rcx"),
                    fixed(3, Access::Use, "r8"),
                    fixed(4, Access::Def, "rax"),
                ],
            );
            row.implicit_uses = sorted_units(
                ["rsp", "control_flags"]
                    .into_iter()
                    .flat_map(|name| view(name).units.iter().copied()),
            );
            row.clobbers = sorted_units(
                scratch
                    .iter()
                    .flat_map(|name| view(name).units.iter().copied()),
            );
            row
        };
        let code_segment_reload = segment_reload(
            crate::X86_64_CODE_SEGMENT_RELOAD,
            &["r9", "r10", "condition_flags"],
        );
        let data_segment_reload = segment_reload(
            crate::X86_64_DATA_SEGMENT_RELOAD,
            &["rcx", "rdx", "r8", "r9", "r10", "r11", "condition_flags"],
        );
        let segment_selector_read = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_SEGMENT_SELECTOR_READ,
            vec![allocatable(0, Access::Def, GPR64)],
        );
        let control_rows = [
            (crate::X86_64_CONTROL_REGISTER_READ, true),
            (crate::X86_64_CONTROL_REGISTER_WRITE, false),
        ]
        .map(|(key, read)| {
            let mut row = instruction_row(
                RegisterConstraintId(0),
                key,
                vec![allocatable(
                    0,
                    if read { Access::Def } else { Access::Use },
                    GPR64,
                )],
            );
            let scratch = if read {
                &["r10", "r15"][..]
            } else {
                &["rax", "r10", "r11", "r15"][..]
            };
            row.clobbers = sorted_units(
                scratch
                    .iter()
                    .flat_map(|name| view(name).units.iter().copied()),
            );
            row
        });
        let mut interrupt_control = instruction_row(
            RegisterConstraintId(0),
            crate::X86_64_INTERRUPT_CONTROL,
            Vec::new(),
        );
        interrupt_control.implicit_uses = sorted_units(view("control_flags").units.iter().copied());
        interrupt_control.implicit_defs = interrupt_control.implicit_uses.clone();
        let mut constraints = vec![
            interrupt_control,
            instruction_row(
                RegisterConstraintId(0),
                crate::X86_64_MEMORY_FENCE,
                Vec::new(),
            ),
            flags_snapshot,
            flags_restore,
            global_descriptor_table,
            task_register_activate,
            task_register_read,
            installed_interrupt_cpu_check,
            installed_interrupt_task_commit,
            installed_interrupt_table_publication,
            code_segment_reload,
            data_segment_reload,
            segment_selector_read,
            control_rows[0].clone(),
            control_rows[1].clone(),
            port_write,
            instruction_row(
                RegisterConstraintId(0),
                crate::X86_64_PROCESSOR_HALT,
                Vec::new(),
            ),
            instruction_row(
                RegisterConstraintId(0),
                crate::X86_64_ATOMIC_FENCE,
                Vec::new(),
            ),
            jump_row(RegisterConstraintId(0), crate::X86_64_CRASH, Vec::new()),
            RegisterInstructionConstraint {
                clobbers: crate::register_model::units_of(&["control_flags"]),
                ..jump_row(
                    RegisterConstraintId(0),
                    crate::X86_64_PRIVILEGED_CRASH,
                    Vec::new(),
                )
            },
            RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key: X86_64_SYSTEM_V_CALL,
                operands: ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                    .into_iter()
                    .enumerate()
                    .map(|(operand, name)| {
                        fixed(
                            u16::try_from(operand).expect("operand index fits u16"),
                            Access::Use,
                            name,
                        )
                    })
                    .chain([fixed(6, Access::Def, "rax")])
                    .collect(),
                implicit_uses: rsp_units.clone(),
                implicit_defs: control_defs.clone(),
                clobbers: call_clobbers(sysv),
            },
            RegisterInstructionConstraint {
                id: RegisterConstraintId(1),
                key: X86_64_MICROSOFT_CALL,
                operands: ["rcx", "rdx", "r8", "r9"]
                    .into_iter()
                    .enumerate()
                    .map(|(operand, name)| {
                        fixed(
                            u16::try_from(operand).expect("operand index fits u16"),
                            Access::Use,
                            name,
                        )
                    })
                    .chain([fixed(4, Access::Def, "rax")])
                    .collect(),
                implicit_uses: rsp_units.clone(),
                implicit_defs: control_defs.clone(),
                clobbers: call_clobbers(microsoft),
            },
            RegisterInstructionConstraint {
                id: RegisterConstraintId(18),
                key: X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
                operands: vec![
                    fixed(0, Access::Use, "rdi"),
                    fixed(1, Access::Use, "rsi"),
                    fixed(2, Access::Def, "rax"),
                ],
                implicit_uses: sorted_units(
                    rsp_units.iter().copied().chain(rip_units.iter().copied()),
                ),
                implicit_defs: control_defs.clone(),
                clobbers: call_clobbers(sysv),
            },
            return_row(
                RegisterConstraintId(2),
                X86_64_SYSTEM_V_RETURN,
                vec![fixed(0, Access::Use, "rax")],
            ),
            return_row(
                RegisterConstraintId(3),
                X86_64_MICROSOFT_RETURN,
                vec![fixed(0, Access::Use, "rax")],
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(4),
                key: X86_64_LINUX_SYSTEM_CALL,
                operands: ["rax", "rdi", "rsi", "rdx", "r10", "r8", "r9"]
                    .into_iter()
                    .enumerate()
                    .map(|(operand, name)| {
                        fixed(
                            u16::try_from(operand).expect("operand index fits u16"),
                            if operand == 0 {
                                Access::UseDef
                            } else {
                                Access::Use
                            },
                            name,
                        )
                    })
                    .collect(),
                implicit_uses: rip_units.clone(),
                implicit_defs: rip_units.clone(),
                clobbers: syscall_clobbers,
            },
            RegisterInstructionConstraint {
                id: RegisterConstraintId(5),
                key: X86_64_INLINE_ASSEMBLY_DEFAULT,
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: complement(&all_units, &fixed_machine_state, &[]),
            },
            instruction_row(
                RegisterConstraintId(6),
                X86_64_MATERIALIZE_I64,
                vec![allocatable(0, Access::Def, GPR64)],
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(6),
                key: X86_64_MATERIALIZE_BOOLEAN,
                operands: vec![allocatable(0, Access::Def, GPR64)],
                implicit_uses: view("condition_flags").units.clone(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
            },
            instruction_row(
                RegisterConstraintId(7),
                X86_64_COPY_I64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Def, GPR64),
                ],
            ),
            condition_defining_row(
                RegisterConstraintId(8),
                X86_64_COMPARE_I64_ZERO,
                vec![allocatable(0, Access::Use, GPR64)],
            ),
            RegisterInstructionConstraint {
                id: RegisterConstraintId(9),
                key: X86_64_CONDITIONAL_BRANCH,
                operands: Vec::new(),
                implicit_uses: sorted_units(
                    view("condition_flags")
                        .units
                        .iter()
                        .copied()
                        .chain(view("rip").units.iter().copied()),
                ),
                implicit_defs: view("rip").units.clone(),
                clobbers: Vec::new(),
            },
            instruction_row(
                RegisterConstraintId(10),
                X86_64_ADD_I64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                    allocatable(2, Access::Def, GPR64),
                ],
            ),
            instruction_row(
                RegisterConstraintId(11),
                X86_64_ADD_I64_IMMEDIATE,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Def, GPR64),
                ],
            ),
            condition_clobbering_row(
                RegisterConstraintId(12),
                X86_64_SUBTRACT_I64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                    allocatable(2, Access::Def, GPR64),
                ],
            ),
            instruction_row(
                RegisterConstraintId(13),
                X86_64_SUBTRACT_I64_IMMEDIATE,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Def, GPR64),
                ],
            ),
            condition_clobbering_row(
                RegisterConstraintId(20),
                X86_64_MULTIPLY_I64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                    allocatable(2, Access::Def, GPR64),
                ],
            ),
            condition_defining_row(
                RegisterConstraintId(17),
                X86_64_COMPARE_I64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                ],
            ),
            condition_defining_row(
                RegisterConstraintId(19),
                X86_64_COMPARE_I64_IMMEDIATE,
                vec![allocatable(0, Access::Use, GPR64)],
            ),
            return_row(
                RegisterConstraintId(14),
                X86_64_SYSTEM_V_RETURN_UNIT,
                Vec::new(),
            ),
            return_row(
                RegisterConstraintId(15),
                X86_64_MICROSOFT_RETURN_UNIT,
                Vec::new(),
            ),
            jump_row(RegisterConstraintId(0), X86_64_JUMP, Vec::new()),
        ];

        let saturation_output = super::early_clobber(allocatable(2, Access::Def, GPR64));
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_SATURATING_SUBTRACT_UNSIGNED,
            vec![
                allocatable(0, Access::Use, GPR64),
                allocatable(1, Access::Use, GPR64),
                saturation_output,
            ],
        ));
        macro_rules! fixed_rax_pair {
            ($key:expr) => {
                constraints.push(condition_clobbering_row(
                    RegisterConstraintId(0),
                    $key,
                    vec![
                        fixed(0, Access::Use, "rax"),
                        fixed(1, Access::Use, "rcx"),
                        fixed(2, Access::Def, "rax"),
                        fixed(3, Access::Def, "rdx"),
                    ],
                ));
            };
        }
        fixed_rax_pair!(X86_64_REMAINDER_I64);
        fixed_rax_pair!(X86_64_DIVIDE_I64);
        fixed_rax_pair!(X86_64_REMAINDER_U64);
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_SHIFT_I64,
            vec![
                allocatable(0, Access::Use, GPR64),
                fixed(1, Access::Use, "rcx"),
                super::early_clobber(allocatable(2, Access::Def, GPR64)),
            ],
        ));
        for key in [X86_64_DIVIDE_U64, X86_64_SATURATING_DIVIDE_SIGNED] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    fixed(0, Access::Use, "rax"),
                    allocatable(1, Access::Use, GPR64),
                    fixed(2, Access::Def, "rax"),
                    fixed(3, Access::Use, "rdx"),
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: ["rdx", "condition_flags"]
                    .into_iter()
                    .flat_map(|name| view(name).units.iter().copied())
                    .collect(),
            });
        }
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_SATURATING_ADD_U64,
            vec![
                allocatable(0, Access::Use, GPR64),
                allocatable(1, Access::Use, GPR64),
                super::early_clobber(allocatable(2, Access::Def, GPR64)),
            ],
        ));
        for key in [
            X86_64_SATURATING_ADD_CLAMPED,
            X86_64_SATURATING_SUBTRACT_CLAMPED,
            X86_64_SATURATING_MULTIPLY_CLAMPED,
            X86_64_TRAPPING_BINARY,
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                    super::early_clobber(allocatable(2, Access::Def, GPR64)),
                    super::early_clobber(allocatable(3, Access::Def, GPR64)),
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: view("condition_flags").units.clone(),
            });
        }
        fixed_rax_pair!(X86_64_SATURATING_MULTIPLY_U64);
        fixed_rax_pair!(X86_64_TRAPPING_FIXED_PAIR);
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_TRAPPING_SHIFT,
            vec![
                allocatable(0, Access::Use, GPR64),
                fixed(1, Access::Use, "rcx"),
                super::early_clobber(allocatable(2, Access::Def, GPR64)),
                super::early_clobber(allocatable(3, Access::Def, GPR64)),
            ],
        ));
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_TRAPPING_CONVERT,
            vec![
                allocatable(0, Access::Use, GPR64),
                super::early_clobber(allocatable(1, Access::Def, GPR64)),
                super::early_clobber(allocatable(2, Access::Def, GPR64)),
            ],
        ));
        let scalar_call = constraints
            .iter()
            .find(|row| row.key == X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64)
            .expect("pair call row")
            .clone();
        constraints.extend(register_access_rows);
        for (arity, key) in x86_64_system_v_register_call_keys().into_iter().enumerate() {
            if key == X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64 {
                continue;
            }
            let mut call = scalar_call.clone();
            call.key = key;
            call.operands = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                .into_iter()
                .take(arity)
                .enumerate()
                .map(|(index, name)| fixed(index as u16, Access::Use, name))
                .chain([fixed(arity as u16, Access::Def, "rax")])
                .collect();
            constraints.push(call);
        }

        for (ordinal, key) in x86_64_system_v_aggregate_call_keys()
            .into_iter()
            .enumerate()
        {
            let arity = ordinal % 7;
            let fragments = ordinal / 7 + 1;
            let mut call = scalar_call.clone();
            call.key = key;
            call.operands =
                ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                    .into_iter()
                    .take(arity)
                    .enumerate()
                    .map(|(position, name)| fixed(position as u16, Access::Use, name))
                    .chain(["rax", "rdx"].into_iter().take(fragments).enumerate().map(
                        |(position, name)| fixed((arity + position) as u16, Access::Def, name),
                    ))
                    .collect();
            for name in ["rax", "rdx"].into_iter().take(fragments) {
                call.clobbers
                    .retain(|unit| !view(name).write_units.contains(unit));
            }
            constraints.push(call);
        }
        let returned = constraints
            .iter()
            .find(|row| row.key == X86_64_SYSTEM_V_RETURN)
            .expect("canonical return row")
            .clone();
        for (ordinal, key) in x86_64_system_v_aggregate_return_keys()
            .into_iter()
            .enumerate()
        {
            let mut row = returned.clone();
            row.key = key;
            row.operands = ["rax", "rdx"]
                .into_iter()
                .take(ordinal + 1)
                .enumerate()
                .map(|(position, name)| fixed(position as u16, Access::Use, name))
                .collect();
            constraints.push(row);
        }
        let abi_call = constraints
            .iter()
            .find(|row| row.key == X86_64_MICROSOFT_CALL)
            .expect("canonical ABI call row")
            .clone();
        for ((arity, key), aggregate_key) in x86_64_microsoft_register_call_keys()
            .into_iter()
            .enumerate()
            .zip(x86_64_microsoft_aggregate_call_keys())
        {
            let mut call = abi_call.clone();
            call.key = key;
            call.implicit_uses = sorted_units(
                view("rsp")
                    .units
                    .iter()
                    .copied()
                    .chain(view("rip").units.iter().copied()),
            );
            call.operands = ["rcx", "rdx", "r8", "r9"]
                .into_iter()
                .take(arity)
                .enumerate()
                .map(|(index, name)| fixed(index as u16, Access::Use, name))
                .chain([fixed(arity as u16, Access::Def, "rax")])
                .collect();
            constraints.push(call.clone());
            call.key = aggregate_key;
            call.clobbers
                .retain(|unit| !view("rax").write_units.contains(unit));
            constraints.push(call);
        }
        let mut returned = returned;
        returned.key = x86_64_microsoft_aggregate_return_keys()[0];
        constraints.push(returned);

        for (key, operands, uses) in [
            (
                X86_64_STORE,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                ],
                Vec::new(),
            ),
            (
                X86_64_ADDRESS_OFFSET,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Def, GPR64),
                ],
                Vec::new(),
            ),
            (
                X86_64_LOAD8_INDEXED,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Use, GPR64),
                    allocatable(2, Access::Def, GPR64),
                ],
                Vec::new(),
            ),
            (
                X86_64_LOAD64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    allocatable(1, Access::Def, GPR64),
                ],
                Vec::new(),
            ),
            (
                X86_64_STORE64,
                vec![
                    allocatable(0, Access::Use, GPR64),
                    super::early_clobber(allocatable(1, Access::Def, GPR64)),
                ],
                view("rsp").units.clone(),
            ),
            (
                X86_64_FRAME_ADDRESS,
                vec![allocatable(0, Access::Def, GPR64)],
                view("rsp").units.clone(),
            ),
            (
                X86_64_BYTE_LITERAL_IMAGE_ADDRESS,
                vec![allocatable(0, Access::Def, GPR64)],
                Vec::new(),
            ),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: uses,
                implicit_defs: Vec::new(),
                clobbers: if matches!(key, X86_64_STORE64 | X86_64_FRAME_ADDRESS) {
                    view("condition_flags").units.clone()
                } else {
                    Vec::new()
                },
            });
        }
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: X86_64_HOSTED_WRITE_BYTE_I32,
            operands: vec![allocatable(0, Access::Use, GPR64)],
            implicit_uses: merged_units(view("rsp").units.clone(), &["rip"]),
            implicit_defs: Vec::new(),
            clobbers: merged_units(
                Vec::new(),
                &["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"],
            ),
        });
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: X86_64_HOSTED_READ_BYTE,
            operands: Vec::new(),
            implicit_uses: merged_units(view("rsp").units.clone(), &["rip"]),
            implicit_defs: Vec::new(),
            clobbers: merged_units(
                Vec::new(),
                &["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"],
            ),
        });
        {
            let key = X86_64_HOSTED_EXIT_PROCESS_I32;
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![allocatable(0, Access::Use, GPR64)],
                implicit_uses: view("rip").units.clone(),
                implicit_defs: Vec::new(),
                clobbers: merged_units(Vec::new(), &["rax", "rdi", "rcx", "r11", "rflags"]),
            });
        }
        for (scalar_key, unit_key) in x86_64_system_v_register_call_keys()
            .into_iter()
            .zip(x86_64_system_v_register_unit_call_keys())
            .chain(
                x86_64_microsoft_register_call_keys()
                    .into_iter()
                    .zip(x86_64_microsoft_register_unit_call_keys()),
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
                    .chain(view("rax").units.iter().copied()),
            );
            constraints.push(call);
        }
        for (key, source_class, destination_class) in [
            (X86_64_LOAD8, GPR64, GPR64),
            (X86_64_LOAD16, GPR64, GPR64),
            (X86_64_LOAD32, GPR64, GPR64),
            (X86_64_FLOAT32_TO_BITS, VECTOR128, GPR64),
            (X86_64_FLOAT64_TO_BITS, VECTOR128, GPR64),
            (X86_64_BITS_TO_FLOAT32, GPR64, VECTOR128),
            (X86_64_BITS_TO_FLOAT64, GPR64, VECTOR128),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: vec![
                    allocatable(0, Access::Use, source_class),
                    allocatable(1, Access::Def, destination_class),
                ],
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
            });
        }
        for (key, inputs) in [
            (crate::X86_64_IEEE_FLOAT_ARITHMETIC, 2),
            (crate::X86_64_IEEE_FLOAT_DIRECTED_BINARY, 2),
            (crate::X86_64_IEEE_FLOAT_DIRECTED_UNARY, 1),
            (crate::X86_64_IEEE_FLOAT_UNARY, 1),
            (crate::X86_64_IEEE_FLOAT_TERNARY, 3),
        ] {
            let mut operands = (0..inputs)
                .map(|index| allocatable(index, Access::Use, GPR64))
                .collect::<Vec<_>>();
            operands.push(allocatable(inputs, Access::Def, GPR64));
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: sorted_units(
                    view("mxcsr").units.iter().copied().chain(
                        (key == crate::X86_64_IEEE_FLOAT_DIRECTED_BINARY
                            || key == crate::X86_64_IEEE_FLOAT_DIRECTED_UNARY)
                            .then(|| view("rsp").units.iter().copied())
                            .into_iter()
                            .flatten(),
                    ),
                ),
                implicit_defs: Vec::new(),
                clobbers: sorted_units(
                    view("xmm4")
                        .units
                        .iter()
                        .chain(&view("xmm5").units)
                        .chain(
                            (key == crate::X86_64_IEEE_FLOAT_TERNARY)
                                .then(|| view("xmm3").units.iter())
                                .into_iter()
                                .flatten(),
                        )
                        .copied(),
                ),
            });
        }
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: crate::X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
            operands: vec![
                allocatable(0, Access::Use, GPR64),
                allocatable(1, Access::Def, GPR64),
            ],
            implicit_uses: sorted_units(view("mxcsr").units.iter().copied()),
            implicit_defs: Vec::new(),
            clobbers: sorted_units(
                view("xmm4")
                    .units
                    .iter()
                    .chain(&view("xmm5").units)
                    .chain(&view("condition_flags").units)
                    .copied(),
            ),
        });
        for (key, inputs) in [
            (crate::X86_64_FLOAT_RESULT_TRAPPING, 1),
            (crate::X86_64_FLOAT_RESULT_SATURATING, 2),
        ] {
            let scratch: &[&str] = if inputs == 1 {
                &["xmm3", "condition_flags"]
            } else {
                &["xmm3", "xmm4", "condition_flags"]
            };
            let mut operands = (0..inputs)
                .map(|position| allocatable(position, Access::Use, GPR64))
                .collect::<Vec<_>>();
            operands.push(allocatable(inputs, Access::Def, GPR64));
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
        mixed_calls::append_constraints(&mut constraints, model);
        mixed_calls::append_normalized_foreign_constraints(&mut constraints, model);
        super::system_calls::append_constraints(&mut constraints, model);
        float_scalar_calls::append_constraints(&mut constraints, model);
        indirect_results::append_constraints(&mut constraints, model);
        packed_memory::append_constraints(&mut constraints, model);
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_COPY_BYTES,
            super::operand_lane(5, 3),
        ));
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_ZERO_BYTES,
            super::operand_lane(3, 2),
        ));
        for (key, input_count) in [
            (X86_64_LOAD_ENCODED_SCALAR, 2),
            (X86_64_STORE_ENCODED_SCALAR, 3),
        ] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: super::operand_lane(6, input_count),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: view("condition_flags").units.clone(),
            });
        }
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_SCAN_BYTE_SEQUENCE,
            super::operand_lane(5, 2),
        ));
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_COMPARE_BYTE_SEQUENCES,
            super::operand_lane(8, 4),
        ));
        mixed_aggregate_calls::append_constraints(&mut constraints, model);
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            X86_64_ATOMIC_LOAD,
            vec![
                allocatable(0, Access::Use, GPR64),
                super::early_clobber(allocatable(1, Access::Def, GPR64)),
            ],
        ));
        constraints.push(instruction_row(
            RegisterConstraintId(0),
            X86_64_ATOMIC_STORE,
            vec![
                allocatable(0, Access::Use, GPR64),
                allocatable(1, Access::Use, GPR64),
            ],
        ));
        constraints.push(condition_clobbering_row(
            RegisterConstraintId(0),
            X86_64_ATOMIC_UPDATE,
            vec![
                allocatable(0, Access::Use, GPR64),
                allocatable(1, Access::Use, GPR64),
                super::early_clobber(allocatable(2, Access::Def, GPR64)),
            ],
        ));
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: X86_64_ATOMIC_BITWISE_UPDATE,
            operands: vec![
                fixed(0, Access::Use, "rdx"),
                fixed(1, Access::Use, "rcx"),
                fixed(2, Access::Def, "r8"),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: {
                let mut clobbers = view("rax").units.clone();
                clobbers.extend(view("condition_flags").units.iter().copied());
                clobbers.sort_unstable();
                clobbers
            },
        });
        for (key, outputs) in [
            (X86_64_ATOMIC_COMPARE_EXCHANGE, 1),
            (crate::X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE, 2),
        ] {
            let mut operands = vec![
                allocatable(0, Access::Use, GPR64),
                fixed(1, Access::Use, "rax"),
                allocatable(2, Access::Use, GPR64),
            ];
            operands.extend(
                (3..3 + outputs)
                    .map(|operand| super::early_clobber(allocatable(operand, Access::Def, GPR64))),
            );
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: {
                    let mut clobbers = view("rax").units.clone();
                    clobbers.extend(view("condition_flags").units.iter().copied());
                    clobbers.sort_unstable();
                    clobbers
                },
            });
        }
        for (key, restore) in [
            (crate::X86_64_SAVE_FLOATING_CONTROL, false),
            (crate::X86_64_RESTORE_FLOATING_CONTROL, true),
        ] {
            let mut implicit_uses = view("rsp").units.clone();
            if !restore {
                implicit_uses.extend(view("mxcsr").units.iter().copied());
            }
            implicit_uses.sort_unstable();
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: Vec::new(),
                implicit_uses,
                implicit_defs: if restore {
                    view("mxcsr").units.clone()
                } else {
                    Vec::new()
                },
                clobbers: Vec::new(),
            });
        }
        for (key, template) in [
            (X86_64_ACCESSOR_READ, X86_64_LOAD32),
            (X86_64_ACCESSOR_WRITE, X86_64_STORE),
        ] {
            let mut row = constraints
                .iter()
                .find(|row| row.key == template)
                .expect("canonical scalar transfer")
                .clone();
            row.key = key;
            constraints.push(row);
        }
        constraints.sort_by_key(|constraint| constraint.key);
        for (id, constraint) in constraints.iter_mut().enumerate() {
            constraint.id =
                RegisterConstraintId(u16::try_from(id).expect("constraint roster fits u16"));
        }
        RegisterConstraintCatalog {
            architecture: Architecture::X86_64,
            required: x86_64_required_register_constraints(),
            constraints,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum X86_64RegisterConstraintCatalogValidationError {
        PhysicalModelArchitectureMismatch,
        NonCanonicalPhysicalModel,
        Structural(RegisterConstraintCatalogValidationError),
        TargetSemanticMismatch(RegisterConstraintKey),
    }

    optimization_core::invalid_error_display!(
        X86_64RegisterConstraintCatalogValidationError,
        "x86-64 register constraint catalog"
    );

    fn check_x86_64_catalog_model(
        model: &ValidatedPhysicalRegisterModel,
    ) -> Result<(), X86_64RegisterConstraintCatalogValidationError> {
        if model.model().architecture != Architecture::X86_64 {
            return Err(
                X86_64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch,
            );
        }
        if model.identity() != canonical_x86_64_physical_register_model_identity() {
            return Err(X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel);
        }
        Ok(())
    }

    pub fn prepare_x86_64_register_constraint_catalog(
        model: &ValidatedPhysicalRegisterModel,
    ) -> Result<PreparedRegisterConstraintCatalog, X86_64RegisterConstraintCatalogValidationError>
    {
        check_x86_64_catalog_model(model)?;
        Ok(canonical_x86_64_register_constraint_catalog().clone())
    }

    fn canonical_x86_64_register_constraint_catalog() -> &'static PreparedRegisterConstraintCatalog
    {
        static CANONICAL: std::sync::OnceLock<PreparedRegisterConstraintCatalog> =
            std::sync::OnceLock::new();
        CANONICAL.get_or_init(|| {
            register_model::produce_register_constraint_catalog(
                validated_x86_64_physical_register_model(),
                x86_64_register_constraint_catalog,
            )
        })
    }

    pub fn validate_x86_64_register_constraint_catalog(
        catalog: RegisterConstraintCatalog,
        model: &ValidatedPhysicalRegisterModel,
    ) -> Result<PreparedRegisterConstraintCatalog, X86_64RegisterConstraintCatalogValidationError>
    {
        check_x86_64_catalog_model(model)?;
        let validated = validate_register_constraint_catalog(catalog, model)
            .map_err(X86_64RegisterConstraintCatalogValidationError::Structural)?;
        let canonical = x86_64_register_constraint_catalog_for(model);
        let actual_rows = &validated.catalog().constraints;
        for key in canonical.required.iter().copied() {
            let Ok(actual_position) = actual_rows.binary_search_by_key(&key, |row| row.key) else {
                return Err(
                    X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(key),
                );
            };
            let expected_position = canonical
                .constraints
                .binary_search_by_key(&key, |row| row.key)
                .expect("target-owned inventory and rows are closed together");
            if actual_rows[actual_position] != canonical.constraints[expected_position] {
                return Err(
                    X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(key),
                );
            }
        }
        if let Some(unexpected) = validated
            .catalog()
            .constraints
            .iter()
            .find(|constraint| canonical.required.binary_search(&constraint.key).is_err())
        {
            return Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    unexpected.key,
                ),
            );
        }
        Ok(validated)
    }

    pub(crate) fn x86_64_register_constraint_catalog_for(
        model: &ValidatedPhysicalRegisterModel,
    ) -> std::borrow::Cow<'static, RegisterConstraintCatalog> {
        if model.identity() == canonical_x86_64_physical_register_model_identity() {
            std::borrow::Cow::Borrowed(canonical_x86_64_register_constraint_catalog().catalog())
        } else {
            std::borrow::Cow::Owned(x86_64_register_constraint_catalog(model))
        }
    }
}
mod float_scalar_calls {
    use super::{
        X86_64_MICROSOFT_RETURN, X86_64_SYSTEM_V_RETURN, x86_64_microsoft_mixed_unit_call_keys,
        x86_64_microsoft_register_unit_call_keys, x86_64_system_v_mixed_unit_call_keys,
        x86_64_system_v_register_unit_call_keys,
    };
    use register_model::{
        RegisterConstraintKey as Key, RegisterInstructionConstraint, RegisterOperandAccess,
        ValidatedPhysicalRegisterModel, find_register_constraint,
    };
    pub fn x86_64_float_scalar_call_keys(microsoft: bool) -> Vec<Key> {
        let count = input_keys(microsoft).len()
            + (if microsoft {
                x86_64_microsoft_mixed_unit_call_keys()
            } else {
                x86_64_system_v_mixed_unit_call_keys()
            })
            .len();
        (0..count)
            .map(|ordinal| Key::call((if microsoft { 1800 } else { 1600 }) + ordinal as u32))
            .collect()
    }

    pub fn x86_64_float_scalar_return_keys(microsoft: bool) -> Vec<Key> {
        vec![Key::ret(if microsoft { 1601 } else { 1600 })]
    }

    fn input_keys(microsoft: bool) -> Vec<Key> {
        (if microsoft {
            x86_64_microsoft_register_unit_call_keys()
        } else {
            x86_64_system_v_register_unit_call_keys()
        })
        .into_iter()
        .chain(if microsoft {
            x86_64_microsoft_mixed_unit_call_keys()
        } else {
            x86_64_system_v_mixed_unit_call_keys()
        })
        .collect()
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        let Some(result) = model.model().view_named("xmm0") else {
            return;
        };
        for microsoft in [false, true] {
            let inputs = input_keys(microsoft)
                .into_iter()
                .map(|key| (key, "xmm0"))
                .chain(
                    (if microsoft {
                        x86_64_microsoft_mixed_unit_call_keys()
                    } else {
                        x86_64_system_v_mixed_unit_call_keys()
                    })
                    .into_iter()
                    .map(|key| (key, "rax")),
                );
            for ((input, result_name), key) in inputs.zip(x86_64_float_scalar_call_keys(microsoft))
            {
                let Some(result) = model.model().view_named(result_name) else {
                    return;
                };
                let Some(mut row) = find_register_constraint(constraints, input).cloned() else {
                    return;
                };
                row.key = key;
                row.operands.push(super::fixed_operand(
                    row.operands.len() as u16,
                    RegisterOperandAccess::Def,
                    result,
                ));
                row.clobbers
                    .retain(|unit| !result.write_units.contains(unit));
                constraints.push(row);
            }
            let base = if microsoft {
                X86_64_MICROSOFT_RETURN
            } else {
                X86_64_SYSTEM_V_RETURN
            };
            let Some(mut row) = find_register_constraint(constraints, base).cloned() else {
                return;
            };
            row.key = x86_64_float_scalar_return_keys(microsoft)[0];
            row.operands = vec![super::fixed_operand(0, RegisterOperandAccess::Use, result)];
            constraints.push(row);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{
            validate_x86_64_register_constraint_catalog, x86_64_physical_register_model,
            x86_64_register_constraint_catalog,
        };
        use super::{
            x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys,
            x86_64_system_v_register_unit_call_keys,
        };

        #[test]
        fn floating_return_uses_ordinary_return_bytes_and_exact_home() {
            let physical =
                register_model::validate_physical_register_model(x86_64_physical_register_model())
                    .unwrap();
            let home = physical.model().view_named("xmm0").unwrap().id;
            let wrong = physical.model().view_named("xmm1").unwrap().id;
            let kind = selected_instructions::SelectedInstructionKind::ReturnScalar;
            let alternative = selected_instructions::MachineAlternativeKey {
                family: selected_instructions::MachineAlternativeFamily::ReturnScalar,
                variant: 0,
            };
            let encoded = crate::encode_x86_64_selected_form(&physical, kind, alternative, &[home])
                .expect("floating return");
            crate::validate_x86_64_selected_form_encoding(
                &physical,
                kind,
                alternative,
                &[home],
                encoded.bytes(),
            )
            .unwrap();
            assert!(
                crate::validate_x86_64_selected_form_encoding(
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
                register_model::validate_physical_register_model(x86_64_physical_register_model())
                    .expect("physical model");
            let catalog = x86_64_register_constraint_catalog(&physical);
            validate_x86_64_register_constraint_catalog(catalog.clone(), &physical)
                .expect("complete catalog");
            let mut keys = Vec::new();
            for microsoft in [false, true] {
                keys.extend(x86_64_float_scalar_call_keys(microsoft));
                keys.extend(x86_64_float_scalar_return_keys(microsoft));
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
            let result = physical.model().view_named("xmm0").unwrap();
            let call_key = x86_64_float_scalar_call_keys(false)[0];
            let row = catalog.constraint(call_key).unwrap();
            assert_eq!(row.operands.last().unwrap().fixed_view, Some(result.id));
            assert!(
                result
                    .write_units
                    .iter()
                    .all(|unit| !row.clobbers.contains(unit))
            );
            let input = catalog
                .constraint(x86_64_system_v_register_unit_call_keys()[0])
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
                Some(physical.model().view_named("xmm1").unwrap().id);
            assert!(validate_x86_64_register_constraint_catalog(wrong_home, &physical).is_err());
            let mut wrong_clobbers = catalog;
            register_model::find_register_constraint_mut(&mut wrong_clobbers.constraints, call_key)
                .unwrap()
                .clobbers
                .clear();
            assert!(
                validate_x86_64_register_constraint_catalog(wrong_clobbers, &physical).is_err()
            );
        }
    }
}
#[cfg(test)]
mod float_transport_tests {
    use super::{
        X86_64_FLOAT32_TO_BITS, validate_x86_64_register_constraint_catalog,
        x86_64_physical_register_model, x86_64_register_constraint_catalog,
        x86_64_system_v_mixed_unit_call_keys,
    };
    use crate::register_model::physical_model::GPR64;
    use register_model::validate_physical_register_model;
    #[test]
    fn transport_and_mixed_call_constraints_reject_forged_classes_and_clobbers() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&physical);
        let mut wrong_transport = catalog.clone();
        let transport = register_model::find_register_constraint_mut(
            &mut wrong_transport.constraints,
            X86_64_FLOAT32_TO_BITS,
        )
        .unwrap();
        transport.operands[0].class = GPR64;
        assert!(validate_x86_64_register_constraint_catalog(wrong_transport, &physical).is_err());
        let mut wrong_call = catalog;
        let call_key = x86_64_system_v_mixed_unit_call_keys()[0];
        let call =
            register_model::find_register_constraint_mut(&mut wrong_call.constraints, call_key)
                .unwrap();
        assert!(call.clobbers.pop().is_some());
        assert!(validate_x86_64_register_constraint_catalog(wrong_call, &physical).is_err());
    }
}
mod indirect_results {
    use super::{
        x86_64_microsoft_mixed_unit_call_keys, x86_64_microsoft_register_unit_call_keys,
        x86_64_system_v_mixed_unit_call_keys, x86_64_system_v_register_unit_call_keys,
    };
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, ValidatedPhysicalRegisterModel,
        find_register_constraint,
    };
    pub fn x86_64_indirect_aggregate_call_keys(microsoft: bool) -> Vec<RegisterConstraintKey> {
        let first = if microsoft { 2300 } else { 2200 };
        let count = if microsoft { 15 } else { 54 };
        (first..first + count)
            .map(RegisterConstraintKey::call)
            .collect()
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        for microsoft in [false, true] {
            let Some(pointer) = model
                .model()
                .view_named(if microsoft { "rcx" } else { "rdi" })
            else {
                return;
            };
            let inputs = if microsoft {
                x86_64_microsoft_register_unit_call_keys()
            } else {
                x86_64_system_v_register_unit_call_keys()
            };
            let mixed = if microsoft {
                x86_64_microsoft_mixed_unit_call_keys()
            } else {
                x86_64_system_v_mixed_unit_call_keys()
            };
            let mut keys = x86_64_indirect_aggregate_call_keys(microsoft).into_iter();
            for input in inputs.into_iter().chain(mixed) {
                let Some(mut row) = find_register_constraint(constraints, input).cloned() else {
                    return;
                };
                let Some(position) = row
                    .operands
                    .iter()
                    .position(|operand| operand.fixed_view == Some(pointer.id))
                else {
                    continue;
                };
                let Some(key) = keys.next() else {
                    return;
                };
                row.key = key;
                let hidden = row.operands.remove(position);
                row.operands.push(hidden);
                for (ordinal, operand) in row.operands.iter_mut().enumerate() {
                    operand.operand = ordinal as u16;
                }
                constraints.push(row);
            }
        }
    }
}
mod mixed_aggregate_calls {
    use super::{x86_64_microsoft_mixed_unit_call_keys, x86_64_system_v_mixed_unit_call_keys};
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
        ValidatedPhysicalRegisterModel,
    };
    pub fn x86_64_system_v_mixed_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
        keys(1100..1212)
    }

    pub fn x86_64_microsoft_mixed_aggregate_call_keys() -> Vec<RegisterConstraintKey> {
        keys(1220..1246)
    }

    fn keys(variants: std::ops::Range<u32>) -> Vec<RegisterConstraintKey> {
        variants.map(RegisterConstraintKey::call).collect()
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        for (input_keys, output_keys, fragment_count) in [
            (
                x86_64_system_v_mixed_unit_call_keys(),
                x86_64_system_v_mixed_aggregate_call_keys(),
                2,
            ),
            (
                x86_64_microsoft_mixed_unit_call_keys(),
                x86_64_microsoft_mixed_aggregate_call_keys(),
                1,
            ),
        ] {
            let mut output_keys = output_keys.into_iter();
            for fragments in 1..=fragment_count {
                for input_key in &input_keys {
                    let Some(mut row) = constraints
                        .iter()
                        .find(|row| row.key == *input_key)
                        .cloned()
                    else {
                        return;
                    };
                    let Some(key) = output_keys.next() else {
                        return;
                    };
                    row.key = key;
                    for name in ["rax", "rdx"].into_iter().take(fragments) {
                        let Some(view) = model.model().view_named(name) else {
                            return;
                        };
                        row.operands.push(super::fixed_operand(
                            row.operands.len() as u16,
                            RegisterOperandAccess::Def,
                            view,
                        ));
                        row.clobbers.retain(|unit| !view.write_units.contains(unit));
                    }
                    constraints.push(row);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{
            validate_x86_64_register_constraint_catalog, x86_64_microsoft_register_unit_call_keys,
            x86_64_physical_register_model, x86_64_register_constraint_catalog,
            x86_64_system_v_register_unit_call_keys,
        };
        use super::{
            RegisterOperandAccess, x86_64_microsoft_mixed_aggregate_call_keys,
            x86_64_system_v_mixed_aggregate_call_keys,
        };
        use register_model::validate_physical_register_model;

        #[test]
        fn mixed_aggregate_rows_preserve_banks_results_and_other_call_effects() {
            let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
            let catalog = x86_64_register_constraint_catalog(&model);
            validate_x86_64_register_constraint_catalog(catalog.clone(), &model).unwrap();
            let mut expected_rows = Vec::new();
            for fragments in 1..=2 {
                for general_count in 0..=6 {
                    for float_count in 1..=8 {
                        let names = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                            .into_iter()
                            .take(general_count)
                            .map(str::to_owned)
                            .chain((0..float_count).map(|position| format!("xmm{position}")))
                            .collect::<Vec<_>>();
                        expected_rows.push((
                            names,
                            fragments,
                            x86_64_system_v_register_unit_call_keys()[0],
                        ));
                    }
                }
            }
            for arity in 1..=4 {
                for float_mask in 1..(1_u16 << arity) {
                    let names = ["rcx", "rdx", "r8", "r9"]
                        .into_iter()
                        .take(arity)
                        .enumerate()
                        .filter(|(position, _)| float_mask & (1 << position) == 0)
                        .map(|(_, name)| name.to_owned())
                        .chain(
                            (0..arity)
                                .filter(|position| float_mask & (1 << position) != 0)
                                .map(|position| format!("xmm{position}")),
                        )
                        .collect::<Vec<_>>();
                    expected_rows.push((names, 1, x86_64_microsoft_register_unit_call_keys()[0]));
                }
            }
            let keys = x86_64_system_v_mixed_aggregate_call_keys()
                .into_iter()
                .chain(x86_64_microsoft_mixed_aggregate_call_keys())
                .collect::<Vec<_>>();
            assert_eq!(keys.len(), expected_rows.len());
            for (key, (inputs, fragments, base_key)) in keys.into_iter().zip(expected_rows) {
                let row = catalog.constraint(key).unwrap();
                let base = catalog.constraint(base_key).unwrap();
                let names = inputs
                    .iter()
                    .map(String::as_str)
                    .chain(["rax", "rdx"].into_iter().take(fragments))
                    .collect::<Vec<_>>();
                assert_eq!(row.operands.len(), names.len());
                for (position, (operand, name)) in row.operands.iter().zip(names).enumerate() {
                    let view = model.model().view_named(name).unwrap();
                    assert_eq!(operand.operand, position as u16);
                    assert_eq!(operand.fixed_view, Some(view.id));
                    assert_eq!(operand.class, view.class);
                    assert_eq!(
                        operand.access,
                        if position < inputs.len() {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        }
                    );
                    assert_eq!(operand.tied_to, None);
                    assert!(!operand.early_clobber);
                }
                let result_units = ["rax", "rdx"]
                    .into_iter()
                    .take(fragments)
                    .flat_map(|name| &model.model().view_named(name).unwrap().write_units)
                    .collect::<Vec<_>>();
                assert_eq!(
                    row.clobbers,
                    base.clobbers
                        .iter()
                        .copied()
                        .filter(|unit| !result_units.contains(&unit))
                        .collect::<Vec<_>>()
                );
                assert_eq!(row.implicit_uses, base.implicit_uses);
                assert_eq!(row.implicit_defs, base.implicit_defs);

                if !matches!(key.variant, 1100 | 1155 | 1156 | 1211 | 1220 | 1245) {
                    continue;
                }
                for mutation in 0..3 {
                    let mut changed = catalog.clone();
                    let row =
                        register_model::find_register_constraint_mut(&mut changed.constraints, key)
                            .unwrap();
                    match mutation {
                        0 => {
                            row.clobbers.push(*result_units[0]);
                            row.clobbers.sort_unstable();
                        }
                        1 => row.operands.last_mut().unwrap().access = RegisterOperandAccess::Use,
                        _ => {
                            row.operands[inputs.len() - 1].fixed_view =
                                Some(model.model().view_named("r11").unwrap().id)
                        }
                    }
                    assert!(
                        validate_x86_64_register_constraint_catalog(changed, &model).is_err(),
                        "{key:?} mutation {mutation}"
                    );
                }
            }
        }
    }
}
mod mixed_calls {
    use super::{
        x86_64_microsoft_register_unit_call_keys, x86_64_system_v_register_unit_call_keys,
    };
    use register_model::call_constraint_keys;
    use register_model::{
        RegisterConstraintKey, RegisterInstructionConstraint, RegisterOperandAccess,
        ValidatedPhysicalRegisterModel, find_register_constraint,
    };
    pub fn x86_64_system_v_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
        call_constraint_keys(800..856)
    }

    pub fn x86_64_microsoft_mixed_unit_call_keys() -> Vec<RegisterConstraintKey> {
        call_constraint_keys(900..926)
    }

    fn append_row(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
        base: &RegisterInstructionConstraint,
        key: RegisterConstraintKey,
        names: impl Iterator<Item = String>,
    ) {
        let mut row = base.clone();
        row.key = key;
        let Some(operands) = names
            .enumerate()
            .map(|(operand, name)| {
                let view = model.model().view_named(&name)?;
                Some(super::fixed_operand(
                    operand as u16,
                    RegisterOperandAccess::Use,
                    view,
                ))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        row.operands = operands;
        constraints.push(row);
    }

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        let Some(system_v) = constraints
            .iter()
            .find(|row| row.key == x86_64_system_v_register_unit_call_keys()[0])
            .cloned()
        else {
            return;
        };
        let mut keys = x86_64_system_v_mixed_unit_call_keys().into_iter();
        for general_count in 0..=6 {
            for float_count in 1..=8 {
                let Some(key) = keys.next() else { return };
                append_row(
                    constraints,
                    model,
                    &system_v,
                    key,
                    ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                        .into_iter()
                        .take(general_count)
                        .map(str::to_owned)
                        .chain((0..float_count).map(|position| format!("xmm{position}"))),
                );
            }
        }
        let Some(microsoft) = constraints
            .iter()
            .find(|row| row.key == x86_64_microsoft_register_unit_call_keys()[0])
            .cloned()
        else {
            return;
        };
        let mut keys = x86_64_microsoft_mixed_unit_call_keys().into_iter();
        for arity in 1..=4 {
            for float_mask in 1..(1_u16 << arity) {
                let Some(key) = keys.next() else { return };
                append_row(
                    constraints,
                    model,
                    &microsoft,
                    key,
                    ["rcx", "rdx", "r8", "r9"]
                        .into_iter()
                        .take(arity)
                        .enumerate()
                        .filter(|(position, _)| float_mask & (1 << position) == 0)
                        .map(|(_, name)| name.to_owned())
                        .chain(
                            (0..arity)
                                .filter(|position| float_mask & (1 << position) != 0)
                                .map(|position| format!("xmm{position}")),
                        ),
                );
            }
        }
    }

    pub(super) fn append_normalized_foreign_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        _model: &ValidatedPhysicalRegisterModel,
    ) {
        for (base_key, keys) in [
            (
                x86_64_system_v_register_unit_call_keys()[0],
                super::x86_64_system_v_normalized_foreign_call_keys(),
            ),
            (
                x86_64_microsoft_register_unit_call_keys()[0],
                super::x86_64_microsoft_normalized_foreign_call_keys(),
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
}
mod operation_keys {
    use super::float_scalar_calls::{
        x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys,
    };
    use super::indirect_results::x86_64_indirect_aggregate_call_keys;
    use super::packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
    use crate::register_model::{
        X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT,
        X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    };
    use register_model::RegisterConstraintKey as Key;

    pub const X86_64_CRASH: Key = Key::instruction(780);
    pub const X86_64_PRIVILEGED_CRASH: Key = Key::instruction(915);

    pub const X86_64_FLOAT_RESULT_TRAPPING: Key = Key::instruction(789);
    pub const X86_64_FLOAT_RESULT_SATURATING: Key = Key::instruction(790);

    pub const X86_64_LOAD8: Key = Key::instruction(730);

    pub const X86_64_LOAD16: Key = Key::instruction(731);

    pub const X86_64_LOAD32: Key = Key::instruction(714);

    pub const X86_64_FLOAT32_TO_BITS: Key = Key::instruction(710);

    pub const X86_64_FLOAT64_TO_BITS: Key = Key::instruction(711);

    pub const X86_64_BITS_TO_FLOAT32: Key = Key::instruction(712);

    pub const X86_64_BITS_TO_FLOAT64: Key = Key::instruction(713);

    pub const X86_64_IEEE_FLOAT_DIRECTED_BINARY: Key = Key::instruction(792);

    pub const X86_64_IEEE_FLOAT_DIRECTED_UNARY: Key = Key::instruction(793);

    pub const X86_64_IEEE_FLOAT_ARITHMETIC: Key = Key::instruction(785);

    pub const X86_64_IEEE_FLOAT_UNARY: Key = Key::instruction(786);

    pub const X86_64_IEEE_FLOAT_CONVERSION_CONTROL: Key = Key::instruction(788);

    pub const X86_64_IEEE_FLOAT_TERNARY: Key = Key::instruction(787);

    pub const X86_64_HOSTED_EXIT_PROCESS_I32: Key = Key::instruction(715);

    pub const X86_64_HOSTED_READ_BYTE: Key = Key::instruction(717);

    pub const X86_64_HOSTED_WRITE_BYTE_I32: Key = Key::instruction(704);

    pub const X86_64_LOAD64: Key = Key::instruction(700);

    pub const X86_64_LOAD8_INDEXED: Key = Key::instruction(703);

    pub const X86_64_SAVE_FLOATING_CONTROL: Key = Key::instruction(753);
    pub const X86_64_RESTORE_FLOATING_CONTROL: Key = Key::instruction(754);

    pub const X86_64_COPY_BYTES: Key = Key::instruction(736);

    pub const X86_64_LOAD_ENCODED_SCALAR: Key = Key::instruction(758);
    pub const X86_64_STORE_ENCODED_SCALAR: Key = Key::instruction(759);

    pub const X86_64_SCAN_BYTE_SEQUENCE: Key = Key::instruction(755);

    pub const X86_64_COMPARE_BYTE_SEQUENCES: Key = Key::instruction(756);

    pub const X86_64_ZERO_BYTES: Key = Key::instruction(757);

    pub const X86_64_STORE: Key = Key::instruction(705);

    pub const X86_64_ADDRESS_OFFSET: Key = Key::instruction(706);

    pub const X86_64_STORE64: Key = Key::instruction(701);

    pub const X86_64_FRAME_ADDRESS: Key = Key::instruction(702);

    pub const X86_64_BYTE_LITERAL_IMAGE_ADDRESS: Key = Key::instruction(791);

    pub(crate) const X86_64_LINUX_SYSTEM_CALL: Key = Key::system_call(0);

    pub(crate) const X86_64_INLINE_ASSEMBLY_DEFAULT: Key = Key::inline_assembly(0);

    pub const X86_64_MATERIALIZE_BOOLEAN: Key = Key::instruction(732);

    pub const X86_64_MATERIALIZE_I64: Key = Key::instruction(0);

    pub const X86_64_COPY_I64: Key = Key::instruction(1);

    pub const X86_64_COMPARE_I64_ZERO: Key = Key::instruction(2);

    pub const X86_64_CONDITIONAL_BRANCH: Key = Key::instruction(3);

    pub const X86_64_ADD_I64: Key = Key::instruction(4);

    pub const X86_64_ADD_I64_IMMEDIATE: Key = Key::instruction(5);

    pub const X86_64_SATURATING_SUBTRACT_UNSIGNED: Key = Key::instruction(54);

    pub const X86_64_SATURATING_ADD_U64: Key = Key::instruction(55);

    pub const X86_64_DIVIDE_U64: Key = Key::instruction(56);

    pub const X86_64_REMAINDER_I64: Key = Key::instruction(57);

    pub const X86_64_SATURATING_ADD_CLAMPED: Key = Key::instruction(58);

    pub const X86_64_SATURATING_SUBTRACT_CLAMPED: Key = Key::instruction(59);

    pub const X86_64_SATURATING_DIVIDE_SIGNED: Key = Key::instruction(60);

    pub const X86_64_SATURATING_MULTIPLY_CLAMPED: Key = Key::instruction(65);

    pub const X86_64_SATURATING_MULTIPLY_U64: Key = Key::instruction(66);

    pub const X86_64_TRAPPING_BINARY: Key = Key::instruction(67);

    pub const X86_64_TRAPPING_FIXED_PAIR: Key = Key::instruction(68);

    pub const X86_64_TRAPPING_SHIFT: Key = Key::instruction(69);

    pub const X86_64_TRAPPING_CONVERT: Key = Key::instruction(70);

    pub const X86_64_PORT_REGISTER_READ: Key = Key::instruction(803);

    pub const X86_64_PORT_REGISTER_WRITE: Key = Key::instruction(804);

    pub const X86_64_MODEL_SPECIFIC_READ: Key = Key::instruction(805);

    pub const X86_64_MODEL_SPECIFIC_WRITE: Key = Key::instruction(806);

    pub const X86_64_PROCESSOR_HALT: Key = Key::instruction(807);

    pub const X86_64_INTERRUPT_CONTROL: Key = Key::instruction(808);

    pub const X86_64_PORT_WRITE: Key = Key::instruction(802);

    pub const X86_64_MEMORY_FENCE: Key = Key::instruction(809);

    pub const X86_64_ATOMIC_FENCE: Key = Key::instruction(801);
    pub const X86_64_ATOMIC_LOAD: Key = Key::instruction(71);

    pub const X86_64_ATOMIC_STORE: Key = Key::instruction(72);

    pub const X86_64_ATOMIC_UPDATE: Key = Key::instruction(73);

    pub const X86_64_ATOMIC_BITWISE_UPDATE: Key = Key::instruction(747);

    pub const X86_64_ATOMIC_COMPARE_EXCHANGE: Key = Key::instruction(74);

    pub const X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE: Key = Key::instruction(810);

    pub const X86_64_MULTIPLY_I64: Key = Key::instruction(61);

    pub const X86_64_DIVIDE_I64: Key = Key::instruction(62);

    pub const X86_64_REMAINDER_U64: Key = Key::instruction(63);

    pub const X86_64_SHIFT_I64: Key = Key::instruction(64);

    pub const X86_64_SUBTRACT_I64: Key = Key::instruction(6);

    pub const X86_64_SUBTRACT_I64_IMMEDIATE: Key = Key::instruction(7);

    pub const X86_64_COMPARE_I64: Key = Key::instruction(8);

    pub const X86_64_COMPARE_I64_IMMEDIATE: Key = Key::instruction(10);

    pub const X86_64_JUMP: Key = Key::instruction(9);

    pub(crate) const X86_64_ORDINARY_REGISTER_CONSTRAINTS: [Key; 102] = [
        X86_64_SYSTEM_V_CALL,
        X86_64_MICROSOFT_CALL,
        X86_64_SYSTEM_V_RETURN,
        X86_64_MICROSOFT_RETURN,
        X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64_MICROSOFT_RETURN_UNIT,
        X86_64_LINUX_SYSTEM_CALL,
        X86_64_INLINE_ASSEMBLY_DEFAULT,
        X86_64_MATERIALIZE_I64,
        X86_64_COPY_I64,
        X86_64_COMPARE_I64_ZERO,
        X86_64_CONDITIONAL_BRANCH,
        X86_64_ADD_I64,
        X86_64_ADD_I64_IMMEDIATE,
        X86_64_SUBTRACT_I64,
        X86_64_SUBTRACT_I64_IMMEDIATE,
        X86_64_COMPARE_I64,
        X86_64_JUMP,
        X86_64_COMPARE_I64_IMMEDIATE,
        X86_64_SATURATING_SUBTRACT_UNSIGNED,
        X86_64_SATURATING_ADD_U64,
        X86_64_DIVIDE_U64,
        X86_64_REMAINDER_I64,
        X86_64_SATURATING_ADD_CLAMPED,
        X86_64_SATURATING_SUBTRACT_CLAMPED,
        X86_64_SATURATING_DIVIDE_SIGNED,
        X86_64_MULTIPLY_I64,
        X86_64_DIVIDE_I64,
        X86_64_REMAINDER_U64,
        X86_64_SHIFT_I64,
        X86_64_SATURATING_MULTIPLY_CLAMPED,
        X86_64_SATURATING_MULTIPLY_U64,
        X86_64_TRAPPING_BINARY,
        X86_64_TRAPPING_FIXED_PAIR,
        X86_64_TRAPPING_SHIFT,
        X86_64_TRAPPING_CONVERT,
        X86_64_ATOMIC_LOAD,
        X86_64_ATOMIC_STORE,
        X86_64_ATOMIC_UPDATE,
        X86_64_ATOMIC_COMPARE_EXCHANGE,
        X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE,
        X86_64_LOAD64,
        X86_64_STORE64,
        X86_64_FRAME_ADDRESS,
        X86_64_LOAD8_INDEXED,
        X86_64_HOSTED_WRITE_BYTE_I32,
        X86_64_STORE,
        X86_64_ADDRESS_OFFSET,
        X86_64_FLOAT32_TO_BITS,
        X86_64_FLOAT64_TO_BITS,
        X86_64_BITS_TO_FLOAT32,
        X86_64_BITS_TO_FLOAT64,
        X86_64_LOAD32,
        X86_64_HOSTED_EXIT_PROCESS_I32,
        X86_64_HOSTED_READ_BYTE,
        X86_64_LOAD8,
        X86_64_LOAD16,
        X86_64_MATERIALIZE_BOOLEAN,
        X86_64_COPY_BYTES,
        X86_64_ATOMIC_BITWISE_UPDATE,
        X86_64_SAVE_FLOATING_CONTROL,
        X86_64_RESTORE_FLOATING_CONTROL,
        X86_64_SCAN_BYTE_SEQUENCE,
        X86_64_COMPARE_BYTE_SEQUENCES,
        X86_64_ZERO_BYTES,
        X86_64_LOAD_ENCODED_SCALAR,
        X86_64_STORE_ENCODED_SCALAR,
        X86_64_CRASH,
        X86_64_PRIVILEGED_CRASH,
        X86_64_IEEE_FLOAT_ARITHMETIC,
        X86_64_IEEE_FLOAT_UNARY,
        X86_64_IEEE_FLOAT_TERNARY,
        X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
        X86_64_FLOAT_RESULT_TRAPPING,
        X86_64_FLOAT_RESULT_SATURATING,
        X86_64_BYTE_LITERAL_IMAGE_ADDRESS,
        X86_64_IEEE_FLOAT_DIRECTED_BINARY,
        X86_64_IEEE_FLOAT_DIRECTED_UNARY,
        X86_64_MEMORY_FENCE,
        X86_64_ATOMIC_FENCE,
        X86_64_PORT_WRITE,
        X86_64_PORT_REGISTER_READ,
        X86_64_PORT_REGISTER_WRITE,
        X86_64_MODEL_SPECIFIC_READ,
        X86_64_MODEL_SPECIFIC_WRITE,
        X86_64_PROCESSOR_HALT,
        X86_64_INTERRUPT_CONTROL,
        X86_64_ACCESSOR_READ,
        X86_64_ACCESSOR_WRITE,
        X86_64_FLAGS_SNAPSHOT,
        X86_64_GLOBAL_DESCRIPTOR_TABLE,
        X86_64_TASK_REGISTER_ACTIVATE,
        X86_64_TASK_REGISTER_READ,
        X86_64_INSTALLED_INTERRUPT_CPU_CHECK,
        X86_64_INSTALLED_INTERRUPT_TASK_COMMIT,
        X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION,
        X86_64_CODE_SEGMENT_RELOAD,
        X86_64_DATA_SEGMENT_RELOAD,
        X86_64_SEGMENT_SELECTOR_READ,
        X86_64_CONTROL_REGISTER_READ,
        X86_64_CONTROL_REGISTER_WRITE,
        X86_64_FLAGS_RESTORE,
    ];

    pub(crate) fn x86_64_required_register_constraints() -> Vec<Key> {
        let mut required = X86_64_ORDINARY_REGISTER_CONSTRAINTS.to_vec();
        required.extend(super::call_keys::x86_64_system_v_register_call_keys());
        required.extend(super::call_keys::x86_64_microsoft_register_call_keys());
        required.extend(super::call_keys::x86_64_system_v_register_unit_call_keys());
        required.extend(super::call_keys::x86_64_microsoft_register_unit_call_keys());
        required.extend(super::call_keys::x86_64_system_v_aggregate_call_keys());
        required.extend(super::call_keys::x86_64_system_v_aggregate_return_keys());
        required.extend(super::call_keys::x86_64_microsoft_aggregate_call_keys());
        required.extend(super::call_keys::x86_64_microsoft_aggregate_return_keys());
        required.extend(super::call_keys::x86_64_system_v_normalized_foreign_call_keys());
        required.extend(super::call_keys::x86_64_microsoft_normalized_foreign_call_keys());
        required.extend(super::mixed_calls::x86_64_system_v_mixed_unit_call_keys());
        required.extend(super::mixed_calls::x86_64_microsoft_mixed_unit_call_keys());
        required.extend(super::mixed_aggregate_calls::x86_64_system_v_mixed_aggregate_call_keys());
        required.extend(super::mixed_aggregate_calls::x86_64_microsoft_mixed_aggregate_call_keys());
        for microsoft in [false, true] {
            required.extend(x86_64_float_scalar_call_keys(microsoft));
            required.extend(x86_64_float_scalar_return_keys(microsoft));
            required.extend(x86_64_indirect_aggregate_call_keys(microsoft));
        }
        required.extend(super::system_calls::x86_64_linux_system_call_keys());
        required.extend([X86_64_LOAD_PACKED, X86_64_STORE_PACKED]);
        required.sort_unstable();
        required
    }

    pub const X86_64_ACCESSOR_READ: Key = Key::instruction(900);

    pub const X86_64_ACCESSOR_WRITE: Key = Key::instruction(901);

    pub const X86_64_FLAGS_SNAPSHOT: Key = Key::instruction(902);

    pub const X86_64_FLAGS_RESTORE: Key = Key::instruction(905);

    pub const X86_64_CONTROL_REGISTER_READ: Key = Key::instruction(903);

    pub const X86_64_CONTROL_REGISTER_WRITE: Key = Key::instruction(904);

    pub const X86_64_GLOBAL_DESCRIPTOR_TABLE: Key = Key::instruction(906);

    pub const X86_64_TASK_REGISTER_ACTIVATE: Key = Key::instruction(907);
    pub const X86_64_TASK_REGISTER_READ: Key = Key::instruction(908);

    pub const X86_64_CODE_SEGMENT_RELOAD: Key = Key::instruction(909);
    pub const X86_64_DATA_SEGMENT_RELOAD: Key = Key::instruction(910);
    pub const X86_64_SEGMENT_SELECTOR_READ: Key = Key::instruction(911);

    pub const X86_64_INSTALLED_INTERRUPT_CPU_CHECK: Key = Key::instruction(912);
    pub const X86_64_INSTALLED_INTERRUPT_TASK_COMMIT: Key = Key::instruction(913);
    pub const X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION: Key = Key::instruction(914);
}
mod packed_memory {
    use register_model::{
        RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
        RegisterOperandAccess, ValidatedPhysicalRegisterModel,
    };
    pub const X86_64_LOAD_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(734);
    pub const X86_64_STORE_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(735);
    use crate::register_model::physical_model::GPR64;

    pub(super) fn append_constraints(
        constraints: &mut Vec<RegisterInstructionConstraint>,
        model: &ValidatedPhysicalRegisterModel,
    ) {
        let Some(flags) = model.model().view_named("condition_flags") else {
            return;
        };
        for (key, load) in [(X86_64_LOAD_PACKED, true), (X86_64_STORE_PACKED, false)] {
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands: (0..3)
                    .map(|operand| {
                        let writes = operand == 2 || (load && operand == 1);
                        let row = super::allocatable_operand(
                            operand,
                            if writes {
                                RegisterOperandAccess::Def
                            } else {
                                RegisterOperandAccess::Use
                            },
                            GPR64,
                        );
                        if writes {
                            super::early_clobber(row)
                        } else {
                            row
                        }
                    })
                    .collect(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: flags.units.clone(),
            });
        }
    }
}
mod physical_model {
    use calling_conventions::MachineRegister;
    use register_model::{
        PhysicalModelBuilder, PhysicalRegisterModel, PhysicalRegisterModelIdentity,
        PreservationConvention, RegisterClassId, RegisterUnitId, RegisterUnitKind, RegisterViewId,
        RegisterWriteSemantics, ReservationReason, ValidatedPhysicalRegisterModel, complement,
        reservation_overlay, sorted_units, units_for, validate_physical_register_model, view_ids,
    };
    use std::collections::BTreeMap;
    use target::Architecture;

    pub(crate) const GPR64: RegisterClassId = RegisterClassId(0);

    const GPR32: RegisterClassId = RegisterClassId(1);

    const GPR16: RegisterClassId = RegisterClassId(2);

    const GPR8_LOW: RegisterClassId = RegisterClassId(3);

    const GPR8_HIGH: RegisterClassId = RegisterClassId(4);

    pub(crate) const VECTOR128: RegisterClassId = RegisterClassId(5);

    const FLAGS: RegisterClassId = RegisterClassId(6);

    const INSTRUCTION_POINTER: RegisterClassId = RegisterClassId(7);
    const FLOAT_CONTROL: RegisterClassId = RegisterClassId(8);

    pub fn x86_64_fixed_register_view(
        model: &ValidatedPhysicalRegisterModel,
        register: MachineRegister,
    ) -> Option<RegisterViewId> {
        if model.identity() != canonical_x86_64_physical_register_model_identity() {
            return None;
        }
        let name = match register {
            MachineRegister::X86Xmm(index @ 0..=15) => {
                return model
                    .model()
                    .view_named(&format!("xmm{index}"))
                    .map(|view| view.id);
            }
            MachineRegister::X86Rax => "rax",
            MachineRegister::X86Rcx => "rcx",
            MachineRegister::X86Rdx => "rdx",
            MachineRegister::X86Rbx => "rbx",
            MachineRegister::X86Rsp => "rsp",
            MachineRegister::X86Rbp => "rbp",
            MachineRegister::X86Rsi => "rsi",
            MachineRegister::X86Rdi => "rdi",
            MachineRegister::X86R8 => "r8",
            MachineRegister::X86R9 => "r9",
            MachineRegister::X86R10 => "r10",
            MachineRegister::X86R11 => "r11",
            MachineRegister::X86R12 => "r12",
            MachineRegister::X86R13 => "r13",
            MachineRegister::X86R14 => "r14",
            MachineRegister::X86R15 => "r15",
            MachineRegister::X86Xmm(_)
            | MachineRegister::Aarch64X(_)
            | MachineRegister::Aarch64V(_) => {
                return None;
            }
        };
        model.model().view_named(name).map(|view| view.id)
    }

    pub(crate) fn x86_64_physical_register_model_ref() -> &'static PhysicalRegisterModel {
        static MODEL: std::sync::OnceLock<PhysicalRegisterModel> = std::sync::OnceLock::new();
        MODEL.get_or_init(build_x86_64_physical_register_model)
    }

    pub fn x86_64_physical_register_model() -> PhysicalRegisterModel {
        x86_64_physical_register_model_ref().clone()
    }

    fn build_x86_64_physical_register_model() -> PhysicalRegisterModel {
        let mut builder = PhysicalModelBuilder::with_classes(&[
            "x86.gpr64",
            "x86.gpr32",
            "x86.gpr16",
            "x86.gpr8-low",
            "x86.gpr8-high",
            "x86.vector128",
            "x86.flags",
            "x86.instruction-pointer",
            "x86.floating-control",
        ]);
        let registers = [
            ("rax", "eax", "ax", "al", Some("ah")),
            ("rbx", "ebx", "bx", "bl", Some("bh")),
            ("rcx", "ecx", "cx", "cl", Some("ch")),
            ("rdx", "edx", "dx", "dl", Some("dh")),
            ("rsi", "esi", "si", "sil", None),
            ("rdi", "edi", "di", "dil", None),
            ("rbp", "ebp", "bp", "bpl", None),
            ("rsp", "esp", "sp", "spl", None),
            ("r8", "r8d", "r8w", "r8b", None),
            ("r9", "r9d", "r9w", "r9b", None),
            ("r10", "r10d", "r10w", "r10b", None),
            ("r11", "r11d", "r11w", "r11b", None),
            ("r12", "r12d", "r12w", "r12b", None),
            ("r13", "r13d", "r13w", "r13b", None),
            ("r14", "r14d", "r14w", "r14b", None),
            ("r15", "r15d", "r15w", "r15b", None),
        ];
        let mut gpr_units = BTreeMap::<String, Vec<RegisterUnitId>>::new();
        let mut named_views = BTreeMap::<String, RegisterViewId>::new();
        for (full, dword, word, low, high) in registers {
            let kind = if full == "rsp" {
                RegisterUnitKind::StackPointer
            } else {
                RegisterUnitKind::IntegerLane
            };
            let lanes = vec![
                builder.unit(format!("{full}.bits0_7"), 8, kind),
                builder.unit(format!("{full}.bits8_15"), 8, kind),
                builder.unit(format!("{full}.bits16_31"), 16, kind),
                builder.unit(format!("{full}.bits32_63"), 32, kind),
            ];
            let allocatable = full != "rsp";
            let full_view = builder.view(
                full.into(),
                GPR64,
                lanes.clone(),
                lanes.clone(),
                64,
                RegisterWriteSemantics::ExactView,
                allocatable,
            );
            let dword_view = builder.view(
                dword.into(),
                GPR32,
                lanes[..3].to_vec(),
                lanes.clone(),
                32,
                RegisterWriteSemantics::ZeroExtendsParent,
                allocatable,
            );
            let word_view = builder.view(
                word.into(),
                GPR16,
                lanes[..2].to_vec(),
                lanes[..2].to_vec(),
                16,
                RegisterWriteSemantics::PreservesUnwritten,
                allocatable,
            );
            let low_view = builder.view(
                low.into(),
                GPR8_LOW,
                lanes[..1].to_vec(),
                lanes[..1].to_vec(),
                8,
                RegisterWriteSemantics::PreservesUnwritten,
                allocatable,
            );
            named_views.extend([
                (full.into(), full_view),
                (dword.into(), dword_view),
                (word.into(), word_view),
                (low.into(), low_view),
            ]);
            if let Some(high) = high {
                let high_view = builder.view(
                    high.into(),
                    GPR8_HIGH,
                    lanes[1..2].to_vec(),
                    lanes[1..2].to_vec(),
                    8,
                    RegisterWriteSemantics::PreservesUnwritten,
                    false,
                );
                named_views.insert(high.into(), high_view);
            }
            gpr_units.insert(full.into(), lanes);
        }
        let mut vector_units = Vec::new();
        for index in 0..16 {
            let name = format!("xmm{index}");
            let unit = builder.unit(
                format!("{name}.bits0_127"),
                128,
                RegisterUnitKind::VectorLane,
            );
            let view = builder.view(
                name.clone(),
                VECTOR128,
                vec![unit],
                vec![unit],
                128,
                RegisterWriteSemantics::ExactView,
                true,
            );
            named_views.insert(name, view);
            vector_units.push(unit);
        }
        let condition_flags = builder.unit("rflags.condition".into(), 6, RegisterUnitKind::Flags);
        let control_flags = builder.unit("rflags.control".into(), 58, RegisterUnitKind::Flags);
        for (name, units, bits) in [
            ("rflags", vec![condition_flags, control_flags], 64),
            ("condition_flags", vec![condition_flags], 6),
            ("control_flags", vec![control_flags], 58),
        ] {
            let flags_view = builder.view(
                name.into(),
                FLAGS,
                units.clone(),
                units,
                bits,
                RegisterWriteSemantics::InstructionDefined,
                false,
            );
            named_views.insert(name.into(), flags_view);
        }
        let rip_unit = builder.unit(
            "rip.storage".into(),
            64,
            RegisterUnitKind::InstructionPointer,
        );
        let rip_view = builder.view(
            "rip".into(),
            INSTRUCTION_POINTER,
            vec![rip_unit],
            vec![rip_unit],
            64,
            RegisterWriteSemantics::InstructionDefined,
            false,
        );
        named_views.insert("rip".into(), rip_view);
        let mxcsr_unit = builder.unit(
            "mxcsr.storage".into(),
            32,
            RegisterUnitKind::FloatingControl,
        );
        let mxcsr_view = builder.view(
            "mxcsr".into(),
            FLOAT_CONTROL,
            vec![mxcsr_unit],
            vec![mxcsr_unit],
            32,
            RegisterWriteSemantics::InstructionDefined,
            false,
        );
        named_views.insert("mxcsr".into(), mxcsr_view);

        let all_units = builder.unit_ids();
        let rsp = gpr_units["rsp"].clone();
        let fixed = sorted_units(rsp.iter().copied().chain([rip_unit, mxcsr_unit]));
        let sysv_callee = units_for(&gpr_units, &["rbx", "rbp", "r12", "r13", "r14", "r15"]);
        let sysv_caller = complement(&all_units, &fixed, &sysv_callee);
        let microsoft_callee = sorted_units(
            units_for(
                &gpr_units,
                &["rbx", "rbp", "rdi", "rsi", "r12", "r13", "r14", "r15"],
            )
            .into_iter()
            .chain(vector_units[6..].iter().copied()),
        );
        let microsoft_caller = complement(&all_units, &fixed, &microsoft_callee);
        let conventions = vec![
            PreservationConvention {
                name: "system-v-amd64".into(),
                argument_views: view_ids(&named_views, &["rdi", "rsi", "rdx", "rcx", "r8", "r9"]),
                result_views: view_ids(&named_views, &["rax"]),
                caller_saved: sysv_caller.clone(),
                callee_saved: sysv_callee,
                fixed: fixed.clone(),
                stack_alignment: 16,
                red_zone_bytes: 128,
            },
            PreservationConvention {
                name: "microsoft-x64".into(),
                argument_views: view_ids(&named_views, &["rcx", "rdx", "r8", "r9"]),
                result_views: view_ids(&named_views, &["rax"]),
                caller_saved: microsoft_caller,
                callee_saved: microsoft_callee,
                fixed: fixed.clone(),
                stack_alignment: 16,
                red_zone_bytes: 0,
            },
        ];
        let reservations = vec![
            reservation_overlay(
                "x86.floating-control",
                ReservationReason::Architectural,
                vec![mxcsr_unit],
            ),
            reservation_overlay("x86.stack-pointer", ReservationReason::StackPointer, rsp),
            reservation_overlay(
                "x86.instruction-pointer",
                ReservationReason::Architectural,
                vec![rip_unit],
            ),
            reservation_overlay(
                "x86.frame-pointer",
                ReservationReason::FramePointer,
                gpr_units["rbp"].clone(),
            ),
            reservation_overlay(
                "omega.x86.dispatch",
                ReservationReason::Dispatch,
                gpr_units["r14"].clone(),
            ),
            reservation_overlay(
                "omega.x86.metering",
                ReservationReason::Metering,
                gpr_units["r15"].clone(),
            ),
        ];
        builder.finish(Architecture::X86_64, conventions, reservations)
    }

    pub fn validated_x86_64_physical_register_model() -> &'static ValidatedPhysicalRegisterModel {
        static CANONICAL: std::sync::OnceLock<ValidatedPhysicalRegisterModel> =
            std::sync::OnceLock::new();
        CANONICAL.get_or_init(|| {
            validate_physical_register_model(x86_64_physical_register_model()).unwrap_or_else(
                |error| panic!("canonical x86-64 physical register model must validate: {error}"),
            )
        })
    }

    pub(crate) fn canonical_x86_64_physical_register_model_identity()
    -> PhysicalRegisterModelIdentity {
        validated_x86_64_physical_register_model().identity()
    }
}
mod system_calls {
    use register_model::{
        RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
        RegisterOperandAccess, ValidatedPhysicalRegisterModel,
    };

    pub fn x86_64_linux_system_call_keys() -> Vec<RegisterConstraintKey> {
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
        let arguments = ["rdi", "rsi", "rdx", "r10", "r8", "r9"];
        let result = view("rax");
        let clobbers = ["rax", "rcx", "r11", "rflags"]
            .into_iter()
            .flat_map(|name| view(name).units.iter().copied())
            .collect::<Vec<_>>();
        for (position, key) in x86_64_linux_system_call_keys().into_iter().enumerate() {
            let argument_count = position / 2;
            let returns_value = position % 2 != 0;
            let operands = arguments[..argument_count]
                .iter()
                .map(|name| (view(name), RegisterOperandAccess::Use))
                .chain(returns_value.then_some((result, RegisterOperandAccess::Def)))
                .enumerate()
                .map(|(operand, (register, access))| {
                    super::fixed_operand(operand as u16, access, register)
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
                implicit_uses: view("rip").units.clone(),
                implicit_defs: view("rip").units.clone(),
                clobbers,
            });
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn system_call_rows_retain_exact_arguments_results_and_clobbers() {
            let physical = crate::validated_x86_64_physical_register_model();
            let catalog = crate::x86_64_register_constraint_catalog(physical);
            let validated =
                crate::validate_x86_64_register_constraint_catalog(catalog.clone(), physical)
                    .expect("canonical syscall constraints");
            for (position, key) in x86_64_linux_system_call_keys().into_iter().enumerate() {
                let argument_count = position / 2;
                let returns_value = position % 2 != 0;
                let row = validated.catalog().constraint(key).unwrap();
                assert_eq!(
                    row.operands.len(),
                    argument_count + usize::from(returns_value)
                );
                let expected = ["rdi", "rsi", "rdx", "r10", "r8", "r9"];
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
                        Some(physical.model().view_named("rax").unwrap().id)
                    );
                }
                let mut written = row.clobbers.clone();
                if returns_value {
                    written.extend_from_slice(&physical.model().view_named("rax").unwrap().units);
                }
                written.sort_unstable();
                written.dedup();
                let mut expected_written = ["rax", "rcx", "r11", "rflags"]
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
                    physical.model().view_named("rip").unwrap().units
                );
                assert_eq!(row.implicit_defs, row.implicit_uses);
            }

            let key = x86_64_linux_system_call_keys()[8];
            let mut wrong_argument = catalog.clone();
            let row =
                register_model::find_register_constraint_mut(&mut wrong_argument.constraints, key)
                    .unwrap();
            row.operands[3].fixed_view = Some(physical.model().view_named("rcx").unwrap().id);
            assert!(
                crate::validate_x86_64_register_constraint_catalog(wrong_argument, physical)
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
                crate::validate_x86_64_register_constraint_catalog(missing_clobbers, physical)
                    .is_err()
            );
        }
    }
}
pub use system_calls::x86_64_linux_system_call_keys;
#[cfg(test)]
mod tests {
    use super::operation_keys::X86_64_ORDINARY_REGISTER_CONSTRAINTS;
    use super::{
        X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO,
        X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_LINUX_SYSTEM_CALL,
        X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL_UNIT, X86_64_MICROSOFT_RETURN,
        X86_64_REMAINDER_I64, X86_64_SUBTRACT_I64, X86_64_SYSTEM_V_CALL,
        X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64, X86_64RegisterConstraintCatalogValidationError,
        validate_x86_64_register_constraint_catalog, x86_64_fixed_register_view,
        x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys,
        x86_64_indirect_aggregate_call_keys, x86_64_microsoft_aggregate_call_keys,
        x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_mixed_aggregate_call_keys,
        x86_64_microsoft_mixed_unit_call_keys, x86_64_microsoft_normalized_foreign_call_keys,
        x86_64_microsoft_register_call_keys, x86_64_microsoft_register_unit_call_keys,
        x86_64_physical_register_model, x86_64_preservation_convention_for_target,
        x86_64_register_constraint_catalog, x86_64_system_v_aggregate_call_keys,
        x86_64_system_v_aggregate_return_keys, x86_64_system_v_mixed_aggregate_call_keys,
        x86_64_system_v_mixed_unit_call_keys, x86_64_system_v_normalized_foreign_call_keys,
        x86_64_system_v_register_call_keys, x86_64_system_v_register_unit_call_keys,
    };
    use crate::register_model::physical_model::GPR64;
    use crate::register_model::physical_model::VECTOR128;
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

    macro_rules! mismatch {
        ($catalog:expr, $model:expr, $key:expr) => {
            assert_eq!(
                validate_x86_64_register_constraint_catalog($catalog, $model),
                Err(X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch($key))
            );
        };
    }

    #[test]
    fn aggregate_result_fragments_are_definitions_not_unknown_clobbers() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        assert_aggregate_fragments_are_definitions(
            &model,
            &catalog,
            &AggregateFragmentRoster {
                keys: x86_64_system_v_aggregate_call_keys(),
                keys_per_group: 7,
                second_view: "rdx",
                scratch_view: "rcx",
            },
            |catalog, model| validate_x86_64_register_constraint_catalog(catalog, model).is_ok(),
        );
    }

    #[test]
    fn fixed_machine_register_views_are_target_owned() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        assert_eq!(
            x86_64_fixed_register_view(&model, MachineRegister::X86Rdi),
            model.model().view_named("rdi").map(|view| view.id)
        );
        assert_eq!(
            x86_64_fixed_register_view(&model, MachineRegister::Aarch64X(0)),
            None
        );
    }

    #[test]
    fn microsoft_direct_aggregate_rows_preserve_positional_arguments_and_volatile_units() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        let rax = model.model().view_named("rax").unwrap();
        let rdx = model.model().view_named("rdx").unwrap();
        let rsi = model.model().view_named("rsi").unwrap();
        for (arity, key) in x86_64_microsoft_aggregate_call_keys()
            .into_iter()
            .enumerate()
        {
            let call = row(&catalog, key);
            let expected = ["rcx", "rdx", "r8", "r9"]
                .into_iter()
                .take(arity)
                .chain(["rax"])
                .map(|name| model.model().view_named(name).unwrap().id)
                .collect::<Vec<_>>();
            assert_eq!(
                call.operands
                    .iter()
                    .map(|operand| operand.fixed_view.unwrap())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                call.operands.last().unwrap().access,
                RegisterOperandAccess::Def
            );
            assert!(
                rax.write_units
                    .iter()
                    .all(|unit| !call.clobbers.contains(unit))
            );
            assert!(
                rdx.write_units
                    .iter()
                    .all(|unit| call.clobbers.contains(unit))
            );
            assert!(
                rsi.write_units
                    .iter()
                    .all(|unit| !call.clobbers.contains(unit))
            );
            let mut changed = catalog.clone();
            row_mut(&mut changed, key)
                .operands
                .last_mut()
                .unwrap()
                .fixed_view = Some(rdx.id);
            assert!(validate_x86_64_register_constraint_catalog(changed, &model).is_err());
        }
        let returns = x86_64_microsoft_aggregate_return_keys();
        assert_eq!(returns.len(), 1);
        assert_eq!(row(&catalog, returns[0]).operands.len(), 1);
        assert_eq!(
            row(&catalog, returns[0]).operands[0].fixed_view,
            Some(rax.id)
        );
    }

    #[test]
    fn preservation_convention_is_selected_by_exact_target_policy() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let system_v =
            x86_64_preservation_convention_for_target(&model, NativeTarget::linux_x64()).unwrap();
        let microsoft =
            x86_64_preservation_convention_for_target(&model, NativeTarget::windows_x64()).unwrap();
        assert_eq!(system_v.name, "system-v-amd64");
        assert_eq!(microsoft.name, "microsoft-x64");
        let rsi = model.model().view_named("rsi").unwrap().units[0];
        assert!(!system_v.callee_saved.contains(&rsi));
        assert!(microsoft.callee_saved.contains(&rsi));
        assert!(
            x86_64_preservation_convention_for_target(&model, NativeTarget::macos_arm64())
                .is_none()
        );
    }

    #[test]
    fn model_validates_and_partial_register_aliases_are_exact() {
        let validated = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let model = validated.model();
        let view = |name| model.view_named(name).unwrap().id;
        assert!(model.aliases(view("rax"), view("eax")));
        assert!(model.aliases(view("rax"), view("ah")));
        assert!(!model.aliases(view("al"), view("ah")));
        assert!(!model.aliases(view("rax"), view("xmm0")));
        assert_eq!(
            model.view_named("eax").unwrap().write_units,
            model.view_named("rax").unwrap().units
        );
        assert!(!model.view_named("ah").unwrap().allocatable);
        assert!(!model.view_named("rsp").unwrap().allocatable);
    }

    #[test]
    fn register_constraint_catalog_closes_the_required_x86_64_inventory() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let validated = validate_x86_64_register_constraint_catalog(
            x86_64_register_constraint_catalog(&model),
            &model,
        )
        .unwrap();
        let catalog = validated.catalog();
        assert!(
            X86_64_ORDINARY_REGISTER_CONSTRAINTS
                .iter()
                .all(|key| catalog.required.contains(key))
        );
        assert_eq!(
            catalog.required.len(),
            X86_64_ORDINARY_REGISTER_CONSTRAINTS.len()
                + x86_64_system_v_register_call_keys().len()
                + x86_64_microsoft_register_call_keys().len()
                + x86_64_system_v_register_unit_call_keys().len()
                + x86_64_microsoft_register_unit_call_keys().len()
                + x86_64_indirect_aggregate_call_keys(false).len()
                + x86_64_indirect_aggregate_call_keys(true).len()
                + x86_64_float_scalar_call_keys(false).len()
                + x86_64_float_scalar_call_keys(true).len()
                + x86_64_float_scalar_return_keys(false).len()
                + x86_64_float_scalar_return_keys(true).len()
                + 2
                + x86_64_system_v_mixed_unit_call_keys().len()
                + x86_64_microsoft_mixed_unit_call_keys().len()
                + x86_64_system_v_mixed_aggregate_call_keys().len()
                + x86_64_microsoft_mixed_aggregate_call_keys().len()
                + x86_64_system_v_aggregate_call_keys().len()
                + x86_64_system_v_aggregate_return_keys().len()
                + x86_64_microsoft_aggregate_call_keys().len()
                + x86_64_microsoft_aggregate_return_keys().len()
                + x86_64_system_v_normalized_foreign_call_keys().len()
                + x86_64_microsoft_normalized_foreign_call_keys().len()
                + crate::x86_64_linux_system_call_keys().len()
        );

        let sysv_call = row(catalog, X86_64_SYSTEM_V_CALL);
        assert_eq!(sysv_call.key, X86_64_SYSTEM_V_CALL);
        assert_eq!(sysv_call.operands.len(), 7);
        assert_eq!(sysv_call.operands[6].access, RegisterOperandAccess::Def);
        assert_eq!(
            sysv_call.operands[6].fixed_view,
            Some(model.model().view_named("rax").unwrap().id)
        );
        assert!(
            model
                .model()
                .view_named("rsp")
                .unwrap()
                .units
                .iter()
                .all(|unit| sysv_call.implicit_uses.contains(unit)
                    && sysv_call.implicit_defs.contains(unit))
        );

        let unit_call = row(catalog, X86_64_MICROSOFT_CALL_UNIT);
        assert_eq!(unit_call.key, X86_64_MICROSOFT_CALL_UNIT);
        assert_eq!(unit_call.operands.len(), 2);
        for (operand, name) in unit_call.operands.iter().zip(["rcx", "rdx"]) {
            assert_eq!(operand.access, RegisterOperandAccess::Use);
            assert_eq!(
                operand.fixed_view,
                Some(model.model().view_named(name).unwrap().id)
            );
        }
        for used in ["rsp", "rip"] {
            assert!(
                model
                    .model()
                    .view_named(used)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| unit_call.implicit_uses.contains(unit))
            );
        }
        for clobbered in [
            "rax", "rcx", "rdx", "r8", "r9", "r10", "r11", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4",
            "xmm5", "rflags",
        ] {
            assert!(
                model
                    .model()
                    .view_named(clobbered)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| unit_call.clobbers.contains(unit))
            );
        }
        let scalar_call = row(catalog, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64);
        assert_eq!(scalar_call.key, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64);
        assert_eq!(scalar_call.operands.len(), 3);
        assert_eq!(
            scalar_call.operands[0].fixed_view,
            Some(model.model().view_named("rdi").unwrap().id)
        );
        assert_eq!(
            scalar_call.operands[1].fixed_view,
            Some(model.model().view_named("rsi").unwrap().id)
        );
        assert_eq!(
            scalar_call.operands[2].fixed_view,
            Some(model.model().view_named("rax").unwrap().id)
        );

        let syscall = row(catalog, X86_64_LINUX_SYSTEM_CALL);
        assert_eq!(syscall.key, X86_64_LINUX_SYSTEM_CALL);
        assert_eq!(syscall.operands[0].access, RegisterOperandAccess::UseDef);
        assert_eq!(
            syscall.operands[0].fixed_view,
            Some(model.model().view_named("rax").unwrap().id)
        );
        for clobbered in ["rcx", "r11", "rflags"] {
            assert!(
                model
                    .model()
                    .view_named(clobbered)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| syscall.clobbers.contains(unit))
            );
        }

        let materialize = row(catalog, X86_64_MATERIALIZE_I64);
        assert_eq!(materialize.key, X86_64_MATERIALIZE_I64);
        assert_eq!(materialize.operands.len(), 1);
        assert_eq!(materialize.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(materialize.operands[0].class, GPR64);

        let copy = row(catalog, X86_64_COPY_I64);
        assert_eq!(copy.key, X86_64_COPY_I64);
        assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(copy.operands[1].access, RegisterOperandAccess::Def);

        let compare = row(catalog, X86_64_COMPARE_I64_ZERO);
        assert_eq!(compare.key, X86_64_COMPARE_I64_ZERO);
        assert_eq!(compare.operands[0].class, GPR64);
        assert_eq!(
            compare.implicit_defs,
            model.model().view_named("condition_flags").unwrap().units
        );

        let branch = row(catalog, X86_64_CONDITIONAL_BRANCH);
        assert_eq!(branch.key, X86_64_CONDITIONAL_BRANCH);
        for state in ["condition_flags", "rip"] {
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

        let add = row(catalog, X86_64_ADD_I64);
        assert_eq!(add.key, X86_64_ADD_I64);
        assert_eq!(add.operands.len(), 3);
        assert_eq!(add.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(add.operands[1].access, RegisterOperandAccess::Use);
        assert_eq!(add.operands[2].access, RegisterOperandAccess::Def);
        assert!(add.operands.iter().all(|operand| operand.class == GPR64));
        assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
        assert!(add.implicit_uses.is_empty());
        assert!(add.implicit_defs.is_empty());
        assert!(add.clobbers.is_empty());

        let add_immediate = row(catalog, X86_64_ADD_I64_IMMEDIATE);
        assert_eq!(add_immediate.key, X86_64_ADD_I64_IMMEDIATE);
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

        let subtract = row(catalog, X86_64_SUBTRACT_I64);
        assert_eq!(subtract.key, X86_64_SUBTRACT_I64);
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
        assert_eq!(
            subtract.clobbers,
            model.model().view_named("condition_flags").unwrap().units
        );
        assert_eq!(
            branch.implicit_defs,
            model.model().view_named("rip").unwrap().units
        );
    }

    #[test]
    fn signed_remainder_row_keeps_the_divisor_out_of_rdx() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        let rax = model.model().view_named("rax").unwrap().id;
        let rcx = model.model().view_named("rcx").unwrap().id;
        let rdx = model.model().view_named("rdx").unwrap();
        let remainder = row(&catalog, X86_64_REMAINDER_I64);
        assert_eq!(remainder.key, X86_64_REMAINDER_I64);
        assert_eq!(remainder.operands.len(), 4);
        for (operand, access, fixed) in [
            (0, RegisterOperandAccess::Use, Some(rax)),
            (1, RegisterOperandAccess::Use, Some(rcx)),
            (2, RegisterOperandAccess::Def, Some(rax)),
            (3, RegisterOperandAccess::Def, Some(rdx.id)),
        ] {
            assert_eq!(remainder.operands[operand].operand, operand as u16);
            assert_eq!(remainder.operands[operand].access, access);
            assert_eq!(remainder.operands[operand].fixed_view, fixed);
            assert!(
                !remainder.operands[operand].early_clobber,
                "operand {operand} must not be early-clobber"
            );
            assert_eq!(remainder.operands[operand].class, GPR64);
        }
        assert!(remainder.implicit_uses.is_empty());
        assert!(remainder.implicit_defs.is_empty());
        assert_eq!(
            remainder.clobbers,
            model.model().view_named("condition_flags").unwrap().units
        );

        let mut changed = x86_64_register_constraint_catalog(&model);
        let operand = &mut row_mut(&mut changed, X86_64_REMAINDER_I64).operands[3];
        operand.early_clobber = true;
        mismatch!(changed, &model, X86_64_REMAINDER_I64);

        let mut changed = x86_64_register_constraint_catalog(&model);
        let operand = &mut row_mut(&mut changed, X86_64_REMAINDER_I64).operands[1];
        operand.fixed_view = None;
        mismatch!(changed, &model, X86_64_REMAINDER_I64);
    }

    #[test]
    fn no_fixed_view_operand_is_early_clobber() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        for constraint in &catalog.constraints {
            for operand in &constraint.operands {
                assert!(
                    !(operand.early_clobber && operand.fixed_view.is_some()),
                    "{:?} operand {} pins an early-clobber fixed view, which \
                 fixed-precolored interval validation rejects",
                    constraint.key,
                    operand.operand,
                );
            }
        }
    }

    #[test]
    fn missing_required_x86_64_constraint_rejects() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut catalog = x86_64_register_constraint_catalog(&model);
        catalog
            .constraints
            .retain(|constraint| constraint.key != X86_64_MICROSOFT_RETURN);
        for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
            constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
        }
        assert_eq!(
            validate_x86_64_register_constraint_catalog(catalog, &model),
            Err(X86_64RegisterConstraintCatalogValidationError::Structural(
                RegisterConstraintCatalogValidationError::MissingRequiredConstraint(
                    X86_64_MICROSOFT_RETURN,
                ),
            )),
        );
    }

    #[test]
    fn target_inventory_cannot_erase_a_required_key_and_its_row_together() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut catalog = x86_64_register_constraint_catalog(&model);
        catalog
            .required
            .retain(|key| *key != X86_64_MICROSOFT_RETURN);
        catalog
            .constraints
            .retain(|constraint| constraint.key != X86_64_MICROSOFT_RETURN);
        for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
            constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
        }
        mismatch!(catalog, &model, X86_64_MICROSOFT_RETURN);
    }

    #[test]
    fn x86_64_constraint_semantic_corruption_rejects() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut wrong_class = x86_64_register_constraint_catalog(&model);
        let call_id = row(&wrong_class, X86_64_SYSTEM_V_CALL).id;
        row_mut(&mut wrong_class, X86_64_SYSTEM_V_CALL).operands[6].class = VECTOR128;
        assert_eq!(
            validate_x86_64_register_constraint_catalog(wrong_class, &model),
            Err(X86_64RegisterConstraintCatalogValidationError::Structural(
                RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                    constraint: call_id,
                    operand: 6,
                },
            )),
        );

        let mut contradictory_post_state = x86_64_register_constraint_catalog(&model);
        let unit = row_mut(&mut contradictory_post_state, X86_64_SYSTEM_V_CALL).implicit_defs[0];
        row_mut(&mut contradictory_post_state, X86_64_SYSTEM_V_CALL)
            .clobbers
            .push(unit);
        row_mut(&mut contradictory_post_state, X86_64_SYSTEM_V_CALL)
            .clobbers
            .sort_unstable();
        assert_eq!(
            validate_x86_64_register_constraint_catalog(contradictory_post_state, &model),
            Err(X86_64RegisterConstraintCatalogValidationError::Structural(
                RegisterConstraintCatalogValidationError::DefClobberOverlap {
                    constraint: call_id,
                    unit,
                },
            )),
        );
    }

    #[test]
    fn x86_64_target_semantics_reject_compatible_substitution_and_missing_clobbers() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut wrong_syscall_register = x86_64_register_constraint_catalog(&model);
        row_mut(&mut wrong_syscall_register, X86_64_LINUX_SYSTEM_CALL).operands[4].fixed_view =
            Some(model.model().view_named("r11").unwrap().id);
        mismatch!(wrong_syscall_register, &model, X86_64_LINUX_SYSTEM_CALL);

        for clobber in ["rcx", "r11", "rflags"] {
            let mut missing_clobber = x86_64_register_constraint_catalog(&model);
            let omitted = model.model().view_named(clobber).unwrap().units[0];
            row_mut(&mut missing_clobber, X86_64_LINUX_SYSTEM_CALL)
                .clobbers
                .retain(|unit| *unit != omitted);
            assert_eq!(
                validate_x86_64_register_constraint_catalog(missing_clobber, &model),
                Err(
                    X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                        X86_64_LINUX_SYSTEM_CALL,
                    )
                ),
                "omitting {clobber} state must reject",
            );
        }

        let mut wrong_add_role = x86_64_register_constraint_catalog(&model);
        row_mut(&mut wrong_add_role, X86_64_ADD_I64).operands[1].access =
            RegisterOperandAccess::Def;
        mismatch!(wrong_add_role, &model, X86_64_ADD_I64);

        let mut wrong_immediate_role = x86_64_register_constraint_catalog(&model);
        row_mut(&mut wrong_immediate_role, X86_64_ADD_I64_IMMEDIATE).operands[0].access =
            RegisterOperandAccess::Def;
        mismatch!(wrong_immediate_role, &model, X86_64_ADD_I64_IMMEDIATE);

        let mut wrong_subtract_role = x86_64_register_constraint_catalog(&model);
        row_mut(&mut wrong_subtract_role, X86_64_SUBTRACT_I64).operands[1].access =
            RegisterOperandAccess::Def;
        mismatch!(wrong_subtract_role, &model, X86_64_SUBTRACT_I64);

        let mut missing_subtract_flags = x86_64_register_constraint_catalog(&model);
        row_mut(&mut missing_subtract_flags, X86_64_SUBTRACT_I64)
            .clobbers
            .clear();
        mismatch!(missing_subtract_flags, &model, X86_64_SUBTRACT_I64);
    }

    #[test]
    fn x86_64_branch_rejects_one_field_missing_flags_use() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut catalog = x86_64_register_constraint_catalog(&model);
        let flags = model.model().view_named("condition_flags").unwrap().units[0];
        row_mut(&mut catalog, X86_64_CONDITIONAL_BRANCH)
            .implicit_uses
            .retain(|unit| *unit != flags);
        mismatch!(catalog, &model, X86_64_CONDITIONAL_BRANCH);
    }

    #[test]
    fn x86_64_catalog_validation_rejects_same_architecture_forged_physical_model() {
        let canonical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&canonical);
        let mut forged = x86_64_physical_register_model();
        forged.views[0].name = "forged.rax".into();
        let forged = validate_physical_register_model(forged).unwrap();
        assert_eq!(
            x86_64_fixed_register_view(&forged, MachineRegister::X86Rax),
            None
        );
        assert_eq!(
            validate_x86_64_register_constraint_catalog(catalog, &forged),
            Err(X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel)
        );
    }

    #[test]
    fn omitted_and_overlapping_units_reject() {
        let mut omitted = x86_64_physical_register_model();
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

        let mut overlap = x86_64_physical_register_model();
        let unit = overlap.conventions[0].caller_saved[0];
        overlap.conventions[0].callee_saved.push(unit);
        overlap.conventions[0].callee_saved.sort_unstable();
        assert_eq!(
            validate_physical_register_model(overlap),
            Err(RegisterModelValidationError::ConventionPartitionOverlap(
                unit
            ))
        );
    }

    fn x86_64_foreign_template_roster() -> ForeignTemplateRoster {
        ForeignTemplateRoster {
            templates: vec![
                (
                    super::x86_64_system_v_register_unit_call_keys()[0],
                    super::x86_64_system_v_normalized_foreign_call_keys(),
                ),
                (
                    super::x86_64_microsoft_register_unit_call_keys()[0],
                    super::x86_64_microsoft_normalized_foreign_call_keys(),
                ),
            ],
            signature_views: ["rax", "xmm0"],
            effect_view: "rcx",
        }
    }

    #[test]
    fn normalized_foreign_templates_retain_complete_target_call_effects() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        assert_templates_retain_call_effects(&catalog, &x86_64_foreign_template_roster());
    }

    #[test]
    fn normalized_foreign_templates_reject_signature_rows_and_changed_effects() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        assert_templates_reject_changed_effects(
            &model,
            &catalog,
            &x86_64_foreign_template_roster(),
            |catalog, model| validate_x86_64_register_constraint_catalog(catalog, model).is_ok(),
        );
    }

    #[test]
    fn frame_spills_preserve_control_flags_but_interfere_with_live_conditions() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let catalog = x86_64_register_constraint_catalog(&model);
        let units = |name| &model.model().view_named(name).unwrap().units;
        let conditions = units("condition_flags");
        let controls = units("control_flags");
        assert!(conditions.iter().all(|unit| !controls.contains(unit)));
        let mut whole = conditions.clone();
        whole.extend(controls);
        whole.sort_unstable();
        assert_eq!(&whole, units("rflags"));

        for key in [crate::X86_64_FRAME_ADDRESS, crate::X86_64_STORE64] {
            let spill = row(&catalog, key);
            assert_eq!(&spill.clobbers, conditions);
            let branch = row(&catalog, X86_64_CONDITIONAL_BRANCH);
            assert!(
                spill
                    .clobbers
                    .iter()
                    .all(|unit| branch.implicit_uses.contains(unit))
            );
            for check in [
                crate::X86_64_INSTALLED_INTERRUPT_CPU_CHECK,
                crate::X86_64_INSTALLED_INTERRUPT_TASK_COMMIT,
                crate::X86_64_TASK_REGISTER_ACTIVATE,
                crate::X86_64_CODE_SEGMENT_RELOAD,
                crate::X86_64_DATA_SEGMENT_RELOAD,
            ] {
                let check = row(&catalog, check);
                assert!(
                    controls
                        .iter()
                        .all(|unit| check.implicit_uses.contains(unit))
                );
                assert!(
                    spill
                        .clobbers
                        .iter()
                        .all(|unit| !check.implicit_uses.contains(unit))
                );
            }
        }
        let interrupts = row(&catalog, crate::X86_64_INTERRUPT_CONTROL);
        assert_eq!(&interrupts.implicit_uses, controls);
        assert_eq!(&interrupts.implicit_defs, controls);
        let snapshot = row(&catalog, crate::X86_64_FLAGS_SNAPSHOT);
        assert!(
            whole
                .iter()
                .all(|unit| snapshot.implicit_uses.contains(unit))
        );
        assert_eq!(
            row(&catalog, crate::X86_64_FLAGS_RESTORE).implicit_defs,
            whole
        );
        assert!(whole.iter().all(|unit| {
            row(&catalog, X86_64_LINUX_SYSTEM_CALL)
                .clobbers
                .contains(unit)
        }));
    }
}
pub(crate) use call_keys::X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64;
pub use call_keys::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN,
    X86_64_SYSTEM_V_RETURN_UNIT, x86_64_microsoft_aggregate_call_keys,
    x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_normalized_foreign_call_keys,
    x86_64_microsoft_register_call_keys, x86_64_microsoft_register_unit_call_keys,
    x86_64_preservation_convention_for_target, x86_64_system_v_aggregate_call_keys,
    x86_64_system_v_aggregate_return_keys, x86_64_system_v_normalized_foreign_call_keys,
    x86_64_system_v_register_call_keys, x86_64_system_v_register_unit_call_keys,
};
pub(crate) use constraint_catalog::x86_64_register_constraint_catalog_for;
pub use constraint_catalog::{
    X86_64RegisterConstraintCatalogValidationError, prepare_x86_64_register_constraint_catalog,
    validate_x86_64_register_constraint_catalog, x86_64_register_constraint_catalog,
};
pub use float_scalar_calls::{x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys};
pub use indirect_results::x86_64_indirect_aggregate_call_keys;
pub use mixed_aggregate_calls::{
    x86_64_microsoft_mixed_aggregate_call_keys, x86_64_system_v_mixed_aggregate_call_keys,
};
pub use mixed_calls::{
    x86_64_microsoft_mixed_unit_call_keys, x86_64_system_v_mixed_unit_call_keys,
};
pub use operation_keys::{
    X86_64_ACCESSOR_READ, X86_64_ACCESSOR_WRITE, X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE,
    X86_64_ADDRESS_OFFSET, X86_64_ATOMIC_BITWISE_UPDATE, X86_64_ATOMIC_COMPARE_EXCHANGE,
    X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE, X86_64_ATOMIC_FENCE, X86_64_ATOMIC_LOAD,
    X86_64_ATOMIC_STORE, X86_64_ATOMIC_UPDATE, X86_64_BITS_TO_FLOAT32, X86_64_BITS_TO_FLOAT64,
    X86_64_BYTE_LITERAL_IMAGE_ADDRESS, X86_64_CODE_SEGMENT_RELOAD, X86_64_COMPARE_BYTE_SEQUENCES,
    X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO,
    X86_64_CONDITIONAL_BRANCH, X86_64_CONTROL_REGISTER_READ, X86_64_CONTROL_REGISTER_WRITE,
    X86_64_COPY_BYTES, X86_64_COPY_I64, X86_64_CRASH, X86_64_DATA_SEGMENT_RELOAD,
    X86_64_DIVIDE_I64, X86_64_DIVIDE_U64, X86_64_FLAGS_RESTORE, X86_64_FLAGS_SNAPSHOT,
    X86_64_FLOAT_RESULT_SATURATING, X86_64_FLOAT_RESULT_TRAPPING, X86_64_FLOAT32_TO_BITS,
    X86_64_FLOAT64_TO_BITS, X86_64_FRAME_ADDRESS, X86_64_GLOBAL_DESCRIPTOR_TABLE,
    X86_64_HOSTED_EXIT_PROCESS_I32, X86_64_HOSTED_READ_BYTE, X86_64_HOSTED_WRITE_BYTE_I32,
    X86_64_IEEE_FLOAT_ARITHMETIC, X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
    X86_64_IEEE_FLOAT_DIRECTED_BINARY, X86_64_IEEE_FLOAT_DIRECTED_UNARY, X86_64_IEEE_FLOAT_TERNARY,
    X86_64_IEEE_FLOAT_UNARY, X86_64_INSTALLED_INTERRUPT_CPU_CHECK,
    X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION, X86_64_INSTALLED_INTERRUPT_TASK_COMMIT,
    X86_64_INTERRUPT_CONTROL, X86_64_JUMP, X86_64_LOAD_ENCODED_SCALAR, X86_64_LOAD8,
    X86_64_LOAD8_INDEXED, X86_64_LOAD16, X86_64_LOAD32, X86_64_LOAD64, X86_64_MATERIALIZE_BOOLEAN,
    X86_64_MATERIALIZE_I64, X86_64_MEMORY_FENCE, X86_64_MODEL_SPECIFIC_READ,
    X86_64_MODEL_SPECIFIC_WRITE, X86_64_MULTIPLY_I64, X86_64_PORT_REGISTER_READ,
    X86_64_PORT_REGISTER_WRITE, X86_64_PORT_WRITE, X86_64_PRIVILEGED_CRASH, X86_64_PROCESSOR_HALT,
    X86_64_REMAINDER_I64, X86_64_REMAINDER_U64, X86_64_RESTORE_FLOATING_CONTROL,
    X86_64_SATURATING_ADD_CLAMPED, X86_64_SATURATING_ADD_U64, X86_64_SATURATING_DIVIDE_SIGNED,
    X86_64_SATURATING_MULTIPLY_CLAMPED, X86_64_SATURATING_MULTIPLY_U64,
    X86_64_SATURATING_SUBTRACT_CLAMPED, X86_64_SATURATING_SUBTRACT_UNSIGNED,
    X86_64_SAVE_FLOATING_CONTROL, X86_64_SCAN_BYTE_SEQUENCE, X86_64_SEGMENT_SELECTOR_READ,
    X86_64_SHIFT_I64, X86_64_STORE, X86_64_STORE_ENCODED_SCALAR, X86_64_STORE64,
    X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_TASK_REGISTER_ACTIVATE,
    X86_64_TASK_REGISTER_READ, X86_64_TRAPPING_BINARY, X86_64_TRAPPING_CONVERT,
    X86_64_TRAPPING_FIXED_PAIR, X86_64_TRAPPING_SHIFT, X86_64_ZERO_BYTES,
};
pub(crate) use operation_keys::{
    X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_LINUX_SYSTEM_CALL, x86_64_required_register_constraints,
};
pub use packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
pub(crate) use physical_model::{
    canonical_x86_64_physical_register_model_identity, x86_64_physical_register_model_ref,
};
pub use physical_model::{
    validated_x86_64_physical_register_model, x86_64_fixed_register_view,
    x86_64_physical_register_model,
};
