use std::collections::{BTreeMap, BTreeSet};

use omega_regalloc::{
    FixedRegisterOperand, PhysicalRegisterModel, PreservationConvention, RegisterClass,
    RegisterClassId, RegisterConstraint, RegisterReservationOverlay, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics, ReservationReason,
};
use omega_target::Architecture;

const GPR64: RegisterClassId = RegisterClassId(0);
const GPR32: RegisterClassId = RegisterClassId(1);
const GPR16: RegisterClassId = RegisterClassId(2);
const GPR8_LOW: RegisterClassId = RegisterClassId(3);
const GPR8_HIGH: RegisterClassId = RegisterClassId(4);
const VECTOR128: RegisterClassId = RegisterClassId(5);
const FLAGS: RegisterClassId = RegisterClassId(6);
const INSTRUCTION_POINTER: RegisterClassId = RegisterClassId(7);

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
    let constraints = vec![
        RegisterConstraint {
            name: "linux-x86_64-syscall".into(),
            fixed_inputs: fixed_operands(
                &named_views,
                &["rax", "rdi", "rsi", "rdx", "r10", "r8", "r9"],
            ),
            fixed_outputs: fixed_operands(&named_views, &["rax"]),
            early_clobbers: Vec::new(),
            clobbers: sorted_units(
                gpr_units["rcx"]
                    .iter()
                    .copied()
                    .chain(gpr_units["r11"].iter().copied())
                    .chain([flags_unit]),
            ),
        },
        RegisterConstraint {
            name: "x86-inline-assembly-default".into(),
            fixed_inputs: Vec::new(),
            fixed_outputs: Vec::new(),
            early_clobbers: Vec::new(),
            clobbers: complement(&all_units, &fixed, &[]),
        },
    ];
    PhysicalRegisterModel {
        architecture: Architecture::X86_64,
        units: builder.units,
        views: builder.views,
        classes: builder.classes,
        conventions,
        reservations,
        constraints,
    }
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

fn fixed_operands(
    views: &BTreeMap<String, RegisterViewId>,
    names: &[&str],
) -> Vec<FixedRegisterOperand> {
    names
        .iter()
        .enumerate()
        .map(|(operand, name)| FixedRegisterOperand {
            operand: u16::try_from(operand).expect("operand index fits u16"),
            view: views[*name],
        })
        .collect()
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
    use omega_regalloc::{RegisterModelValidationError, validate_physical_register_model};

    use super::*;

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
        assert!(
            model
                .constraints
                .iter()
                .any(|row| row.name == "linux-x86_64-syscall")
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
