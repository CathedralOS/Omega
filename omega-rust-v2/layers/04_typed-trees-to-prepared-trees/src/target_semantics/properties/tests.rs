use super::*;
use crate::{LayoutExtent, LayoutGeometry, TargetIdentity, TargetObservation, TargetSemanticsError};

#[test]
fn replays_the_exact_version_subject_and_meaning_of_closed_properties() {
    let target = TargetIdentity {
        profile: "uefi-x64".into(),
        semantics_version: 1,
        realization: Arc::from(&b"selected target model"[..]),
    };
    let layout = Arc::new(SelectedLayout {
        target: target.clone(),
        subject: ValueType::Integer {
            signed: false,
            width: Some(32),
        },
        schema: Arc::from(&b"32-bit integer schema"[..]),
        plan: Arc::from(&b"selected data layout"[..]),
        geometry: LayoutGeometry {
            extent: LayoutExtent::Fixed(4),
            alignment: Some(4),
        },
    });
    let address_space = AddressSpaceIdentity(Arc::from(&b"application addresses"[..]));
    let profile = EntryProfileIdentity(Arc::from(&b"returning UEFI application"[..]));
    let bound = BigUint::from(1u8) << 64usize;
    let stack = EntryStackGuarantee {
        minimum_available_bytes: 128 * 1024,
        alignment: 16,
    };
    let properties = [
        TargetProperty::ExclusiveAddressBound {
            address_space: address_space.clone(),
            exclusive_bound: bound.clone(),
        },
        TargetProperty::LayoutEndianness {
            layout: layout.clone(),
            endianness: Endianness::Little,
        },
        TargetProperty::EntryStack {
            profile: profile.clone(),
            guarantee: stack,
        },
    ]
    .map(|value| SelectedTargetProperty { projection_version: 1, value });
    let capsule = || TargetSemantics::new(target.clone(), [layout.as_ref().clone()], []).unwrap();
    let selected = capsule().with_target_properties(properties.clone()).unwrap();
    let queries = [
        TargetPropertyQuery::ExclusiveAddressBound(&address_space),
        TargetPropertyQuery::LayoutEndianness(&layout.subject),
        TargetPropertyQuery::EntryStack(&profile),
    ];
    let observations = queries.map(|query| selected.observe_target_property(query).unwrap());
    assert_eq!(
        observations,
        properties.clone().map(|property| TargetObservation::Property {
            target: target.clone(),
            property
        })
    );
    assert_eq!(bound.to_string(), "18446744073709551616");

    let changed_meanings = [
        TargetProperty::ExclusiveAddressBound {
            address_space: address_space.clone(),
            exclusive_bound: &bound - 1u8,
        },
        TargetProperty::LayoutEndianness {
            layout: layout.clone(),
            endianness: Endianness::Big,
        },
        TargetProperty::EntryStack {
            profile: profile.clone(),
            guarantee: EntryStackGuarantee { alignment: 32, ..stack },
        },
    ];
    for (index, (receipt, changed_value)) in observations.iter().zip(changed_meanings).enumerate() {
        assert!(selected.matches_observation(receipt));
        let missing = capsule()
            .with_target_properties(
                properties
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| *position != index)
                    .map(|(_, value)| value.clone()),
            )
            .unwrap();
        assert!(missing.observe_target_property(queries[index]).is_none());
        assert!(!missing.matches_observation(receipt));

        let mut changed = properties.clone();
        changed[index].value = changed_value;
        let changed = capsule().with_target_properties(changed).unwrap();
        assert!(!changed.matches_observation(receipt));

        let mut updated = properties.clone();
        updated[index].projection_version += 1;
        let updated = capsule().with_target_properties(updated).unwrap();
        assert!(!updated.matches_observation(receipt));
    }

    let other_layout = Arc::new(SelectedLayout {
        plan: Arc::from(&b"another plan with the same geometry"[..]),
        ..layout.as_ref().clone()
    });
    let other_properties = [
        TargetProperty::ExclusiveAddressBound {
            address_space: AddressSpaceIdentity(Arc::from(&b"device addresses"[..])),
            exclusive_bound: bound,
        },
        TargetProperty::LayoutEndianness {
            layout: other_layout.clone(),
            endianness: Endianness::Little,
        },
        TargetProperty::EntryStack {
            profile: EntryProfileIdentity(Arc::from(&b"non-returning UEFI loader"[..])),
            guarantee: stack,
        },
    ]
    .map(|value| SelectedTargetProperty { projection_version: 1, value });
    assert!(matches!(
        capsule().with_target_properties(other_properties.clone()),
        Err(TargetSemanticsError::InvalidTargetProperty)
    ));
    let other = TargetSemantics::new(target.clone(), [other_layout.as_ref().clone()], [])
        .unwrap()
        .with_target_properties(other_properties)
        .unwrap();
    assert!(
        observations
            .iter()
            .all(|receipt| !other.matches_observation(receipt))
    );
    assert!(matches!(
        selected.with_target_properties([properties[0].clone()]),
        Err(TargetSemanticsError::DuplicateTargetProperty)
    ));
}
