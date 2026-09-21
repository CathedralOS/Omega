//! The physical register model: register classes, the fixed register view
//! and the builder that lays out every unit, view and reservation overlay.

use calling_conventions::MachineRegister;
use register_model::{
    PhysicalRegisterModel, PhysicalRegisterModelIdentity, PreservationConvention, RegisterClass,
    RegisterClassId, RegisterReservationOverlay, RegisterUnit, RegisterUnitId, RegisterUnitKind,
    RegisterView, RegisterViewId, RegisterWriteSemantics, ReservationReason,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use std::collections::{BTreeMap, BTreeSet};
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

/// Resolve one ABI-visible register through the AArch64 target owner's
/// canonical model.
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

pub(crate) fn sorted_units(units: impl IntoIterator<Item = RegisterUnitId>) -> Vec<RegisterUnitId> {
    units
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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

/// The canonical AArch64 physical register model, validated once per
/// process.
///
/// Encode and validate paths run per instruction; they compare canonical
/// identity instead of rebuilding and deep-comparing the model per request.
pub fn validated_aarch64_physical_register_model() -> &'static ValidatedPhysicalRegisterModel {
    static CANONICAL: std::sync::OnceLock<ValidatedPhysicalRegisterModel> =
        std::sync::OnceLock::new();
    CANONICAL.get_or_init(|| {
        validate_physical_register_model(aarch64_physical_register_model()).unwrap_or_else(
            |error| panic!("canonical aarch64 physical register model must validate: {error}"),
        )
    })
}

/// The content identity of the canonical AArch64 physical register model.
pub fn canonical_aarch64_physical_register_model_identity() -> PhysicalRegisterModelIdentity {
    validated_aarch64_physical_register_model().identity()
}
