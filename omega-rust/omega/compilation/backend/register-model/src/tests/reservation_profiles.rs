use super::{miniature_model, validated_miniature_model};
use crate::{
    RegisterReservationProfile, RegisterReservationProfileValidationError, RegisterUnitId,
    validate_physical_register_model, validate_register_reservation_profile,
};

#[test]
fn active_reservation_profile_is_exact_canonical_and_model_bound() {
    let model = validated_miniature_model();
    let target = target::NativeTarget::linux_x64();
    let profile = RegisterReservationProfile {
        name: "test.policy".into(),
        active_overlays: vec!["test.reserve-r0".into(), "test.reserve-v0".into()],
    };
    let validated = validate_register_reservation_profile(profile.clone(), target, &model)
        .expect("canonical profile must validate");
    assert_eq!(
        validated.reserved_units(),
        &[RegisterUnitId(0), RegisterUnitId(1)]
    );
    assert_eq!(
        validated.identity(),
        validate_register_reservation_profile(profile.clone(), target, &model)
            .unwrap()
            .identity()
    );

    let one_overlay = validate_register_reservation_profile(
        RegisterReservationProfile {
            name: profile.name.clone(),
            active_overlays: vec!["test.reserve-r0".into()],
        },
        target,
        &model,
    )
    .unwrap();
    assert_eq!(one_overlay.reserved_units(), &[RegisterUnitId(0)]);
    assert_ne!(one_overlay.identity(), validated.identity());

    let renamed = validate_register_reservation_profile(
        RegisterReservationProfile {
            name: "test.policy-renamed".into(),
            active_overlays: profile.active_overlays.clone(),
        },
        target,
        &model,
    )
    .unwrap();
    assert_ne!(renamed.identity(), validated.identity());
    let windows = validate_register_reservation_profile(
        profile.clone(),
        target::NativeTarget::windows_x64(),
        &model,
    )
    .unwrap();
    assert_ne!(windows.identity(), validated.identity());
    let mut changed_model = miniature_model();
    changed_model.units[0].name.push_str(".changed");
    let changed_model = validate_physical_register_model(changed_model).unwrap();
    let changed_model_profile =
        validate_register_reservation_profile(profile.clone(), target, &changed_model).unwrap();
    assert_ne!(changed_model_profile.identity(), validated.identity());

    let mut duplicate = profile.clone();
    duplicate.active_overlays[1] = duplicate.active_overlays[0].clone();
    assert_eq!(
        validate_register_reservation_profile(duplicate, target, &model),
        Err(RegisterReservationProfileValidationError::NonCanonicalOverlayNames)
    );
    let unknown = RegisterReservationProfile {
        name: profile.name,
        active_overlays: vec!["unknown".into()],
    };
    assert_eq!(
        validate_register_reservation_profile(unknown, target, &model),
        Err(RegisterReservationProfileValidationError::UnknownOverlay(
            "unknown".into()
        ))
    );
    assert_eq!(
        validate_register_reservation_profile(
            RegisterReservationProfile {
                name: "test.policy".into(),
                active_overlays: Vec::new(),
            },
            target::NativeTarget::linux_arm64(),
            &model,
        ),
        Err(RegisterReservationProfileValidationError::TargetArchitectureMismatch)
    );
}
