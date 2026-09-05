use super::*;
use semantic_vocabulary::PackageKeyIdentity;

const REACHABLE_PROOF_SCC: &str = r#"
    data ProofTree {
        case Leaf;
        case Branch(first: ProofTree, second: ProofTree);
    }

    data Root {}
    machine Root::main(&mut self)
    requires left(ProofTree::Leaf) == ProofTree::Leaf
    {}

    machine left(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: right(first),
                second: right(second),
            }
        }
    }

    machine right(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: left(first),
                second: left(second),
            }
        }
    }
"#;

const REACHABLE_SINGLETON_PROOF_SCC: &str = r#"
    data ProofTree {
        case Leaf;
        case Branch(first: ProofTree, second: ProofTree);
    }

    data Root {}
    machine Root::main(&mut self)
    requires descend(ProofTree::Leaf) == ProofTree::Leaf
    {}

    machine descend(n: ProofTree)
    terminates by n;
    -> ProofTree
    {
        transition n {
            ProofTree::Leaf -> ProofTree::Leaf
            ProofTree::Branch { first, second } -> ProofTree::Branch {
                first: descend(first),
                second: second,
            }
        }
    }
"#;

#[test]
fn selected_proof_closure_lowers_exact_recursive_component() {
    let checked = checked_managed_source(REACHABLE_PROOF_SCC);
    let root_symbol = select_terminal_machine(&checked, "Root::main")
        .expect("root selection")
        .machine;
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == root_symbol)
        .expect("root machine");
    let left = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "left")
        .expect("left machine");
    let left_entry = checked
        .typed
        .machine_states(left)
        .first()
        .expect("left entry state");
    let dependencies = validation::machine_call_dependency_symbols(&checked.typed, root);
    assert!(
        dependencies.contains(&left_entry.symbol),
        "closure must resolve an entry-state call target back to its owning machine"
    );
    let lowered = lower_machine(&checked, "Root::main").expect("proof SCC should lower");
    let [component] = lowered
        .semantic_module
        .proof_recursive_components
        .as_slice()
    else {
        panic!("one reachable proof SCC")
    };
    assert_eq!(component.members.len(), 2);
    assert_eq!(component.edges.len(), 4);
    assert_eq!(component.types.len(), 1);
    assert_eq!(component.types[0].fields.len(), 2);

    let left = component
        .members
        .iter()
        .find(|member| member.machine_identity.ends_with("::left"))
        .expect("left member")
        .contract;
    let right = component
        .members
        .iter()
        .find(|member| member.machine_identity.ends_with("::right"))
        .expect("right member")
        .contract;
    assert_eq!(
        component
            .edges
            .iter()
            .filter(|edge| edge.caller == left && edge.callee == right)
            .count(),
        2,
        "same-pair calls remain exact rows"
    );
    assert_eq!(
        component
            .edges
            .iter()
            .filter(|edge| edge.caller == right && edge.callee == left)
            .count(),
        2,
        "reverse same-pair calls remain exact rows"
    );
    assert_eq!(
        component
            .edges
            .iter()
            .map(|edge| (edge.caller, edge.site.clone()))
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
    assert!(
        component
            .edges
            .iter()
            .all(|edge| edge.strict_member_path.len() == 1)
    );
    assert_eq!(
        component
            .edges
            .iter()
            .map(|edge| edge.strict_member_path[0].as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2,
        "both exact recursive payload fields survive"
    );

    let [certificate] = lowered.proof_bundle.recursive_components.as_slice() else {
        panic!("one grouped recursive certificate")
    };
    assert_eq!(certificate.certificate.edges.len(), 4);
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("source-produced recursive certificate verifies");
    let synopsis =
        terminal_codec::render_verified_proof_synopsis(&verified).expect("proof synopsis");
    assert_eq!(synopsis.matches("recursive-component ").count(), 1);
    assert_eq!(synopsis.matches("  member ").count(), 2);
    assert_eq!(synopsis.matches("  well-founded obligation ").count(), 1);
    assert_eq!(synopsis.matches("  decrease obligation ").count(), 4);

    let module_bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&module_bytes).expect("decode"),
        lowered.semantic_module
    );
    let proof_bytes =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    assert_eq!(
        terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof"),
        lowered.proof_bundle
    );
    let _artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &terminal_codec::build_identity_optimization_execution_record(
            &lowered.semantic_module,
            &lowered.proof_bundle,
        )
        .expect("identity optimization execution"),
        lowered.debug_map.as_ref(),
    )
    .expect("canonical artifact");
}

