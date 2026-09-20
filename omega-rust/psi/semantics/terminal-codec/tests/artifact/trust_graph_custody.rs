//! One-field mutation coverage for the canonical terminal trust graph.
//!
//! The validated trust graph is the reconstruction-trust record bound into
//! the obligation ledger and artifact manifest: the canonical ledger stamps
//! `graph.identity()` into its body, so a substituted graph whose honestly
//! recomputed identity diverges is rejected by the independent ledger replay
//! even when the substitution still passes structural admission. Every
//! representable field — the entry identity and each node's identity, kind,
//! status, semantic subject, version, owner, scope, rationale, accepting
//! policy, dependency roster, and the stated-source roster feeding the
//! dependency digest — is substituted independently. A substitution either
//! fails canonical graph admission with a named error, or produces a
//! well-formed graph whose divergent identity the ledger replay rejects.
//!
//! `digest`, `identity`, and `fully_derived` are derived, not representable:
//! `TrustDependencyNode::new` re-derives each substituted node's dependency
//! digest over its declared fields and stated sources, and admission
//! recomputes the graph identity and derivation flag. Substituted nodes
//! therefore carry a test-local stated-source roster; the stated-source leg
//! isolates that input by holding every declared field fixed.

use terminal_codec::{
    CodecError, TrustAcceptingPolicy, TrustDependencyKind, TrustDependencyNode,
    TrustDependencyStatus, TrustGraphError, ValidatedTerminalTrustGraph,
    build_terminal_obligation_ledger, current_terminal_trust_graph,
    validate_terminal_obligation_ledger, validate_terminal_trust_graph,
};

use super::semantic_module;

const TEST_SOURCES: &[(&str, &[u8])] = &[("substituted-source", b"omega")];

struct NodeFields {
    identity: String,
    kind: TrustDependencyKind,
    status: TrustDependencyStatus,
    semantic_subject: String,
    version: String,
    owner: String,
    scope: String,
    rationale: String,
    accepting_policy: TrustAcceptingPolicy,
    dependencies: Vec<String>,
    exact_sources: Vec<(&'static str, &'static [u8])>,
}

fn fields_of(node: &TrustDependencyNode) -> NodeFields {
    NodeFields {
        identity: node.identity().to_owned(),
        kind: node.kind(),
        status: node.status(),
        semantic_subject: node.semantic_subject().to_owned(),
        version: node.version().to_owned(),
        owner: node.owner().to_owned(),
        scope: node.scope().to_owned(),
        rationale: node.rationale().to_owned(),
        accepting_policy: node.accepting_policy(),
        dependencies: node.dependencies().to_vec(),
        exact_sources: TEST_SOURCES.to_vec(),
    }
}

fn rebuild(fields: NodeFields) -> TrustDependencyNode {
    TrustDependencyNode::new(
        fields.identity,
        fields.kind,
        fields.status,
        fields.semantic_subject,
        fields.version,
        fields.owner,
        fields.scope,
        fields.rationale,
        fields.accepting_policy,
        fields.dependencies,
        &fields.exact_sources,
    )
}

/// Replace node `index` with one reconstructed through the honest public
/// constructor after `edit` substitutes one declared field.
fn substitute_node(
    graph: &ValidatedTerminalTrustGraph,
    index: usize,
    edit: impl FnOnce(&mut NodeFields),
) -> Vec<TrustDependencyNode> {
    let mut nodes = graph.nodes().to_vec();
    let mut fields = fields_of(&nodes[index]);
    edit(&mut fields);
    nodes[index] = rebuild(fields);
    nodes
}

fn admit(
    graph: &ValidatedTerminalTrustGraph,
    nodes: Vec<TrustDependencyNode>,
) -> Result<ValidatedTerminalTrustGraph, TrustGraphError> {
    validate_terminal_trust_graph(graph.entry().to_owned(), nodes)
}

