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
    CodecError, TrustAcceptingPolicy, TrustDependencyDigest, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus, TrustGraphError, build_terminal_obligation_ledger,
    current_terminal_trust_graph, validate_terminal_obligation_ledger,
    validate_terminal_trust_graph,
};

use mutation_matrix::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};

#[path = "trust_graph_custody_fields.rs"]
mod trust_graph_custody_fields;

use super::semantic_module;
use trust_graph_custody_fields::TrustGraphCustodyFieldForTest;

const TEST_SOURCES: &[(&str, &[u8])] = &[("substituted-source", b"omega")];
const DONOR_SOURCES: &[(&str, &[u8])] = &[("substituted-source", b"omega-donor")];

/// The record family under substitution: the entry identity plus the node
/// roster `validate_terminal_trust_graph` admits into a validated graph.
type TrustGraphParts = (String, Vec<TrustDependencyNode>);

/// The family's combined independent checker result: canonical admission
/// first, then the obligation-ledger replay that binds the graph identity.
#[derive(Debug, PartialEq)]
enum TrustGraphCheck {
    Admission(TrustGraphError),
    LedgerReplay(CodecError),
}

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
    // The entry bound to a registered root visits that node alone; the first
    // roster member admission reports as unreachable is the lowest canonical
    // identity that is not the entry.
    let unreachable_after_root_entry = graph
        .nodes()
        .iter()
        .map(|node| node.identity())
        .find(|identity| *identity != root_identity)
        .expect("a non-entry node")
        .to_owned();
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
    let cycle_identity = graph.nodes()[cycle_dependent].identity().to_owned();

    let honest = || (graph.entry().to_owned(), graph.nodes().to_vec());
    // A foreign record of the same family: the same roster with the non-root
    // node honestly rebuilt over a different subject and stated sources, so
    // its retained custody provably differs.
    let donor = {
        let mut nodes = graph.nodes().to_vec();
        let mut fields = fields_of(&nodes[nonroot_index]);
        fields.semantic_subject = "donor semantic subject".to_owned();
        fields.exact_sources = DONOR_SOURCES.to_vec();
        nodes[nonroot_index] = rebuild(fields);
        (graph.entry().to_owned(), nodes)
    };
    let custody = |parts: &TrustGraphParts| -> (String, Vec<TrustDependencyDigest>) {
        (
            parts.0.clone(),
            parts.1.iter().map(|node| node.digest()).collect(),
        )
    };
    let substitute = |parts: &mut TrustGraphParts,
                      field: TrustGraphCustodyFieldForTest,
                      _: &TrustGraphParts| {
        use TrustGraphCustodyFieldForTest as Leg;
        let edit_node =
            |nodes: &mut Vec<TrustDependencyNode>, index: usize, edit: &dyn Fn(&mut NodeFields)| {
                let mut fields = fields_of(&nodes[index]);
                edit(&mut fields);
                nodes[index] = rebuild(fields);
            };
        match field {
            Leg::EntryCleared => parts.0 = String::new(),
            Leg::EntryUnknown => parts.0 = "closure:not-a-node".to_owned(),
            Leg::EntryNamesRegisteredRoot => parts.0 = root_identity.clone(),
            Leg::NodeRosterReordered => parts.1.swap(0, 1),
            Leg::NodeRosterDuplicated => {
                let identity = parts.1[1].identity().to_owned();
                edit_node(&mut parts.1, 0, &move |fields| {
                    fields.identity = identity.clone();
                });
            }
            Leg::NodeRosterMemberExtended => {
                let mut fields = fields_of(&parts.1[nonroot_index]);
                fields.identity = "zzz:unreachable-substituted-node".to_owned();
                parts.1.push(rebuild(fields));
            }
            Leg::NonRootIdentityCleared => {
                let emptied = parts.1.remove(nonroot_index);
                let mut fields = fields_of(&emptied);
                fields.identity = String::new();
                // Keep the emptied node canonical in the roster so
                // admission reaches the per-node field check.
                parts.1.insert(0, rebuild(fields));
            }
            Leg::NonRootKindAlternate => {
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
                .find(|kind| *kind != parts.1[nonroot_index].kind())
                .expect("an alternate non-root kind");
                edit_node(&mut parts.1, nonroot_index, &move |fields| {
                    fields.kind = alternate_kind;
                });
            }
            Leg::NonRootKindRegisteredRoot => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.kind = TrustDependencyKind::RegisteredRoot;
                });
            }
            Leg::RootKindNonRoot => {
                edit_node(&mut parts.1, root_index, &|fields| {
                    fields.kind = TrustDependencyKind::TrustedImplementation;
                });
            }
            Leg::NonRootStatusAlternate => {
                let status = parts.1[nonroot_index].status();
                let alternate_status = [
                    TrustDependencyStatus::FullyDerived,
                    TrustDependencyStatus::LocallyDerivedPendingComposition,
                    TrustDependencyStatus::TrustedJudgment,
                ]
                .into_iter()
                .find(|candidate| *candidate != status)
                .expect("an alternate non-registered status");
                edit_node(&mut parts.1, nonroot_index, &move |fields| {
                    fields.status = alternate_status;
                });
            }
            Leg::NonRootStatusRegistered => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.status = TrustDependencyStatus::Registered;
                });
            }
            Leg::RootStatusDerived => {
                edit_node(&mut parts.1, root_index, &|fields| {
                    fields.status = TrustDependencyStatus::FullyDerived;
                });
            }
            Leg::NonRootPolicyAlternate => {
                let alternate_policy = [
                    TrustAcceptingPolicy::ExplicitMigrationTrust,
                    TrustAcceptingPolicy::KernelCheckedDerivation,
                    TrustAcceptingPolicy::RegisteredSemanticFoundation,
                ]
                .into_iter()
                .find(|policy| *policy != parts.1[nonroot_index].accepting_policy())
                .expect("an alternate accepting policy");
                edit_node(&mut parts.1, nonroot_index, &move |fields| {
                    fields.accepting_policy = alternate_policy;
                });
            }
            Leg::RootPolicyKernelChecked => {
                edit_node(&mut parts.1, root_index, &|fields| {
                    fields.accepting_policy = TrustAcceptingPolicy::KernelCheckedDerivation;
                });
            }
            Leg::NonRootSemanticSubjectSubstituted => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.semantic_subject = "substituted semantic subject".to_owned();
                });
            }
            Leg::NonRootSemanticSubjectCleared => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.semantic_subject = String::new();
                });
            }
            Leg::NonRootVersionSubstituted => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.version = "substituted-version".to_owned();
                });
            }
            Leg::NonRootVersionCleared => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.version = String::new();
                });
            }
            Leg::NonRootOwnerSubstituted => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.owner = "substituted owner".to_owned();
                });
            }
            Leg::NonRootOwnerCleared => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.owner = String::new();
                });
            }
            Leg::NonRootScopeSubstituted => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.scope = "substituted scope".to_owned();
                });
            }
            Leg::NonRootScopeCleared => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.scope = String::new();
                });
            }
            Leg::NonRootRationaleSubstituted => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.rationale = "substituted rationale".to_owned();
                });
            }
            Leg::NonRootRationaleCleared => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.rationale = String::new();
                });
            }
            Leg::DependentDependencyMemberAdded => {
                let member = added_member.clone();
                // A registered root has no dependencies, so extending the
                // roster with it cannot introduce a cycle and keeps every
                // member reachable.
                edit_node(&mut parts.1, dependent_index, &move |fields| {
                    fields.dependencies.push(member.clone());
                    fields.dependencies.sort();
                });
            }
            Leg::DependentDependencyOrderReordered => {
                let mut reordered = dependencies.clone();
                reordered.swap(0, 1);
                edit_node(&mut parts.1, dependent_index, &move |fields| {
                    fields.dependencies = reordered.clone();
                });
            }
            Leg::DependentDependencySelfAdded => {
                edit_node(&mut parts.1, dependent_index, &|fields| {
                    let self_identity = fields.identity.clone();
                    fields.dependencies.push(self_identity);
                    fields.dependencies.sort();
                });
            }
            Leg::DependentDependencyUnknownAdded => {
                edit_node(&mut parts.1, dependent_index, &|fields| {
                    fields.dependencies.push("dependency:not-a-node".to_owned());
                    fields.dependencies.sort();
                });
            }
            Leg::DependentDependenciesCleared => {
                edit_node(&mut parts.1, dependent_index, &|fields| {
                    fields.dependencies = Vec::new();
                });
            }
            Leg::RootDependencySet => {
                let member = nonroot_identity.clone();
                edit_node(&mut parts.1, root_index, &move |fields| {
                    fields.dependencies = vec![member.clone()];
                });
            }
            Leg::DependencyCycleIntroduced => {
                let mut fields = fields_of(&parts.1[cycle_target]);
                fields.dependencies = vec![cycle_identity.clone()];
                parts.1[cycle_target] = rebuild(fields);
            }
            Leg::NonRootStatedSourcesSubstituted => {
                edit_node(&mut parts.1, nonroot_index, &|fields| {
                    fields.exact_sources =
                        vec![("substituted-source", b"omega-substituted-sources")];
                });
            }
        }
    };
    // The independent checker replays both layers a substituted record must
    // survive: canonical admission, then the obligation-ledger replay bound
    // to the honest graph — a substitution that admits must still rebind a
    // ledger honestly and be rejected by the retained one.
    let check =
        |parts: &TrustGraphParts| -> Result<(String, Vec<TrustDependencyDigest>), TrustGraphCheck> {
            let admitted = validate_terminal_trust_graph(parts.0.clone(), parts.1.clone())
                .map_err(TrustGraphCheck::Admission)?;
            let rebound = build_terminal_obligation_ledger(&module, &admitted)
                .expect("ledger bound to the substituted graph");
            assert_eq!(
                rebound.trust_graph(),
                admitted.identity(),
                "rebound ledger names the substituted graph"
            );
            match validate_terminal_obligation_ledger(&ledger, &module, &admitted) {
                Ok(()) => Ok(custody(parts)),
                Err(error) => Err(TrustGraphCheck::LedgerReplay(error)),
            }
        };
    let outcome = |field: TrustGraphCustodyFieldForTest| {
        use TrustGraphCustodyFieldForTest as Leg;
        MutationOutcome::ExactError(match field {
            Leg::EntryCleared => TrustGraphCheck::Admission(TrustGraphError::EmptyEntry),
            Leg::EntryUnknown => TrustGraphCheck::Admission(TrustGraphError::UnknownEntry(
                "closure:not-a-node".to_owned(),
            )),
            Leg::EntryNamesRegisteredRoot => TrustGraphCheck::Admission(
                TrustGraphError::UnreachableNode(unreachable_after_root_entry.clone()),
            ),
            Leg::NodeRosterReordered => {
                TrustGraphCheck::Admission(TrustGraphError::NonCanonicalNodeOrder)
            }
            Leg::NodeRosterDuplicated => TrustGraphCheck::Admission(
                TrustGraphError::DuplicateNode(graph.nodes()[1].identity().to_owned()),
            ),
            Leg::NodeRosterMemberExtended => TrustGraphCheck::Admission(
                TrustGraphError::UnreachableNode("zzz:unreachable-substituted-node".to_owned()),
            ),
            Leg::NonRootIdentityCleared => {
                TrustGraphCheck::Admission(TrustGraphError::EmptyField {
                    node: String::new(),
                    field: "identity",
                })
            }
            Leg::NonRootKindAlternate
            | Leg::NonRootStatusAlternate
            | Leg::NonRootPolicyAlternate
            | Leg::NonRootSemanticSubjectSubstituted
            | Leg::NonRootVersionSubstituted
            | Leg::NonRootOwnerSubstituted
            | Leg::NonRootScopeSubstituted
            | Leg::NonRootRationaleSubstituted
            | Leg::DependentDependencyMemberAdded
            | Leg::NonRootStatedSourcesSubstituted => {
                TrustGraphCheck::LedgerReplay(CodecError::ObligationLedgerMismatch)
            }
            Leg::NonRootKindRegisteredRoot => TrustGraphCheck::Admission(
                TrustGraphError::RootHasDependencies(nonroot_identity.clone()),
            ),
            Leg::RootKindNonRoot => TrustGraphCheck::Admission(
                TrustGraphError::NonRootHasNoDependencies(root_identity.clone()),
            ),
            Leg::NonRootStatusRegistered => TrustGraphCheck::Admission(
                TrustGraphError::NonRootHasRegisteredStatus(nonroot_identity.clone()),
            ),
            Leg::RootStatusDerived => TrustGraphCheck::Admission(
                TrustGraphError::RootHasInvalidStatus(root_identity.clone()),
            ),
            Leg::RootPolicyKernelChecked => TrustGraphCheck::Admission(
                TrustGraphError::RootHasInvalidPolicy(root_identity.clone()),
            ),
            Leg::NonRootSemanticSubjectCleared => {
                TrustGraphCheck::Admission(TrustGraphError::EmptyField {
                    node: nonroot_identity.clone(),
                    field: "semantic_subject",
                })
            }
            Leg::NonRootVersionCleared => TrustGraphCheck::Admission(TrustGraphError::EmptyField {
                node: nonroot_identity.clone(),
                field: "version",
            }),
            Leg::NonRootOwnerCleared => TrustGraphCheck::Admission(TrustGraphError::EmptyField {
                node: nonroot_identity.clone(),
                field: "owner",
            }),
            Leg::NonRootScopeCleared => TrustGraphCheck::Admission(TrustGraphError::EmptyField {
                node: nonroot_identity.clone(),
                field: "scope",
            }),
            Leg::NonRootRationaleCleared => {
                TrustGraphCheck::Admission(TrustGraphError::EmptyField {
                    node: nonroot_identity.clone(),
                    field: "rationale",
                })
            }
            Leg::DependentDependencyOrderReordered => {
                TrustGraphCheck::Admission(TrustGraphError::NonCanonicalDependencyOrder {
                    node: dependent_identity.clone(),
                })
            }
            Leg::DependentDependencySelfAdded => TrustGraphCheck::Admission(
                TrustGraphError::SelfDependency(dependent_identity.clone()),
            ),
            Leg::DependentDependencyUnknownAdded => {
                TrustGraphCheck::Admission(TrustGraphError::UnknownDependency {
                    node: dependent_identity.clone(),
                    dependency: "dependency:not-a-node".to_owned(),
                })
            }
            Leg::DependentDependenciesCleared => TrustGraphCheck::Admission(
                TrustGraphError::NonRootHasNoDependencies(dependent_identity.clone()),
            ),
            Leg::RootDependencySet => TrustGraphCheck::Admission(
                TrustGraphError::RootHasDependencies(root_identity.clone()),
            ),
            Leg::DependencyCycleIntroduced => {
                TrustGraphCheck::Admission(TrustGraphError::DependencyCycle(cycle_identity.clone()))
            }
        })
    };
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "terminal trust graph",
        fields: TrustGraphCustodyFieldForTest::INVENTORY,
        honest: &honest,
        donor,
        custody: &custody,
        substitute: &substitute,
        check: &check,
        outcome: &outcome,
        joined_replay: None,
    });

    // Roster-member removal is deliberately outside the field inventory: the
    // first reporting node is whichever canonically-ordered node loses the
    // dependency, so the authored leg keeps its either-of match.
    let dependency_target = graph.nodes()[dependent_index].dependencies()[0].clone();
    let target_index = graph
        .nodes()
        .iter()
        .position(|node| node.identity() == dependency_target)
        .expect("dependency target present");
    let mut removed = graph.nodes().to_vec();
    removed.remove(target_index);
    assert!(matches!(
        validate_terminal_trust_graph(graph.entry().to_owned(), removed),
        Err(TrustGraphError::UnknownDependency { .. }) | Err(TrustGraphError::UnreachableNode(_))
    ));

    // A substituted identity sorts to a new canonical position, so the first
    // named rejection is whichever of the order/dependency/entry checks it
    // trips; the authored leg keeps its either-of match.
    let mut substituted = graph.nodes().to_vec();
    let mut fields = fields_of(&substituted[nonroot_index]);
    fields.identity = format!("{nonroot_identity}-substituted");
    substituted[nonroot_index] = rebuild(fields);
    assert!(matches!(
        validate_terminal_trust_graph(graph.entry().to_owned(), substituted),
        Err(TrustGraphError::NonCanonicalNodeOrder)
            | Err(TrustGraphError::UnknownDependency { .. })
            | Err(TrustGraphError::UnknownEntry(_))
    ));
}
