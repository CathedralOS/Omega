use std::collections::BTreeSet;

use omega_isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_DARWIN_CALL, Aarch64RegisterConstraintCatalogValidationError,
    aarch64_preservation_convention_for_target,
};
use omega_isa_x86_64::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, X86_64_SYSTEM_V_CALL,
    X86_64RegisterConstraintCatalogValidationError, x86_64_preservation_convention_for_target,
};
use omega_register_model::{
    PhysicalRegisterModel, PreservationConvention, RegisterConstraintCatalog,
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterUnitId,
    validate_physical_register_model,
};
use omega_target::{Architecture, NativeTarget};

use super::super::catalog::{
    conservative_baseline_reservation_profile, scalar_call_constraint_key,
    target_constraint_catalog, target_physical_register_model,
};
use super::super::{
    TargetRegisterEnvironmentValidationError, baseline_target_register_environment,
    validate_target_register_environment, validate_target_register_environment_with_reservations,
};

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
        },
    ]
}

fn convention_for<'model>(
    case: ScalarAbiCase,
    model: &'model omega_register_model::ValidatedPhysicalRegisterModel,
) -> &'model PreservationConvention {
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

fn row_mut(
    catalog: &mut RegisterConstraintCatalog,
    key: RegisterConstraintKey,
) -> &mut RegisterInstructionConstraint {
    catalog
        .constraints
        .iter_mut()
        .find(|row| row.key == key)
        .unwrap_or_else(|| panic!("ABI matrix missing call row {key:?}"))
}

fn assert_target_semantic_error(
    target: NativeTarget,
    key: RegisterConstraintKey,
    error: TargetRegisterEnvironmentValidationError,
) {
    match target.architecture {
        Architecture::X86_64 => assert_eq!(
            error,
            TargetRegisterEnvironmentValidationError::X86_64(
                X86_64RegisterConstraintCatalogValidationError::TargetSemanticMismatch(key),
            )
        ),
        Architecture::Aarch64 => assert_eq!(
            error,
            TargetRegisterEnvironmentValidationError::Aarch64(
                Aarch64RegisterConstraintCatalogValidationError::TargetSemantics(key),
            )
        ),
    }
}

