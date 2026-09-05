use crate::tests::*;

use super::fixture::{call_requirements, stage, wide_budget};

fn rejects_noncanonical(
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    plan: NonAuthoritativeCalleeSaveStoragePlan,
) {
    assert_eq!(
        validate_non_authoritative_callee_save_storage(requirements, environment, plan),
        Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage)
    );
}

macro_rules! modified_function {
    ($plan:expr) => {{
        let index = $plan
            .functions
            .iter()
            .position(|function| !function.slots.is_empty())
            .expect("call fixture has callee-save storage");
        &mut $plan.functions[index]
    }};
}

#[test]
fn replay_rejects_every_root_and_retained_storage_field_corruption() {
    let (requirements, environment) = call_requirements(NativeTarget::linux_x64());
    let canonical = stage(&requirements, &environment, wide_budget())
        .unwrap()
        .plan()
        .clone();

    let mut root = canonical.clone();
    root.callee_saved_requirements =
        AllocatedCalleeSavedRequirementIdentity::from_bytes([0x61; 32]);
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, root),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    let mut catalog = canonical.clone();
    catalog.register_environment =
        register_model::TargetRegisterEnvironmentIdentity::from_bytes([0x62; 32]);
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, catalog),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    let mut physical = canonical.clone();
    physical.physical_register_model =
        register_model::PhysicalRegisterModelIdentity::from_bytes([0x63; 32]);
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, physical),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    let mut storage_catalog = canonical.clone();
    storage_catalog.preservation_storage_catalog =
        register_model::PreservationStorageCatalogIdentity::from_bytes([0x64; 32]);
    assert_eq!(
        validate_non_authoritative_callee_save_storage(
            &requirements,
            &environment,
            storage_catalog,
        ),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    let mut target = canonical.clone();
    target.target = NativeTarget::linux_arm64();
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, target),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    let mut usage = canonical.clone();
    usage.usage.validation_steps += 1;
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, usage),
        Err(NonAuthoritativeCalleeSaveStorageError::UsageMismatch)
    );

    let mut abi = canonical.clone();
    abi.abi = FrameAbiPreservationConvention::MicrosoftX64;
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, abi),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    let mut roster = canonical.clone();
    roster.callee_saved_units.pop();
    assert_eq!(
        validate_non_authoritative_callee_save_storage(&requirements, &environment, roster),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );

    for corrupt in [
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            plan.functions.reverse();
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).machine = MachineId::new(96_001).unwrap();
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).kind = AllocatedCalleeSavedFunctionKind::StructuralUnit;
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].id = NonAuthoritativeCalleeSaveSlotId(99);
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            let duplicate = modified_function!(plan).slots[0].clone();
            modified_function!(plan).slots.insert(0, duplicate);
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].storage_group =
                register_model::PreservationStorageGroupId(u16::MAX);
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].storage_view = RegisterViewId(u16::MAX);
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].preserved_units.pop();
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].preserved_units.reverse();
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].modified_units[0].unit = RegisterUnitId(u16::MAX);
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].modified_units[0].witnesses[0] =
                CalleeSavedModificationWitness::ImplicitClobber {
                    block: selected_instructions::SelectedBlockId(999),
                    instruction: SelectedInstructionId(999),
                };
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].abstract_offset_bytes += 1;
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].size_bytes += 1;
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).slots[0].alignment_bytes *= 2;
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).abstract_area_bytes += 1;
        },
        |plan: &mut NonAuthoritativeCalleeSaveStoragePlan| {
            modified_function!(plan).abstract_area_alignment *= 2;
        },
    ] {
        let mut changed = canonical.clone();
        corrupt(&mut changed);
        rejects_noncanonical(&requirements, &environment, changed);
    }

    let (foreign_requirements, foreign_environment) =
        call_requirements(NativeTarget::linux_arm64());
    assert_eq!(
        validate_non_authoritative_callee_save_storage(
            &foreign_requirements,
            &foreign_environment,
            canonical,
        ),
        Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch)
    );
}
