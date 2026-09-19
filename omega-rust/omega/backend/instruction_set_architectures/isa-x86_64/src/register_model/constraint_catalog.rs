//! The register constraint catalog of every x86-64 operation and its
//! validation against the closed inventory.

use crate::register_model::packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
use crate::register_model::physical_model::{GPR64, VECTOR128, complement, sorted_units};
use crate::register_model::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_ADDRESS_OFFSET, X86_64_BITS_TO_FLOAT32,
    X86_64_BITS_TO_FLOAT64, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_BYTES, X86_64_COPY_I64,
    X86_64_DIVIDE_I64, X86_64_DIVIDE_U64, X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS,
    X86_64_FRAME_ADDRESS, X86_64_HOSTED_EXIT_PROCESS_I32, X86_64_HOSTED_READ_BYTE,
    X86_64_HOSTED_WRITE_BYTE_I32, X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_JUMP,
    X86_64_LINUX_SYSTEM_CALL, X86_64_LOAD8, X86_64_LOAD8_INDEXED, X86_64_LOAD16, X86_64_LOAD32,
    X86_64_LOAD64, X86_64_MATERIALIZE_BOOLEAN, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL,
    X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT, X86_64_MULTIPLY_I64,
    X86_64_REMAINDER_I64, X86_64_REMAINDER_U64, X86_64_REQUIRED_REGISTER_CONSTRAINTS,
    X86_64_SATURATING_ADD_CLAMPED, X86_64_SATURATING_ADD_U64, X86_64_SATURATING_DIVIDE_SIGNED,
    X86_64_SATURATING_SUBTRACT_CLAMPED, X86_64_SATURATING_SUBTRACT_UNSIGNED, X86_64_STORE,
    X86_64_STORE64, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_CALL,
    X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    x86_64_microsoft_aggregate_call_keys, x86_64_microsoft_aggregate_return_keys,
    x86_64_microsoft_normalized_foreign_call_keys, x86_64_microsoft_register_call_keys,
    x86_64_microsoft_register_unit_call_keys, x86_64_physical_register_model,
    x86_64_system_v_aggregate_call_keys, x86_64_system_v_aggregate_return_keys,
    x86_64_system_v_normalized_foreign_call_keys, x86_64_system_v_register_call_keys,
    x86_64_system_v_register_unit_call_keys,
};
use crate::register_model::{
    float_scalar_calls, indirect_results, mixed_aggregate_calls, mixed_calls, packed_memory,
};
use crate::{
    x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys,
    x86_64_indirect_aggregate_call_keys, x86_64_microsoft_mixed_aggregate_call_keys,
    x86_64_microsoft_mixed_unit_call_keys, x86_64_system_v_mixed_aggregate_call_keys,
    x86_64_system_v_mixed_unit_call_keys,
};
use register_model::{
    PreservationConvention, RegisterClassId, RegisterConstraintCatalog,
    RegisterConstraintCatalogValidationError, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_register_constraint_catalog,
};
use target::Architecture;

