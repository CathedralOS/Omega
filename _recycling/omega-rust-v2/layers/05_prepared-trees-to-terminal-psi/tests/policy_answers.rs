//! Lowering publishes a domain that selects an access policy only for the
//! return of an execution of this package's own policy unit. These tests give
//! it everything a caller can produce without that execution, or with an
//! execution that belongs elsewhere, and require a refusal each time.
#[path = "support/source.rs"]
mod sources;

use omega::lower_package_with_policy_answers;
use omega_interpreter::{Ceilings, PolicyReceipt, WORK_SCHEDULE, execute_policy};
use omega_lowering::{
    DomainIssuers, LoweringError, PolicySchemaValue, PolicyUnit, lower_package, lower_policy_construction_inputs, lower_selected_policy_definition,
    lower_selected_policy_unit,
};
use omega_preparation::{PlanIssuance, PreparedTrees, PreparedValue, prepare_source_package, read_returned_access_plan};
use omega_typed::AccessPolicySelection;
use std::sync::Arc;

const UNRESTRICTED: &str = include_str!("fixtures/domain_access/unrestricted/access_plan.omg");
const NOBODY: &str = include_str!("fixtures/domain_access/empty-issuers/policy.omg");
const ISSUE: &str = "\nmachine issue(x:i32)->i32 in Locked { x as +Locked }";
const PENDING: &str = "domain access policy needs admitted execution of the selected plan for its semantic schema";
const FOREIGN: &str = "policy answer belongs to a different preparation of this domain";

fn prepare(source: &str) -> PreparedTrees<'_> {
    let mut package = sources::resolve(&[("policy.omg", source)]);
    package.bind_package_key("app", [0x42; 32]).unwrap();
    let mut prepared = prepare_source_package(&package).unwrap();
    omega::check_pending_policy_constructions(&mut prepared).unwrap();
    prepared
}

fn only_policy<'a>(prepared: &'a PreparedTrees<'_>) -> &'a AccessPolicySelection {
    let [policy] = prepared.typed().access_policies.as_slice() else {
        panic!("one selected domain policy")
    };
    policy
}

fn issuers(
    lowered: Result<omega_lowering::LoweredPackage, LoweringError>,
    domain: &str,
) -> DomainIssuers {
    let package = lowered.unwrap();
    let row = package
        .domains
        .iter()
        .find(|row| row.declaration.path == domain);
    row.expect("the domain row").issuers
}

fn refusal(lowered: Result<omega_lowering::LoweredPackage, LoweringError>) -> String {
    lowered.expect_err("the domain must not be published").message
}

