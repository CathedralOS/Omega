use crate::encoding::encode::encoder::Encoder;

pub(crate) fn encode_carry_policy(encoder: &mut Encoder, policy: language_semantics::CarryPolicy) {
    let _ = encoder.field("suspension", |encoder| {
        match policy.suspension {
            language_semantics::CarrySuspension::Forbidden => encoder.tag("forbidden", 0),
            language_semantics::CarrySuspension::Allowed => encoder.tag("allowed", 1),
        };
        Ok(())
    });
    let _ = encoder.field("cpu", |encoder| {
        match policy.cpu {
            language_semantics::CarryCpu::Origin => encoder.tag("origin", 0),
            language_semantics::CarryCpu::Any => encoder.tag("any", 1),
        };
        Ok(())
    });
    let _ = encoder.field("host_thread", |encoder| {
        match policy.host_thread {
            language_semantics::CarryHostThread::Origin => encoder.tag("origin", 0),
            language_semantics::CarryHostThread::Any => encoder.tag("any", 1),
        };
        Ok(())
    });
    let _ = encoder.field("address", |encoder| {
        match policy.address {
            language_semantics::CarryAddress::Stable => encoder.tag("stable", 0),
            language_semantics::CarryAddress::Movable => encoder.tag("movable", 1),
        };
        Ok(())
    });
}
