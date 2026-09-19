//! The register constraint catalog of every AArch64 operation and its
//! validation against the closed inventory.

use crate::register_model::physical_model::{FLOAT64, GPR64, complement, sorted_units};
use crate::register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE,
    AARCH64_ADDRESS_OFFSET, AARCH64_BITS_TO_FLOAT32, AARCH64_BITS_TO_FLOAT64, AARCH64_COMPARE_I64,
    AARCH64_COMPARE_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_BYTES, AARCH64_COPY_I64, AARCH64_DARWIN_CALL,
    AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32, AARCH64_DARWIN_HOSTED_READ_BYTE,
    AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
    AARCH64_DIVIDE_I64, AARCH64_DIVIDE_U64, AARCH64_FLOAT32_TO_BITS, AARCH64_FLOAT64_TO_BITS,
    AARCH64_FRAME_ADDRESS, AARCH64_HOSTED_EXIT_PROCESS_I32, AARCH64_HOSTED_READ_BYTE,
    AARCH64_HOSTED_WRITE_BYTE_I32, AARCH64_INLINE_ASSEMBLY_DEFAULT, AARCH64_JUMP,
    AARCH64_LINUX_SYSTEM_CALL, AARCH64_LOAD_PACKED, AARCH64_LOAD8, AARCH64_LOAD8_INDEXED,
    AARCH64_LOAD16, AARCH64_LOAD32, AARCH64_LOAD64, AARCH64_MATERIALIZE_BOOLEAN,
    AARCH64_MATERIALIZE_I64, AARCH64_MULTIPLY_I64, AARCH64_REMAINDER_I64, AARCH64_REMAINDER_U64,
    AARCH64_REQUIRED_REGISTER_CONSTRAINTS, AARCH64_SATURATING_ADD_CLAMPED,
    AARCH64_SATURATING_ADD_U64, AARCH64_SATURATING_DIVIDE_SIGNED,
    AARCH64_SATURATING_SUBTRACT_CLAMPED, AARCH64_SATURATING_SUBTRACT_UNSIGNED, AARCH64_STORE,
    AARCH64_STORE_PACKED, AARCH64_STORE64, AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
    aarch64_aapcs64_normalized_foreign_call_keys, aarch64_aapcs64_register_call_keys,
    aarch64_aapcs64_register_unit_call_keys, aarch64_darwin_normalized_foreign_call_keys,
    aarch64_darwin_register_call_keys, aarch64_darwin_register_unit_call_keys,
    aarch64_physical_register_model, aarch64_register_aggregate_call_keys,
    aarch64_register_aggregate_return_keys,
};
use crate::register_model::{float_scalar_calls, indirect_results, mixed_calls};
use crate::{
    aarch64_aapcs64_mixed_unit_call_keys, aarch64_darwin_mixed_unit_call_keys,
    aarch64_float_scalar_call_keys, aarch64_float_scalar_return_keys,
    aarch64_indirect_aggregate_call_keys, aarch64_mixed_aggregate_call_keys,
};
use register_model::{
    PreservationConvention, RegisterClassId, RegisterConstraintCatalog,
    RegisterConstraintCatalogValidationError, RegisterConstraintId, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
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

/// Build the bounded AArch64 constraint catalog against an independently
/// validated physical model. Call rows describe the currently represented
/// scalar ABI register bank; they do not claim complete aggregate, vector, or
/// feature-specific instruction coverage.
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

    let mut constraints = vec![
        RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: AARCH64_AAPCS64_CALL,
            operands: call_operands(),
            implicit_uses: call_uses.clone(),
            implicit_defs: call_defs.clone(),
            clobbers: call_clobbers(aapcs),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(1),
            key: AARCH64_DARWIN_CALL,
            operands: call_operands(),
            implicit_uses: call_uses.clone(),
            implicit_defs: call_defs.clone(),
            clobbers: call_clobbers(darwin),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(17),
            key: AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64,
            operands: vec![
                fixed(0, RegisterOperandAccess::Use, "x0"),
                fixed(1, RegisterOperandAccess::Use, "x1"),
                fixed(2, RegisterOperandAccess::Def, "x0"),
            ],
            implicit_uses: call_uses.clone(),
            implicit_defs: call_defs.clone(),
            clobbers: call_clobbers(aapcs),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(2),
            key: AARCH64_AAPCS64_RETURN,
            operands: vec![fixed(0, RegisterOperandAccess::Use, "x0")],
            implicit_uses: return_uses.clone(),
            implicit_defs: pc_units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(3),
            key: AARCH64_DARWIN_RETURN,
            operands: vec![fixed(0, RegisterOperandAccess::Use, "x0")],
            implicit_uses: return_uses.clone(),
            implicit_defs: pc_units.clone(),
            clobbers: Vec::new(),
        },
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
        RegisterInstructionConstraint {
            id: RegisterConstraintId(6),
            key: AARCH64_MATERIALIZE_I64,
            operands: vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(6),
            key: AARCH64_MATERIALIZE_BOOLEAN,
            operands: vec![allocatable(0, RegisterOperandAccess::Def, GPR64)],
            implicit_uses: view("nzcv").units.clone(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(7),
            key: AARCH64_COPY_I64,
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
            key: AARCH64_COMPARE_I64_ZERO,
            operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            implicit_uses: Vec::new(),
            implicit_defs: view("nzcv").units.clone(),
            clobbers: Vec::new(),
        },
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
        RegisterInstructionConstraint {
            id: RegisterConstraintId(10),
            key: AARCH64_ADD_I64,
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
            key: AARCH64_ADD_I64_IMMEDIATE,
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
            key: AARCH64_SUBTRACT_I64,
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
            id: RegisterConstraintId(13),
            key: AARCH64_SUBTRACT_I64_IMMEDIATE,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Def, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(16),
            key: AARCH64_COMPARE_I64,
            operands: vec![
                allocatable(0, RegisterOperandAccess::Use, GPR64),
                allocatable(1, RegisterOperandAccess::Use, GPR64),
            ],
            implicit_uses: Vec::new(),
            implicit_defs: view("nzcv").units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(18),
            key: AARCH64_COMPARE_I64_IMMEDIATE,
            operands: vec![allocatable(0, RegisterOperandAccess::Use, GPR64)],
            implicit_uses: Vec::new(),
            implicit_defs: view("nzcv").units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(14),
            key: AARCH64_AAPCS64_RETURN_UNIT,
            operands: Vec::new(),
            implicit_uses: return_uses.clone(),
            implicit_defs: pc_units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(15),
            key: AARCH64_DARWIN_RETURN_UNIT,
            operands: Vec::new(),
            implicit_uses: return_uses,
            implicit_defs: pc_units.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key: AARCH64_JUMP,
            operands: Vec::new(),
            implicit_uses: view("pc").units.clone(),
            implicit_defs: view("pc").units.clone(),
            clobbers: Vec::new(),
        },
    ];

    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_LOAD64,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_LOAD8_INDEXED,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            allocatable(2, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    });
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
    for (key, access) in [
        (AARCH64_STORE64, RegisterOperandAccess::Use),
        (AARCH64_FRAME_ADDRESS, RegisterOperandAccess::Def),
    ] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: vec![allocatable(0, access, GPR64)],
            implicit_uses: view("sp").units.clone(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
        });
    }
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
                    |(index, name)| fixed((arity + index) as u16, RegisterOperandAccess::Def, name),
                ))
                .collect();
            // Each returned fragment is an explicit post-call definition, not
            // an unknown architectural clobber competing for the same point.
            // Keep all caller-saved units outside the actual result untouched.
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

    // Per-plan normalized foreign call rows: one row per (integer bank,
    // register arity, scalar-result presence). Stack-passed arguments are
    // outgoing custody, never row operands; every caller-saved register is
    // clobbered regardless of how many bank registers carry arguments. A row
    // without a scalar result leaves X0 clobbered, like the unit-call rows.
    for (keys, call_convention) in [
        (aarch64_aapcs64_normalized_foreign_call_keys(), aapcs),
        (aarch64_darwin_normalized_foreign_call_keys(), darwin),
    ] {
        for (index, key) in keys.into_iter().enumerate() {
            let arity = index / 2;
            let mut operands = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
                .into_iter()
                .take(arity)
                .enumerate()
                .map(|(index, name)| fixed(index as u16, RegisterOperandAccess::Use, name))
                .collect::<Vec<_>>();
            let mut clobbers = call_clobbers(call_convention);
            if index % 2 == 1 {
                operands.push(fixed(arity as u16, RegisterOperandAccess::Def, "x0"));
            } else {
                clobbers = sorted_units(clobbers.into_iter().chain(x0_units.iter().copied()));
            }
            constraints.push(RegisterInstructionConstraint {
                id: RegisterConstraintId(0),
                key,
                operands,
                implicit_uses: call_uses.clone(),
                implicit_defs: call_defs.clone(),
                clobbers,
            });
        }
    }

    for (key, syscall_register) in [
        (AARCH64_HOSTED_READ_BYTE, "x8"),
        (AARCH64_DARWIN_HOSTED_READ_BYTE, "x16"),
    ] {
        constraints.push(RegisterInstructionConstraint {
            id: RegisterConstraintId(0),
            key,
            operands: Vec::new(),
            implicit_uses: {
                let mut units = view("sp").units.clone();
                units.extend(view("pc").units.iter().copied());
                units.sort_unstable();
                units.dedup();
                units
            },
            implicit_defs: Vec::new(),
            clobbers: {
                let mut units = Vec::new();
                for name in ["x0", "x1", "x2", syscall_register, "x9", "nzcv"] {
                    units.extend(view(name).units.iter().copied());
                }
                units.sort_unstable();
                units.dedup();
                units
            },
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
            implicit_uses: {
                let mut units = view("sp").units.clone();
                units.extend(view("pc").units.iter().copied());
                units.sort_unstable();
                units.dedup();
                units
            },
            implicit_defs: Vec::new(),
            clobbers: {
                let mut units = Vec::new();
                for name in ["x0", "x1", "x2", syscall_register, "nzcv"] {
                    units.extend(view(name).units.iter().copied());
                }
                units.sort_unstable();
                units.dedup();
                units
            },
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
            clobbers: {
                let mut units = Vec::new();
                for name in ["x0", syscall_register, "nzcv"] {
                    units.extend(view(name).units.iter().copied());
                }
                units.sort_unstable();
                units.dedup();
                units
            },
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
        // Unit has no result operand; X0 remains caller-clobbered.
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
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_COPY_BYTES,
        operands: (0..5)
            .map(|operand| {
                let mut row = allocatable(
                    operand,
                    if operand < 3 {
                        RegisterOperandAccess::Use
                    } else {
                        RegisterOperandAccess::Def
                    },
                    GPR64,
                );
                row.early_clobber = operand >= 3;
                row
            })
            .collect(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: view("nzcv").units.clone(),
    });
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
        let mut scratch = allocatable(2, RegisterOperandAccess::Def, GPR64);
        scratch.early_clobber = true;
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
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_SATURATING_SUBTRACT_UNSIGNED,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            allocatable(2, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: view("nzcv").units.clone(),
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_REMAINDER_I64,
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
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_DIVIDE_I64,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            allocatable(2, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_REMAINDER_U64,
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
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_DIVIDE_U64,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            allocatable(2, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_MULTIPLY_I64,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            allocatable(2, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    });
    constraints.push(RegisterInstructionConstraint {
        id: RegisterConstraintId(0),
        key: AARCH64_SATURATING_ADD_U64,
        operands: vec![
            allocatable(0, RegisterOperandAccess::Use, GPR64),
            allocatable(1, RegisterOperandAccess::Use, GPR64),
            allocatable(2, RegisterOperandAccess::Def, GPR64),
        ],
        implicit_uses: Vec::new(),
        implicit_defs: view("nzcv").units.clone(),
        clobbers: Vec::new(),
    });
    // The clamped saturating forms clamp through a bound scratch (operand 3).
    // Both outputs are early-clobber: liveness admits independent early
    // outputs only when every definition of the instruction is early, which
    // keeps the scratch distinct from the inputs and from the result.
    for key in [
        AARCH64_SATURATING_ADD_CLAMPED,
        AARCH64_SATURATING_SUBTRACT_CLAMPED,
        AARCH64_SATURATING_DIVIDE_SIGNED,
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
            implicit_defs: view("nzcv").units.clone(),
            clobbers: Vec::new(),
        });
    }
    mixed_calls::append_constraints(&mut constraints, model);
    float_scalar_calls::append_constraints(&mut constraints, model);
    indirect_results::append_constraints(&mut constraints, model);
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
            required.sort_unstable();
            required
        },
        constraints,
    }
}

/// Apply generic structural validation and then independently compare every
/// keyed row with the AArch64 target owner's canonical semantics.
pub fn validate_aarch64_register_constraint_catalog(
    catalog: RegisterConstraintCatalog,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedRegisterConstraintCatalog, Aarch64RegisterConstraintCatalogValidationError> {
    if model.model().architecture != Architecture::Aarch64 {
        return Err(
            Aarch64RegisterConstraintCatalogValidationError::PhysicalModelArchitectureMismatch,
        );
    }
    if model.model() != &aarch64_physical_register_model() {
        return Err(Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel);
    }
    let validated = validate_register_constraint_catalog(catalog, model)
        .map_err(Aarch64RegisterConstraintCatalogValidationError::Structural)?;
    let canonical = aarch64_register_constraint_catalog(model);
    // Structural validation requires strictly sorted, unique actual keys;
    // the canonical factory sorts its rows before publication.
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
