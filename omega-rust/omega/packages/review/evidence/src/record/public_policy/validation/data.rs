use super::*;

pub(super) fn validate(value: &PackagePolicyDataShape) -> Result {
    let scope = Scope {
        domain_subject: true,
        ..super::scope(&value.type_parameters, value.lifetime_parameter_count)
    };
    signatures::parameters(&scope, 0)?;
    if let PackageReviewDataKind::Quotient { carrier, relation } = &value.kind {
        value_type(carrier)?;
        nominal(relation)?;
    }
    ordered(&value.invariants)?;
    for fact in &value.invariants {
        contracts::fact(fact, &scope, 0)?;
    }
    ordered(&value.retired_identities)?;
    for (index, member) in value.members.iter().enumerate() {
        let (name, identity) = coordinate(member);
        text(name)?;
        if value.members[..index].iter().any(|prior| {
            let (prior_name, prior_identity) = coordinate(prior);
            prior_name == name || (identity.is_some() && prior_identity == identity)
        }) {
            return Err("public data repeats a live member name or identity");
        }
        if identity.is_some_and(|identity| value.retired_identities.contains(&identity)) {
            return Err("public data reuses a retired member identity");
        }
        match member {
            PackageReviewDataMember::Field(field) => field_check(field)?,
            PackageReviewDataMember::Variant {
                payload,
                retired_payload_identities,
                ..
            } => {
                ordered(retired_payload_identities)?;
                for (index, field) in payload.iter().enumerate() {
                    field_check(field)?;
                    if payload[..index].iter().any(|prior| {
                        prior.name == field.name
                            || (field.identity.is_some() && prior.identity == field.identity)
                    }) {
                        return Err("public variant repeats a payload field name or identity");
                    }
                    if field
                        .identity
                        .is_some_and(|identity| retired_payload_identities.contains(&identity))
                    {
                        return Err("public variant reuses a retired payload identity");
                    }
                }
            }
        }
    }
    Ok(())
}

fn coordinate(member: &PackageReviewDataMember) -> (&str, Option<u64>) {
    match member {
        PackageReviewDataMember::Field(value) => (&value.name, value.identity),
        PackageReviewDataMember::Variant { name, identity, .. } => (name, *identity),
    }
}

fn field_check(value: &PackageReviewDataField) -> Result {
    text(&value.name)?;
    value_type(&value.type_identity)
}
