use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::MachineRegister;
use register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterReservationOverlay, RegisterUnit,
    RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    ReservationReason, ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_register_constraint_catalog,
};
use target::{Architecture, NativeTarget, ObjectFormat};

#[cfg(test)]
mod float_transport_tests;
mod mixed_calls;
pub use mixed_calls::*;

pub const AARCH64_LOAD8: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 730,
};
pub const AARCH64_LOAD16: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 731,
};
pub const AARCH64_LOAD32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 714,
};
pub const AARCH64_FLOAT32_TO_BITS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 710,
};
pub const AARCH64_FLOAT64_TO_BITS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 711,
};
pub const AARCH64_BITS_TO_FLOAT32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 712,
};
pub const AARCH64_BITS_TO_FLOAT64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 713,
};

const GPR64: RegisterClassId = RegisterClassId(0);
const GPR32: RegisterClassId = RegisterClassId(1);
const VECTOR128: RegisterClassId = RegisterClassId(2);
const FLOAT64: RegisterClassId = RegisterClassId(3);
const FLOAT32: RegisterClassId = RegisterClassId(4);
const FLAGS: RegisterClassId = RegisterClassId(5);
const STACK64: RegisterClassId = RegisterClassId(6);
const STACK32: RegisterClassId = RegisterClassId(7);
const ZERO: RegisterClassId = RegisterClassId(8);
const FLOAT_CONTROL: RegisterClassId = RegisterClassId(9);
const INSTRUCTION_POINTER: RegisterClassId = RegisterClassId(10);

/// Resolve one ABI-visible register through the AArch64 target owner's
/// canonical model.
pub fn aarch64_fixed_register_view(
    model: &ValidatedPhysicalRegisterModel,
    register: MachineRegister,
) -> Option<RegisterViewId> {
    if model.model() != &aarch64_physical_register_model() {
        return None;
    }
    let name = match register {
        MachineRegister::Aarch64X(index @ 0..=30) => format!("x{index}"),
        MachineRegister::Aarch64V(index @ 0..=31) => format!("d{index}"),
        _ => return None,
    };
    model.model().view_named(&name).map(|view| view.id)
}

/// Resolve the exact preservation convention selected by the clean terminal
/// lane for one supported AArch64 target. The ISA owner, rather than generic
/// orchestration, owns this target/object-format to ABI-policy mapping.
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
    model
        .model()
        .conventions
        .iter()
        .find(|convention| convention.name == name)
}

pub const AARCH64_AAPCS64_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 0,
};
pub const AARCH64_DARWIN_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 1,
};
/// Arity-ordered keys for the darwin register-only U64 call ABI.
pub fn aarch64_darwin_register_call_keys() -> Vec<RegisterConstraintKey> {
    (11..=19)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Complete integer-bank result fragments, ordered by fragment count then input arity.
pub fn aarch64_register_aggregate_call_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 1020 } else { 1000 };
    (first..first + 18)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

pub fn aarch64_register_aggregate_return_keys(darwin: bool) -> Vec<RegisterConstraintKey> {
    let first = if darwin { 12 } else { 10 };
    (first..first + 2)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Return,
            variant,
        })
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn aarch64_aapcs64_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (700..=708)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Register-only Unit call keys, indexed by argument count.
pub fn aarch64_darwin_register_unit_call_keys() -> Vec<RegisterConstraintKey> {
    (720..=728)
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Arity-ordered keys for the complete register-only U64 call ABI.
pub fn aarch64_aapcs64_register_call_keys() -> Vec<RegisterConstraintKey> {
    [3, 4, 2, 5, 6, 7, 8, 9, 10]
        .into_iter()
        .map(|variant| RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant,
        })
        .collect()
}

/// Exact Linux AAPCS64 scalar call with two U64 arguments and one U64 result.
pub const AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 2,
};
pub const AARCH64_AAPCS64_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 0,
};
pub const AARCH64_DARWIN_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 1,
};
pub const AARCH64_AAPCS64_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 2,
};
pub const AARCH64_DARWIN_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 3,
};
pub const AARCH64_LINUX_SYSTEM_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::SystemCall,
    variant: 0,
};
pub const AARCH64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::InlineAssembly,
    variant: 0,
};
pub const AARCH64_MATERIALIZE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 0,
};
pub const AARCH64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 1,
};
pub const AARCH64_COMPARE_I64_ZERO: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 2,
};
pub const AARCH64_CONDITIONAL_BRANCH: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 3,
};
/// Flag-transparent three-address i64 addition, matching the ordinary AArch64
/// register ADD form.
pub const AARCH64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 4,
};
/// Flag-transparent `result = left + immediate`, matching the AArch64 ADD
/// immediate form for the named admitted immediate domain.
pub const AARCH64_ADD_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 5,
};
/// Flag-transparent three-address exact i64 subtraction, matching the
/// ordinary AArch64 `SUB` register form.
pub const AARCH64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 6,
};
/// Flag-transparent `result = left - immediate`, matching the AArch64 SUB
/// immediate form for the named admitted U12 domain.
pub const AARCH64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 7,
};
/// Two-input i64 comparison. Both operands are read and NZCV is defined.
pub const AARCH64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 8,
};

