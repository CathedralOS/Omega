//! The physical register model: register classes, the fixed register view
//! and the builder that lays out every unit, view and reservation overlay.

use calling_conventions::MachineRegister;
use register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterReservationOverlay, RegisterUnit, RegisterUnitId, RegisterUnitKind, RegisterView,
    RegisterViewId, RegisterWriteSemantics, ReservationReason, ValidatedPhysicalRegisterModel,
};
use std::collections::{BTreeMap, BTreeSet};
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
                "x86.floating-control",
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

    let all_units = builder.units.iter().map(|unit| unit.id).collect::<Vec<_>>();
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
        overlay(
            "x86.floating-control",
            ReservationReason::Architectural,
            vec![mxcsr_unit],
        ),
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

pub(crate) fn sorted_units(units: impl IntoIterator<Item = RegisterUnitId>) -> Vec<RegisterUnitId> {
    units
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn units_for(units: &BTreeMap<String, Vec<RegisterUnitId>>, names: &[&str]) -> Vec<RegisterUnitId> {
    sorted_units(names.iter().flat_map(|name| units[*name].iter().copied()))
}

pub(crate) fn complement(
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
