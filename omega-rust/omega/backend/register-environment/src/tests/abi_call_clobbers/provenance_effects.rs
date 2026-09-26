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

/// A supported same-architecture target whose selected contract differs.
/// `windows_x64` and `uefi_x64` pin the same architecture/format and are one
/// contract at this layer, so the sibling of each COFF member is the ELF
/// target.
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

/// A supported target on the other architecture.
fn foreign_architecture_target(target: NativeTarget) -> NativeTarget {
    match target.architecture {
        Architecture::X86_64 => NativeTarget::linux_arm64(),
        Architecture::Aarch64 => NativeTarget::linux_x64(),
    }
}

/// A constraint key no architecture catalog owns.
fn forged_constraint_key() -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant: u32::MAX,
    }
}

/// The position of the one declaration carrying `semantic`/`key` in this
/// catalog's canonical roster.
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

/// Every selected rule on every declared target/ABI pair rides on a catalog
/// whose provenance is bound end to end: the exact target, the validated
/// constraint-catalog root, the environment's own selected keys, a canonical
/// semantic-major declaration roster over real constraint rows, and a sealed
/// identity that distinguishes every distinct target contract.
#[test]
fn every_selected_rule_binds_the_declared_catalog_provenance() {
    let mut targets = Vec::new();
    let mut identities = Vec::new();
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = validated_effects(case, environment.constraints());
        let selected = environment.selected_keys();

        // The catalog names this exact target, this validated constraint
        // catalog's root, and this environment's selected keys — provenance,
        // not a re-derived roster.
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

        // The declaration roster is exactly the selected keys' canonical
        // semantic-major expansion: one declaration per (semantic, key) pair,
        // in declared order, each bound to a real constraint row.
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

        // Every selected key is claimed by at least one declaration; no
        // selection silently drops out of the roster.
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

        // The sealed identity replays deterministically from the raw
        // catalog. Two declared pairs naming the same contract — the COFF
        // targets share one `NativeTarget` — seal the same catalog; every
        // distinct target contract seals a distinct one.
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

/// The catalog head's provenance is independently enforced on every declared
/// target/ABI pair: a foreign or sibling target claim, a forged or foreign
/// constraint root, and a duplicated, dropped, borrowed, or wholesale-replaced
/// selection each fail admission at their own check.
#[test]
fn every_selected_rule_rejects_catalog_provenance_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = produced_effects(case, environment.constraints());
        let foreign = foreign_architecture_target(case.target);

        // A catalog cannot claim a foreign architecture: admission binds
        // catalog.target to the constraint catalog's architecture.
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

        // A same-architecture sibling claim passes structural admission —
        // the architecture matches — but canonical re-derivation binds the
        // exact target contract.
        let mut corrupted = catalog.clone();
        corrupted.target = sibling_target(case.target);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::SemanticMismatch),
            "{} sibling target claim must reject",
            case.convention
        );

        // A foreign-architecture constraint catalog cannot join this ISA's
        // effect validation at all.
        let foreign_environment = baseline_target_register_environment(foreign).unwrap();
        assert_eq!(
            validate_effects(case, foreign_environment.constraints(), catalog.clone()),
            Err(EffectRejection::TargetArchitectureMismatch),
            "{} foreign-architecture constraint catalog must reject",
            case.convention
        );

        // A forged constraint root fails admission: the catalog must name the
        // validated catalog's sealed identity, not any byte pattern.
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

        // The selection cannot name one row twice.
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

        // The selection cannot borrow a real row the ABI does not select: the
        // opposite ABI's call row exists in the constraint catalog but is not
        // this pair's CopyBytes authority.
        let mut corrupted = catalog.clone();
        corrupted.selected_keys.copy_bytes = Some(case.opposite_call);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} borrowed unselected row must reject",
            case.convention
        );

        // The selection cannot swap two owned rows between their semantics.
        let mut corrupted = catalog.clone();
        std::mem::swap(
            &mut corrupted.selected_keys.add_i64,
            &mut corrupted.selected_keys.subtract_i64,
        );
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} swapped selected rows must reject",
            case.convention
        );

        // The selection cannot drop an optional row it does select.
        let mut corrupted = catalog.clone();
        corrupted.selected_keys.load8 = None;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} dropped selected row must reject",
            case.convention
        );

        // The selection cannot grow an extra call row it does not select.
        let mut corrupted = catalog.clone();
        corrupted.selected_keys.call_scalar.push(case.opposite_call);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} appended unselected call row must reject",
            case.convention
        );

        // The selection cannot be replaced wholesale by a different ABI's.
        let mut corrupted = catalog.clone();
        corrupted.selected_keys = selected_constraint_keys(sibling_target(case.target))
            .expect("the sibling target declares a selected key set");
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} borrowed sibling selection must reject",
            case.convention
        );
    }
}

/// The declaration roster's provenance is independently enforced on every
/// declared target/ABI pair: canonical ordering, exact length, and each
/// declaration's own constraint binding, plus the consistent forgeries that
/// survive structural admission only to fail canonical re-derivation.
#[test]
fn every_selected_rule_rejects_declaration_roster_forgery_on_every_target() {
    for case in scalar_abi_cases() {
        let environment = baseline_target_register_environment(case.target).unwrap();
        let catalog = produced_effects(case, environment.constraints());
        let keys = environment.selected_keys();

        // Reordering the roster across a semantic boundary breaks canonical
        // declaration order before any roster comparison runs.
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

        // The roster cannot lose its last declaration.
        let mut corrupted = catalog.clone();
        corrupted.declarations.pop();
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} dropped declaration must reject",
            case.convention
        );

        // A duplicated declaration inserted beside its twin keeps canonical
        // order but breaks the exact roster length.
        let mut corrupted = catalog.clone();
        let twin = corrupted.declarations[0].clone();
        corrupted.declarations.insert(1, twin);
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} duplicated declaration must reject",
            case.convention
        );

        // A declaration cannot point at a real row the selection does not
        // name.
        let mut corrupted = catalog.clone();
        corrupted.declarations[0].constraint = case.opposite_call;
        assert_eq!(
            validate_effects(case, environment.constraints(), corrupted),
            Err(EffectRejection::Structural(
                MachineEffectCatalogValidationError::DeclarationRosterMismatch
            )),
            "{} borrowed constraint row must reject",
            case.convention
        );

        // A consistent forgery — the selection and its declaration moved
        // together to a key no constraint catalog owns — keeps the roster
        // self-consistent and fails at the row lookup.
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

        // A consistent cross-binding — the Load8 and Load16 selections and
        // their declarations exchanged — survives structural admission
        // because each declaration still pins its own semantic surface; only
        // canonical re-derivation sees the borrowed rows.
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