/// Unconditional relative control without a condition-register dependency.
pub const AARCH64_JUMP: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 9,
};

/// Process exit using the Linux syscall register convention.
pub const AARCH64_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 715,
};
pub const AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 716,
};

pub const AARCH64_DARWIN_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 718,
};

pub const AARCH64_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 717,
};

pub const AARCH64_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 14,
};
/// The same hosted byte operation using Darwin's syscall register convention.
pub const AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 17,
};

pub const AARCH64_LOAD64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 10,
};
/// One byte read through a base and runtime index, zero extended to 64 bits.
pub const AARCH64_LOAD8_INDEXED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 11,
};
pub const AARCH64_STORE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 15,
};
pub const AARCH64_ADDRESS_OFFSET: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 16,
};
pub const AARCH64_STORE64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 12,
};
pub const AARCH64_FRAME_ADDRESS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 13,
};

/// Closed baseline constraint inventory owned by the AArch64 target.
/// Includes scalar control, arithmetic, calls, and pointer loads; other
/// ordinary and feature-specific instruction rows remain absent.
pub const AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 73] = [
    AARCH64_AAPCS64_CALL,
    AARCH64_DARWIN_CALL,
    AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64,
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 3,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 4,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 5,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 6,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 7,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 8,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 9,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 10,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 11,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 12,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 13,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 14,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 15,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 16,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 17,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 18,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 19,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 700,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 701,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 702,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 703,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 704,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 705,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 706,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 707,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 708,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 720,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 721,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 722,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 723,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 724,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 725,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 726,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 727,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 728,
    },
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
];

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

struct ModelBuilder {
    units: Vec<RegisterUnit>,
    views: Vec<RegisterView>,
    classes: Vec<RegisterClass>,
}

impl ModelBuilder {
    fn new() -> Self {
        Self {
            units: Vec::new(),
            views: Vec::new(),
            classes: [
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
            ]
            .into_iter()
            .enumerate()
            .map(|(id, name)| RegisterClass {
                id: RegisterClassId(u16::try_from(id).expect("class id fits u16")),
                name: name.into(),
                views: Vec::new(),
            })
            .collect(),
        }
    }

    fn unit(&mut self, name: String, bits: u16, kind: RegisterUnitKind) -> RegisterUnitId {
        let id = RegisterUnitId(u16::try_from(self.units.len()).expect("unit id fits u16"));
        self.units.push(RegisterUnit {
            id,
            name,
            bits,
            kind,
        });
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn view(
        &mut self,
        name: String,
        class: RegisterClassId,
        units: Vec<RegisterUnitId>,
        bits: u16,
        write_semantics: RegisterWriteSemantics,
        allocatable: bool,
    ) -> RegisterViewId {
        let id = RegisterViewId(u16::try_from(self.views.len()).expect("view id fits u16"));
        self.views.push(RegisterView {
            id,
            name,
            class,
            write_units: units.clone(),
            units,
            bits,
            write_semantics,
            allocatable,
        });
        self.classes[usize::from(class.0)].views.push(id);
        id
    }
}

pub fn aarch64_physical_register_model() -> PhysicalRegisterModel {
    let mut builder = ModelBuilder::new();
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
            64,
            RegisterWriteSemantics::ExactView,
            true,
        );
        let w_view = builder.view(
            w.clone(),
            GPR32,
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
        64,
        RegisterWriteSemantics::ExactView,
        false,
    );
    let wsp_view = builder.view(
        "wsp".into(),
        STACK32,
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
        64,
        RegisterWriteSemantics::Discards,
        false,
    );
    let wzr_view = builder.view(
        "wzr".into(),
        ZERO,
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
            128,
            RegisterWriteSemantics::ExactView,
            true,
        );
        let d_view = builder.view(
            d.clone(),
            FLOAT64,
            vec![low],
            64,
            RegisterWriteSemantics::InstructionDefined,
            true,
        );
        // Scalar FMOV destinations clear the rest of the architectural vector.
        // Normalized scalar transports therefore interfere with both lanes.
        builder.views[usize::from(d_view.0)].write_units = vec![low, high];
        let s_view = builder.view(
            s.clone(),
            FLOAT32,
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
        64,
        RegisterWriteSemantics::InstructionDefined,
        false,
    );
    named_views.insert("pc".into(), pc_view);