#[test]
fn terminal_trust_graph_rejects_every_one_field_substitution() {
    let graph = current_terminal_trust_graph().expect("current trust graph");
    let module = semantic_module();
    let ledger =
        build_terminal_obligation_ledger(&module, &graph).expect("authentic obligation ledger");
    assert_eq!(
        validate_terminal_obligation_ledger(&ledger, &module, &graph),
        Ok(())
    );

    let root_index = graph
        .nodes()
        .iter()
        .position(|node| node.kind() == TrustDependencyKind::RegisteredRoot)
        .expect("a registered root node");
    let nonroot_index = graph
        .nodes()
        .iter()
        .position(|node| node.kind() != TrustDependencyKind::RegisteredRoot)
        .expect("a non-root node");
    let dependent_index = graph
        .nodes()
        .iter()
        .position(|node| {
            node.kind() != TrustDependencyKind::RegisteredRoot && node.dependencies().len() >= 2
        })
        .expect("a non-root node with several dependencies");
    let nonroot_identity = graph.nodes()[nonroot_index].identity().to_owned();
    let dependent_identity = graph.nodes()[dependent_index].identity().to_owned();
    let root_identity = graph.nodes()[root_index].identity().to_owned();

    // A surviving substitution admits into a well-formed graph whose honestly
    // recomputed identity diverges, so the independent ledger replay rejects
    // the authentic ledger bound to the original graph.
    let replay_rejects = |name: &str, nodes: Vec<TrustDependencyNode>| {
        let substituted = admit(&graph, nodes)
            .unwrap_or_else(|error| panic!("{name}: substitution must still admit: {error}"));
        assert_ne!(
            substituted.identity(),
            graph.identity(),
            "{name}: graph identity must change"
        );
        let rebound = build_terminal_obligation_ledger(&module, &substituted)
            .expect("ledger bound to the substituted graph");
        assert_eq!(
            rebound.trust_graph(),
            substituted.identity(),
            "{name}: rebound ledger names the substituted graph"
        );
        assert_eq!(
            validate_terminal_obligation_ledger(&ledger, &module, &substituted),
            Err(CodecError::ObligationLedgerMismatch),
            "{name}: ledger replay must reject the substitution"
        );
    };
    let admission_rejects =
        |name: &str, nodes: Vec<TrustDependencyNode>, expected: TrustGraphError| {
            assert_eq!(
                admit(&graph, nodes),
                Err(expected),
                "{name}: canonical admission must reject the substitution"
            );
        };

    // Entry: the only representable values still fail admission — empty,
    // absent from the roster, or a member node that cannot reach the whole
    // graph.
    assert_eq!(
        validate_terminal_trust_graph(String::new(), graph.nodes().to_vec()),
        Err(TrustGraphError::EmptyEntry)
    );
    assert_eq!(
        validate_terminal_trust_graph("closure:not-a-node".to_owned(), graph.nodes().to_vec()),
        Err(TrustGraphError::UnknownEntry(
            "closure:not-a-node".to_owned()
        ))
    );
    assert!(matches!(
        validate_terminal_trust_graph(root_identity.clone(), graph.nodes().to_vec()),
        Err(TrustGraphError::UnreachableNode(_))
    ));

    // Node roster: order, uniqueness, and completeness are canonical
    // admission fields; representable roster mutations are named rejections.
    let mut swapped = graph.nodes().to_vec();
    swapped.swap(0, 1);
    admission_rejects(
        "non-canonical node order",
        swapped,
        TrustGraphError::NonCanonicalNodeOrder,
    );
    let mut duplicated = graph.nodes().to_vec();
    let duplicated_identity = duplicated[1].identity().to_owned();
    duplicated[0] = rebuild(NodeFields {
        identity: duplicated_identity.clone(),
        ..fields_of(&duplicated[0])
    });
    admission_rejects(
        "duplicate node",
        duplicated,
        TrustGraphError::DuplicateNode(duplicated_identity),
    );
    let dependency_target = graph.nodes()[dependent_index].dependencies()[0].clone();
    let target_index = graph
        .nodes()
        .iter()
        .position(|node| node.identity() == dependency_target)
        .expect("dependency target present");
    let mut removed = graph.nodes().to_vec();
    removed.remove(target_index);
    assert!(matches!(
        admit(&graph, removed),
        Err(TrustGraphError::UnknownDependency { .. }) | Err(TrustGraphError::UnreachableNode(_))
    ));
    let mut extended = graph.nodes().to_vec();
    let mut extra = fields_of(&graph.nodes()[nonroot_index]);
    extra.identity = "zzz:unreachable-substituted-node".to_owned();
    extended.push(rebuild(extra));
    admission_rejects(
        "unreachable added node",
        extended,
        TrustGraphError::UnreachableNode("zzz:unreachable-substituted-node".to_owned()),
    );

    // Identity: every representable substitution is a named rejection —
    // dependents name the missing identity, order breaks first when the
    // substitute sorts elsewhere, and clearing the field is rejected as
    // empty.
    assert!(matches!(
        admit(
            &graph,
            substitute_node(&graph, nonroot_index, |fields| {
                fields.identity = format!("{nonroot_identity}-substituted");
            })
        ),
        Err(TrustGraphError::NonCanonicalNodeOrder)
            | Err(TrustGraphError::UnknownDependency { .. })
            | Err(TrustGraphError::UnknownEntry(_))
    ));
    let mut empty_identity = substitute_node(&graph, nonroot_index, |fields| {
        fields.identity = String::new();
    });
    // Keep the emptied node canonical in the roster so admission reaches the
    // per-node field check.
    let emptied = empty_identity.remove(nonroot_index);
    empty_identity.insert(0, emptied);
    admission_rejects(
        "empty node identity",
        empty_identity,
        TrustGraphError::EmptyField {
            node: String::new(),
            field: "identity",
        },
    );

    // Kind: a non-root kind substitutes freely on a non-root node; crossing
    // the root boundary in either direction is a named rejection.
    let alternate_kind = [
        TrustDependencyKind::AcceptanceClosure,
        TrustDependencyKind::CallComposition,
        TrustDependencyKind::DenotationSchema,
        TrustDependencyKind::LedgerFramework,
        TrustDependencyKind::StructuralEffectSchema,
        TrustDependencyKind::SufficientFormReduction,
        TrustDependencyKind::TrustedImplementation,
    ]
    .into_iter()
    .find(|kind| *kind != graph.nodes()[nonroot_index].kind())
    .expect("an alternate non-root kind");
    replay_rejects(
        "kind",
        substitute_node(&graph, nonroot_index, |fields| {
            fields.kind = alternate_kind;
        }),
    );
    admission_rejects(
        "root kind on a dependent node",
        substitute_node(&graph, nonroot_index, |fields| {
            fields.kind = TrustDependencyKind::RegisteredRoot;
        }),
        TrustGraphError::RootHasDependencies(nonroot_identity.clone()),
    );
    admission_rejects(
        "non-root kind on a root",
        substitute_node(&graph, root_index, |fields| {
            fields.kind = TrustDependencyKind::TrustedImplementation;
        }),
        TrustGraphError::NonRootHasNoDependencies(root_identity.clone()),
    );

    // Status: non-registered statuses substitute on a non-root node; the
    // registered status crosses the named boundary in both directions.
    let status = graph.nodes()[nonroot_index].status();
    let alternate_status = [
        TrustDependencyStatus::FullyDerived,
        TrustDependencyStatus::LocallyDerivedPendingComposition,
        TrustDependencyStatus::TrustedJudgment,
    ]
    .into_iter()
    .find(|candidate| *candidate != status)
    .expect("an alternate non-registered status");
    replay_rejects(
        "status",
        substitute_node(&graph, nonroot_index, |fields| {
            fields.status = alternate_status;
        }),
    );
    admission_rejects(
        "registered status on a non-root",
        substitute_node(&graph, nonroot_index, |fields| {
            fields.status = TrustDependencyStatus::Registered;
        }),
        TrustGraphError::NonRootHasRegisteredStatus(nonroot_identity.clone()),
    );
    admission_rejects(
        "derived status on a root",
        substitute_node(&graph, root_index, |fields| {
            fields.status = TrustDependencyStatus::FullyDerived;
        }),
        TrustGraphError::RootHasInvalidStatus(root_identity.clone()),
    );

    // Accepting policy: free on non-root nodes; a root pinned to a
    // kernel-checked policy is a named rejection.
    let alternate_policy = [
        TrustAcceptingPolicy::ExplicitMigrationTrust,
        TrustAcceptingPolicy::KernelCheckedDerivation,
        TrustAcceptingPolicy::RegisteredSemanticFoundation,
    ]
    .into_iter()
    .find(|policy| *policy != graph.nodes()[nonroot_index].accepting_policy())
    .expect("an alternate accepting policy");
    replay_rejects(
        "accepting policy",
        substitute_node(&graph, nonroot_index, |fields| {
            fields.accepting_policy = alternate_policy;
        }),
    );
    admission_rejects(
        "kernel policy on a root",
        substitute_node(&graph, root_index, |fields| {
            fields.accepting_policy = TrustAcceptingPolicy::KernelCheckedDerivation;
        }),
        TrustGraphError::RootHasInvalidPolicy(root_identity.clone()),
    );

    // Free descriptive fields: each substitutes independently on a non-root
    // node, and each is named when cleared to empty.
    for (name, field, edit) in [
        (
            "semantic subject",
            "semantic_subject",
            (|fields: &mut NodeFields| {
                fields.semantic_subject = "substituted semantic subject".to_owned();
            }) as fn(&mut NodeFields),
        ),
        (
            "version",
            "version",
            (|fields: &mut NodeFields| {
                fields.version = "substituted-version".to_owned();
            }) as fn(&mut NodeFields),
        ),
        (
            "owner",
            "owner",
            (|fields: &mut NodeFields| {
                fields.owner = "substituted owner".to_owned();
            }) as fn(&mut NodeFields),
        ),
        (
            "scope",
            "scope",
            (|fields: &mut NodeFields| {
                fields.scope = "substituted scope".to_owned();
            }) as fn(&mut NodeFields),
        ),
        (
            "rationale",
            "rationale",
            (|fields: &mut NodeFields| {
                fields.rationale = "substituted rationale".to_owned();
            }) as fn(&mut NodeFields),
        ),
    ] {
        replay_rejects(name, substitute_node(&graph, nonroot_index, edit));
        admission_rejects(
            name,
            substitute_node(&graph, nonroot_index, |fields| match field {
                "semantic_subject" => fields.semantic_subject = String::new(),
                "version" => fields.version = String::new(),
                "owner" => fields.owner = String::new(),
                "scope" => fields.scope = String::new(),
                _ => fields.rationale = String::new(),
            }),
            TrustGraphError::EmptyField {
                node: nonroot_identity.clone(),
                field,
            },
        );
    }

    // Dependency roster: a different valid member extends the roster and is
    // rejected on replay; non-canonical order, unknown entries, self
    // references, cycles, and empty rosters are named rejections.
    let dependencies = graph.nodes()[dependent_index].dependencies().to_vec();
    let added_member = graph
        .nodes()
        .iter()
        .filter(|node| node.kind() == TrustDependencyKind::RegisteredRoot)
        .map(|node| node.identity())
        .find(|candidate| {
            !dependencies
                .iter()
                .any(|dependency| dependency == candidate)
        })
        .expect("a registered root absent from the dependency roster")
        .to_owned();
    replay_rejects(
        "dependency member",
        substitute_node(&graph, dependent_index, |fields| {
            // A registered root has no dependencies, so extending the roster
            // with it cannot introduce a cycle and keeps every member
            // reachable.
            fields.dependencies.push(added_member.clone());
            fields.dependencies.sort();
        }),
    );
    let mut reordered = dependencies.clone();
    reordered.swap(0, 1);
    admission_rejects(
        "non-canonical dependency order",
        substitute_node(&graph, dependent_index, |fields| {
            fields.dependencies = reordered;
        }),
        TrustGraphError::NonCanonicalDependencyOrder {
            node: dependent_identity.clone(),
        },
    );
    admission_rejects(
        "self dependency",
        substitute_node(&graph, dependent_index, |fields| {
            let self_identity = fields.identity.clone();
            fields.dependencies.push(self_identity);
            fields.dependencies.sort();
        }),
        TrustGraphError::SelfDependency(dependent_identity.clone()),
    );
    admission_rejects(
        "unknown dependency",
        substitute_node(&graph, dependent_index, |fields| {
            fields.dependencies.push("dependency:not-a-node".to_owned());
            fields.dependencies.sort();
        }),
        TrustGraphError::UnknownDependency {
            node: dependent_identity.clone(),
            dependency: "dependency:not-a-node".to_owned(),
        },
    );
    // A real two-node cycle: point a non-root dependency target back at one
    // of its dependents.
    let (cycle_dependent, cycle_target) = graph
        .nodes()
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            node.dependencies().iter().find_map(|dependency| {
                graph
                    .nodes()
                    .iter()
                    .position(|target| {
                        target.kind() != TrustDependencyKind::RegisteredRoot
                            && target.identity() == *dependency
                    })
                    .map(|target| (index, target))
            })
        })
        .expect("a non-root dependency edge");
    let mut cycled = graph.nodes().to_vec();
    let cycle_identity = cycled[cycle_dependent].identity().to_owned();
    let mut target_fields = fields_of(&cycled[cycle_target]);
    target_fields.dependencies = vec![cycle_identity.clone()];
    cycled[cycle_target] = rebuild(target_fields);
    admission_rejects(
        "dependency cycle",
        cycled,
        TrustGraphError::DependencyCycle(cycle_identity),
    );
    admission_rejects(
        "empty dependency roster on a non-root",
        substitute_node(&graph, dependent_index, |fields| {
            fields.dependencies = Vec::new();
        }),
        TrustGraphError::NonRootHasNoDependencies(dependent_identity.clone()),
    );
    admission_rejects(
        "dependency on a root",
        substitute_node(&graph, root_index, |fields| {
            fields.dependencies = vec![nonroot_identity.clone()];
        }),
        TrustGraphError::RootHasDependencies(root_identity.clone()),
    );

    // Stated-source roster: the dependency digest is derived from it, so a
    // substitution holding every declared field fixed still produces a
    // different node and a divergent graph identity.
    let substituted = substitute_node(&graph, nonroot_index, |fields| {
        fields.exact_sources = vec![("substituted-source", b"omega-substituted-sources")];
    });
    assert_ne!(
        substituted[nonroot_index].digest(),
        graph.nodes()[nonroot_index].digest(),
        "digest: stated sources must recompute the node digest"
    );
    replay_rejects("stated sources", substituted);
}
