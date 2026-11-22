//! Lowering decides no issuer for a domain that selects an access policy.
//! `lower_pending_policies` returns that domain's row without one, beside a
//! package that lacks the row, and names each cast that adds the domain. These
//! tests run no policy and supply no answer.
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, DomainIssuers, IssuersPending, LoweredPackage, LoweringError, PendingPolicyDomain, QualificationDirection, ScalarQualificationChange,
    lower_package, lower_pending_policies, unanswered_policy_refusal,
};
use omega_preparation::{PreparedTrees, prepare_source_package};

const UNRESTRICTED: &str = include_str!("fixtures/domain_access/unrestricted/access_plan.omg");
const NOBODY: &str = include_str!("fixtures/domain_access/empty-issuers/policy.omg");
const ISSUE: &str = "\nmachine issue(x:i32)->i32 in Locked { x as +Locked }";
const FORWARD: &str = "\nmachine forward(x:i32 in Locked)->i32 in Locked { x }";
const UNCHECKED: &str = "domain access policy needs checked establishment of `language\\core\\AccessPlan` in `language\\core\\access\\new_plan`";

/// Prepare the package under a Build owner. No policy is checked or executed.
fn prepare(source: &str) -> PreparedTrees<'_> {
    let mut package = sources::resolve(&[("policy.omg", source)]);
    package.bind_package_key("app", [0x42; 32]).unwrap();
    prepare_source_package(&package).unwrap()
}

fn spelled<'a>(
    site: &omega_lowering::Site,
    source: &'a str,
) -> &'a str {
    std::str::from_utf8(site.span.slice(source.as_bytes()).unwrap()).unwrap()
}

/// Each introduction as the machine that owns it and the cast it spells. The
/// machine is read from the package row the introduction names.
fn introductions<'a>(
    package: &LoweredPackage,
    domain: &PendingPolicyDomain,
    source: &'a str,
) -> Vec<(String, &'a str)> {
    domain
        .introductions
        .iter()
        .map(|cast| {
            let machine = &package.machines[usize::try_from(cast.machine.get()).unwrap() - 1];
            assert_eq!((machine.id, &machine.declaration), (cast.machine, &cast.declaration));
            (machine.declaration.path.clone(), spelled(&cast.site, source))
        })
        .collect()
}

fn direction(
    package: &LoweredPackage,
    machine: &str,
) -> Option<QualificationDirection> {
    let machine = package
        .machines
        .iter()
        .find(|row| row.declaration.path == machine)
        .expect("the machine has a body");
    let BlockTerminator::Jump(edge) = &machine.body.blocks[0].terminator else {
        return None;
    };
    let [ScalarQualificationChange { argument_index: 0, direction }] = edge.scalar_qualification_changes[..] else {
        panic!("expected one qualification change")
    };
    Some(direction)
}

#[test]
fn a_selected_domain_comes_back_without_an_issuer_beside_the_casts_that_add_it() {
    for policy in [NOBODY, UNRESTRICTED] {
        let source = format!(
            "{policy}{ISSUE}{FORWARD}
            domain i32::Kept = i32::Locked;
            machine keep(x:i32)->i32 in Kept {{ x as +Kept }}
            machine relay(x:i32 in Locked)->i32 in Locked {{ forward(x) }}
            machine release(x:i32 in Locked)->i32 {{ x as -Locked }}"
        );
        let prepared = prepare(&source);

        // The default entry still refuses the package, at the clause that selects the policy.
        let refused = lower_package(&prepared).unwrap_err();
        assert_eq!((refused.message.as_str(), spelled(&refused.site, &source)), (UNCHECKED, "access Rule"));

        let pending = lower_pending_policies(&prepared).unwrap();
        let [listed] = pending.domains() else { panic!("one selected domain") };
        assert_eq!(listed.source, prepared.typed().access_policies[0].domain);
        let (package, domains) = pending.into_parts();
        let [locked] = domains.as_slice() else { panic!("one selected domain") };

        // The row is the domain's, and it states no issuer either way.
        assert_eq!(
            (locked.source.path.as_str(), locked.row.declaration.path.as_str()),
            ("i32::Locked", "i32::Locked")
        );
        assert_eq!(spelled(&locked.selection, &source), "access Rule");
        assert_eq!(locked.row.issuers, IssuersPending);
        assert!(locked.row.requires.is_empty() && locked.row.parameters.is_empty());

        // The package publishes no row for it. The alias row names it and stays.
        assert!(package.domains.is_empty());
        let [kept] = package.domain_aliases.as_slice() else { panic!("the alias row") };
        assert_eq!(
            (kept.declaration.path.as_str(), &kept.members[0].declaration),
            ("i32::Kept", &locked.row.declaration)
        );

        // Two casts add the domain, one of them through the alias. A formal,
        // a call and a result that carry it, and the cast that erases it, do not.
        assert_eq!(
            introductions(&package, locked, &source),
            [
                ("issue".to_string(), "x as +Locked"),
                ("keep".to_string(), "x as +Kept")
            ]
        );
        assert_eq!(direction(&package, "issue"), Some(QualificationDirection::Add));
        assert_eq!(direction(&package, "keep"), Some(QualificationDirection::Add));
        assert_eq!(direction(&package, "release"), Some(QualificationDirection::Erase));
        assert_eq!(direction(&package, "forward"), None);

        // The driver refuses with this when the plan admits no issuer.
        let refusal = locked.introductions[0].refusal(locked);
        assert_eq!(
            (refusal.message.as_str(), spelled(&refusal.site, &source)),
            (
                "machine `issue` cannot introduce domain `i32::Locked`: its access plan admits no issuer",
                "x as +Locked"
            )
        );
    }
}

