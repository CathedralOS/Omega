use std::collections::BTreeSet;

mod arithmetic_effects;
mod branch_effects;
mod call_effects;
mod cleanup_effects;
mod memory_effects;
mod provenance_effects;
mod register_calls;
mod return_effects;

use isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_DARWIN_CALL, Aarch64MachineEffectCatalogValidationError,
    aarch64_machine_effect_catalog, aarch64_preservation_convention_for_target,
    validate_aarch64_machine_effect_catalog,
};
use isa_x86_64::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT, X86_64_SYSTEM_V_CALL,
    X86_64MachineEffectCatalogValidationError, validate_x86_64_machine_effect_catalog,
    x86_64_machine_effect_catalog, x86_64_preservation_convention_for_target,
};
use register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterConstraintKey, RegisterUnitId,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    MachineEffectCatalog, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    ValidatedMachineEffectCatalog,
};
use target::{Architecture, NativeTarget};

use super::super::baseline_target_register_environment;
use super::super::catalog::scalar_call_constraint_key;

#[derive(Clone, Copy)]
struct ScalarAbiCase {
    target: NativeTarget,
    convention: &'static str,
    call: RegisterConstraintKey,
    arguments: &'static [&'static str],
    result: &'static str,
    implicit_uses: &'static [&'static str],
    implicit_defs: &'static [&'static str],
    stack_alignment: u16,
    red_zone_bytes: u16,
    opposite_call: RegisterConstraintKey,
    /// Ordered integer result views consumed by the selected scalar and
    /// aggregate return rules: the scalar uses the first, each aggregate
    /// ordinal extends the prefix by one.
    integer_results: &'static [&'static str],
    /// ABI floating-point result view used by the selected float return rule.
    float_result: &'static str,
    /// Constraint-row implicit uses carried by every selected return rule.
    return_uses: &'static [&'static str],
    /// Constraint-row implicit defs carried by every selected return rule.
    return_defs: &'static [&'static str],
}

fn scalar_abi_cases() -> Vec<ScalarAbiCase> {
    vec![
        ScalarAbiCase {
            target: NativeTarget::linux_x64(),
            convention: "system-v-amd64",
            call: X86_64_SYSTEM_V_CALL,
            arguments: &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
            result: "rax",
            implicit_uses: &["rsp"],
            implicit_defs: &["rsp", "rip"],
            stack_alignment: 16,
            red_zone_bytes: 128,
            opposite_call: X86_64_MICROSOFT_CALL,
            integer_results: &["rax", "rdx"],
            float_result: "xmm0",
            return_uses: &["rsp"],
            return_defs: &["rsp", "rip"],
        },
        ScalarAbiCase {
            target: NativeTarget::windows_x64(),
            convention: "microsoft-x64",
            call: X86_64_MICROSOFT_CALL,
            arguments: &["rcx", "rdx", "r8", "r9"],
            result: "rax",
            implicit_uses: &["rsp"],
            implicit_defs: &["rsp", "rip"],
            stack_alignment: 16,
            red_zone_bytes: 0,
            opposite_call: X86_64_SYSTEM_V_CALL,
            integer_results: &["rax", "rdx"],
            float_result: "xmm0",
            return_uses: &["rsp"],
            return_defs: &["rsp", "rip"],
        },
        ScalarAbiCase {
            target: NativeTarget::uefi_x64(),
            convention: "microsoft-x64",
            call: X86_64_MICROSOFT_CALL,
            arguments: &["rcx", "rdx", "r8", "r9"],
            result: "rax",
            implicit_uses: &["rsp"],
            implicit_defs: &["rsp", "rip"],
            stack_alignment: 16,
            red_zone_bytes: 0,
            opposite_call: X86_64_SYSTEM_V_CALL,
            integer_results: &["rax", "rdx"],
            float_result: "xmm0",
            return_uses: &["rsp"],
            return_defs: &["rsp", "rip"],
        },
        ScalarAbiCase {
            target: NativeTarget::linux_arm64(),
            convention: "aapcs64",
            call: AARCH64_AAPCS64_CALL,
            arguments: &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
            result: "x0",
            implicit_uses: &["sp", "pc"],
            implicit_defs: &["x30", "pc"],
            stack_alignment: 16,
            red_zone_bytes: 0,
            opposite_call: AARCH64_DARWIN_CALL,
            integer_results: &["x0", "x1"],
            float_result: "d0",
            return_uses: &["sp", "x30"],
            return_defs: &["pc"],
        },
        ScalarAbiCase {
            target: NativeTarget::macos_arm64(),
            convention: "darwin-aapcs64",
            call: AARCH64_DARWIN_CALL,
            arguments: &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
            result: "x0",
            implicit_uses: &["sp", "pc"],
            implicit_defs: &["x30", "pc"],
            stack_alignment: 16,
            red_zone_bytes: 0,
            opposite_call: AARCH64_AAPCS64_CALL,
            integer_results: &["x0", "x1"],
            float_result: "d0",
            return_uses: &["sp", "x30"],
            return_defs: &["pc"],
        },
    ]
}

