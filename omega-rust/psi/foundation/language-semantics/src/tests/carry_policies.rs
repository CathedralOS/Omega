use crate::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension,
    Multiplicity,
};

#[test]
fn multiplicity_default_is_affine() {
    // Ordinary data defaults to Affine (the record's mapping); `[copy]`
    // opts into Unrestricted, `[linear]` into Linear.
    assert_eq!(Multiplicity::default(), Multiplicity::Affine);
}

#[test]
fn carry_axes_compose_independently_and_fail_closed() {
    let cpu_local = CarryPolicy {
        suspension: CarrySuspension::Allowed,
        cpu: CarryCpu::Origin,
        host_thread: CarryHostThread::Any,
        address: CarryAddress::Movable,
    };
    let pinned = CarryPolicy {
        suspension: CarrySuspension::Allowed,
        cpu: CarryCpu::Any,
        host_thread: CarryHostThread::Any,
        address: CarryAddress::Stable,
    };

    assert_eq!(
        cpu_local.intersect(pinned),
        CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        }
    );
    assert!(CarryPolicy::PERMISSIVE.permits(cpu_local));
    assert!(!CarryPolicy::STRICT.permits(cpu_local));
    assert_eq!(CarryPolicy::default(), CarryPolicy::STRICT);
}

#[test]
fn carry_permissions_are_closed_named_positive_relaxations() {
    let mut policy = CarryPolicy::STRICT;
    for permission in CarryPermission::ALL {
        assert_eq!(
            CarryPermission::from_name(permission.name()),
            Some(permission)
        );
        policy = permission.relax(policy);
    }
    assert_eq!(policy, CarryPolicy::PERMISSIVE);
    assert_eq!(CarryPermission::from_name("Carry::Portable"), None);
    assert_eq!(CarryPermission::from_name("Carry::Anywhere"), None);
}
