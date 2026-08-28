use sha2::{Digest, Sha256};

use crate::{
    TerminalAllocatorAvailabilityIdentity, TerminalAllocatorAvailabilityPlan,
    TerminalAllocatorAvailabilityPolicy,
};

pub fn terminal_allocator_availability_identity(
    plan: &TerminalAllocatorAvailabilityPlan,
) -> TerminalAllocatorAvailabilityIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocator-availability.v1\0");
    bytes.extend_from_slice(&encode_terminal_allocator_availability_content(plan));
    TerminalAllocatorAvailabilityIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_allocator_availability_content(
    plan: &TerminalAllocatorAvailabilityPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.physical.bytes());
    match &plan.policy {
        TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1 => bytes.push(0),
        TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } => {
            bytes.push(1);
            length(&mut bytes, views.len());
            for view in views {
                bytes.extend_from_slice(&view.0.to_le_bytes());
            }
        }
    }
    length(&mut bytes, plan.classes.len());
    for row in &plan.classes {
        bytes.extend_from_slice(&row.class.0.to_le_bytes());
        length(&mut bytes, row.unconstrained_views.len());
        for view in &row.unconstrained_views {
            bytes.extend_from_slice(&view.0.to_le_bytes());
        }
    }
    bytes
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("allocator-availability identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
        TargetRegisterEnvironmentIdentity,
    };

    use super::*;
    use crate::TerminalRegisterClassAvailability;

    fn plan() -> TerminalAllocatorAvailabilityPlan {
        TerminalAllocatorAvailabilityPlan {
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([1; 32]),
            physical: PhysicalRegisterModelIdentity::from_bytes([2; 32]),
            policy: TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                views: vec![RegisterViewId(1)],
            },
            classes: vec![TerminalRegisterClassAvailability {
                class: RegisterClassId(0),
                unconstrained_views: vec![RegisterViewId(1)],
            }],
        }
    }

    #[test]
    fn identity_binds_every_availability_domain() {
        let baseline = terminal_allocator_availability_identity(&plan());
        let mut changes = Vec::new();

        let mut changed = plan();
        changed.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([3; 32]);
        changes.push(changed);
        let mut changed = plan();
        changed.physical = PhysicalRegisterModelIdentity::from_bytes([4; 32]);
        changes.push(changed);
        let mut changed = plan();
        changed.policy = TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1;
        changes.push(changed);
        let mut changed = plan();
        changed.classes[0].class = RegisterClassId(1);
        changes.push(changed);
        let mut changed = plan();
        changed.classes[0].unconstrained_views[0] = RegisterViewId(2);
        changes.push(changed);

        assert_eq!(baseline, terminal_allocator_availability_identity(&plan()));
        for changed in changes {
            assert_ne!(baseline, terminal_allocator_availability_identity(&changed));
        }
    }

    #[test]
    fn codec_round_trips_plain_plan_and_rejects_framing_or_tamper() {
        let source = plan();
        let encoded = source.encode();
        assert_eq!(
            TerminalAllocatorAvailabilityPlan::decode(&encoded).unwrap(),
            source
        );

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            TerminalAllocatorAvailabilityPlan::decode(&wrong_magic),
            Err(crate::TerminalAllocatorAvailabilityDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            TerminalAllocatorAvailabilityPlan::decode(&wrong_version),
            Err(crate::TerminalAllocatorAvailabilityDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            TerminalAllocatorAvailabilityPlan::decode(&encoded[..encoded.len() - 1]),
            Err(crate::TerminalAllocatorAvailabilityDecodeError::Truncated)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            TerminalAllocatorAvailabilityPlan::decode(&trailing),
            Err(crate::TerminalAllocatorAvailabilityDecodeError::TrailingBytes)
        );
        let mut tampered = encoded;
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(
            TerminalAllocatorAvailabilityPlan::decode(&tampered),
            Err(crate::TerminalAllocatorAvailabilityDecodeError::IdentityMismatch)
        );
    }
}
