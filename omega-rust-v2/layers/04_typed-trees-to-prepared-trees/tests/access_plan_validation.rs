//! A domain's access plan exists only after preparation validates the value its
//! policy returned. These tests hand the validator that value directly, so each
//! one decides exactly which value the validator sees.
use omega_preparation::{PlanIssuance, PreparedTrees, PreparedValue, prepare_source_package, read_returned_access_plan};
use omega_resolved::DeclarationId;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;
use returned_plan::{field, new_plan_return, with_field};
use std::sync::Arc;

#[path = "support/returned_plan.rs"]
mod returned_plan;

const POLICY: &str = include_str!("fixtures/access_policy/selected.omg");

fn prepare(source: &str) -> PreparedTrees<'_> {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let syntax = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let mut package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "main.omg",
            text: source,
            tokens: &tokens,
            tree: &syntax,
        },
    }])
    .unwrap();
    package.bind_package_key("app", [0x42; 32]).unwrap();
    prepare_source_package(&package).unwrap()
}

fn domain(
    prepared: &PreparedTrees<'_>,
    path: &str,
) -> DeclarationId {
    prepared
        .typed()
        .access_policies
        .iter()
        .find(|selection| selection.domain.path == path)
        .expect("selected domain")
        .domain
        .clone()
}

fn schema_image(
    prepared: &PreparedTrees<'_>,
    domain: &DeclarationId,
) -> Arc<[u8]> {
    let PreparedValue::Record { fields, .. } = prepared.policy_invocation(domain).unwrap().schema else {
        panic!("expected the schema record")
    };
    let [PreparedValue::SharedBytes(image)] = fields.as_ref() else {
        panic!("expected the schema image")
    };
    image.clone()
}

fn validate(
    prepared: &PreparedTrees<'_>,
    domain: &DeclarationId,
    returned: &PreparedValue,
) -> Result<PlanIssuance, &'static str> {
    read_returned_access_plan(prepared, domain, returned)
}

/// Every refusal must leave the domain exactly as pending as it was.
fn refused(
    prepared: &PreparedTrees<'_>,
    domain: &DeclarationId,
    returned: &PreparedValue,
) -> &'static str {
    let reason = validate(prepared, domain, returned).expect_err("the returned value must not validate");
    assert!(prepared.pending_access_plan(domain).is_some(), "{reason}");
    reason
}

#[test]
fn the_returned_issuance_case_decides_who_may_introduce_the_domain() {
    let empty = POLICY.replace("Unrestricted{}", "Only{routes:[]}");
    let omitted = POLICY.replace("Unrestricted{}", "Only{..}");
    for (source, expected) in [
        (POLICY, PlanIssuance::Unrestricted),
        (&empty, PlanIssuance::Nobody),
        (&omitted, PlanIssuance::Nobody),
    ] {
        let prepared = prepare(source);
        let locked = domain(&prepared, "i32::Locked");
        let returned = new_plan_return(&prepared, &locked);

        assert_eq!(validate(&prepared, &locked, &returned).unwrap(), expected);
        assert!(prepared.pending_access_plan(&locked).is_some(), "reading a value cannot publish a domain");
    }
}

#[test]
fn a_plan_for_another_schema_region_or_preparation_authorizes_nothing() {
    let source = format!("{POLICY}\ndomain u8::Small access Rule;");
    let prepared = prepare(&source);
    let (locked, small) = (domain(&prepared, "i32::Locked"), domain(&prepared, "u8::Small"));
    let typed_schema = |image: Arc<[u8]>| PreparedValue::Record {
        owner: omega_toolchain::core_interface()
            .id(r"reflect\TypeSchema")
            .clone(),
        fields: [PreparedValue::SharedBytes(image)].into(),
    };

    // Both domains select `Rule`, and each plan is unrestricted. The plan
    // returned for `u8::Small` still describes only that domain's schema.
    let for_small = new_plan_return(&prepared, &small);
    assert_eq!(refused(&prepared, &locked, &for_small), "returned plan describes a different schema");

    // Equal bytes in another allocation are another borrow region.
    let own = new_plan_return(&prepared, &locked);
    let image = schema_image(&prepared, &locked);
    let copy = with_field(prepared.typed(), &own, "AccessPlan::schema", typed_schema(Arc::from(image.to_vec())));
    assert_eq!(
        refused(&prepared, &locked, &copy),
        "returned plan's schema is a copy outside the region of this invocation's schema"
    );
    let mut altered = image.to_vec();
    *altered.last_mut().unwrap() ^= 1;
    let altered = with_field(prepared.typed(), &own, "AccessPlan::schema", typed_schema(altered.into()));
    assert_eq!(refused(&prepared, &locked, &altered), "returned plan describes a different schema");

    // A second preparation of the same source and owner freezes equal inputs
    // in its own storage. Its plan and its occurrence both belong to it.
    let other = prepare(&source);
    let foreign = new_plan_return(&other, &locked);
    assert_eq!(schema_image(&other, &locked), image);
    assert_eq!(
        refused(&prepared, &locked, &foreign),
        "returned plan's schema is a copy outside the region of this invocation's schema"
    );
    assert_eq!(validate(&prepared, &locked, &own).unwrap(), PlanIssuance::Unrestricted);
    assert_eq!(validate(&prepared, &small, &for_small).unwrap(), PlanIssuance::Unrestricted);
    assert!(!Arc::ptr_eq(&schema_image(&prepared, &locked), &schema_image(&prepared, &small)));
}

