/*
We verify that machine-effect catalog validation binds exact target,
architecture, constraint-catalog root, and selected-key provenance, rejecting
cross-architecture, sibling-ABI, or tampered catalog identities.
*/

use super::{
    Architecture, EffectRejection, NativeTarget, RegisterConstraintKey,
    baseline_target_register_environment, produced_effects, scalar_abi_cases, validate_effects,
    validated_effects,
};
use crate::catalog::selected_constraint_keys;
use register_model::{RegisterConstraintCatalogIdentity, RegisterConstraintFamily};
use selected_instructions::{
    MachineEffectCatalog, MachineEffectCatalogValidationError, MachineSemanticKind,
    machine_effect_catalog_identity,
};

fn sibling_target(target: NativeTarget) -> NativeTarget {
    match target.architecture {
        Architecture::X86_64 => {
            if target == NativeTarget::linux_x64() {
                NativeTarget::windows_x64()
            } else {
                NativeTarget::linux_x64()
            }
        }
        Architecture::Aarch64 => {
            if target == NativeTarget::linux_arm64() {
                NativeTarget::macos_arm64()
            } else {
                NativeTarget::linux_arm64()
            }
        }
    }
}

fn foreign_architecture_target(target: NativeTarget) -> NativeTarget {
    match target.architecture {
        Architecture::X86_64 => NativeTarget::linux_arm64(),
        Architecture::Aarch64 => NativeTarget::linux_x64(),
    }
}

fn forged_constraint_key() -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: u32::MAX,
    }
}

fn declaration_position(
    catalog: &MachineEffectCatalog,
    semantic: MachineSemanticKind,
    key: RegisterConstraintKey,
) -> usize {
    catalog
        .declarations
        .iter()
        .position(|declaration| declaration.semantic == semantic && declaration.constraint == key)
        .unwrap_or_else(|| panic!("effect catalog missing declaration {semantic:?}/{key:?}"))
}

#[test]
fn every_selected_rule_binds_the_declared_catalog_provenance() {
    let mut targets = Vec::new();
    let mut identities = Vec::new();
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = validated_effects(case, environment.constraints());
        let selected = environment.selected_keys();

        assert_eq!(catalog.catalog().target, case.target, "{}", case.convention);
        assert_eq!(
            catalog.catalog().register_constraints,
            environment.constraints().identity(),
            "{}",
            case.convention
        );
        assert_eq!(
            catalog.catalog().selected_keys,
            selected,
            "{}",
            case.convention
        );

        let expected = selected.declaration_keys();
        let actual = catalog
            .catalog()
            .declarations
            .iter()
            .map(|declaration| (declaration.semantic, declaration.constraint))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{}", case.convention);
        assert!(
            catalog
                .catalog()
                .declarations
                .windows(2)
                .all(|pair| pair[0].semantic <= pair[1].semantic),
            "{} declarations must stay in canonical semantic order",
            case.convention
        );
        for declaration in &catalog.catalog().declarations {
            assert!(
                environment.constraint(declaration.constraint).is_some(),
                "{} declaration {declaration:?} names no constraint row",
                case.convention
            );
        }

        for key in selected.in_identity_order() {
            assert!(
                catalog
                    .catalog()
                    .declarations
                    .iter()
                    .any(|declaration| declaration.constraint == key),
                "{} selected key {key:?} claims no declaration",
                case.convention
            );
        }

        assert_eq!(
            catalog.identity(),
            machine_effect_catalog_identity(catalog.catalog()),
            "{}",
            case.convention
        );
        if !targets.contains(&case.target) {
            targets.push(case.target);
        }
        if !identities.contains(&catalog.identity()) {
            identities.push(catalog.identity());
        }
    }
    assert_eq!(identities.len(), targets.len());
    assert_eq!(targets.len(), scalar_abi_cases().len() - 1);
}

#[test]
fn float_catalog_declares_fused_multiply_add_on_every_target() {
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation as Operation, IeeeFloatFormat,
    };
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = produced_effects(case, environment.constraints());
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let fused = MachineSemanticKind::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm {
                operation: Operation::FusedMultiplyAdd,
                format,
            });
            assert!(catalog.declarations.iter().any(|row| row.semantic == fused));
            if case.target.architecture == Architecture::X86_64 {
                let unfused =
                    MachineSemanticKind::NearestIeeeFloatArithmetic(IeeeFloatArithmeticForm {
                        operation: Operation::MultiplyThenAdd,
                        format,
                    });
                let mut substituted = catalog.clone();
                substituted
                    .declaration_by_semantic_mut(unfused)
                    .unwrap()
                    .semantic = fused;
                substituted.declarations.sort_by_key(|row| row.semantic);
                assert!(validate_effects(case, environment.constraints(), substituted).is_err());
            }
        }
    }
}