/// Build the authoritative x86-64 register-constraint catalog v1 against one
/// independently validated physical model.
///
/// Call and return rows describe the currently represented scalar ABI lane.
/// The result operand is kept distinct so its definition does not masquerade
/// as an undifferentiated caller-saved clobber.
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
            .conventions
            .iter()
            .find(|convention| convention.name == name)
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

    let mut constraints = vec![
        RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: X86_64_SYSTEM_V_CALL,
            operands: ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
                .into_iter()
                .enumerate()
                .map(|(operand, name)| {
                    fixed(
                        u16::try_from(operand).expect("operand index fits u16"),
                        RegisterOperandAccess::Use,
                        name,
                    )
                })
                .chain([fixed(6, RegisterOperandAccess::Def, "rax")])
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
                        RegisterOperandAccess::Use,
                        name,
                    )
                })
                .chain([fixed(4, RegisterOperandAccess::Def, "rax")])
                .collect(),
            implicit_uses: rsp_units.clone(),
            implicit_defs: control_defs.clone(),
            clobbers: call_clobbers(microsoft),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(18),
            key: X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
            operands: vec![
                fixed(0, RegisterOperandAccess::Use, "rdi"),
                fixed(1, RegisterOperandAccess::Use, "rsi"),
                fixed(2, RegisterOperandAccess::Def, "rax"),
            ],
            implicit_uses: sorted_units(rsp_units.iter().copied().chain(rip_units.iter().copied())),
            implicit_defs: control_defs.clone(),
            clobbers: call_clobbers(sysv),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(2),
            key: X86_64_SYSTEM_V_RETURN,
            operands: vec![fixed(0, RegisterOperandAccess::Use, "rax")],
            implicit_uses: rsp_units.clone(),
            implicit_defs: control_defs.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(3),
            key: X86_64_MICROSOFT_RETURN,
            operands: vec![fixed(0, RegisterOperandAccess::Use, "rax")],
            implicit_uses: rsp_units.clone(),
            implicit_defs: control_defs.clone(),
            clobbers: Vec::new(),
        },
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
                            RegisterOperandAccess::UseDef
                        } else {
                            RegisterOperandAccess::Use
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
        RegisterInstructionConstraint {
            id: RegisterConstraintId(6),
            key: X86_64_MATERIALIZE_I64,
            operands: vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(6),
            key: X86_64_MATERIALIZE_BOOLEAN,
            operands: vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            implicit_uses: view("rflags").units.clone(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(7),
            key: X86_64_COPY_I64,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(8),
            key: X86_64_COMPARE_I64_ZERO,
            operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            implicit_uses: Vec::new(),
            implicit_defs: view("rflags").units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(9),
            key: X86_64_CONDITIONAL_BRANCH,
            operands: Vec::new(),
            implicit_uses: sorted_units(
                view("rflags")
                    .units
                    .iter()
                    .copied()
                    .chain(view("rip").units.iter().copied()),
            ),
            implicit_defs: view("rip").units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(10),
            key: X86_64_ADD_I64,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(11),
            key: X86_64_ADD_I64_IMMEDIATE,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(12),
            key: X86_64_SUBTRACT_I64,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: view("rflags").units.clone(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(13),
            key: X86_64_SUBTRACT_I64_IMMEDIATE,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(20),
            key: X86_64_MULTIPLY_I64,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: view("rflags").units.clone(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(17),
            key: X86_64_COMPARE_I64,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: view("rflags").units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(19),
            key: X86_64_COMPARE_I64_IMMEDIATE,
            operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            implicit_uses: Vec::new(),
            implicit_defs: view("rflags").units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(14),
            key: X86_64_SYSTEM_V_RETURN_UNIT,
            operands: Vec::new(),
            implicit_uses: rsp_units.clone(),
            implicit_defs: control_defs.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(15),
            key: X86_64_MICROSOFT_RETURN_UNIT,
            operands: Vec::new(),
            implicit_uses: rsp_units.clone(),
            implicit_defs: control_defs.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: X86_64_JUMP,
            operands: Vec::new(),
            implicit_uses: view("rip").units.clone(),
            implicit_defs: view("rip").units.clone(),
            clobbers: Vec::new(),
        },
    ];

    let mut saturation_output = allocatable(2, RegisterOperandAccess::Def, GPR64);
    saturation_output.early_clobber = true;
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_SATURATING_SUBTRACT_UNSIGNED,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            saturation_output,
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("rflags").units.clone(),
    });
    // The realized form zeroes RDX before reading the divisor, so the divisor
    // is pinned to RCX — a fixed view that cannot overlap the scratch — rather
    // than left allocatable over an early-clobbered RDX. The RDX scratch is an
    // ordinary late definition: the row's two outputs both define at the
    // instruction's after-point, which the fixed precolored contract accepts.
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_REMAINDER_I64,
        operands: vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("rflags").units.clone(),
    });
    // Signed wrapping division shares the signed remainder's fixed row: RAX
    // carries the dividend and result quotient, RCX owns the divisor so the
    // CQO clobber cannot consume it, and RDX is the sign-extension output.
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_DIVIDE_I64,
        operands: vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("rflags").units.clone(),
    });
    // Unsigned remainder keeps the same fixed row: the realized form zeroes
    // RDX before DIV, then moves the RDX remainder into the RAX result home.
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_REMAINDER_U64,
        operands: vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("rflags").units.clone(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_DIVIDE_U64,
        operands: vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Use, "rdx"),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: ["rdx", "rflags"]
            .into_iter()
            .flat_map(|name| view(name).units.iter().copied())
            .collect(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_SATURATING_ADD_U64,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            {
                let mut output = allocatable(2, RegisterOperandAccess::Def, GPR64);
                output.early_clobber = true;
                output
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("rflags").units.clone(),
    });
    // The clamped saturating add (every carrier but u64) and signed subtract
    // accumulate in an early-clobber result and clamp through an early-clobber
    // bound scratch (operand 3); the carrier only changes the emitted bounds.
    for key in [
        X86_64_SATURATING_ADD_CLAMPED,
        X86_64_SATURATING_SUBTRACT_CLAMPED,
    ] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                {
                    let mut output = allocatable(2, RegisterOperandAccess::Def, GPR64);
                    output.early_clobber = true;
                    output
                },
                {
                    let mut scratch = allocatable(3, RegisterOperandAccess::Def, GPR64);
                    scratch.early_clobber = true;
                    scratch
                },
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: view("rflags").units.clone(),
        });
    }
    // Like unsigned division, the explicit RDX input keeps the divisor out of
    // RDX; CQO then redefines RDX and the signed carriers reuse it as the
    // bound scratch (narrow) or the MIN / -1 guard scratch (i64).
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_SATURATING_DIVIDE_SIGNED,
        operands: vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Use, "rdx"),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: ["rdx", "rflags"]
            .into_iter()
            .flat_map(|name| view(name).units.iter().copied())
            .collect(),
    });
    let scalar_call = constraints
        .iter()
        .find(|row| row.key == X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64)
        .expect("pair call row")
        .clone();
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
            .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
            .chain([fixed(arity as u16, RegisterOperandAccess::Def, "rax")])
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
        call.operands = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
            .into_iter()
            .take(arity)
            .enumerate()
            .map(|(position, name)| fixed(position as u16, RegisterOperandAccess::Use, name))
            .chain(["rax", "rdx"].into_iter().take(fragments).enumerate().map(
                |(position, name)| {
                    fixed((arity + position) as u16, RegisterOperandAccess::Def, name)
                },
            ))
            .collect();
        // Result fragments are explicit definitions. The remaining volatile
        // units still clobber live caller values, including rdx for one fragment.
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
            .map(|(position, name)| fixed(position as u16, RegisterOperandAccess::Use, name))
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
            .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
            .chain([fixed(arity as u16, RegisterOperandAccess::Def, "rax")])
            .collect();
        constraints.push(call.clone());
        call.key = aggregate_key;
        // The returned fragment is an explicit definition, while every other
        // Microsoft volatile register retains its ordinary call clobber.
        call.clobbers
            .retain(|unit| !view("rax").write_units.contains(unit));
        constraints.push(call);
    }
    // Both direct integer returns use rax and the same RET machine-state
    // effects; the retained calling plan still distinguishes ABI policy.
    let mut returned = returned;
    returned.key = x86_64_microsoft_aggregate_return_keys()[0];
    constraints.push(returned);

    // Per-plan normalized foreign call rows: one row per (integer bank,
    // register arity, scalar-result presence). Stack-passed arguments are
    // outgoing custody, never row operands; every caller-saved register is
    // clobbered regardless of how many bank registers carry arguments. A row
    // without a scalar result leaves the ABI result register clobbered, like
    // the unit-call rows.
    for (keys, bank, call_convention) in [
        (
            x86_64_system_v_normalized_foreign_call_keys(),
            ["rdi", "rsi", "rdx", "rcx", "r8", "r9"].as_slice(),
            sysv,
        ),
        (
            x86_64_microsoft_normalized_foreign_call_keys(),
            ["rcx", "rdx", "r8", "r9"].as_slice(),
            microsoft,
        ),
    ] {
        for (index, key) in keys.into_iter().enumerate() {
            let arity = index / 2;
            let mut operands = bank[..arity]
                .iter()
                .enumerate()
                .map(|(operand, name)| {
                    fixed(
                        u16::try_from(operand).expect("operand index fits u16"),
                        RegisterOperandAccess::Use,
                        name,
                    )
                })
                .collect::<Vec<_>>();
            let mut clobbers = call_clobbers(call_convention);
            if index % 2 == 1 {
                operands.push(fixed(
                    u16::try_from(arity).expect("operand index fits u16"),
                    RegisterOperandAccess::Def,
                    "rax",
                ));
            } else {
                clobbers = sorted_units(clobbers.into_iter().chain(rax_units.iter().copied()));
            }
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: sorted_units(
                    rsp_units.iter().copied().chain(rip_units.iter().copied()),
                ),
                implicit_defs: control_defs.clone(),
                clobbers,
            });
        }
    }

    for (key, operands, uses) in [
        (
            X86_64_STORE,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
            ],
            Vec::new(),
        ),
        (
            X86_64_ADDRESS_OFFSET,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
            Vec::new(),
        ),
        (
            X86_64_LOAD8_INDEXED,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
            Vec::new(),
        ),
        (
            X86_64_LOAD64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
            Vec::new(),
        ),
        (
            X86_64_STORE64,
            vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            view("rsp").units.clone(),
        ),
        (
            X86_64_FRAME_ADDRESS,
            vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            view("rsp").units.clone(),
        ),
    ] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands,
            implicit_uses: uses,
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        });
    }
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_HOSTED_WRITE_BYTE_I32,
        operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
        implicit_uses: {
            let mut units = view("rsp").units.clone();
            units.extend(view("rip").units.iter().copied());
            units.sort_unstable();
            units.dedup();
            units
        },
        implicit_defs: Vec::new(),
        clobbers: {
            let mut units = Vec::new();
            for name in ["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"] {
                units.extend(view(name).units.iter().copied());
            }
            units.sort_unstable();
            units.dedup();
            units
        },
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_HOSTED_READ_BYTE,
        operands: Vec::new(),
        implicit_uses: {
            let mut units = view("rsp").units.clone();
            units.extend(view("rip").units.iter().copied());
            units.sort_unstable();
            units.dedup();
            units
        },
        implicit_defs: Vec::new(),
        clobbers: {
            let mut units = Vec::new();
            for name in ["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"] {
                units.extend(view(name).units.iter().copied());
            }
            units.sort_unstable();
            units.dedup();
            units
        },
    });
    for key in [X86_64_HOSTED_EXIT_PROCESS_I32] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            implicit_uses: view("rip").units.clone(),
            implicit_defs: Vec::new(),
            clobbers: {
                let mut units = Vec::new();
                for name in ["rax", "rdi", "rcx", "r11", "rflags"] {
                    units.extend(view(name).units.iter().copied());
                }
                units.sort_unstable();
                units.dedup();
                units
            },
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
        // Unit has no result operand; the ABI result register remains clobbered.
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
                allocatable(0, RegisterOperandAccess::Use, source_class),
                allocatable(1, RegisterOperandAccess::Def, destination_class),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        });
    }
    mixed_calls::append_constraints(&mut constraints, model);
    float_scalar_calls::append_constraints(&mut constraints, model);
    indirect_results::append_constraints(&mut constraints, model);
    packed_memory::append_constraints(&mut constraints, model);
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_COPY_BYTES,
        operands: (0..5)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: if operand < 3 {
                    RegisterOperandAccess::Use
                } else {
                    RegisterOperandAccess::Def
                },
                class: GPR64,
                fixed_view: None,
                tied_to: None,
                early_clobber: operand >= 3,
            })
            .collect(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("rflags").units.clone(),
    });
    mixed_aggregate_calls::append_constraints(&mut constraints, model);
    constraints.sort_by_key(|constraint| constraint.key);
    for (id, constraint) in constraints.iter_mut().enumerate() {
        constraint.id =
            RegisterConstraintId(u16::try_from(id).expect("constraint roster fits u16"));
    }
    RegisterConstraintCatalog {
        architecture: Architecture::X86_64,
        required: {
            let mut required = X86_64_REQUIRED_REGISTER_CONSTRAINTS.to_vec();
            required.extend([X86_64_LOAD_PACKED, X86_64_STORE_PACKED]);
            required.extend(x86_64_system_v_mixed_unit_call_keys());
            for microsoft in [false, true] {
                required.extend(x86_64_float_scalar_call_keys(microsoft));
                required.extend(x86_64_float_scalar_return_keys(microsoft));
                required.extend(x86_64_indirect_aggregate_call_keys(microsoft));
            }
            required.extend(x86_64_microsoft_mixed_unit_call_keys());
            required.extend(x86_64_system_v_mixed_aggregate_call_keys());
            required.extend(x86_64_microsoft_mixed_aggregate_call_keys());
            required.extend(x86_64_system_v_aggregate_call_keys());
            required.extend(x86_64_system_v_aggregate_return_keys());
            required.extend(x86_64_microsoft_aggregate_call_keys());
            required.extend(x86_64_microsoft_aggregate_return_keys());
            required.extend(x86_64_system_v_normalized_foreign_call_keys());
            required.extend(x86_64_microsoft_normalized_foreign_call_keys());
            required.sort_unstable();
            required
        },
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

impl std::fmt::Display for X86_64RegisterConstraintCatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 register constraint catalog: {self:?}"
        )
    }
}

impl std::error::Error for X86_64RegisterConstraintCatalogValidationError {}

/// Validate generic catalog structure, then independently require every row
/// to equal the x86-64 target owner's canonical semantics for that exact key.
/// This second comparison rejects class-compatible register substitutions and
/// omitted architectural clobbers that a target-neutral validator cannot name.
pub fn validate_x86_64_register_constraint_catalog(
    catalog: RegisterConstraintCatalog,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedRegisterConstraintCatalog, X86_64RegisterConstraintCatalogValidationError> {
    if model.model().architecture != Architecture::X86_64 {
        return Err(
            X86_64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch,
        );
    }
    if model.model() != &x86_64_physical_register_model() {
        return Err(X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel);
    }
    let validated = validate_register_constraint_catalog(catalog, model)
        .map_err(X86_64RegisterConstraintCatalogValidationError::Structural)?;
    let canonical = x86_64_register_constraint_catalog(model);
    // Structural validation requires strictly sorted, unique actual keys;
    // the canonical factory sorts its rows before publication.
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
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(unexpected.key),
        );
    }
    Ok(validated)
}
