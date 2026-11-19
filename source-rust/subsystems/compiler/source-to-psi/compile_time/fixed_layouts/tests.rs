use super::*;

const AUTHORED: &str =
    include_str!("../../../../../../samples/cli/facts/fixed_layout_storage/local/main.omg");

fn with_reflection(source: &str) -> TypedTrees {
    let library = crate::stages::fixture_repository::root().join("source/library");
    crate::stages::schema_customer_source::actual_library_source(
        &library,
        source,
        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
    )
    .unwrap()
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
    let mut source = with_reflection(AUTHORED);
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
        target: facts.profile.clone(),
        facts: Some(facts),
        entry_declarations: Vec::new(),
        provider_declarations: Vec::new(),
    };
    let mut foreign = scope.clone();
    foreign.facts.as_mut().unwrap().profile = "windows_x86_64".into();
    let mut accounting = crate::compile_time::SourceCompletionAccounting::default();
    let errors = evaluate_fixed_layout_applications(
        &mut source,
        &[foreign],
        None,
        None,
        source_provenance::DependencyScope::Product,
        &mut accounting,
    )
    .expect_err("empty roots do not authenticate another target's facts");
    assert!(
        diagnostics::join_messages(&errors)
            .contains("layout scope and immutable target facts disagree")
    );
    assert!(accounting.receipts().is_empty());
    evaluate_fixed_layout_applications(
        &mut source,
        &[scope],
        None,
        None,
        source_provenance::DependencyScope::Product,
        &mut accounting,
    )
    .expect("an unused layout application needs no policy invocation");
    assert!(source.plan_laid_layouts.is_empty());
    assert!(source.plan_laid_target_layouts.is_empty());
    assert!(accounting.receipts().is_empty());
    assert!(source.deferred_refusals_of(entry).any(|row| {
        row.request == crate::source_state::deferred_refusals::DeferredRequest::LayoutPolicyStorage
    }));
}

#[test]
fn generic_layout_policy_retains_its_declared_machine_and_observed_geometry() {
    // The same closed layout plan must use the declared member in source
    // metadata and its captured body in the actual invocation receipt.
    let authored = AUTHORED
    .replace(
        "PairLayout satisfies FixedLayout<Pair>",
        "public data OtherPair [copy] { lo: u64 in Delivered; hi: u64; }\nPairLayout<T> satisfies FixedLayout<T>",
    );
    let evaluated = |argument: &str| {
        let mut source = with_reflection(&authored.replace(
            "LayoutStorage<Pair, PairLayout>",
            &format!("LayoutStorage<Pair, PairLayout<{argument}>>"),
        ));
        let root = query_machine(&source, "main");
        let facts = layout_plans::LayoutTargetFacts {
            profile: "linux_x86_64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        let mut accounting = crate::compile_time::SourceCompletionAccounting::default();
        let result = evaluate_fixed_layout_applications(
            &mut source,
            &[FixedLayoutTargetScope {
                target: facts.profile.clone(),
                facts: Some(facts),
                entry_declarations: vec![root],
                provider_declarations: Vec::new(),
            }],
            None,
            None,
            source_provenance::DependencyScope::Product,
            &mut accounting,
        );
        (source, root, accounting, result)
    };

    // Geometry alone cannot identify a policy's semantic argument.
    let (wrong, _, accounting, refusal) = evaluated("OtherPair");
    let refusal = diagnostics::join_messages(&refusal.unwrap_err());
    assert!(
        refusal.contains("selected layout policy belongs to another semantic type"),
        "{refusal}"
    );
    assert!(accounting.receipts().is_empty());
    assert!(wrong.plan_laid_layouts.is_empty());

    let (source, root, accounting, result) = evaluated("Pair");
    result.unwrap();
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
    let [crate::compile_time::PureEvaluationRecord::Invocation(receipt)] = accounting.receipts()
    else {
        panic!("one physical policy invocation");
    };
    assert_ne!(receipt.machine(), member.realization_machine);
    assert!(source.machine_by_symbol(receipt.machine()).is_none());
    assert!(
        receipt
            .source()
            .machine_instantiation_path(receipt.machine(), member.realization_machine)
            .is_some()
    );
    accounting.validate_charges().unwrap();
}
