use crate::tests::*;

use super::fixture::{call_requirements, ordinary_requirements, stage, wide_budget};

#[test]
fn preserved_register_units_form_exact_target_storage_slots_and_replay() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (requirements, environment) = call_requirements(target);
        let first = stage(&requirements, &environment, wide_budget()).unwrap();
        let repeated = stage(&requirements, &environment, wide_budget()).unwrap();
        assert_eq!(first, repeated);
        assert_eq!(first.receipt().target(), target);
        assert_eq!(
            first.receipt().policy(),
            NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1
        );
        assert_eq!(
            first.receipt().callee_saved_requirements(),
            requirements.receipt().identity()
        );
        assert_eq!(
            first.receipt().identity(),
            non_authoritative_callee_save_storage_identity(first.plan())
        );
        assert!(first.receipt().slot_count() > 0);

        let slots = first
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.slots)
            .filter(|slot| {
                slot.modified_units.iter().any(|requirement| {
                    requirement.witnesses.iter().any(|witness| {
                        matches!(
                            witness,
                            CalleeSavedModificationWitness::OperandDefinition {
                                virtual_register: VirtualRegisterId(5),
                                ..
                            }
                        )
                    })
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].size_bytes, 8);
        assert_eq!(slots[0].alignment_bytes, 8);
        assert_eq!(slots[0].abstract_offset_bytes, 0);
        assert_eq!(
            slots[0].preserved_units.len(),
            if target == NativeTarget::linux_x64() {
                4
            } else {
                1
            }
        );
        assert_eq!(
            slots[0].modified_units.len(),
            if target == NativeTarget::linux_x64() {
                4
            } else {
                1
            }
        );
        assert!(first.plan().functions.iter().all(|function| {
            function.slots.iter().enumerate().all(|(index, slot)| {
                slot.id
                    == NonAuthoritativeCalleeSaveSlotId(
                        u16::try_from(index).expect("fixture slot index fits u16"),
                    )
            })
        }));

        let replayed = validate_non_authoritative_callee_save_storage(
            &requirements,
            &environment,
            first.plan().clone(),
        )
        .unwrap();
        assert_eq!(replayed, first);
    }
}

#[test]
fn empty_requirements_retain_explicit_functions_and_neutral_geometry() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (requirements, environment) = ordinary_requirements(target);
        let storage = stage(&requirements, &environment, wide_budget()).unwrap();
        assert_eq!(storage.receipt().target(), target);
        assert_eq!(storage.receipt().slot_count(), 0);
        assert_eq!(storage.receipt().max_abstract_area_bytes(), 0);
        assert_eq!(storage.receipt().max_abstract_area_alignment(), 1);
        assert_eq!(
            storage.plan().functions.len(),
            requirements.plan().functions.len()
        );
        assert!(
            storage
                .plan()
                .functions
                .iter()
                .all(|function| function.slots.is_empty())
        );
        assert!(storage.plan().functions.iter().all(|function| {
            function.abstract_area_bytes == 0 && function.abstract_area_alignment == 1
        }));
    }
}