    let all_units = builder.units.iter().map(|unit| unit.id).collect::<Vec<_>>();
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
    let reservations = vec![
        overlay(
            "aarch64.stack-pointer",
            ReservationReason::StackPointer,
            vec![sp_unit],
        ),
        overlay(
            "aarch64.zero-register",
            ReservationReason::Architectural,
            vec![zero_unit],
        ),
        overlay(
            "aarch64.instruction-pointer",
            ReservationReason::Architectural,
            vec![pc_unit],
        ),
        overlay(
            "aarch64.frame-pointer",
            ReservationReason::FramePointer,
            vec![named_units["x29"]],
        ),
        overlay(
            "darwin.aarch64.platform",
            ReservationReason::Platform,
            vec![x18],
        ),
        overlay(
            "omega.aarch64.dispatch",
            ReservationReason::Dispatch,
            vec![named_units["x27"]],
        ),
        overlay(
            "omega.aarch64.metering",
            ReservationReason::Metering,
            vec![named_units["x28"]],
        ),
    ];
    PhysicalRegisterModel {
        architecture: Architecture::Aarch64,
        units: builder.units,
        views: builder.views,
        classes: builder.classes,
        conventions,
        reservations,
    }
}

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
    mixed_calls::append_constraints(&mut constraints, model);
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
                required.extend(aarch64_register_aggregate_call_keys(darwin));
                required.extend(aarch64_register_aggregate_return_keys(darwin));
            }
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
    for key in canonical.required.iter().copied() {
        let Some(actual) = validated
            .catalog()
            .constraints
            .iter()
            .find(|constraint| constraint.key == key)
        else {
            return Err(Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(key));
        };
        let expected = canonical
            .constraints
            .iter()
            .find(|constraint| constraint.key == key)
            .expect("target-owned inventory and rows are closed together");
        if actual != expected {
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

fn sorted_units(units: impl IntoIterator<Item = RegisterUnitId>) -> Vec<RegisterUnitId> {
    units
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn complement(
    all: &[RegisterUnitId],
    fixed: &[RegisterUnitId],
    callee: &[RegisterUnitId],
) -> Vec<RegisterUnitId> {
    all.iter()
        .copied()
        .filter(|unit| !fixed.contains(unit) && !callee.contains(unit))
        .collect()
}

fn view_ids(views: &BTreeMap<String, RegisterViewId>, names: &[&str]) -> Vec<RegisterViewId> {
    names.iter().map(|name| views[*name]).collect()
}

fn overlay(
    name: &str,
    reason: ReservationReason,
    units: Vec<RegisterUnitId>,
) -> RegisterReservationOverlay {
    RegisterReservationOverlay {
        name: name.into(),
        reason,
        units: sorted_units(units),
    }
}

#[cfg(test)]
mod tests {
    use register_model::{
        RegisterConstraintCatalogValidationError, RegisterModelValidationError,
        validate_physical_register_model,
    };

    use super::*;

    fn row(
        catalog: &RegisterConstraintCatalog,
        key: RegisterConstraintKey,
    ) -> &RegisterInstructionConstraint {
        catalog
            .constraints
            .iter()
            .find(|row| row.key == key)
            .unwrap()
    }

    fn row_mut(
        catalog: &mut RegisterConstraintCatalog,
        key: RegisterConstraintKey,
    ) -> &mut RegisterInstructionConstraint {
        catalog
            .constraints
            .iter_mut()
            .find(|row| row.key == key)
            .unwrap()
    }

    #[test]
    fn aggregate_result_fragments_are_definitions_not_unknown_clobbers() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let catalog = aarch64_register_constraint_catalog(&model);
        let second = model.model().view_named("x1").unwrap();
        let scratch = model.model().view_named("x2").unwrap();
        for (ordinal, key) in [false, true]
            .into_iter()
            .flat_map(aarch64_register_aggregate_call_keys)
            .enumerate()
        {
            let fragments = ordinal % 18 / 9 + 1;
            let call = row(&catalog, key);
            assert_eq!(
                call.operands
                    .iter()
                    .filter(|operand| operand.access == RegisterOperandAccess::Def)
                    .count(),
                fragments
            );
            for unit in &second.write_units {
                assert_eq!(call.clobbers.contains(unit), fragments == 1, "{key:?}");
            }
            assert!(
                scratch
                    .write_units
                    .iter()
                    .all(|unit| call.clobbers.contains(unit))
            );
            let mut altered = catalog.clone();
            row_mut(&mut altered, key)
                .clobbers
                .retain(|unit| !scratch.write_units.contains(unit));
            assert!(validate_aarch64_register_constraint_catalog(altered, &model).is_err());
        }
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
        let aapcs = model
            .conventions
            .iter()
            .find(|row| row.name == "aapcs64")
            .unwrap();
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
                + aarch64_aapcs64_mixed_unit_call_keys().len()
                + aarch64_darwin_mixed_unit_call_keys().len()
                + aarch64_register_aggregate_call_keys(false).len()
                + aarch64_register_aggregate_call_keys(true).len()
                + aarch64_register_aggregate_return_keys(false).len()
                + aarch64_register_aggregate_return_keys(true).len()
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
}
