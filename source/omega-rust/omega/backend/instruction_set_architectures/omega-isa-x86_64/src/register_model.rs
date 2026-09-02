use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::MachineRegister;
use omega_register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterReservationOverlay, RegisterUnit,
    RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    ReservationReason, ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_register_constraint_catalog,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

const GPR64: RegisterClassId = RegisterClassId(0);
const GPR32: RegisterClassId = RegisterClassId(1);
const GPR16: RegisterClassId = RegisterClassId(2);
const GPR8_LOW: RegisterClassId = RegisterClassId(3);
const GPR8_HIGH: RegisterClassId = RegisterClassId(4);
const VECTOR128: RegisterClassId = RegisterClassId(5);
const FLAGS: RegisterClassId = RegisterClassId(6);
const INSTRUCTION_POINTER: RegisterClassId = RegisterClassId(7);

/// Resolve one ABI-visible register through the x86-64 target owner's
/// canonical model. Target-neutral selection consumes the resulting fixed
/// constraint and never infers register names.
pub fn x86_64_fixed_register_view(
    model: &ValidatedPhysicalRegisterModel,
    register: MachineRegister,
) -> Option<RegisterViewId> {
    if model.model() != &x86_64_physical_register_model() {
        return None;
    }
    let name = match register {
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

/// Resolve the exact preservation convention selected by the clean terminal
/// lane for one supported x86-64 target. Keeping this mapping in the ISA owner
/// prevents target-neutral orchestration from inferring ABI policy from vector
/// positions or authored names.
pub fn x86_64_preservation_convention_for_target<'model>(
    model: &'model ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Option<&'model PreservationConvention> {
    if target.architecture != Architecture::X86_64 {
        return None;
    }
    let name = match target.object_format {
        ObjectFormat::Elf => "system-v-amd64",
        ObjectFormat::Coff => "microsoft-x64",
        ObjectFormat::MachO => return None,
    };
    model
        .model()
        .conventions
        .iter()
        .find(|convention| convention.name == name)
}

pub const X86_64_SYSTEM_V_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 0,
};
pub const X86_64_MICROSOFT_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 1,
};
/// Atomic value-less Microsoft-x64 call for the bounded pair of owned,
/// indirectly passed structural roots. The roots are ABI state rather than
/// allocator-managed scalar operands, so this row has no operand or result.
pub const X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR: RegisterConstraintKey =
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 2,
    };
/// Exact Linux System-V scalar call with two U64 arguments and one U64 result.
pub const X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 3,
};
pub const X86_64_SYSTEM_V_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 0,
};
pub const X86_64_MICROSOFT_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 1,
};
pub const X86_64_SYSTEM_V_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 2,
};
pub const X86_64_MICROSOFT_RETURN_UNIT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 3,
};
pub const X86_64_LINUX_SYSTEM_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::SystemCall,
    variant: 0,
};
pub const X86_64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::InlineAssembly,
    variant: 0,
};
pub const X86_64_MATERIALIZE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 0,
};
pub const X86_64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 1,
};
pub const X86_64_COMPARE_I64_ZERO: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 2,
};
pub const X86_64_CONDITIONAL_BRANCH: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 3,
};
/// Flag-transparent three-address i64 addition, realizable with an x86-64 LEA
/// form without introducing a false two-address tie.
pub const X86_64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 4,
};
/// Flag-transparent `result = left + immediate`, realizable as LEA for the
/// named admitted immediate domain without a destructive two-address tie.
pub const X86_64_ADD_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 5,
};
/// Exact `result = left - right` three-address pseudo. Its realization must be
/// alias-safe for every allocator result: `XOR result, result` when both inputs
/// share a view, `SUB` when the result is only the left input, `NEG; ADD` when
/// it is only the right input, and `MOV; SUB` otherwise. Those alternatives do
/// not preserve one common flags value, so the row explicitly clobbers RFLAGS.
pub const X86_64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 6,
};
/// Flag-transparent `result = left - immediate`, realized as an x86-64 LEA
/// with the negated admitted U12 displacement and no destructive tie.
pub const X86_64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 7,
};
/// Two-input i64 comparison. Both operands are read and RFLAGS is defined.
pub const X86_64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 8,
};