/// Check `unit`'s two bodies, run its plan on `schema` and hand the receipt to
/// `read`. The receipt borrows the proofs made here, so it cannot leave.
fn with_receipt<R>(
    prepared: &PreparedTrees<'_>,
    unit: &PolicyUnit,
    schema: &PolicySchemaValue,
    ceilings: Ceilings,
    read: impl FnOnce(PolicyReceipt<'_>) -> R,
) -> R {
    let policy = only_policy(prepared);
    let inputs = lower_policy_construction_inputs(prepared, policy).unwrap();
    let definition = lower_selected_policy_definition(prepared, policy).unwrap();
    let constructor = omega_checker::check_record_constructor(unit, &inputs.constructor).unwrap();
    let call = omega_checker::check_selected_policy_call(&definition, &constructor).unwrap();
    let closed = omega_checker::check_closed_policy_call(&call).unwrap();
    read(execute_policy(&closed, schema, ceilings).unwrap())
}

/// The same plan with its issuance replaced by `Issuance::Unrestricted`. Every
/// other field, the schema's backing included, stays the value it was.
fn made_unrestricted(plan: &PreparedValue) -> PreparedValue {
    let core = omega_toolchain::core_interface();
    match plan {
        PreparedValue::Sum { owner, .. } if owner == core.id("Issuance") => PreparedValue::Sum {
            owner: owner.clone(),
            case: core.id("Issuance::Unrestricted").clone(),
            fields: Arc::from([]),
        },
        PreparedValue::Record { owner, fields } => PreparedValue::Record {
            owner: owner.clone(),
            fields: fields.iter().map(made_unrestricted).collect(),
        },
        other => other.clone(),
    }
}

#[test]
fn an_executed_answer_publishes_the_issuer_its_return_states() {
    for (source, expected, issuance) in [
        (UNRESTRICTED, DomainIssuers::Unrestricted, PlanIssuance::Unrestricted),
        (NOBODY, DomainIssuers::Nobody, PlanIssuance::Nobody),
    ] {
        let prepared = prepare(source);
        let unit = lower_selected_policy_unit(&prepared, only_policy(&prepared))
            .unwrap()
            .expect("a closed policy unit");
        let ceilings = Ceilings {
            work: Some(1 << 20),
            live_cells: None,
        };
        let answer = with_receipt(&prepared, &unit, unit.schema().unwrap(), ceilings, |receipt| {
            omega::answer_policy(&unit, receipt)
        })
        .unwrap();
        assert_eq!(unit.domain(), Some(answer.domain()));
        assert_eq!(answer.domain().path, "i32::Locked");
        assert_eq!(answer.ceilings(), ceilings);
        assert_eq!(answer.usage().work_schedule, WORK_SCHEDULE);
        assert_ne!(answer.usage().work_used, 0);
        assert_eq!(read_returned_access_plan(&prepared, answer.domain(), answer.returned()), Ok(issuance));

        // Without the answer the package is refused, with it the domain is published.
        assert_eq!(refusal(lower_package(&prepared)), PENDING);
        assert_eq!(refusal(lower_package_with_policy_answers(&prepared, &[])), PENDING);
        assert_eq!(
            issuers(lower_package_with_policy_answers(&prepared, std::slice::from_ref(&answer)), "i32::Locked"),
            expected
        );

        // One domain has one answer. A second execution of the same unit is a second answer.
        let again = with_receipt(&prepared, &unit, unit.schema().unwrap(), ceilings, |receipt| {
            omega::answer_policy(&unit, receipt)
        })
        .unwrap();
        assert_eq!(
            refusal(lower_package_with_policy_answers(&prepared, &[answer, again])),
            "domain access policy received more than one answer"
        );
    }
}

#[test]
fn an_answer_from_another_preparation_of_the_same_source_is_refused() {
    let source = format!("{UNRESTRICTED}{ISSUE}");
    let (mut first, mut second) = (prepare(&source), prepare(&source));
    let theirs = omega::prepare_policy_answers(&mut first, Ceilings::default()).unwrap();
    let ours = omega::prepare_policy_answers(&mut second, Ceilings::default()).unwrap();

    // The two answers say the same thing: equal plans over equal schema bytes
    // with the same issuer. Each frozen invocation owns a distinct allocation.
    assert_eq!(theirs[0].returned(), ours[0].returned());
    assert_eq!(theirs[0].domain(), ours[0].domain());
    assert_eq!(theirs[0].usage(), ours[0].usage());

    assert_eq!(refusal(lower_package_with_policy_answers(&second, &theirs)), FOREIGN);
    assert_eq!(refusal(lower_package_with_policy_answers(&first, &ours)), FOREIGN);
    assert_eq!(
        issuers(lower_package_with_policy_answers(&second, &ours), "i32::Locked"),
        DomainIssuers::Unrestricted
    );
    assert_eq!(
        issuers(lower_package_with_policy_answers(&first, &theirs), "i32::Locked"),
        DomainIssuers::Unrestricted
    );
}

#[test]
fn a_fabricated_unrestricted_plan_around_the_real_schema_publishes_nothing() {
    // The policy admits nobody. `issue` may introduce the domain only if the
    // package is lowered as unrestricted.
    let source = format!("{NOBODY}{ISSUE}");
    let mut prepared = prepare(&source);
    let answers = omega::prepare_policy_answers(&mut prepared, Ceilings::default()).unwrap();
    let domain = answers[0].domain().clone();

    // Everything in the fabricated plan is real except the issuance: it keeps
    // the frozen schema's own backing, so L4 reads it as this domain's plan.
    let fabricated = made_unrestricted(answers[0].returned());
    assert_eq!(read_returned_access_plan(&prepared, &domain, answers[0].returned()), Ok(PlanIssuance::Nobody));
    assert_eq!(read_returned_access_plan(&prepared, &domain, &fabricated), Ok(PlanIssuance::Unrestricted));

    // Reading a supplied record cannot create an executed answer. The
    // original domain still has only the restricted answer that actually ran.
    assert!(prepared.pending_access_plan(&domain).is_some());
    assert_eq!(refusal(lower_package(&prepared)), PENDING);
    assert_eq!(refusal(lower_package_with_policy_answers(&prepared, &[])), PENDING);
    assert_eq!(
        refusal(lower_package_with_policy_answers(&prepared, &answers)),
        "machine `issue` cannot introduce domain `i32::Locked`: its access plan admits no issuer"
    );
    let unit = lower_selected_policy_unit(&prepared, only_policy(&prepared))
        .unwrap()
        .unwrap();
    assert!(unit.domains.is_empty());
}

#[test]
fn a_unit_answers_only_its_own_execution_on_its_own_schema() {
    let prepared = prepare(NOBODY);
    let policy = only_policy(&prepared);
    let unit = lower_selected_policy_unit(&prepared, policy).unwrap().unwrap();
    let twin = lower_selected_policy_unit(&prepared, policy).unwrap().unwrap();
    assert_eq!(unit.package(), twin.package());
    let schema = unit.schema().unwrap();
    assert!(Arc::ptr_eq(&schema.storage, &twin.schema().unwrap().storage));
    let ceilings = Ceilings::default();

    // An equal unit lowered again from the same package is another unit.
    let error = with_receipt(&prepared, &twin, schema, ceilings, |receipt| omega::answer_policy(&unit, receipt)).unwrap_err();
    assert_eq!(error.message, "policy receipt records the execution of a different unit");

    // Equal schema bytes in another allocation are another argument.
    let copied = PolicySchemaValue {
        record: schema.record.clone(),
        storage: Arc::from(&*schema.storage),
    };
    assert_eq!(&copied, schema);
    let error = with_receipt(&prepared, &unit, &copied, ceilings, |receipt| omega::answer_policy(&unit, receipt)).unwrap_err();
    assert_eq!(error.message, "policy receipt records an execution on a different schema backing");

    // A unit lowered before the package had an owner has no invocation to answer.
    let ownerless = sources::resolve(&[("policy.omg", NOBODY)]);
    let ownerless = prepare_source_package(&ownerless).unwrap();
    if let Some(open) = lower_selected_policy_unit(&ownerless, only_policy(&ownerless)).unwrap() {
        assert!(open.schema().is_none() && open.domain().is_none());
    }

    let answer = with_receipt(&prepared, &unit, schema, ceilings, |receipt| omega::answer_policy(&unit, receipt)).unwrap();
    assert_eq!(
        issuers(lower_package_with_policy_answers(&prepared, &[answer]), "i32::Locked"),
        DomainIssuers::Nobody
    );
}

#[test]
fn two_selected_domains_bind_their_issuers_independently_of_answer_order() {
    let (_, restricted) = NOBODY.split_once("domain i32::Locked").unwrap();
    let source = format!(
        "{UNRESTRICTED}\ndomain i32::Sealed{}\n\
         machine issue(x:i32)->i32 in Locked {{ x as +Locked }}\n\
         machine forward(x:i32 in Sealed)->i32 in Sealed {{ x }}",
        restricted.replace("Rule", "SealedPolicy")
    );
    let mut prepared = prepare(&source);
    let mut answers = omega::prepare_policy_answers(&mut prepared, Ceilings::default()).unwrap();
    assert_eq!(answers.len(), 2);
    let (partial, pending) = omega_lowering::lower_pending_policies(&prepared)
        .unwrap()
        .into_parts();
    assert!(partial.domains.is_empty());
    let pending_casts: Vec<_> = pending
        .iter()
        .map(|domain| {
            (
                domain.source.path.as_str(),
                domain
                    .introductions
                    .iter()
                    .map(|cast| cast.declaration.path.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(pending_casts, [("i32::Locked", vec!["issue"]), ("i32::Sealed", vec![])]);
    answers.reverse();
    let missing = lower_package_with_policy_answers(&prepared, &answers[..1]).unwrap_err();
    let expected_site = &prepared
        .typed()
        .access_policies
        .iter()
        .find(|policy| &policy.domain == answers[1].domain())
        .unwrap()
        .site;
    assert_eq!(&missing.site, expected_site);
    let package = lower_package_with_policy_answers(&prepared, &answers).unwrap();
    let rows: Vec<_> = package
        .domains
        .iter()
        .map(|row| (row.declaration.path.as_str(), row.issuers))
        .collect();
    assert_eq!(
        rows,
        [
            ("i32::Locked", DomainIssuers::Unrestricted),
            ("i32::Sealed", DomainIssuers::Nobody)
        ]
    );
}
