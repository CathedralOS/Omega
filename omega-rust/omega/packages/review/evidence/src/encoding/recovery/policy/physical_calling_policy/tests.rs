use super::*;
use crate::record::{
    PackagePolicyEntryControl, PackagePolicyEntryStack, PackagePolicyMachineRegime,
    PackagePolicyMachineState, PackagePolicyMachineStateSet, PackagePolicyPreemption,
    PackagePolicyStatePlan, PackageReviewBoundaryValueClass, PackageReviewBoundaryValueLocation,
    PackageReviewBoundaryValuePlacement, PackageReviewBoundaryValueShape,
    PackageReviewMachineRegister,
};

fn fixture() -> PackagePolicyPhysicalCallingContract {
    PackagePolicyPhysicalCallingContract {
        policy: PackageReviewBoundaryCallingPolicy::SystemVAMD64,
        parameters: Vec::new(),
        result: None,
        ordinary_clobbers: vec![
            PackageReviewMachineRegister::X86Rax,
            PackageReviewMachineRegister::X86Rdx,
        ],
        stack_alignment: 16,
        shadow_bytes: 0,
        entry_control: PackagePolicyEntryControl::CallReturn,
        state: PackagePolicyStatePlan {
            initial_regime: PackagePolicyMachineRegime::X86Long64,
            interrupted_state: PackagePolicyMachineStateSet::new([
                PackagePolicyMachineState::GeneralRegisters,
                PackagePolicyMachineState::Flags,
            ]),
            saved_state: PackagePolicyMachineStateSet::default(),
            restored_state: PackagePolicyMachineStateSet::default(),
            permitted_transitive_use: PackagePolicyMachineStateSet::default(),
            stack: PackagePolicyEntryStack::ProviderSelected,
            preemption: PackagePolicyPreemption::ProviderDefined,
        },
    }
}

fn recover(bytes: &[u8]) -> Result<PackagePolicyPhysicalCallingContract, Error> {
    PackagePolicyPhysicalCallingContract::recover_canonical(
        bytes,
        PackagePolicyRecoveryLimits::default(),
    )
}

#[test]
fn envelope_rejects_every_truncated_prefix_unknown_version_and_trailing_bytes() {
    let component = fixture();
    let bytes = component.canonical_bytes().unwrap();
    assert_eq!(recover(&bytes).unwrap(), component);
    crate::encoding::encode::text_test_support::component(
        crate::encoding::encode::text_test_support::Component::Physical(&component),
    );
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "truncated at {end}");
    }
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes.clone();
    changed[PHYSICAL_CALLING_POLICY_MAGIC.len()] = 255;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(recover(&changed), Err(Error::TrailingBytes));
}

#[test]
fn decoder_and_writer_reject_reordered_and_duplicate_sets() {
    let component = fixture();
    let bytes = component.canonical_bytes().unwrap();
    let body = PHYSICAL_CALLING_POLICY_MAGIC.len() + 2;
    // policy, parameter count, result option, clobber count.
    let clobbers = body + 1 + 8 + 1 + 8;
    assert_eq!(&bytes[clobbers..clobbers + 2], &[0, 2]);
    let state = clobbers + 2 + 2 + 2 + 1 + 1 + 8;
    assert_eq!(&bytes[state..state + 2], &[0, 2]);
    for offset in [clobbers, state] {
        let mut changed = bytes.clone();
        changed.swap(offset, offset + 1);
        assert_eq!(recover(&changed), Err(Error::NonCanonicalEncoding));
        let mut changed = bytes.clone();
        changed[offset + 1] = changed[offset];
        assert_eq!(recover(&changed), Err(Error::NonCanonicalEncoding));
    }
    let mut changed = component.clone();
    changed.ordinary_clobbers.swap(0, 1);
    assert!(changed.canonical_bytes().is_err());
    let mut changed = component;
    changed.ordinary_clobbers[1] = changed.ordinary_clobbers[0];
    assert!(changed.canonical_bytes().is_err());
}