#[test]
fn a_domain_that_selects_no_policy_is_published_and_nothing_is_pending() {
    // The twin of the test above: the same machines over the same domain, declared without `access`.
    let source = format!("domain i32::Locked;{ISSUE}{FORWARD}");
    let prepared = prepare(&source);
    let (package, domains) = lower_pending_policies(&prepared).unwrap().into_parts();
    assert!(domains.is_empty());
    let [locked] = package.domains.as_slice() else { panic!("the published row") };
    assert_eq!((locked.declaration.path.as_str(), locked.issuers), ("i32::Locked", DomainIssuers::Unrestricted));
    assert_eq!(package, lower_package(&prepared).unwrap());
}

#[test]
fn each_selected_domain_lists_its_own_casts_in_source_order() {
    let policies = format!(
        "{}\n{}",
        UNRESTRICTED.replace("Locked", "Shut").replace("Rule", "Closed"),
        NOBODY
            .replace("Locked", "Open")
            .replace("Rule", "Free")
            .replace("use omega", "// use omega")
    );
    let source = format!(
        "{policies}
        domain i32::Plain;
        machine open(x:i32)->i32 in Open {{ x as +Open }}
        machine both(x:i32 in Shut)->i32 in Shut & Open {{ x as =Shut +Open }}
        machine plain(x:i32)->i32 in Plain {{ x as +Plain }}
        machine keep(x:i32 in Shut)->i32 in Shut {{ x }}"
    );
    let prepared = prepare(&source);
    let (package, domains) = lower_pending_policies(&prepared).unwrap().into_parts();

    // `Shut` is declared first and sorts last. The roster follows the source.
    let [shut, open] = domains.as_slice() else { panic!("two selected domains") };
    assert_eq!((shut.source.path.as_str(), open.source.path.as_str()), ("i32::Shut", "i32::Open"));
    assert!(source.find("domain i32::Shut").unwrap() < source.find("domain i32::Open").unwrap());

    // `both` receives `Shut` and adds only `Open`. Nothing adds `Shut`.
    assert_eq!(introductions(&package, shut, &source), []);
    assert_eq!(
        introductions(&package, open, &source),
        [
            ("both".to_string(), "x as =Shut +Open"),
            ("open".to_string(), "x as +Open")
        ]
    );

    // The domain without a policy is published, and its cast is nobody's concern.
    let [plain] = package.domains.as_slice() else {
        panic!("only the plain domain is published")
    };
    assert_eq!((plain.declaration.path.as_str(), plain.issuers), ("i32::Plain", DomainIssuers::Unrestricted));
}