#[test]
fn every_selected_rule_rejects_catalog_provenance_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = produced_effects(case, environment.constraints());
        let foreign = foreign_architecture_target(case.target);
        macro_rules! reject_roster {
            ($corrupted:expr, $message:literal) => {
                assert_eq!(
                    validate_effects(case, environment.constraints(), $corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::DeclarationRosterMismatch
                    )),
                    $message,
                    case.convention
                );
            };
        }

        let mut corrupted = catalog.clone();
        corrupted.target = foreign;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::TargetArchitectureMismatch
            )),
            "{} foreign-architecture target claim must reject",
            case.convention
        );

        let mut corrupted = catalog.clone();
        corrupted.target = sibling_target(case.target);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::SemanticMismatch),
            "{} sibling target claim must reject",
            case.convention
        );

        let foreign_environment = baseline_target_register_environment(foreign).unwrap();
        assert_eq!(
            validate_effects(case, foreign_environment.constraints(), catalog.clone()),
            Err(EffectRejection::TargetArchitectureMismatch),
            "{} foreign-architecture constraint catalog must reject",
            case.convention
        );

        let mut corrupted = catalog.clone();
        corrupted.register_constraints = RegisterConstraintCatalogIdentity::from_bytes([0xA5; 32]);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::RegisterConstraintRootMismatch
            )),
            "{} forged constraint root must reject",
            case.convention
        );

        let mut corrupted = catalog.clone();
        corrupted.selected_keys.copy_i64 = corrupted.selected_keys.materialize_i64;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DuplicateSelectedConstraintKey
            )),
            "{} duplicated selected key must reject",
            case.convention
        );

        let mut corrupted = catalog.clone();
        corrupted.selected_keys.copy_bytes = Some(case.opposite_call);
        reject_roster!(corrupted, "{} borrowed unselected row must reject");

        let mut corrupted = catalog.clone();
        std::mem::swap(
            &mut corrupted.selected_keys.rows.add_i64,
            &mut corrupted.selected_keys.rows.subtract_i64,
        );
        reject_roster!(corrupted, "{} swapped selected rows must reject");

        let mut corrupted = catalog.clone();
        corrupted.selected_keys.load8 = None;
        reject_roster!(corrupted, "{} dropped selected row must reject");

        let mut corrupted = catalog.clone();
        corrupted.selected_keys.call_scalar.push(case.opposite_call);
        reject_roster!(corrupted, "{} appended unselected call row must reject");

        let mut corrupted = catalog.clone();
        corrupted.selected_keys = selected_constraint_keys(sibling_target(case.target))
            .expect("the sibling target declares a selected key set");
        reject_roster!(corrupted, "{} borrowed sibling selection must reject");
    }
}

#[test]
fn every_selected_rule_rejects_declaration_roster_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = produced_effects(case, environment.constraints());
        let keys = environment.selected_keys();
        macro_rules! reject_roster {
            ($corrupted:expr, $message:literal) => {
                assert_eq!(
                    validate_effects(case, environment.constraints(), $corrupted),
                    Err(EffectRejection::Structural(
                        MachineEffectCatalogValidationError::DeclarationRosterMismatch
                    )),
                    $message,
                    case.convention
                );
            };
        }

        let boundary = catalog
            .declarations
            .windows(2)
            .position(|pair| pair[0].semantic < pair[1].semantic)
            .expect("the roster crosses a semantic boundary");
        let mut corrupted = catalog.clone();
        corrupted.declarations.swap(boundary, boundary + 1);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::NonCanonicalDeclarations
            )),
            "{} reordered declarations must reject",
            case.convention
        );

        let mut corrupted = catalog.clone();
        corrupted.declarations.pop();
        reject_roster!(corrupted, "{} dropped declaration must reject");

        let mut corrupted = catalog.clone();
        let twin = corrupted.declarations[0].clone();
        corrupted.declarations.insert(1, twin);
        reject_roster!(corrupted, "{} duplicated declaration must reject");

        let mut corrupted = catalog.clone();
        corrupted.declarations[0].constraint = case.opposite_call;
        reject_roster!(corrupted, "{} borrowed constraint row must reject");

        let forged = forged_constraint_key();
        let mut corrupted = catalog.clone();
        corrupted.selected_keys.copy_bytes = Some(forged);
        let position = declaration_position(
            &corrupted,
            MachineSemanticKind::CopyBytes,
            keys.copy_bytes.unwrap(),
        );
        corrupted.declarations[position].constraint = forged;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::UnknownConstraint(
                    MachineSemanticKind::CopyBytes
                )
            )),
            "{} consistent unknown-row forgery must reject",
            case.convention
        );

        let mut corrupted = catalog.clone();
        let load8 = keys.load8.unwrap();
        let load16 = keys.load16.unwrap();
        corrupted.selected_keys.load8 = Some(load16);
        corrupted.selected_keys.load16 = Some(load8);
        let load8_position = declaration_position(&corrupted, MachineSemanticKind::Load8, load8);
        let load16_position = declaration_position(&corrupted, MachineSemanticKind::Load16, load16);
        corrupted.declarations[load8_position].constraint = load16;
        corrupted.declarations[load16_position].constraint = load8;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::SemanticMismatch),
            "{} consistent cross-bound selection must reject",
            case.convention
        );
    }
}