fn assert_noncanonical_model_error(
    target: NativeTarget,
    error: TargetRegisterEnvironmentValidationError,
) {
    match target.architecture {
        Architecture::X86_64 => assert_eq!(
            error,
            TargetRegisterEnvironmentValidationError::X86_64(
                X86_64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel,
            )
        ),
        Architecture::Aarch64 => assert_eq!(
            error,
            TargetRegisterEnvironmentValidationError::Aarch64(
                Aarch64RegisterConstraintCatalogValidationError::NonCanonicalPhysicalModel,
            )
        ),
    }
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
fn every_scalar_call_fact_rejects_one_field_corruption_on_every_target() {
    for case in scalar_abi_cases() {
        let raw = target_physical_register_model(case.target);
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let catalog = target_constraint_catalog(case.target, &physical);
        let canonical = catalog
            .constraints
            .iter()
            .find(|row| row.key == case.call)
            .unwrap()
            .clone();

        for omitted in &canonical.clobbers {
            let mut corrupted = catalog.clone();
            row_mut(&mut corrupted, case.call)
                .clobbers
                .retain(|unit| unit != omitted);
            let error = validate_target_register_environment(case.target, raw.clone(), corrupted)
                .expect_err("every omitted caller-saved clobber must reject");
            assert_target_semantic_error(case.target, case.call, error);
        }

        let convention = convention_for(case, &physical);
        let preserved = convention.callee_saved[0];
        let mut added_preserved = catalog.clone();
        let clobbers = &mut row_mut(&mut added_preserved, case.call).clobbers;
        clobbers.push(preserved);
        clobbers.sort_unstable();
        let error = validate_target_register_environment(case.target, raw.clone(), added_preserved)
            .expect_err("a callee-saved call clobber must reject");
        assert_target_semantic_error(case.target, case.call, error);

        let mut wrong_argument = catalog.clone();
        let row = row_mut(&mut wrong_argument, case.call);
        row.operands[0].fixed_view = row.operands[1].fixed_view;
        let error = validate_target_register_environment(case.target, raw.clone(), wrong_argument)
            .expect_err("same-class argument substitution must reject");
        assert_target_semantic_error(case.target, case.call, error);

        let mut wrong_result = catalog.clone();
        let row = row_mut(&mut wrong_result, case.call);
        row.operands.last_mut().unwrap().fixed_view = row.operands[1].fixed_view;
        let error = validate_target_register_environment(case.target, raw.clone(), wrong_result)
            .expect_err("same-class result substitution must reject");
        assert_target_semantic_error(case.target, case.call, error);

        let mut missing_use = catalog.clone();
        row_mut(&mut missing_use, case.call).implicit_uses.remove(0);
        let error = validate_target_register_environment(case.target, raw.clone(), missing_use)
            .expect_err("missing stack/control use must reject");
        assert_target_semantic_error(case.target, case.call, error);

        let mut missing_definition = catalog.clone();
        row_mut(&mut missing_definition, case.call)
            .implicit_defs
            .remove(0);
        let error =
            validate_target_register_environment(case.target, raw.clone(), missing_definition)
                .expect_err("missing control/link definition must reject");
        assert_target_semantic_error(case.target, case.call, error);

        let opposite = catalog
            .constraints
            .iter()
            .find(|row| row.key == case.opposite_call)
            .unwrap()
            .clone();
        let mut substituted = catalog.clone();
        let row = row_mut(&mut substituted, case.call);
        row.operands = opposite.operands;
        row.implicit_uses = opposite.implicit_uses;
        row.implicit_defs = opposite.implicit_defs;
        row.clobbers = opposite.clobbers;
        let error = validate_target_register_environment(case.target, raw, substituted)
            .expect_err("copying the other platform ABI under this key must reject");
        assert_target_semantic_error(case.target, case.call, error);
    }
}

#[test]
fn structurally_valid_preservation_convention_corruption_rejects_for_every_target() {
    for case in scalar_abi_cases() {
        let canonical_raw = target_physical_register_model(case.target);
        let canonical_physical = validate_physical_register_model(canonical_raw.clone()).unwrap();
        let canonical_catalog = target_constraint_catalog(case.target, &canonical_physical);
        for corruption in 0..7 {
            let mut raw = canonical_raw.clone();
            let convention = raw
                .conventions
                .iter_mut()
                .find(|row| row.name == case.convention)
                .unwrap();
            match corruption {
                0 => convention.argument_views.swap(0, 1),
                1 => convention.result_views[0] = convention.argument_views[1],
                2 => {
                    let unit = convention.caller_saved.remove(0);
                    convention.callee_saved.push(unit);
                    convention.callee_saved.sort_unstable();
                }
                3 => {
                    let unit = convention.fixed.remove(0);
                    convention.caller_saved.push(unit);
                    convention.caller_saved.sort_unstable();
                }
                4 => convention.stack_alignment *= 2,
                5 => convention.red_zone_bytes ^= 8,
                6 => convention.name.push_str(".forged"),
                _ => unreachable!(),
            }
            let reservations = conservative_baseline_reservation_profile(case.target, &raw);
            let error = validate_target_register_environment_with_reservations(
                case.target,
                raw,
                canonical_catalog.clone(),
                reservations,
            )
            .expect_err("structurally valid ABI convention drift must reject");
            assert_noncanonical_model_error(case.target, error);
        }
    }
}

#[test]
fn microsoft_structural_unit_call_clobbers_reject_corruption_for_every_coff_target() {
    for target in [NativeTarget::windows_x64(), NativeTarget::uefi_x64()] {
        let environment = baseline_target_register_environment(target).unwrap();
        let row = environment
            .constraint(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR)
            .unwrap();
        assert!(row.operands.is_empty());
        assert_eq!(
            row.implicit_uses,
            units_for_names(
                environment.physical().model(),
                &["rcx", "rdx", "rsp", "rip"]
            )
        );
        assert_eq!(
            row.implicit_defs,
            units_for_names(environment.physical().model(), &["rsp", "rip"])
        );
        let convention =
            x86_64_preservation_convention_for_target(environment.physical(), target).unwrap();
        assert_eq!(row.clobbers, convention.caller_saved);

        let raw = target_physical_register_model(target);
        let physical = validate_physical_register_model(raw.clone()).unwrap();
        let catalog = target_constraint_catalog(target, &physical);
        for omitted in &row.clobbers {
            let mut corrupted = catalog.clone();
            row_mut(
                &mut corrupted,
                X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
            )
            .clobbers
            .retain(|unit| unit != omitted);
            let error = validate_target_register_environment(target, raw.clone(), corrupted)
                .expect_err("structural call must retain every Microsoft caller clobber");
            assert_target_semantic_error(
                target,
                X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR,
                error,
            );
        }
    }
}
