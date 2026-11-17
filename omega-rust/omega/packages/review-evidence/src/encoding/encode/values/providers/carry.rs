// Canonical encoding for carry policies across suspension, CPU, host-thread, and address axes.

use crate::encoding::encode::encoder::Encoder;

pub(crate) fn encode_carry_policy(encoder: &mut Encoder, policy: declaration_contracts::CarryPolicy) {
    let _ = encoder.field("suspension", |encoder| {
        match policy.suspension {
            declaration_contracts::CarrySuspension::Forbidden => encoder.tag("forbidden", 0),
            declaration_contracts::CarrySuspension::Allowed => encoder.tag("allowed", 1),
        };
        Ok(())
    });
    let _ = encoder.field("cpu", |encoder| {
        match policy.cpu {
            declaration_contracts::CarryCpu::Origin => encoder.tag("origin", 0),
            declaration_contracts::CarryCpu::Any => encoder.tag("any", 1),
        };
        Ok(())
    });
    let _ = encoder.field("host_thread", |encoder| {
        match policy.host_thread {
            declaration_contracts::CarryHostThread::Origin => encoder.tag("origin", 0),
            declaration_contracts::CarryHostThread::Any => encoder.tag("any", 1),
        };
        Ok(())
    });
    let _ = encoder.field("address", |encoder| {
        match policy.address {
            declaration_contracts::CarryAddress::Stable => encoder.tag("stable", 0),
            declaration_contracts::CarryAddress::Movable => encoder.tag("movable", 1),
        };
        Ok(())
    });
}