/// Closed v1 inventory owned by the x86-64 target.
///
/// The ordinary rows are deliberately limited to the baseline operations
/// required by a register-passed scalar conditional-return CFG plus the first
/// arithmetic row needed by the pressure vertical. This is not a claim that
/// the target's ordinary instruction inventory is complete.
pub const X86_64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 19] = [
    X86_64_SYSTEM_V_CALL,
    X86_64_MICROSOFT_CALL,
    X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
    X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
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
];

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
                "x86.gpr64",
                "x86.gpr32",
                "x86.gpr16",
                "x86.gpr8-low",
                "x86.gpr8-high",
                "x86.vector128",
                "x86.flags",
                "x86.instruction-pointer",
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
        write_units: Vec<RegisterUnitId>,
        bits: u16,
        write_semantics: RegisterWriteSemantics,
        allocatable: bool,
    ) -> RegisterViewId {
        let id = RegisterViewId(u16::try_from(self.views.len()).expect("view id fits u16"));
        self.views.push(RegisterView {
            id,
            name,
            class,
            units,
            write_units,
            bits,
            write_semantics,
            allocatable,
        });
        self.classes[usize::from(class.0)].views.push(id);
        id
    }
}

pub fn x86_64_physical_register_model() -> PhysicalRegisterModel {
    let mut builder = ModelBuilder::new();
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
    let flags_unit = builder.unit("rflags.storage".into(), 64, RegisterUnitKind::Flags);
    let flags_view = builder.view(
        "rflags".into(),
        FLAGS,
        vec![flags_unit],
        vec![flags_unit],
        64,
        RegisterWriteSemantics::InstructionDefined,
        false,
    );
    named_views.insert("rflags".into(), flags_view);
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

    let all_units = builder.units.iter().map(|unit| unit.id).collect::<Vec<_>>();
    let rsp = gpr_units["rsp"].clone();
    let fixed = sorted_units(rsp.iter().copied().chain([rip_unit]));
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
        overlay("x86.stack-pointer", ReservationReason::StackPointer, rsp),
        overlay(
            "x86.instruction-pointer",
            ReservationReason::Architectural,
            vec![rip_unit],
        ),
        overlay(
            "x86.frame-pointer",
            ReservationReason::FramePointer,
            gpr_units["rbp"].clone(),
        ),
        overlay(
            "omega.x86.dispatch",
            ReservationReason::Dispatch,
            gpr_units["r14"].clone(),
        ),
        overlay(
            "omega.x86.metering",
            ReservationReason::Metering,
            gpr_units["r15"].clone(),
        ),
    ];
    PhysicalRegisterModel {
        architecture: Architecture::X86_64,
        units: builder.units,
        views: builder.views,
        classes: builder.classes,
        conventions,
        reservations,
    }
}

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
    let structural_unit_call_uses = sorted_units(
        view("rcx")
            .units
            .iter()
            .copied()
            .chain(view("rdx").units.iter().copied())
            .chain(rsp_units.iter().copied())
            .chain(rip_units.iter().copied()),
    );
    let structural_unit_call_clobbers = microsoft
        .caller_saved
        .iter()
        .copied()
        .filter(|unit| !control_defs.contains(unit))
        .collect::<Vec<_>>();
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
            implicit_uses: rsp_units.clone(),
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
            implicit_defs: rip_units,
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
            implicit_uses: rsp_units,
            implicit_defs: control_defs.clone(),
            clobbers: Vec::new(),
        },
        RegisterInstructionConstraint {
            id: RegisterConstraintId(16),
            key: X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
            operands: Vec::new(),
            implicit_uses: structural_unit_call_uses,
            implicit_defs: control_defs,
            clobbers: structural_unit_call_clobbers,
        },
    ];

    constraints.sort_by_key(|constraint| constraint.key);
    for (id, constraint) in constraints.iter_mut().enumerate() {
        constraint.id =
            RegisterConstraintId(u16::try_from(id).expect("constraint roster fits u16"));
    }
    RegisterConstraintCatalog {
        architecture: Architecture::X86_64,
        required: X86_64_REQUIRED_REGISTER_CONSTRAINTS.to_vec(),
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
    for key in X86_64_REQUIRED_REGISTER_CONSTRAINTS {
        let Some(actual) = validated
            .catalog()
            .constraints
            .iter()
            .find(|constraint| constraint.key == key)
        else {
            return Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(key),
            );
        };
        let expected = canonical
            .constraints
            .iter()
            .find(|constraint| constraint.key == key)
            .expect("target-owned inventory and rows are closed together");
        if actual != expected {
            return Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(key),
            );
        }
    }
    if let Some(unexpected) = validated.catalog().constraints.iter().find(|constraint| {
        X86_64_REQUIRED_REGISTER_CONSTRAINTS
            .binary_search(&constraint.key)
            .is_err()
    }) {
        return Err(
            X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(unexpected.key),
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

fn units_for(units: &BTreeMap<String, Vec<RegisterUnitId>>, names: &[&str]) -> Vec<RegisterUnitId> {
    sorted_units(names.iter().flat_map(|name| units[*name].iter().copied()))
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
    use omega_register_model::{
        RegisterConstraintCatalogValidationError, RegisterModelValidationError,
        validate_physical_register_model,
    };

    use super::*;

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
        assert_eq!(
            catalog.required.as_slice(),
            X86_64_REQUIRED_REGISTER_CONSTRAINTS
        );

        let sysv_call = &catalog.constraints[0];
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

        let structural_unit_call = &catalog.constraints[2];
        assert_eq!(
            structural_unit_call.key,
            X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR
        );
        assert!(structural_unit_call.operands.is_empty());
        for used in ["rcx", "rdx", "rsp", "rip"] {
            assert!(
                model
                    .model()
                    .view_named(used)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| structural_unit_call.implicit_uses.contains(unit))
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
                    .all(|unit| structural_unit_call.clobbers.contains(unit))
            );
        }
        let scalar_call = &catalog.constraints[3];
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

        let syscall = &catalog.constraints[8];
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

        let materialize = &catalog.constraints[10];
        assert_eq!(materialize.key, X86_64_MATERIALIZE_I64);
        assert_eq!(materialize.operands.len(), 1);
        assert_eq!(materialize.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(materialize.operands[0].class, GPR64);

        let copy = &catalog.constraints[11];
        assert_eq!(copy.key, X86_64_COPY_I64);
        assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(copy.operands[1].access, RegisterOperandAccess::Def);

        let compare = &catalog.constraints[12];
        assert_eq!(compare.key, X86_64_COMPARE_I64_ZERO);
        assert_eq!(compare.operands[0].class, GPR64);
        assert_eq!(
            compare.implicit_defs,
            model.model().view_named("rflags").unwrap().units
        );

        let branch = &catalog.constraints[13];
        assert_eq!(branch.key, X86_64_CONDITIONAL_BRANCH);
        for state in ["rflags", "rip"] {
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

        let add = &catalog.constraints[14];
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

        let add_immediate = &catalog.constraints[15];
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

        let subtract = &catalog.constraints[16];
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
            model.model().view_named("rflags").unwrap().units
        );
        assert_eq!(
            branch.implicit_defs,
            model.model().view_named("rip").unwrap().units
        );
    }

    #[test]
    fn missing_required_x86_64_constraint_rejects() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut catalog = x86_64_register_constraint_catalog(&model);
        catalog.constraints.remove(5);
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
        catalog.required.remove(5);
        catalog.constraints.remove(5);
        for (id, constraint) in catalog.constraints.iter_mut().enumerate() {
            constraint.id = RegisterConstraintId(u16::try_from(id).unwrap());
        }
        assert_eq!(
            validate_x86_64_register_constraint_catalog(catalog, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_MICROSOFT_RETURN,
                )
            )
        );
    }

    #[test]
    fn x86_64_constraint_semantic_corruption_rejects() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut wrong_class = x86_64_register_constraint_catalog(&model);
        wrong_class.constraints[0].operands[6].class = VECTOR128;
        assert_eq!(
            validate_x86_64_register_constraint_catalog(wrong_class, &model),
            Err(X86_64RegisterConstraintCatalogValidationError::Structural(
                RegisterConstraintCatalogValidationError::FixedViewClassMismatch {
                    constraint: RegisterConstraintId(0),
                    operand: 6,
                },
            )),
        );

        let mut contradictory_post_state = x86_64_register_constraint_catalog(&model);
        let unit = contradictory_post_state.constraints[0].implicit_defs[0];
        contradictory_post_state.constraints[0].clobbers.push(unit);
        contradictory_post_state.constraints[0]
            .clobbers
            .sort_unstable();
        assert_eq!(
            validate_x86_64_register_constraint_catalog(contradictory_post_state, &model),
            Err(X86_64RegisterConstraintCatalogValidationError::Structural(
                RegisterConstraintCatalogValidationError::DefClobberOverlap {
                    constraint: RegisterConstraintId(0),
                    unit,
                },
            )),
        );
    }

    #[test]
    fn x86_64_target_semantics_reject_compatible_substitution_and_missing_clobbers() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut wrong_syscall_register = x86_64_register_constraint_catalog(&model);
        wrong_syscall_register.constraints[8].operands[4].fixed_view =
            Some(model.model().view_named("r11").unwrap().id);
        assert_eq!(
            validate_x86_64_register_constraint_catalog(wrong_syscall_register, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_LINUX_SYSTEM_CALL,
                )
            )
        );

        for clobber in ["rcx", "r11", "rflags"] {
            let mut missing_clobber = x86_64_register_constraint_catalog(&model);
            let omitted = model.model().view_named(clobber).unwrap().units[0];
            missing_clobber.constraints[8]
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
        wrong_add_role.constraints[14].operands[1].access = RegisterOperandAccess::Def;
        assert_eq!(
            validate_x86_64_register_constraint_catalog(wrong_add_role, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_ADD_I64,
                )
            )
        );

        let mut wrong_immediate_role = x86_64_register_constraint_catalog(&model);
        wrong_immediate_role.constraints[15].operands[0].access = RegisterOperandAccess::Def;
        assert_eq!(
            validate_x86_64_register_constraint_catalog(wrong_immediate_role, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_ADD_I64_IMMEDIATE,
                )
            )
        );

        let mut wrong_subtract_role = x86_64_register_constraint_catalog(&model);
        wrong_subtract_role.constraints[16].operands[1].access = RegisterOperandAccess::Def;
        assert_eq!(
            validate_x86_64_register_constraint_catalog(wrong_subtract_role, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_SUBTRACT_I64,
                )
            )
        );

        let mut missing_subtract_flags = x86_64_register_constraint_catalog(&model);
        missing_subtract_flags.constraints[16].clobbers.clear();
        assert_eq!(
            validate_x86_64_register_constraint_catalog(missing_subtract_flags, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_SUBTRACT_I64,
                )
            )
        );
    }

    #[test]
    fn x86_64_branch_rejects_one_field_missing_flags_use() {
        let model = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let mut catalog = x86_64_register_constraint_catalog(&model);
        let flags = model.model().view_named("rflags").unwrap().units[0];
        catalog.constraints[13]
            .implicit_uses
            .retain(|unit| *unit != flags);
        assert_eq!(
            validate_x86_64_register_constraint_catalog(catalog, &model),
            Err(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(
                    X86_64_CONDITIONAL_BRANCH,
                )
            )
        );
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
}