#[test]
fn all_closed_tag_entrances_reject_unknown_variants() {
    let bytes = fixture().canonical_bytes().unwrap();
    let body = PHYSICAL_CALLING_POLICY_MAGIC.len() + 2;
    let clobbers = body + 18;
    let control = clobbers + 6;
    let regime = control + 1;
    let state = regime + 9;
    let stack = state + 2 + 3 * 8;
    let preemption = stack + 1;
    assert_eq!(preemption + 1, bytes.len());
    for offset in [
        body,
        body + 9,
        clobbers,
        control,
        regime,
        state,
        stack,
        preemption,
    ] {
        let mut changed = bytes.clone();
        changed[offset] = 255;
        assert_eq!(recover(&changed), Err(Error::InvalidTag), "tag at {offset}");
    }
    // Placement helpers use the exact established representation byte tags.
    for tag in [5, 127, 255] {
        let bytes = [tag];
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(
            placement::value_placement(&mut reader),
            Err(Error::InvalidTag)
        );
    }
    for bytes in [&[3, 2][..], &[3, 0, 2][..]] {
        let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(
            placement::value_placement(&mut reader),
            Err(Error::InvalidTag)
        );
    }
}

#[test]
fn malformed_nested_locations_and_optional_copy_tags_reject() {
    let mut component = fixture();
    component.parameters = vec![PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: PackageReviewBoundaryValueClass::Integer,
            byte_size: 8,
            alignment: 8,
        },
        locations: vec![PackageReviewBoundaryValueLocation::Indirect {
            pointer: crate::record::PackageReviewIndirectPointerLocation::Register(
                PackageReviewMachineRegister::X86Rax,
            ),
            copy_stack_byte_offset: None,
            byte_size: 8,
            alignment: 8,
        }],
    }];
    let bytes = component.canonical_bytes().unwrap();
    assert_eq!(recover(&bytes).unwrap(), component);
    crate::encoding::encode::text_test_support::component(
        crate::encoding::encode::text_test_support::Component::Physical(&component),
    );
    // Header + policy + parameter count + class/geometry + location count.
    let location = PHYSICAL_CALLING_POLICY_MAGIC.len() + 2 + 1 + 8 + 5 + 8;
    assert_eq!(&bytes[location..location + 4], &[2, 0, 0, 0]);
    for offset in location..location + 4 {
        let mut changed = bytes.clone();
        changed[offset] = 255;
        assert_eq!(recover(&changed), Err(Error::InvalidTag));
    }
}

#[test]
fn component_recovery_enforces_byte_element_and_owned_storage_limits() {
    let bytes = fixture().canonical_bytes().unwrap();
    let limits = |maximum_bytes, elements, owned| {
        PackagePolicyRecoveryLimits::new(maximum_bytes, usize::MAX, elements, owned, usize::MAX)
    };
    assert_eq!(
        PackagePolicyPhysicalCallingContract::recover_canonical(
            &bytes,
            limits(bytes.len() - 1, usize::MAX, usize::MAX),
        ),
        Err(Error::InputTooLarge)
    );
    assert_eq!(
        PackagePolicyPhysicalCallingContract::recover_canonical(
            &bytes,
            limits(usize::MAX, 3, usize::MAX),
        ),
        Err(Error::ElementLimitExceeded)
    );
    assert_eq!(
        PackagePolicyPhysicalCallingContract::recover_canonical(
            &bytes,
            limits(usize::MAX, usize::MAX, 0),
        ),
        Err(Error::AllocationLimitExceeded)
    );
    let owned = 2 * std::mem::size_of::<PackageReviewMachineRegister>()
        + 2 * std::mem::size_of::<PackagePolicyMachineState>()
        + bytes.len();
    assert_eq!(
        PackagePolicyPhysicalCallingContract::recover_canonical(
            &bytes,
            limits(bytes.len(), 4, owned),
        )
        .unwrap(),
        fixture()
    );
    assert_eq!(
        PackagePolicyPhysicalCallingContract::recover_canonical(
            &bytes,
            limits(bytes.len(), 4, owned - 1),
        ),
        Err(Error::AllocationLimitExceeded)
    );

    let mut changed = bytes.clone();
    let count = PHYSICAL_CALLING_POLICY_MAGIC.len() + 3;
    changed[count..count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(recover(&changed).is_err());
    let mut changed = bytes;
    changed[count..count + 8].copy_from_slice(&65_536u64.to_le_bytes());
    assert_eq!(recover(&changed), Err(Error::UnexpectedEnd));
}

#[test]
fn writer_enforces_aggregate_elements() {
    let mut component = fixture();
    let placement = PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: PackageReviewBoundaryValueClass::Integer,
            byte_size: 8,
            alignment: 8,
        },
        locations: Vec::new(),
    };
    component.parameters = vec![placement; 65_537];
    assert!(component.canonical_bytes().is_err());
}