#[test]
fn the_refusal_for_an_unanswered_policy_is_the_one_lower_package_gives() {
    let owned = format!("{NOBODY}{ISSUE}");
    let ownerless = sources::resolve(&[("policy.omg", NOBODY)]);
    let ownerless = prepare_source_package(&ownerless).unwrap();
    let nominal = NOBODY.replace("domain i32::Locked", "data Token { value:i32; }\ndomain Token::Locked");
    assert_ne!(nominal, NOBODY);
    for (prepared, reason) in [
        (prepare(&owned), UNCHECKED),
        (ownerless, "domain access policy needs the package owner for its semantic schema"),
        (prepare(&nominal), "domain access policy needs a semantic schema for this carrier"),
    ] {
        let [selection] = prepared.typed().access_policies.as_slice() else {
            panic!("one selected policy")
        };
        let refused: LoweringError = lower_package(&prepared).unwrap_err();
        assert_eq!(refused.message, reason);
        assert_eq!(unanswered_policy_refusal(&prepared, selection), refused);

        // Deferring does not turn any of them into a decision.
        let pending = lower_pending_policies(&prepared).unwrap();
        let [domain] = pending.domains() else {
            panic!("the selected domain stays pending")
        };
        assert_eq!(
            (&domain.source, &domain.selection, domain.row.issuers),
            (&selection.domain, &selection.site, IssuersPending)
        );
    }
}

#[test]
fn a_carrier_generic_domain_that_selects_a_policy_is_refused_and_not_dropped() {
    let source = NOBODY.replace("domain i32::Locked", "domain<T> T::Locked");
    assert_ne!(source, NOBODY);
    let prepared = prepare(&source);
    let [selection] = prepared.typed().access_policies.as_slice() else {
        panic!("one selected policy")
    };
    let refused = lower_pending_policies(&prepared).unwrap_err();
    assert_eq!(
        (refused.message.as_str(), spelled(&refused.site, &source)),
        ("domain access policy needs a semantic schema for this carrier", "access Rule")
    );
    assert_eq!(refused, unanswered_policy_refusal(&prepared, selection));
    assert_eq!(refused, lower_package(&prepared).unwrap_err());

    // The same family without `access` is a template row, so the clause is what was refused.
    let plain = source.replace(" access Rule", "");
    let prepared = prepare(&plain);
    let (package, domains) = lower_pending_policies(&prepared).unwrap().into_parts();
    assert!(domains.is_empty());
    let [family] = package.domain_templates.as_slice() else {
        panic!("the carrier family")
    };
    assert_eq!(family.issuers, DomainIssuers::Unrestricted);
}

#[test]
fn each_introducing_cast_is_listed_under_the_machine_that_owns_it() {
    // A nested machine owns its own cast, each arm's cast is its own
    // introduction, and a value that gave the domain up needs it added again.
    let source = format!(
        "{NOBODY}
        machine outer(x:i32)->i32 in Locked {{ inner(x) machine inner(y:i32)->i32 in Locked {{ y as +Locked }} }}
        machine pick(x:i32, f:bool)->i32 in Locked {{ match f {{ true -> x as +Locked, false -> (x +% 1) as +Locked, }} }}
        machine twice(x:i32)->i32 in Locked {{ let a = x as +Locked; let b = a as -Locked; b as +Locked }}"
    );
    let prepared = prepare(&source);
    let (package, domains) = lower_pending_policies(&prepared).unwrap().into_parts();
    let [locked] = domains.as_slice() else { panic!("one selected domain") };
    assert_eq!(
        introductions(&package, locked, &source),
        [
            ("outer::inner".to_string(), "y as +Locked"),
            ("pick".to_string(), "x as +Locked"),
            ("pick".to_string(), "(x +% 1) as +Locked"),
            ("twice".to_string(), "x as +Locked"),
            ("twice".to_string(), "b as +Locked"),
        ]
    );

    // An indexed family is one pending row, and a cast to one application adds it.
    let family = NOBODY.replace(
        "domain i32::Locked access Rule;",
        "domain<const N: i32> i32::Above<N> requires self > N access Rule;",
    );
    assert_ne!(family, NOBODY);
    let source = format!("{family}\nmachine make(x:i32)->i32 in Above<5> requires x > 5 {{ x as +Above<5> }}");
    let prepared = prepare(&source);
    let (package, domains) = lower_pending_policies(&prepared).unwrap().into_parts();
    let [above] = domains.as_slice() else { panic!("one selected domain") };
    assert_eq!(
        (above.row.parameters.len(), above.row.requires.len(), above.row.issuers),
        (1, 1, IssuersPending)
    );
    assert_eq!(introductions(&package, above, &source), [("make".to_string(), "x as +Above<5>")]);
    assert!(package.domains.is_empty());
}