#[test]
fn selected_singleton_proof_closure_uses_the_grouped_certificate_path() {
    let checked = checked_managed_source(REACHABLE_SINGLETON_PROOF_SCC);
    let lowered = lower_machine(&checked, "Root::main").expect("proof SCC should lower");
    let [component] = lowered
        .semantic_module
        .proof_recursive_components
        .as_slice()
    else {
        panic!("one reachable singleton proof SCC")
    };
    assert_eq!(component.members.len(), 1);
    assert_eq!(component.edges.len(), 1);
    assert_eq!(component.edges[0].caller, component.members[0].contract);
    assert_eq!(component.edges[0].callee, component.members[0].contract);
    assert_eq!(component.edges[0].strict_member_path.len(), 1);

    let [certificate] = lowered.proof_bundle.recursive_components.as_slice() else {
        panic!("one grouped recursive certificate")
    };
    assert_eq!(certificate.certificate.edges.len(), 1);
    let verified = terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("source-produced singleton certificate verifies");
    let synopsis =
        terminal_codec::render_verified_proof_synopsis(&verified).expect("proof synopsis");
    assert_eq!(synopsis.matches("recursive-component ").count(), 1);
    assert_eq!(synopsis.matches("  member ").count(), 1);
    assert_eq!(synopsis.matches("  well-founded obligation ").count(), 1);
    assert_eq!(synopsis.matches("  decrease obligation ").count(), 1);

    let module_bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&module_bytes).expect("decode"),
        lowered.semantic_module
    );
    let proof_bytes =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    assert_eq!(
        terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof"),
        lowered.proof_bundle
    );
    let _artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &terminal_codec::build_identity_optimization_execution_record(
            &lowered.semantic_module,
            &lowered.proof_bundle,
        )
        .expect("identity optimization execution"),
        lowered.debug_map.as_ref(),
    )
    .expect("canonical artifact");

    let mut missing = lowered.proof_bundle.clone();
    missing.recursive_components.clear();
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &proof_admission::AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingRecursiveComponentEvidence(_))
    ));

    let mut changed_site = lowered.semantic_module.clone();
    match &mut changed_site.proof_recursive_components[0].edges[0].site {
        terminal_psi::TerminalProofRecursiveCallSite::Expression {
            expression_ordinal, ..
        } => *expression_ordinal += 100,
        _ => panic!("fixture uses an expression call site"),
    }
    assert!(
        terminal_verifier::verify_module(
            &changed_site,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn unreachable_proof_scc_is_not_retained() {
    let source = REACHABLE_PROOF_SCC.replace(
        "    requires left(ProofTree::Leaf) == ProofTree::Leaf\n",
        "",
    );
    let checked = checked_managed_source(&source);
    let lowered = lower_machine(&checked, "Root::main").expect("unrelated entry should lower");
    assert!(
        lowered
            .semantic_module
            .proof_recursive_components
            .is_empty()
    );
    assert!(lowered.proof_bundle.recursive_components.is_empty());
}

#[test]
fn unreachable_singleton_proof_scc_is_not_retained() {
    let source = REACHABLE_SINGLETON_PROOF_SCC.replace(
        "    requires descend(ProofTree::Leaf) == ProofTree::Leaf\n",
        "",
    );
    let checked = checked_managed_source(&source);
    let lowered = lower_machine(&checked, "Root::main").expect("unrelated entry should lower");
    assert!(
        lowered
            .semantic_module
            .proof_recursive_components
            .is_empty()
    );
    assert!(lowered.proof_bundle.recursive_components.is_empty());
}

#[test]
fn stale_or_missing_source_recursive_evidence_rejects() {
    let checked = checked_managed_source(REACHABLE_PROOF_SCC);
    let lowered = lower_machine(&checked, "Root::main").expect("proof SCC should lower");

    let mut missing = lowered.proof_bundle.clone();
    missing.recursive_components.clear();
    assert!(matches!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &missing,
            &proof_admission::AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingRecursiveComponentEvidence(_))
    ));

    let mut changed_site = lowered.semantic_module.clone();
    let edge = &mut changed_site.proof_recursive_components[0].edges[0];
    match &mut edge.site {
        terminal_psi::TerminalProofRecursiveCallSite::Expression {
            expression_ordinal, ..
        } => *expression_ordinal += 100,
        _ => panic!("fixture uses expression call sites"),
    }
    changed_site.proof_recursive_components[0].edges.sort();
    assert!(matches!(
        terminal_verifier::verify_module(
            &changed_site,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::MissingRecursiveComponentEvidence(_))
    ));

    let mut collapsed_site = lowered.semantic_module.clone();
    let edges = &mut collapsed_site.proof_recursive_components[0].edges;
    let (first, second) = edges
        .iter()
        .enumerate()
        .find_map(|(first_index, first)| {
            edges
                .iter()
                .enumerate()
                .skip(first_index + 1)
                .find(|(_, second)| second.caller == first.caller)
                .map(|(second_index, _)| (first_index, second_index))
        })
        .expect("fixture has repeated same-caller edges");
    edges[second].site = edges[first].site.clone();
    edges.sort();
    assert!(
        terminal_verifier::verify_module(
            &collapsed_site,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn stale_checked_edge_rank_parameter_rejects_before_erasure() {
    let mut checked = checked_managed_source(REACHABLE_PROOF_SCC);
    checked.facts.termination.proof_recursive_components[0].edges[0].callee_rank_parameter =
        symbols::SymbolHandle::invalid();
    assert!(matches!(
        lower_machine(&checked, "Root::main"),
        Err(LoweringError::Unsupported(
            "checked recursive edge rank parameter is stale"
        ))
    ));
}

fn checked_managed_source(source: &str) -> CheckedTrees {
    let package = PackageKeyIdentity::from_digest([0x91; 32]).expect("nonzero package identity");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("managed/proof-recursion/main.omg"),
            source.to_owned(),
            PathBuf::from("managed/proof-recursion"),
            Some(package),
            SourceOrigin::User,
        )
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}
