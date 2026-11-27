use super::*;

const AUTHORED: &str =
    include_str!("../../../../../../../samples/cli/facts/fixed_layout_storage/local/main.omg");

fn with_reflection(source: &str) -> Result<TypedTrees, String> {
    let library = crate::test_support::fixture_repository::root().join("source/library");
    crate::test_support::schema_customer_source::actual_library_source(
        &library,
        source,
        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
    )
}

/// Evaluates the layouts `scopes` demand of `source`, the way source
/// preparation does. The request has no entry, so nothing else is demanded.
fn evaluate(
    source: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    evaluate_selected(source, scopes, &[], accounting)
}

fn evaluate_selected(
    source: &mut TypedTrees,
    scopes: &[FixedLayoutTargetScope],
    selected: &[SymbolHandle],
    accounting: &mut SourceCompletionAccounting,
) -> Result<(), Vec<Diagnostic>> {
    crate::compile_time_invocations::evaluate_demanded_values(
        source,
        &crate::compile_time_invocations::ValueDemand {
            compilation: production::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected,
                bindings: &[],
                supplies: &[],
            },
            layout_scopes: scopes,
            authority: None,
            source_consent: None,
        },
        accounting,
    )
    .map(drop)
}

fn query_machine(source: &TypedTrees, name: &str) -> SymbolHandle {
    source
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol
}

#[test]
fn an_empty_layout_scope_preserves_the_unrequested_application() {
    // The same kept application must remain a real deferred request when
    // no computation asks for its geometry; no successful plan is invented.
    let mut source = with_reflection(AUTHORED).unwrap();
    let entry = query_machine(&source, "main");
    let facts = layout_plans::LayoutTargetFacts {
        profile: "linux_x86_64".into(),
        pointer_size: 8,
        pointer_alignment: 8,
        case_tag_size: 4,
        case_tag_alignment: 4,
        little_endian: true,
    };
    let scope = FixedLayoutTargetScope {
        checks_subject: false,
        target: facts.profile.clone(),
        facts: Some(facts),
        entry_declarations: Vec::new(),
        provider_declarations: Vec::new(),
    };
    let mut foreign = scope.clone();
    foreign.facts.as_mut().unwrap().profile = "windows_x86_64".into();
    let mut accounting = crate::compile_time_invocations::SourceCompletionAccounting::default();
    let errors = evaluate(&mut source, &[foreign], &mut accounting)
    .expect_err("empty roots do not authenticate another target's facts");
    assert!(
        diagnostics::join_messages(&errors)
            .contains("layout scope and immutable target facts disagree")
    );
    assert!(accounting.receipts().is_empty());
    evaluate(&mut source, &[scope], &mut accounting)
    .expect("an unused layout application needs no policy invocation");
    assert!(source.plan_laid_layouts.is_empty());
    assert!(source.plan_laid_target_layouts.is_empty());
    assert!(accounting.receipts().is_empty());
    assert!(source.deferred_refusals_of(entry).any(|row| {
        row.request == crate::source_state::deferred_refusals::DeferredRequest::LayoutPolicyStorage
    }));
}

#[test]
fn an_entryless_recast_demands_its_own_storage_only_on_checked_targets() {
    let policy = AUTHORED.split("machine make()").next().unwrap();
    let other = policy[policy.find("PairLayout satisfies").unwrap()..]
        .replace("PairLayout", "UnusedLayout")
        .replace("Layout<Pair>", "Layout<Unused>")
        .replace("size_fixed: 16", "size_fixed: 1");
    let source = format!(
        r#"{policy}
public data Unused {{ lo: u64; hi: u64; }}
{other}
public machine inspect(value: @|layout|::LayoutStorage<Pair, PairLayout>,
    unused: @|layout|::LayoutStorage<Unused, UnusedLayout>) -> u8 {{
    let bytes: @[u8; 16] = value as @[u8; 16];
    bytes[8]
}}
public machine inspect_with<Policy satisfies |layout|::FixedLayout<Pair>>(
    value: @|layout|::LayoutStorage<Pair, Policy>) -> u8 {{
    let bytes: @[u8; 16] = value as @[u8; 16];
    bytes[8]
}}"#
    );
    let mut source = with_reflection(&source).unwrap();
    let consumers = ["inspect", "inspect_with"].map(|name| query_machine(&source, name));
    let scopes = ["linux_x86_64", "windows_x86_64"].map(|target| FixedLayoutTargetScope {
        target: target.into(),
        facts: Some(layout_plans::LayoutTargetFacts {
            profile: target.into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        }),
        checks_subject: target == "windows_x86_64",
        entry_declarations: Vec::new(),
        provider_declarations: Vec::new(),
    });
    let mut accounting = SourceCompletionAccounting::default();
    evaluate_selected(&mut source, &scopes, &consumers, &mut accounting)
        .unwrap_or_else(|diagnostics| panic!("{}", diagnostics::join_messages(&diagnostics)));
    let [plan] = source.plan_laid_layouts.as_slice() else {
        panic!("only the viewed layout is demanded")
    };
    let [realization] = plan.target_layouts(&source).as_array().unwrap();
    assert_eq!(realization.target_facts.profile, "windows_x86_64");
    assert_eq!(
        realization.geometry.validated_layout.offsets.as_deref(),
        Some([8, 0].as_slice())
    );
    assert_eq!(realization.geometry.validated_layout.size, Some(16));
    assert_eq!(realization.geometry.validated_layout.align, 8);
    assert_eq!(accounting.receipts().len(), 1);
}