#[test]
fn a_plan_with_rules_routes_or_reach_stays_pending() {
    let prepared = prepare(POLICY);
    let locked = domain(&prepared, "i32::Locked");
    let core = omega_toolchain::core_interface();
    let plan = new_plan_return(&prepared, &locked);
    let integer = |value: u64| PreparedValue::Integer(value.into());
    let replace = |prepared: &PreparedTrees<'_>, record: &PreparedValue, name: &str, value: PreparedValue| with_field(prepared.typed(), record, name, value);

    const DISAGREES: &str = "returned plan count disagrees with its occupied slots";
    const RULES: &str = "returned plan has field, reader or writer rules, which need validated regions and callers";

    // A count with no entry behind it, and an entry no count admits.
    for count in [
        "AccessPlan::field_count",
        "AccessPlan::reader_count",
        "AccessPlan::writer_count",
    ] {
        let counted = replace(&prepared, &plan, count, integer(1));
        assert_eq!(refused(&prepared, &locked, &counted), DISAGREES, "{count}");
    }
    let PreparedValue::Array(readers) = field(prepared.typed(), &plan, "AccessPlan::readers") else {
        panic!("expected the reader slots")
    };
    let mut occupied = readers.to_vec();
    occupied[0] = PreparedValue::Sum {
        owner: core.id("AccessRuleSlot").clone(),
        case: core.id("AccessRuleSlot::Rule").clone(),
        fields: [integer(0)].into(),
    };
    let hidden = replace(&prepared, &plan, "AccessPlan::readers", PreparedValue::Array(occupied.into()));
    assert_eq!(refused(&prepared, &locked, &hidden), DISAGREES);
    // A consistent reader rule restricts readers. Nothing after preparation
    // enforces it yet, so the domain cannot be published without it.
    let ruled = replace(&prepared, &hidden, "AccessPlan::reader_count", integer(1));
    assert_eq!(refused(&prepared, &locked, &ruled), RULES);

    let reach = field(prepared.typed(), &plan, "AccessPlan::reach").clone();
    let reaching = replace(&prepared, &reach, "BoundaryReach::service_count", integer(1));
    let reaching = replace(&prepared, &plan, "AccessPlan::reach", reaching);
    assert_eq!(
        refused(&prepared, &locked, &reaching),
        "returned plan names boundary reach, which needs validated service identities"
    );
    let PreparedValue::Array(services) = field(prepared.typed(), &reach, "BoundaryReach::services") else {
        panic!("expected the service slots")
    };
    let mut named = services.to_vec();
    named[3] = integer(7);
    let uncounted = replace(&prepared, &reach, "BoundaryReach::services", PreparedValue::Array(named.into()));
    let uncounted = replace(&prepared, &plan, "AccessPlan::reach", uncounted);
    assert_eq!(refused(&prepared, &locked, &uncounted), DISAGREES);

    // One route names an issuer. Its description is three byte views that no
    // frozen descriptor can be compared with, so it admits nobody and nothing.
    let view = || PreparedValue::SharedBytes(Arc::from(&b"issue"[..]));
    let routed = PreparedValue::Sum {
        owner: core.id("Issuance").clone(),
        case: core.id("Issuance::Only").clone(),
        fields: [PreparedValue::Array(
            [PreparedValue::Sum {
                owner: core.id(r"reflect\CallableInfo").clone(),
                case: core.id(r"reflect\CallableInfo::Machine").clone(),
                fields: [view(), view(), view()].into(),
            }]
            .into(),
        )]
        .into(),
    };
    let routed = replace(&prepared, &plan, "AccessPlan::issuance", routed);
    assert_eq!(
        refused(&prepared, &locked, &routed),
        "returned plan's issuance routes need frozen callable descriptors"
    );

    // The schema argument and a shortened record are not plans at all.
    let schema = field(prepared.typed(), &plan, "AccessPlan::schema").clone();
    assert_eq!(refused(&prepared, &locked, &schema), "returned value is not a core AccessPlan record");
    let PreparedValue::Record { owner, fields } = &plan else { unreachable!() };
    let shortened = PreparedValue::Record {
        owner: owner.clone(),
        fields: fields[..fields.len() - 1].into(),
    };
    assert_eq!(refused(&prepared, &locked, &shortened), "returned value is not a core AccessPlan record");

    assert_eq!(validate(&prepared, &locked, &plan).unwrap(), PlanIssuance::Unrestricted);
}
