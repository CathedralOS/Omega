//! The register constraint catalog of every x86-64 operation and its
//! validation against the closed inventory.

use crate::register_model::packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
use crate::register_model::physical_model::{GPR64, VECTOR128, complement};
use crate::register_model::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_ADDRESS_OFFSET, X86_64_ATOMIC_BITWISE_UPDATE,
    X86_64_ATOMIC_COMPARE_EXCHANGE, X86_64_ATOMIC_LOAD, X86_64_ATOMIC_STORE, X86_64_ATOMIC_UPDATE,
    X86_64_BITS_TO_FLOAT32, X86_64_BITS_TO_FLOAT64, X86_64_BYTE_LITERAL_IMAGE_ADDRESS,
    X86_64_COMPARE_BYTE_SEQUENCES, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_BYTES, X86_64_COPY_I64,
    X86_64_DIVIDE_I64, X86_64_DIVIDE_U64, X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS,
    X86_64_FRAME_ADDRESS, X86_64_HOSTED_EXIT_PROCESS_I32, X86_64_HOSTED_READ_BYTE,
    X86_64_HOSTED_WRITE_BYTE_I32, X86_64_INLINE_ASSEMBLY_DEFAULT, X86_64_JUMP,
    X86_64_LINUX_SYSTEM_CALL, X86_64_LOAD_ENCODED_SCALAR, X86_64_LOAD8, X86_64_LOAD8_INDEXED,
    X86_64_LOAD16, X86_64_LOAD32, X86_64_LOAD64, X86_64_MATERIALIZE_BOOLEAN,
    X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_MULTIPLY_I64, X86_64_REMAINDER_I64, X86_64_REMAINDER_U64,
    X86_64_REQUIRED_REGISTER_CONSTRAINTS, X86_64_SATURATING_ADD_CLAMPED, X86_64_SATURATING_ADD_U64,
    X86_64_SATURATING_DIVIDE_SIGNED, X86_64_SATURATING_MULTIPLY_CLAMPED,
    X86_64_SATURATING_MULTIPLY_U64, X86_64_SATURATING_SUBTRACT_CLAMPED,
    X86_64_SATURATING_SUBTRACT_UNSIGNED, X86_64_SCAN_BYTE_SEQUENCE, X86_64_SHIFT_I64, X86_64_STORE,
    X86_64_STORE_ENCODED_SCALAR, X86_64_STORE64, X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
    X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT, X86_64_TRAPPING_BINARY,
    X86_64_TRAPPING_CONVERT, X86_64_TRAPPING_FIXED_PAIR, X86_64_TRAPPING_SHIFT, X86_64_ZERO_BYTES,
    canonical_x86_64_physical_register_model_identity, validated_x86_64_physical_register_model,
    x86_64_microsoft_aggregate_call_keys, x86_64_microsoft_aggregate_return_keys,
    x86_64_microsoft_normalized_foreign_call_keys, x86_64_microsoft_register_call_keys,
    x86_64_microsoft_register_unit_call_keys, x86_64_system_v_aggregate_call_keys,
    x86_64_system_v_aggregate_return_keys, x86_64_system_v_normalized_foreign_call_keys,
    x86_64_system_v_register_call_keys, x86_64_system_v_register_unit_call_keys,
};
use crate::register_model::{X86_64_PLACED_READ, X86_64_PLACED_WRITE};
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
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog, sorted_units,
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
    // Catalog rows share a handful of implicit-unit tails; these builders mint
    // each recurring tail so a row states only its id, key and operands.
    let instruction_row = |id: RegisterConstraintId,
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
    let rflags_clobbering_row = |id, key, operands| RegisterInstructionConstraint {
        clobbers: view("rflags").units.clone(),
        ..instruction_row(id, key, operands)
    };
    let rflags_defining_row = |id, key, operands| RegisterInstructionConstraint {
        implicit_defs: view("rflags").units.clone(),
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
                allocatable(0, RegisterOperandAccess::Use, view("rax").class),
                allocatable(
                    1,
                    if read {
                        RegisterOperandAccess::Def
                    } else {
                        RegisterOperandAccess::Use
                    },
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
    let mut constraints = vec![
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
        return_row(
            RegisterConstraintId(2),
            X86_64_SYSTEM_V_RETURN,
            vec![fixed(0, RegisterOperandAccess::Use, "rax")],
        ),
        return_row(
            RegisterConstraintId(3),
            X86_64_MICROSOFT_RETURN,
            vec![fixed(0, RegisterOperandAccess::Use, "rax")],
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
        instruction_row(
            RegisterConstraintId(6),
            X86_64_MATERIALIZE_I64,
            vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
        ),
        RegisterInstructionConstraint {
            id: RegisterConstraintId(6),
            key: X86_64_MATERIALIZE_BOOLEAN,
            operands: vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            implicit_uses: view("rflags").units.clone(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        instruction_row(
            RegisterConstraintId(7),
            X86_64_COPY_I64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
        ),
        rflags_defining_row(
            RegisterConstraintId(8),
            X86_64_COMPARE_I64_ZERO,
            vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
        ),
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
        instruction_row(
            RegisterConstraintId(10),
            X86_64_ADD_I64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
        ),
        instruction_row(
            RegisterConstraintId(11),
            X86_64_ADD_I64_IMMEDIATE,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
        ),
        rflags_clobbering_row(
            RegisterConstraintId(12),
            X86_64_SUBTRACT_I64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
        ),
        instruction_row(
            RegisterConstraintId(13),
            X86_64_SUBTRACT_I64_IMMEDIATE,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
        ),
        rflags_clobbering_row(
            RegisterConstraintId(20),
            X86_64_MULTIPLY_I64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
                allocatable(2, RegisterOperandAccess::Def, GPR64),
            ],
        ),
        rflags_defining_row(
            RegisterConstraintId(17),
            X86_64_COMPARE_I64,
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
            ],
        ),
        rflags_defining_row(
            RegisterConstraintId(19),
            X86_64_COMPARE_I64_IMMEDIATE,
            vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
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

    let mut saturation_output = allocatable(2, RegisterOperandAccess::Def, GPR64);
    saturation_output.early_clobber = true;
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_SATURATING_SUBTRACT_UNSIGNED,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            saturation_output,
        ],
    ));
    // The realized form zeroes RDX before reading the divisor, so the divisor
    // is pinned to RCX — a fixed view that cannot overlap the scratch — rather
    // than left allocatable over an early-clobbered RDX. The RDX scratch is an
    // ordinary late definition: the row's two outputs both define at the
    // instruction's after-point, which the fixed precolored contract accepts.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_REMAINDER_I64,
        vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
    ));
    // Signed wrapping division shares the signed remainder's fixed row: RAX
    // carries the dividend and result quotient, RCX owns the divisor so the
    // CQO clobber cannot consume it, and RDX is the sign-extension output.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_DIVIDE_I64,
        vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
    ));
    // Unsigned remainder keeps the same fixed row: the realized form zeroes
    // RDX before DIV, then moves the RDX remainder into the RAX result home.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_REMAINDER_U64,
        vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
    ));
    // Variable shifts read the count through CL, so the count operand is
    // pinned to RCX. The result is an early-clobber definition: it is written
    // by the value copy before the shift reads CL, so it can share neither
    // the live count home nor the value home it is initialized from.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_SHIFT_I64,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            {
                let mut output = allocatable(2, RegisterOperandAccess::Def, GPR64);
                output.early_clobber = true;
                output
            },
        ],
    ));
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
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_SATURATING_ADD_U64,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            {
                let mut output = allocatable(2, RegisterOperandAccess::Def, GPR64);
                output.early_clobber = true;
                output
            },
        ],
    ));
    // The clamped saturating add and multiply (every carrier but u64) and
    // signed subtract accumulate in an early-clobber result and clamp through
    // an early-clobber bound scratch (operand 3); the carrier only changes the
    // emitted bounds. The trapping add, subtract, and multiply share the row:
    // the result accumulates while both inputs are live, and the narrow range
    // check writes the scratch before the inputs are dead.
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
    // Saturating u64 multiplication shares the unsigned remainder's fixed row:
    // MUL reads RAX and the RCX-pinned right operand and defines the RDX high
    // half, whose overflow borrow then saturates RAX.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_SATURATING_MULTIPLY_U64,
        vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
    ));
    // Trapping division, remainder, and u64 multiplication share the same
    // fixed row: the guards read the RCX-pinned divisor before RDX is
    // defined by the XOR, CQO, or MUL high half, and the result is RAX.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_TRAPPING_FIXED_PAIR,
        vec![
            fixed(0, RegisterOperandAccess::Use, "rax"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "rax"),
            fixed(3, RegisterOperandAccess::Def, "rdx"),
        ],
    ));
    // Trapping shifts read the count through CL like the wrapping shifts, and
    // add an early-clobber scratch for the left shift's round-trip or range
    // check, which runs while the value and count are still live.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_TRAPPING_SHIFT,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
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
    ));
    // A trapping conversion checks its input through the scratch before the
    // result copy, so both outputs are early-clobber against the input.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_TRAPPING_CONVERT,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            {
                let mut output = allocatable(1, RegisterOperandAccess::Def, GPR64);
                output.early_clobber = true;
                output
            },
            {
                let mut scratch = allocatable(2, RegisterOperandAccess::Def, GPR64);
                scratch.early_clobber = true;
                scratch
            },
        ],
    ));
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
            vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                RegisterOperandConstraint {
                    early_clobber: true,
                    ..allocatable(1, RegisterOperandAccess::Def, GPR64)
                },
            ],
            view("rsp").units.clone(),
        ),
        (
            X86_64_FRAME_ADDRESS,
            vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            view("rsp").units.clone(),
        ),
        (
            X86_64_BYTE_LITERAL_IMAGE_ADDRESS,
            vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            Vec::new(),
        ),
    ] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands,
            implicit_uses: uses,
            implicit_defs: Vec::new(),
            // Absolute frame-address rounding uses AND. Advertise its flags
            // before scheduling even when this particular home needs no mask.
            clobbers: if matches!(key, X86_64_STORE64 | X86_64_FRAME_ADDRESS) {
                view("rflags").units.clone()
            } else {
                Vec::new()
            },
        });
    }
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_HOSTED_WRITE_BYTE_I32,
        operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
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
    for key in [X86_64_HOSTED_EXIT_PROCESS_I32] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
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
    // Scalar IEEE operations read MXCSR's rounding controls and clobber their
    // scratch vectors (ternary forms also hold their addend in xmm3); the
    // effect declaration in
    // `selected_form_encoding::ieee_float_arithmetic` restates both rows.
    for (key, inputs) in [
        (crate::X86_64_IEEE_FLOAT_ARITHMETIC, 2),
        (crate::X86_64_IEEE_FLOAT_DIRECTED_BINARY, 2),
        (crate::X86_64_IEEE_FLOAT_DIRECTED_UNARY, 1),
        (crate::X86_64_IEEE_FLOAT_UNARY, 1),
        (crate::X86_64_IEEE_FLOAT_TERNARY, 3),
    ] {
        let mut operands = (0..inputs)
            .map(|index| allocatable(index, RegisterOperandAccess::Use, GPR64))
            .collect::<Vec<_>>();
        operands.push(allocatable(inputs, RegisterOperandAccess::Def, GPR64));
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
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: sorted_units(view("mxcsr").units.iter().copied()),
        implicit_defs: Vec::new(),
        clobbers: sorted_units(
            view("xmm4")
                .units
                .iter()
                .chain(&view("xmm5").units)
                .chain(&view("rflags").units)
                .copied(),
        ),
    });
    // Capture every input before the output becomes integer scratch, allowing
    // output/input aliasing without an undeclared general-register clobber.
    for (key, inputs) in [
        (crate::X86_64_FLOAT_RESULT_TRAPPING, 1),
        (crate::X86_64_FLOAT_RESULT_SATURATING, 2),
    ] {
        let scratch: &[&str] = if inputs == 1 {
            &["xmm3", "rflags"]
        } else {
            &["xmm3", "xmm4", "rflags"]
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
    mixed_calls::append_constraints(&mut constraints, model);
    mixed_calls::append_normalized_foreign_constraints(&mut constraints, model);
    super::system_calls::append_constraints(&mut constraints, model);
    float_scalar_calls::append_constraints(&mut constraints, model);
    indirect_results::append_constraints(&mut constraints, model);
    packed_memory::append_constraints(&mut constraints, model);
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_COPY_BYTES,
        (0..5)
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
    ));
    // The zero fill reads its preserved destination/count inputs on every
    // iteration; the cursor is written before the count guard reads them.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_ZERO_BYTES,
        (0..3)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: if operand < 2 {
                    RegisterOperandAccess::Use
                } else {
                    RegisterOperandAccess::Def
                },
                class: GPR64,
                fixed_view: None,
                tied_to: None,
                early_clobber: operand >= 2,
            })
            .collect(),
    ));
    // Scalar codecs carry the actual byte footprint. Every working home is
    // early-clobber, so neither input nor another scratch home can alias it.
    for (key, input_count) in [
        (X86_64_LOAD_ENCODED_SCALAR, 2),
        (X86_64_STORE_ENCODED_SCALAR, 3),
    ] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: (0..6)
                .map(|operand| {
                    let mut row = allocatable(
                        operand,
                        if operand < input_count {
                            RegisterOperandAccess::Use
                        } else {
                            RegisterOperandAccess::Def
                        },
                        GPR64,
                    );
                    row.early_clobber = operand >= input_count;
                    row
                })
                .collect(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: view("rflags").units.clone(),
        });
    }
    // The scan reads its preserved base/count inputs while the loop cursors
    // are written before any input home is consumed; the verdict operand is
    // the only lasting definition. Flags fall to the compare/branch pair.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_SCAN_BYTE_SEQUENCE,
        (0..5)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: match operand {
                    0 | 1 => RegisterOperandAccess::Use,
                    _ => RegisterOperandAccess::Def,
                },
                class: GPR64,
                fixed_view: None,
                tied_to: None,
                early_clobber: matches!(operand, 2 | 3 | 4),
            })
            .collect(),
    ));
    // The compare reads its four preserved inputs while the cursors are
    // written before any input home is consumed; the verdict operand is the
    // only lasting definition. Flags fall to the compare/branch pairs.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_COMPARE_BYTE_SEQUENCES,
        (0..8)
            .map(|operand| RegisterOperandConstraint {
                operand,
                access: match operand {
                    0..=3 => RegisterOperandAccess::Use,
                    _ => RegisterOperandAccess::Def,
                },
                class: GPR64,
                fixed_view: None,
                tied_to: None,
                early_clobber: operand >= 4,
            })
            .collect(),
    ));
    mixed_aggregate_calls::append_constraints(&mut constraints, model);
    // Atomic rows carry the LOCK-capable vocabulary every x86-64 target has
    // had since the 486. The address is an ordinary base register; the cell
    // it points at belongs to the memory effect, not this operand roster.
    // `mov` plus `mfence` is the fixed-work store because `xchg` would
    // destroy the incoming value register. Bitwise fetch has its own row:
    // its compare-exchange retry control and RAX interference are explicit. The
    // observed prior value is an early-clobber definition on every row it
    // appears in, the same shape every other target's atomic rows carry.
    constraints.push(instruction_row(
        RegisterConstraintId(0),
        X86_64_ATOMIC_LOAD,
        vec![allocatable(0, RegisterOperandAccess::Use, GPR64), {
            let mut output = allocatable(1, RegisterOperandAccess::Def, GPR64);
            output.early_clobber = true;
            output
        }],
    ));
    constraints.push(instruction_row(
        RegisterConstraintId(0),
        X86_64_ATOMIC_STORE,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
        ],
    ));
    // The update family reads the address and the incoming value and
    // produces the observed prior cell. `LOCK XADD` and `XCHG` write the
    // observed value into the register they read, so the realization copies
    // the incoming value into the early-clobber output before the LOCK form.
    constraints.push(rflags_clobbering_row(
        RegisterConstraintId(0),
        X86_64_ATOMIC_UPDATE,
        vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            {
                let mut output = allocatable(2, RegisterOperandAccess::Def, GPR64);
                output.early_clobber = true;
                output
            },
        ],
    ));
    // CMPXCHG's hidden accumulator must not overwrite either input while a
    // failed attempt prepares its next replacement. The current constraint
    // vocabulary has no allocatable-class exclusion for RAX, so fixed
    // caller-volatile homes make that exclusion explicit through ordinary
    // fixed-view copies, without changing the physical register model or
    // reserving an undeclared scratch. R8 is the distinct fixed replacement
    // and final observed-prior home; no input is modified by the encoding.
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_ATOMIC_BITWISE_UPDATE,
        operands: vec![
            fixed(0, RegisterOperandAccess::Use, "rdx"),
            fixed(1, RegisterOperandAccess::Use, "rcx"),
            fixed(2, RegisterOperandAccess::Def, "r8"),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: {
            let mut clobbers = view("rax").units.clone();
            clobbers.extend(view("rflags").units.iter().copied());
            clobbers.sort_unstable();
            clobbers
        },
    });
    // CMPXCHG reads the expected input from RAX and leaves the observed
    // prior cell in it: the expected operand pins RAX, the observed output
    // is an ordinary early-clobber register the encoding fills from RAX
    // after the LOCK form, and RAX itself is a declared family clobber.
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: X86_64_ATOMIC_COMPARE_EXCHANGE,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            fixed(1, RegisterOperandAccess::Use, "rax"),
            allocatable(2, RegisterOperandAccess::Use, GPR64),
            {
                let mut output = allocatable(3, RegisterOperandAccess::Def, GPR64);
                output.early_clobber = true;
                output
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: {
            let mut clobbers = view("rax").units.clone();
            clobbers.extend(view("rflags").units.iter().copied());
            clobbers.sort_unstable();
            clobbers
        },
    });
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
        (X86_64_PLACED_READ, X86_64_LOAD32),
        (X86_64_PLACED_WRITE, X86_64_STORE),
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
            required.extend(super::system_calls::x86_64_linux_system_call_keys());
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
    if model.identity() != canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel);
    }
    let validated = validate_register_constraint_catalog(catalog, model)
        .map_err(X86_64RegisterConstraintCatalogValidationError::Structural)?;
    let canonical = x86_64_register_constraint_catalog_for(model);
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

/// Borrow the canonical catalog when `model` is the canonical x86-64 model,
/// and build a fresh owned catalog for any other validated model.
///
/// The catalog is a pure function of the validated model. Encode and
/// validate paths run once per instruction row; reusing the process-wide
/// canonical instance keeps each row at a hash lookup instead of rebuilding
/// the full inventory.
pub(crate) fn x86_64_register_constraint_catalog_for(
    model: &ValidatedPhysicalRegisterModel,
) -> std::borrow::Cow<'static, RegisterConstraintCatalog> {
    static CANONICAL: std::sync::OnceLock<RegisterConstraintCatalog> = std::sync::OnceLock::new();
    if model.identity() == canonical_x86_64_physical_register_model_identity() {
        std::borrow::Cow::Borrowed(CANONICAL.get_or_init(|| {
            x86_64_register_constraint_catalog(validated_x86_64_physical_register_model())
        }))
    } else {
        std::borrow::Cow::Owned(x86_64_register_constraint_catalog(model))
    }
}