#[test]
fn generic_layout_policy_retains_its_declared_machine_and_observed_geometry() {
    // The same closed layout plan must use the declared member in source
    // metadata and its captured body in the actual invocation receipt.
    let authored = AUTHORED
    .replace(
        "PairLayout satisfies |layout|::FixedLayout<Pair>",
        "public data OtherPair [copy] { lo: u64 in Delivered; hi: u64; }\nPairLayout<T> satisfies |layout|::FixedLayout<T>",
    );
    let subject = |argument: &str| {
        with_reflection(&authored.replace(
            "LayoutStorage<Pair, PairLayout>",
            &format!("LayoutStorage<Pair, PairLayout<{argument}>>"),
        ))
    };
    // Different records can have identical geometry. Typing must refuse the
    // wrong FixedLayout application before its policy can be evaluated.
    let refusal = subject("OtherPair").expect_err("wrong policy subject");
    assert!(
        refusal.contains("implementation `PairLayout<OtherPair>` does not supply the declared `FixedLayout` application of `LayoutStorage`"),
        "{refusal}"
    );

    let mut source = subject("Pair").unwrap();
    let root = query_machine(&source, "main");
    let facts = layout_plans::LayoutTargetFacts {
        profile: "linux_x86_64".into(),
        pointer_size: 8,
        pointer_alignment: 8,
        case_tag_size: 4,
        case_tag_alignment: 4,
        little_endian: true,
    };
    let mut accounting = crate::compile_time_invocations::SourceCompletionAccounting::default();
    evaluate(
        &mut source,
        &[FixedLayoutTargetScope {
            checks_subject: false,
            target: facts.profile.clone(),
            facts: Some(facts),
            entry_declarations: vec![root],
            provider_declarations: Vec::new(),
        }],
        &mut accounting,
    )
    .unwrap();
    let [plan] = source.plan_laid_layouts.as_slice() else {
        panic!("one exact layout for Pair");
    };
    let policy = source.conformance_by_symbol(plan.policy_symbol).unwrap();
    let [member] = policy.rows.as_slice() else {
        panic!("the authored plan member");
    };
    assert_eq!(plan.policy_plan_machine_symbol, member.realization_machine);
    let Some([layout]) = plan.target_layouts(&source).as_array() else {
        panic!("one plan for the selected target");
    };
    assert_eq!(layout.machine, member.realization_machine);
    assert_eq!(
        layout.geometry.validated_layout.offsets.as_deref(),
        Some([8, 0].as_slice())
    );
    assert_eq!(layout.geometry.validated_layout.size, Some(16));
    assert_eq!(layout.geometry.validated_layout.align, 8);
    assert!(!source.deferred_refusals_of(root).any(|row| row.request
        == crate::source_state::deferred_refusals::DeferredRequest::LayoutPolicyStorage));
    let [crate::compile_time_invocations::PureEvaluationRecord::Invocation(receipt)] =
        accounting.receipts()
    else {
        panic!("one physical policy invocation");
    };
    assert_ne!(receipt.machine(), member.realization_machine);
    assert!(source.machine_by_symbol(receipt.machine()).is_none());
    assert_eq!(receipt.machine_template(), Some(member.realization_machine));
    accounting.validate_charges().unwrap();
}

#[test]
fn authored_policy_observes_only_its_invocations_issued_target() {
    // The two targets use the same carriers. Only the profile bytes issued to
    // this invocation distinguish their two legitimate placement sizes.
    let program = AUTHORED
        .replace(
            "use {omega}/language/core/layout.omg;",
            r#"use {omega}/language/core/layout.omg;
machine packet_size<'target>(target: @'target |layout|::LayoutTarget) -> u64 reaches none terminates {
    let facts: |layout|::LayoutTargetFacts<'target> = |layout|::layout_target_facts(target);
    transition facts.profile.length == 12 { true -> 16u64, _ -> 24u64 }
}"#,
        )
        .replace("size_fixed: 16", "size_fixed: packet_size(@target)");
    let mut source = with_reflection(&program).unwrap();
    let root = query_machine(&source, "main");
    let scopes = ["linux_x86_64", "windows_x86_64"].map(|name| FixedLayoutTargetScope {
        checks_subject: false,
        target: name.into(),
        facts: Some(layout_plans::LayoutTargetFacts {
            profile: name.into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        }),
        entry_declarations: vec![root],
        provider_declarations: Vec::new(),
    });
    let mut accounting = crate::compile_time_invocations::SourceCompletionAccounting::default();
    evaluate(&mut source, &scopes, &mut accounting)
    .unwrap_or_else(|errors| panic!("{}", diagnostics::join_messages(&errors)));
    let [plan] = source.plan_laid_layouts.as_slice() else {
        panic!("one authored policy");
    };
    let observations: Vec<_> = plan
        .target_layouts(&source)
        .iter()
        .map(|row| {
            (
                row.target_facts.profile.as_str(),
                row.geometry.validated_layout.size,
            )
        })
        .collect();
    assert_eq!(
        observations,
        [("linux_x86_64", Some(16)), ("windows_x86_64", Some(24))]
    );
    assert_eq!(accounting.receipts().len(), 2);
    accounting.validate_charges().unwrap();
}
