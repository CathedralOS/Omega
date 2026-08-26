use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterClass, RegisterClassId,
    RegisterConstraintCatalog, RegisterConstraintCatalogValidationError, RegisterConstraintFamily,
    RegisterConstraintId, RegisterConstraintKey, RegisterInstructionConstraint,
    RegisterOperandAccess, RegisterOperandConstraint, RegisterReservationOverlay, RegisterUnit,
    RegisterUnitId, RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    ReservationReason, ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    validate_register_constraint_catalog,
};
use omega_target::Architecture;

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

pub const AARCH64_AAPCS64_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 0,
};
pub const AARCH64_DARWIN_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Call,
    variant: 1,
};
pub const AARCH64_AAPCS64_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 0,
};
pub const AARCH64_DARWIN_RETURN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Return,
    variant: 1,
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

/// Closed baseline constraint inventory currently owned by the AArch64 target.
/// The ordinary rows are limited to the baseline operations required by a
/// register-passed scalar conditional-return CFG. Other ordinary and
/// feature-specific instruction rows remain intentionally absent.
pub const AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 10] = [
    AARCH64_AAPCS64_CALL,
    AARCH64_DARWIN_CALL,
    AARCH64_AAPCS64_RETURN,
    AARCH64_DARWIN_RETURN,
    AARCH64_LINUX_SYSTEM_CALL,
    AARCH64_INLINE_ASSEMBLY_DEFAULT,
    AARCH64_MATERIALIZE_I64,
    AARCH64_COPY_I64,
    AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64RegisterConstraintCatalogValidationError {
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

    let constraints = vec![
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
            implicit_uses: call_uses,
            implicit_defs: call_defs.clone(),
            clobbers: call_clobbers(darwin),
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
            implicit_uses: return_uses,
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
            implicit_defs: pc_units,
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
    ];

    RegisterConstraintCatalog {
        architecture: Architecture::Aarch64,
        required: AARCH64_REQUIRED_REGISTER_CONSTRAINTS.to_vec(),
        constraints,
    }
}

/// Apply generic structural validation and then independently compare every
/// keyed row with the AArch64 target owner's canonical semantics.
pub fn validate_aarch64_register_constraint_catalog(
    catalog: RegisterConstraintCatalog,
    model: &ValidatedPhysicalRegisterModel,
) -> Result<ValidatedRegisterConstraintCatalog, Aarch64RegisterConstraintCatalogValidationError> {
    let validated = validate_register_constraint_catalog(catalog, model)
        .map_err(Aarch64RegisterConstraintCatalogValidationError::Structural)?;
    let canonical = aarch64_register_constraint_catalog(model);
    for key in AARCH64_REQUIRED_REGISTER_CONSTRAINTS {
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
    if let Some(unexpected) = validated.catalog().constraints.iter().find(|constraint| {
        AARCH64_REQUIRED_REGISTER_CONSTRAINTS
            .binary_search(&constraint.key)
            .is_err()
    }) {
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
    use omega_register_model::{
        RegisterConstraintCatalogValidationError, RegisterModelValidationError,
        validate_physical_register_model,
    };

    use super::*;

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
        assert_eq!(
            catalog.required.as_slice(),
            AARCH64_REQUIRED_REGISTER_CONSTRAINTS
        );

        let call = &catalog.constraints[0];
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

        let returned = &catalog.constraints[2];
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

        let syscall = &catalog.constraints[4];
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

        let materialize = &catalog.constraints[6];
        assert_eq!(materialize.key, AARCH64_MATERIALIZE_I64);
        assert_eq!(materialize.operands.len(), 1);
        assert_eq!(materialize.operands[0].access, RegisterOperandAccess::Def);
        assert_eq!(materialize.operands[0].class, GPR64);

        let copy = &catalog.constraints[7];
        assert_eq!(copy.key, AARCH64_COPY_I64);
        assert_eq!(copy.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(copy.operands[1].access, RegisterOperandAccess::Def);

        let compare = &catalog.constraints[8];
        assert_eq!(compare.key, AARCH64_COMPARE_I64_ZERO);
        assert_eq!(compare.operands[0].class, GPR64);
        assert_eq!(
            compare.implicit_defs,
            model.model().view_named("nzcv").unwrap().units
        );

        let branch = &catalog.constraints[9];
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
        assert_eq!(
            branch.implicit_defs,
            model.model().view_named("pc").unwrap().units
        );
    }

    #[test]
    fn every_missing_required_aarch64_constraint_rejects() {
        let model = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        for (position, expected) in AARCH64_REQUIRED_REGISTER_CONSTRAINTS
            .iter()
            .copied()
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
        wrong_syscall_register.constraints[4].operands[5].fixed_view =
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
        missing_nzcv.constraints[4].clobbers.clear();
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
        missing_link_state.constraints[0]
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
        catalog.constraints[8].implicit_defs.clear();
        assert_eq!(
            validate_aarch64_register_constraint_catalog(catalog, &model),
            Err(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(
                    AARCH64_COMPARE_I64_ZERO,
                )
            )
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