fn convention_for(
    case: ScalarAbiCase,
    model: &register_model::ValidatedPhysicalRegisterModel,
) -> &PreservationConvention {
    let convention = match case.target.architecture {
        Architecture::X86_64 => x86_64_preservation_convention_for_target(model, case.target),
        Architecture::Aarch64 => aarch64_preservation_convention_for_target(model, case.target),
    }
    .expect("the supported target selects one preservation convention");
    assert_eq!(convention.name, case.convention);
    convention
}

fn units_for_names(model: &PhysicalRegisterModel, names: &[&str]) -> Vec<RegisterUnitId> {
    names
        .iter()
        .flat_map(|name| {
            model
                .view_named(name)
                .unwrap_or_else(|| panic!("ABI matrix names missing view `{name}`"))
                .units
                .iter()
                .copied()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The produced effect catalog joined through the ISA's own independent
/// validator: structural admission plus canonical re-derivation equality.
fn validated_effects(
    case: ScalarAbiCase,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> ValidatedMachineEffectCatalog {
    match case.target.architecture {
        Architecture::X86_64 => validate_x86_64_machine_effect_catalog(
            case.target,
            constraints,
            x86_64_machine_effect_catalog(case.target, constraints).unwrap(),
        )
        .expect("the produced effect catalog must validate for the target ABI"),
        Architecture::Aarch64 => validate_aarch64_machine_effect_catalog(
            case.target,
            constraints,
            aarch64_machine_effect_catalog(case.target, constraints).unwrap(),
        )
        .expect("the produced effect catalog must validate for the target ABI"),
    }
}

/// The produced catalog without admission, for per-fact corruption.
fn produced_effects(
    case: ScalarAbiCase,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> MachineEffectCatalog {
    match case.target.architecture {
        Architecture::X86_64 => x86_64_machine_effect_catalog(case.target, constraints).unwrap(),
        Architecture::Aarch64 => aarch64_machine_effect_catalog(case.target, constraints).unwrap(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum EffectRejection {
    SemanticMismatch,
    Structural(MachineEffectCatalogValidationError),
    TargetArchitectureMismatch,
    UnsupportedTargetAbi,
}

fn validate_effects(
    case: ScalarAbiCase,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: MachineEffectCatalog,
) -> Result<ValidatedMachineEffectCatalog, EffectRejection> {
    match case.target.architecture {
        Architecture::X86_64 => {
            validate_x86_64_machine_effect_catalog(case.target, constraints, catalog).map_err(
                |error| match error {
                    X86_64MachineEffectCatalogValidationError::Structural(inner) => {
                        EffectRejection::Structural(inner)
                    }
                    X86_64MachineEffectCatalogValidationError::TargetSemanticMismatch => {
                        EffectRejection::SemanticMismatch
                    }
                    X86_64MachineEffectCatalogValidationError::TargetArchitectureMismatch => {
                        EffectRejection::TargetArchitectureMismatch
                    }
                    X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi => {
                        EffectRejection::UnsupportedTargetAbi
                    }
                },
            )
        }
        Architecture::Aarch64 => {
            validate_aarch64_machine_effect_catalog(case.target, constraints, catalog).map_err(
                |error| match error {
                    Aarch64MachineEffectCatalogValidationError::Structural(inner) => {
                        EffectRejection::Structural(inner)
                    }
                    Aarch64MachineEffectCatalogValidationError::TargetSemanticMismatch => {
                        EffectRejection::SemanticMismatch
                    }
                    Aarch64MachineEffectCatalogValidationError::TargetArchitectureMismatch => {
                        EffectRejection::TargetArchitectureMismatch
                    }
                    Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi => {
                        EffectRejection::UnsupportedTargetAbi
                    }
                },
            )
        }
    }
}

fn declaration_mut(
    catalog: &mut MachineEffectCatalog,
    key: RegisterConstraintKey,
) -> &mut MachineEffectDeclaration {
    catalog
        .declarations
        .iter_mut()
        .find(|declaration| declaration.constraint == key)
        .unwrap_or_else(|| panic!("effect catalog missing declaration {key:?}"))
}

#[test]
fn every_supported_target_selects_its_exact_scalar_call_abi() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        assert_eq!(scalar_call_constraint_key(case.target), Some(case.call));
        let row = environment
            .scalar_call_constraint()
            .expect("the target environment exposes its scalar-call row");
        assert_eq!(row.key, case.call);
        assert_eq!(row.operands.len(), case.arguments.len() + 1);
        let model = environment.physical().model();
        for (operand, expected) in row.operands.iter().zip(case.arguments) {
            assert_eq!(
                operand.fixed_view,
                Some(model.view_named(expected).unwrap().id),
                "{} argument view `{expected}`",
                case.convention
            );
        }
        assert_eq!(
            row.operands.last().unwrap().fixed_view,
            Some(model.view_named(case.result).unwrap().id)
        );
        assert_eq!(
            row.implicit_uses,
            units_for_names(model, case.implicit_uses)
        );
        assert_eq!(
            row.implicit_defs,
            units_for_names(model, case.implicit_defs)
        );

        let convention = convention_for(case, environment.physical());
        assert_eq!(convention.stack_alignment, case.stack_alignment);
        assert_eq!(convention.red_zone_bytes, case.red_zone_bytes);
        let excluded = units_for_names(model, &[case.result])
            .into_iter()
            .chain(row.implicit_defs.iter().copied())
            .collect::<BTreeSet<_>>();
        let expected_clobbers = convention
            .caller_saved
            .iter()
            .copied()
            .filter(|unit| !excluded.contains(unit))
            .collect::<Vec<_>>();
        assert_eq!(row.clobbers, expected_clobbers, "{}", case.convention);
        assert!(row.clobbers.iter().all(
            |unit| !convention.callee_saved.contains(unit) && !convention.fixed.contains(unit)
        ));
    }
}

#[test]
fn platform_abi_differences_are_explicit_call_clobber_facts() {
    let linux_x64 = baseline_target_register_environment(NativeTarget::linux_x64()).unwrap();
    let windows_x64 = baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
    for name in ["rsi", "xmm6"] {
        let units = units_for_names(linux_x64.physical().model(), &[name]);
        assert!(units.iter().all(|unit| {
            linux_x64
                .scalar_call_constraint()
                .unwrap()
                .clobbers
                .contains(unit)
        }));
        assert!(units.iter().all(|unit| {
            !windows_x64
                .scalar_call_constraint()
                .unwrap()
                .clobbers
                .contains(unit)
        }));
    }

    let linux_arm64 = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    let macos_arm64 = baseline_target_register_environment(NativeTarget::macos_arm64()).unwrap();
    let x18 = units_for_names(linux_arm64.physical().model(), &["x18"]);
    assert!(x18.iter().all(|unit| {
        linux_arm64
            .scalar_call_constraint()
            .unwrap()
            .clobbers
            .contains(unit)
    }));
    assert!(x18.iter().all(|unit| {
        !macos_arm64
            .scalar_call_constraint()
            .unwrap()
            .clobbers
            .contains(unit)
    }));
    assert!(
        x18.iter()
            .all(|unit| macos_arm64.reservations().reserved_units().contains(unit))
    );

    let aapcs = convention_for(scalar_abi_cases()[3], linux_arm64.physical());
    let d8_low = units_for_names(linux_arm64.physical().model(), &["d8"]);
    let q8 = units_for_names(linux_arm64.physical().model(), &["q8"]);
    assert!(d8_low.iter().all(|unit| aapcs.callee_saved.contains(unit)));
    assert!(q8.iter().filter(|unit| !d8_low.contains(unit)).all(|unit| {
        linux_arm64
            .scalar_call_constraint()
            .unwrap()
            .clobbers
            .contains(unit)
    }));
}

#[test]
fn microsoft_unit_call_clobbers_match_every_coff_target() {
    for target in [NativeTarget::windows_x64(), NativeTarget::uefi_x64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let row = environment.constraint(X86_64_MICROSOFT_CALL_UNIT).unwrap();
        assert_eq!(row.operands.len(), 2);
        for (operand, name) in row.operands.iter().zip(["rcx", "rdx"]) {
            assert_eq!(operand.access, register_model::RegisterOperandAccess::Use);
            assert_eq!(
                operand.fixed_view,
                Some(environment.physical().model().view_named(name).unwrap().id)
            );
        }
        assert_eq!(
            row.implicit_uses,
            units_for_names(environment.physical().model(), &["rsp", "rip"])
        );
        assert_eq!(
            row.implicit_defs,
            units_for_names(environment.physical().model(), &["rsp", "rip"])
        );
        let convention =
            x86_64_preservation_convention_for_target(environment.physical(), target).unwrap();
        assert_eq!(row.clobbers, convention.caller_saved);
    }
}
