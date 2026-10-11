//! Source-free provider schema identity contract used by the public runtime facade.
//! Separate owning tables must compare canonical identities, not mint positions.
use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceSchema,
};

fn schema(types: [&str; 3], reverse: bool) -> ServiceSchema {
    let mut schema = ServiceSchema::default();
    let identities = [
        "SampleBoundary",
        "SampleBoundary::accept",
        types[0],
        types[1],
        types[2],
    ];
    if reverse {
        for identity in identities.into_iter().rev() {
            schema.identities.intern(identity);
        }
    } else {
        for identity in identities {
            schema.identities.intern(identity);
        }
    }
    schema.trait_name = schema.identities.intern("SampleBoundary");
    schema.methods.push(ServiceMethod {
        name: "accept".into(),
        requirement_owner: "SampleBoundary".into(),
        requirement_identity: schema.identities.intern("SampleBoundary::accept"),
        parameter_count: 1,
        parameter_type_identities: vec![schema.identities.intern(types[0])],
        entry_claims: vec![ServiceEntryClaim {
            parameter_index: 0,
            carrier_identity: schema.identities.intern(types[1]),
            domain: "AcceptedResource".into(),
            predicate_body: Default::default(),
            effective_carry: Default::default(),
            authority_flow: ServiceEntryAuthorityFlow::Accepts,
        }],
        has_result: true,
        result_type_identity: schema.identities.intern(types[2]),
        ..Default::default()
    });
    schema
}

#[test]
fn provider_schema_identity_ignores_table_mint_order() {
    let types = ["ParameterType", "ClaimCarrier", "ResultType"];
    let left = schema(types, false);
    let right = schema(types, true);
    assert_eq!(left.identity_digest(), right.identity_digest());
    assert_eq!(left, right);
}

#[test]
fn provider_schema_identity_rejects_equal_indices_with_different_types() {
    let types = ["ParameterType", "ClaimCarrier", "ResultType"];
    let left = schema(types, false);
    for position in 0..types.len() {
        let mut different = types;
        different[position] = "DifferentType";
        let right = schema(different, false);
        assert_ne!(left.identity_digest(), right.identity_digest());
        assert_ne!(left, right, "different type at position {position}");
    }
}
