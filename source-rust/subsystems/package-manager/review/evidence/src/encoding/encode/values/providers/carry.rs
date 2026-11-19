// Canonical encoding for carry policies across suspension, CPU, host-thread, and address axes.

use crate::encoding::encode::encoder::Encoder;

pub(crate) fn encode_carry_policy(
    encoder: &mut Encoder,
    policy: language_core::CarryPolicy,
) {
    let _ = encoder.field("suspension", |encoder| {
        match policy.suspension {
            language_core::CarrySuspension::Forbidden => encoder.tag("forbidden", 0),
            language_core::CarrySuspension::Allowed => encoder.tag("allowed", 1),
        };
        Ok(())
    });
    let _ = encoder.field("cpu", |encoder| {
        match policy.cpu {
            language_core::CarryCpu::Origin => encoder.tag("origin", 0),
            language_core::CarryCpu::Any => encoder.tag("any", 1),
        };
        Ok(())
    });
    let _ = encoder.field("host_thread", |encoder| {
        match policy.host_thread {
            language_core::CarryHostThread::Origin => encoder.tag("origin", 0),
            language_core::CarryHostThread::Any => encoder.tag("any", 1),
        };
        Ok(())
    });
    let _ = encoder.field("address", |encoder| {
        match policy.address {
            language_core::CarryAddress::Stable => encoder.tag("stable", 0),
            language_core::CarryAddress::Movable => encoder.tag("movable", 1),
        };
        Ok(())
    });
}
