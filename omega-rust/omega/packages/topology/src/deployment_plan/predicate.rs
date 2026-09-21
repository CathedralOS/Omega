//! The fixed `no_route`/`only_via` reference predicates.
//!
//! Policies are ordinary deterministic computations over the immutable
//! normalized graph: no host observation, no mutation, finite worklists, and
//! at most two traversals each. Evaluation order cannot affect verdicts —
//! callers evaluate the required set in `PolicyCall::canonical_key` order.
//!
//! Correctness evidence here is structural, not a rerun of the same
//! traversal: `check_outcome` verifies that a recorded `Satisfied`
//! certificate states a property that *proves* the predicate (a closed
//! reachable set disjoint from the targets; a valid path plus a closed
//! reduced reachable set), and that a recorded `Violation` witness is a real
//! path in the right graph. `evaluate_policy` independently computes the
//! verdict a selected verifier must reach; `verify` uses both legs.

use crate::deployment_plan::graph::NormalizedGraph;
use crate::deployment_plan::{
    Certificate, InstanceName, PolicyCall, PolicyOutcome, PolicyPredicate, PolicySelector,
    Violation,
};
use std::collections::BTreeSet;
use std::fmt;

/// Which selector role failed validation, for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorRole {
    Sources,
    Targets,
    Via,
}

impl fmt::Display for SelectorRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sources => formatter.write_str("sources"),
            Self::Targets => formatter.write_str("targets"),
            Self::Via => formatter.write_str("via"),
        }
    }
}

/// Selector inputs that reject instead of vacuously satisfying a routing
/// requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    Empty {
        role: SelectorRole,
    },
    /// The selector names an instance the roster does not contain.
    UnknownMember {
        role: SelectorRole,
        name: InstanceName,
    },
    /// The same member appears twice in one selector.
    DuplicateMember {
        role: SelectorRole,
        name: InstanceName,
    },
    /// Two selector sets share an instance (`via` may overlap neither
    /// endpoint set, and source/target may not overlap each other).
    Overlapping {
        roles: (SelectorRole, SelectorRole),
        name: InstanceName,
    },
    /// `via` was supplied to a predicate that takes none, or omitted from
    /// `only_via`.
    ViaArity,
    LimitExceeded {
        role: SelectorRole,
    },
}

impl fmt::Display for SelectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { role } => write!(formatter, "{role} selector must not be empty"),
            Self::UnknownMember { role, name } => {
                write!(formatter, "{role} names unknown instance `{name}`")
            }
            Self::DuplicateMember { role, name } => {
                write!(formatter, "{role} lists `{name}` twice")
            }
            Self::Overlapping { roles, name } => {
                write!(
                    formatter,
                    "instance `{name}` appears in both {} and {}",
                    roles.0, roles.1
                )
            }
            Self::ViaArity => {
                formatter.write_str("via must be empty for no_route and non-empty for only_via")
            }
            Self::LimitExceeded { role } => {
                write!(
                    formatter,
                    "{role} selector exceeds the bounded member count"
                )
            }
        }
    }
}

impl std::error::Error for SelectorError {}

/// Validate one selector against the roster: non-empty, bounded, unique,
/// every member resolving to a real instance. Returns the vertex index set.
pub fn validate_selector(
    graph: &NormalizedGraph,
    selector: &PolicySelector,
    role: SelectorRole,
) -> Result<BTreeSet<u32>, SelectorError> {
    if selector.members.is_empty() {
        return Err(SelectorError::Empty { role });
    }
    if selector.members.len() > crate::deployment_plan::codec::MAX_SELECTOR_MEMBERS {
        return Err(SelectorError::LimitExceeded { role });
    }
    let mut resolved = BTreeSet::new();
    for member in &selector.members {
        let Some(index) = graph.index_of(member) else {
            return Err(SelectorError::UnknownMember {
                role,
                name: member.clone(),
            });
        };
        if !resolved.insert(index) {
            return Err(SelectorError::DuplicateMember {
                role,
                name: member.clone(),
            });
        }
    }
    Ok(resolved)
}

fn validate_disjoint(
    graph: &NormalizedGraph,
    left: &BTreeSet<u32>,
    right: &BTreeSet<u32>,
    roles: (SelectorRole, SelectorRole),
) -> Result<(), SelectorError> {
    if let Some(&shared) = left.intersection(right).next() {
        return Err(SelectorError::Overlapping {
            roles,
            name: graph.instances()[shared as usize].name.clone(),
        });
    }
    Ok(())
}

/// The complete evaluation of one required policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyEvaluation {
    /// Inputs rejected before any verdict — never a satisfied predicate.
    InvalidSelectors(Vec<SelectorError>),
    /// The predicate computed a verdict with its evidence.
    Decided(PolicyOutcome),
}

/// Validate a call's three selectors and their disjointness rules, then
/// evaluate the predicate. This is the reference computation the selected
/// verifier and the independent replay must agree on.
pub fn evaluate_policy(graph: &NormalizedGraph, call: &PolicyCall) -> PolicyEvaluation {
    let mut errors = Vec::new();
    let sources = validate_selector(graph, &call.sources, SelectorRole::Sources);
    let targets = validate_selector(graph, &call.targets, SelectorRole::Targets);
    if let Err(error) = &sources {
        errors.push(error.clone());
    }
    if let Err(error) = &targets {
        errors.push(error.clone());
    }
    match call.predicate {
        PolicyPredicate::NoRoute if !call.via.members.is_empty() => {
            errors.push(SelectorError::ViaArity);
        }
        PolicyPredicate::OnlyVia if call.via.members.is_empty() => {
            errors.push(SelectorError::ViaArity);
        }
        _ => {}
    }
    let via = validate_selector(graph, &call.via, SelectorRole::Via);
    if call.predicate == PolicyPredicate::OnlyVia
        && let Err(error) = &via
    {
        errors.push(error.clone());
    }
    let (Ok(sources), Ok(targets)) = (sources, targets) else {
        return PolicyEvaluation::InvalidSelectors(errors);
    };
    if let Err(error) = validate_disjoint(
        graph,
        &sources,
        &targets,
        (SelectorRole::Sources, SelectorRole::Targets),
    ) {
        errors.push(error);
    }
    let via_set = match call.predicate {
        PolicyPredicate::NoRoute => BTreeSet::new(),
        PolicyPredicate::OnlyVia => {
            let Ok(via_set) = via else {
                return PolicyEvaluation::InvalidSelectors(errors);
            };
            for (role, set) in [
                (SelectorRole::Sources, &sources),
                (SelectorRole::Targets, &targets),
            ] {
                if let Err(error) =
                    validate_disjoint(graph, set, &via_set, (role, SelectorRole::Via))
                {
                    errors.push(error);
                }
            }
            via_set
        }
    };
    if !errors.is_empty() {
        return PolicyEvaluation::InvalidSelectors(errors);
    }

    let outcome = match call.predicate {
        PolicyPredicate::NoRoute => evaluate_no_route(graph, &sources, &targets),
        PolicyPredicate::OnlyVia => evaluate_only_via(graph, &sources, &targets, &via_set),
    };
    PolicyEvaluation::Decided(outcome)
}

fn evaluate_no_route(
    graph: &NormalizedGraph,
    sources: &BTreeSet<u32>,
    targets: &BTreeSet<u32>,
) -> PolicyOutcome {
    let excluded = BTreeSet::new();
    let reachable = graph.reachable_from(sources, &excluded);
    if reachable.iter().any(|vertex| targets.contains(vertex)) {
        let path = graph
            .shortest_path(sources, targets, &excluded)
            .expect("a target in the reachable set has a path");
        PolicyOutcome::Violated {
            violation: Violation::Bypass { path },
        }
    } else {
        PolicyOutcome::Satisfied {
            certificate: Certificate::NoRoute { reachable },
        }
    }
}

fn evaluate_only_via(
    graph: &NormalizedGraph,
    sources: &BTreeSet<u32>,
    targets: &BTreeSet<u32>,
    via: &BTreeSet<u32>,
) -> PolicyOutcome {
    let empty = BTreeSet::new();
    // Non-vacuity: at least one source-to-target path must exist.
    let Some(path) = graph.shortest_path(sources, targets, &empty) else {
        return PolicyOutcome::Violated {
            violation: Violation::Disconnected,
        };
    };
    // Every such path must visit `via`: check source reachability in G−V.
    let reduced = graph.reachable_from(sources, via);
    if reduced.iter().any(|vertex| targets.contains(vertex)) {
        let bypass = graph
            .shortest_path(sources, targets, via)
            .expect("a target in the reduced reachable set has a path avoiding via");
        PolicyOutcome::Violated {
            violation: Violation::Bypass { path: bypass },
        }
    } else {
        PolicyOutcome::Satisfied {
            certificate: Certificate::OnlyVia {
                path,
                reachable: reduced,
            },
        }
    }
}

/// Why a recorded certificate fails independent checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateRejection {
    /// Recorded outcome kind does not match the predicate it claims evidence
    /// for.
    WrongEvidenceShape,
    /// A certificate set is not in canonical sorted-unique order.
    NotCanonical,
    /// The recorded reachable set omits a source vertex.
    MissingSource { vertex: u32 },
    /// The recorded reachable set is not closed under the graph's edges.
    NotClosed { from: u32, to: u32 },
    /// The recorded reachable set contains a target (or, for `only_via`, a
    /// `via`) vertex it must be disjoint from.
    NotDisjoint { vertex: u32 },
    /// A bypass/existence witness is not a real path in the required graph.
    InvalidPath,
    /// A recorded `Disconnected` violation cannot be evidenced; replay must
    /// agree.
    Unevidenced,
}

impl fmt::Display for CertificateRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongEvidenceShape => {
                formatter.write_str("evidence does not match the predicate/outcome shape")
            }
            Self::NotCanonical => formatter.write_str("certificate set is not in canonical order"),
            Self::MissingSource { vertex } => {
                write!(formatter, "certificate omits source vertex {vertex}")
            }
            Self::NotClosed { from, to } => {
                write!(
                    formatter,
                    "certificate set contains {from} but not edge target {to}"
                )
            }
            Self::NotDisjoint { vertex } => {
                write!(
                    formatter,
                    "certificate set contains forbidden vertex {vertex}"
                )
            }
            Self::InvalidPath => {
                formatter.write_str("witness is not a valid path in the required graph")
            }
            Self::Unevidenced => formatter.write_str("violation carries no checkable witness"),
        }
    }
}

impl std::error::Error for CertificateRejection {}

fn sorted_unique(indices: &[u32]) -> bool {
    indices.windows(2).all(|pair| pair[0] < pair[1])
}

/// Check that a recorded set is closed under every edge whose source it
/// contains, skipping edges that enter `excluded` vertices (the reduced-graph
/// rule used by `only_via`).
fn check_closed(
    graph: &NormalizedGraph,
    set: &BTreeSet<u32>,
    excluded: &BTreeSet<u32>,
) -> Result<(), CertificateRejection> {
    for &vertex in set {
        for &successor in graph.successors(vertex) {
            if excluded.contains(&successor) {
                continue;
            }
            if !set.contains(&successor) {
                return Err(CertificateRejection::NotClosed {
                    from: vertex,
                    to: successor,
                });
            }
        }
    }
    Ok(())
}

/// Independently check one recorded outcome against the actual edges —
/// without trusting whichever traversal produced it. A `Satisfied`
/// certificate must state the property that proves the predicate; a
/// `Violated` witness must be a real path the predicate forbids.
pub fn check_outcome(
    graph: &NormalizedGraph,
    call: &PolicyCall,
    outcome: &PolicyOutcome,
) -> Result<(), CertificateRejection> {
    let sources = validate_selector(graph, &call.sources, SelectorRole::Sources)
        .map_err(|_| CertificateRejection::InvalidPath)?;
    let targets = validate_selector(graph, &call.targets, SelectorRole::Targets)
        .map_err(|_| CertificateRejection::InvalidPath)?;
    let via = match call.predicate {
        PolicyPredicate::OnlyVia => validate_selector(graph, &call.via, SelectorRole::Via)
            .map_err(|_| CertificateRejection::InvalidPath)?,
        PolicyPredicate::NoRoute => BTreeSet::new(),
    };
    let empty = BTreeSet::new();

    match (call.predicate, outcome) {
        (
            PolicyPredicate::NoRoute,
            PolicyOutcome::Satisfied {
                certificate: Certificate::NoRoute { reachable },
            },
        ) => {
            if !sorted_unique(reachable) {
                return Err(CertificateRejection::NotCanonical);
            }
            let set: BTreeSet<u32> = reachable.iter().copied().collect();
            for &source in &sources {
                if !set.contains(&source) {
                    return Err(CertificateRejection::MissingSource { vertex: source });
                }
            }
            if let Some(&vertex) = set.intersection(&targets).next() {
                return Err(CertificateRejection::NotDisjoint { vertex });
            }
            check_closed(graph, &set, &empty)
        }
        (
            PolicyPredicate::OnlyVia,
            PolicyOutcome::Satisfied {
                certificate: Certificate::OnlyVia { path, reachable },
            },
        ) => {
            if !graph.path_is_valid(path, &sources, &targets, &empty) {
                return Err(CertificateRejection::InvalidPath);
            }
            if !sorted_unique(reachable) {
                return Err(CertificateRejection::NotCanonical);
            }
            let set: BTreeSet<u32> = reachable.iter().copied().collect();
            for &source in &sources {
                if !set.contains(&source) {
                    return Err(CertificateRejection::MissingSource { vertex: source });
                }
            }
            if let Some(&vertex) = set.intersection(&targets).next() {
                return Err(CertificateRejection::NotDisjoint { vertex });
            }
            if let Some(&vertex) = set.intersection(&via).next() {
                return Err(CertificateRejection::NotDisjoint { vertex });
            }
            check_closed(graph, &set, &via)
        }
        (
            PolicyPredicate::NoRoute,
            PolicyOutcome::Violated {
                violation: Violation::Bypass { path },
            },
        ) => {
            if graph.path_is_valid(path, &sources, &targets, &empty) {
                Ok(())
            } else {
                Err(CertificateRejection::InvalidPath)
            }
        }
        (
            PolicyPredicate::OnlyVia,
            PolicyOutcome::Violated {
                violation: Violation::Bypass { path },
            },
        ) => {
            if graph.path_is_valid(path, &sources, &targets, &via) {
                Ok(())
            } else {
                Err(CertificateRejection::InvalidPath)
            }
        }
        (
            PolicyPredicate::OnlyVia,
            PolicyOutcome::Violated {
                violation: Violation::Disconnected,
            },
        ) => Err(CertificateRejection::Unevidenced),
        (
            PolicyPredicate::NoRoute,
            PolicyOutcome::Violated {
                violation: Violation::Disconnected,
            },
        ) => Err(CertificateRejection::WrongEvidenceShape),
        (_, _) => Err(CertificateRejection::WrongEvidenceShape),
    }
}

/// Reorder a policy roster into `PolicyCall::canonical_key` order — the
/// deterministic check sequence.
pub fn canonical_policy_order(calls: &mut [PolicyCall]) {
    calls.sort_by_key(|call| call.canonical_key());
}

#[cfg(test)]
mod tests {
    use super::{
        CertificateRejection, NormalizedGraph, PolicyEvaluation, check_outcome, evaluate_policy,
    };
    use crate::deployment_plan::*;

    fn identity(byte: u8) -> Identity {
        [byte; 32]
    }

    fn instance(name: &str, imports: &[u32], exports: &[u32]) -> PlanInstance {
        let mut endpoints = Vec::new();
        for &slot in imports {
            endpoints.push(Endpoint {
                slot,
                direction: EndpointDirection::Import,
                contract: identity(0xC0),
            });
        }
        for &slot in exports {
            endpoints.push(Endpoint {
                slot,
                direction: EndpointDirection::Export,
                contract: identity(0xC1),
            });
        }
        PlanInstance {
            name: InstanceName::new(name).unwrap(),
            component: ComponentDescription {
                subject: identity(0x10),
                verification_profile: identity(0x20),
                completeness: Completeness::VerifiedComplete {
                    closure: identity(0x30),
                },
                assumptions: Vec::new(),
            },
            role: InstanceRole::Component,
            endpoints,
        }
    }

    fn payment_graph() -> NormalizedGraph {
        // api -> authorization -> billing. Authorization deliberately reuses
        // slot 1 for both its import and its export, pinning (slot,direction)
        // keying.
        let instances = vec![
            instance("api", &[1], &[]),
            instance("authorization", &[1], &[1]),
            instance("billing", &[], &[1]),
        ];
        let bindings = vec![
            Binding {
                import: EndpointKey {
                    instance: 0,
                    slot: 1,
                },
                export: EndpointKey {
                    instance: 1,
                    slot: 1,
                },
                transport: identity(0x77),
            },
            Binding {
                import: EndpointKey {
                    instance: 1,
                    slot: 1,
                },
                export: EndpointKey {
                    instance: 2,
                    slot: 1,
                },
                transport: identity(0x77),
            },
        ];
        NormalizedGraph::new(instances, bindings).unwrap()
    }

    fn selector(names: &[&str]) -> PolicySelector {
        PolicySelector::new(names.iter().map(|name| InstanceName::new(*name).unwrap()))
    }

    #[test]
    fn payment_only_via_is_satisfied_with_checkable_certificate() {
        let graph = payment_graph();
        let call = PolicyCall::only_via(
            selector(&["api"]),
            selector(&["billing"]),
            selector(&["authorization"]),
        );
        let evaluation = evaluate_policy(&graph, &call);
        let PolicyEvaluation::Decided(outcome) = &evaluation else {
            panic!("policy must decide: {evaluation:?}");
        };
        assert!(matches!(outcome, PolicyOutcome::Satisfied { .. }));
        check_outcome(&graph, &call, outcome).expect("certificate checks");
    }

    #[test]
    fn direct_bypass_violates_with_shortest_witness() {
        let mut graph = payment_graph();
        let mut bindings = graph.bindings().to_vec();
        bindings.push(Binding {
            import: EndpointKey {
                instance: 0,
                slot: 2,
            },
            export: EndpointKey {
                instance: 2,
                slot: 1,
            },
            transport: identity(0x77),
        });
        // api needs a second import slot declared; rebuild with it.
        let mut instances = graph.instances().to_vec();
        instances[0].endpoints.push(Endpoint {
            slot: 2,
            direction: EndpointDirection::Import,
            contract: identity(0xC0),
        });
        graph = NormalizedGraph::new(instances, bindings).unwrap();
        let call = PolicyCall::only_via(
            selector(&["api"]),
            selector(&["billing"]),
            selector(&["authorization"]),
        );
        let PolicyEvaluation::Decided(PolicyOutcome::Violated {
            violation: Violation::Bypass { path },
        }) = evaluate_policy(&graph, &call)
        else {
            panic!("bypass must violate");
        };
        let names: Vec<&str> = path
            .iter()
            .map(|index| graph.instances()[*index as usize].name.as_str())
            .collect();
        assert_eq!(names, ["api", "billing"]);
        check_outcome(
            &graph,
            &call,
            &PolicyOutcome::Violated {
                violation: Violation::Bypass { path },
            },
        )
        .expect("violation witness checks");
    }

    #[test]
    fn cycles_and_disconnection_behave() {
        // billing -> api completes a cycle; only_via still terminates.
        let mut instances = payment_graph().instances().to_vec();
        instances[2].endpoints.push(Endpoint {
            slot: 7,
            direction: EndpointDirection::Import,
            contract: identity(0xC0),
        });
        instances[0].endpoints.push(Endpoint {
            slot: 9,
            direction: EndpointDirection::Export,
            contract: identity(0xC1),
        });
        let mut bindings = payment_graph().bindings().to_vec();
        bindings.push(Binding {
            import: EndpointKey {
                instance: 2,
                slot: 7,
            },
            export: EndpointKey {
                instance: 0,
                slot: 9,
            },
            transport: identity(0x77),
        });
        let graph = NormalizedGraph::new(instances, bindings).unwrap();
        let call = PolicyCall::only_via(
            selector(&["api"]),
            selector(&["billing"]),
            selector(&["authorization"]),
        );
        let PolicyEvaluation::Decided(outcome) = evaluate_policy(&graph, &call) else {
            panic!("decides")
        };
        assert!(matches!(outcome, PolicyOutcome::Satisfied { .. }));

        // A disconnected target violates only_via but satisfies no_route —
        // deliberate disconnection is a no_route property, not a vacuous
        // only_via pass.
        let graph = payment_graph();
        let call = PolicyCall::only_via(
            selector(&["billing"]),
            selector(&["api"]),
            selector(&["authorization"]),
        );
        let PolicyEvaluation::Decided(PolicyOutcome::Violated {
            violation: Violation::Disconnected,
        }) = evaluate_policy(&graph, &call)
        else {
            panic!("disconnected must violate only_via")
        };
        let call = PolicyCall::no_route(selector(&["billing"]), selector(&["api"]));
        let PolicyEvaluation::Decided(outcome) = evaluate_policy(&graph, &call) else {
            panic!("decides")
        };
        assert!(matches!(outcome, PolicyOutcome::Satisfied { .. }));
    }

    fn external(name: &str, exports: &[u32]) -> PlanInstance {
        let mut instance = instance(name, &[], exports);
        instance.role = InstanceRole::ExternalParticipant;
        instance
    }

    /// A multiplexed adapter (`mux`) mediating clients' access to a broadly
    /// connected external participant (`net`). `wrapped` routes both client
    /// imports through `mux`; `unwrapped` additionally binds `batch` straight
    /// to `net` — the adapter's broad authority escapes its mediation
    /// boundary.
    fn mediated_graph(unwrapped: bool) -> NormalizedGraph {
        let mut batch = instance("batch", &[1], &[]);
        if unwrapped {
            batch.endpoints.push(Endpoint {
                slot: 2,
                direction: EndpointDirection::Import,
                contract: identity(0xC0),
            });
        }
        let instances = vec![
            instance("api", &[1], &[]),
            batch,
            instance("mux", &[1], &[1]),
            external("net", &[1]),
        ];
        let mut bindings = vec![
            Binding {
                import: EndpointKey {
                    instance: 0,
                    slot: 1,
                },
                export: EndpointKey {
                    instance: 2,
                    slot: 1,
                },
                transport: identity(0x77),
            },
            Binding {
                import: EndpointKey {
                    instance: 1,
                    slot: 1,
                },
                export: EndpointKey {
                    instance: 2,
                    slot: 1,
                },
                transport: identity(0x77),
            },
            Binding {
                import: EndpointKey {
                    instance: 2,
                    slot: 1,
                },
                export: EndpointKey {
                    instance: 3,
                    slot: 1,
                },
                transport: identity(0x78),
            },
        ];
        if unwrapped {
            bindings.push(Binding {
                import: EndpointKey {
                    instance: 1,
                    slot: 2,
                },
                export: EndpointKey {
                    instance: 3,
                    slot: 1,
                },
                transport: identity(0x78),
            });
        }
        NormalizedGraph::new(instances, bindings).unwrap()
    }

    #[test]
    fn broad_transport_is_admissible_only_inside_the_wrapper() {
        // Wrapped: every client path to the broad participant is pinned
        // through the mediation adapter — the spec's wrapped broad-transport
        // control is the `only_via` policy over a checked boundary instance.
        let graph = mediated_graph(false);
        let call = PolicyCall::only_via(
            selector(&["api", "batch"]),
            selector(&["net"]),
            selector(&["mux"]),
        );
        let PolicyEvaluation::Decided(outcome @ PolicyOutcome::Satisfied { .. }) =
            evaluate_policy(&graph, &call)
        else {
            panic!("wrapped transport must satisfy only_via")
        };
        check_outcome(&graph, &call, &outcome).expect("certificate checks");

        // Unwrapped: `batch` binds the broad transport directly, bypassing
        // the adapter — the predicate reports the concrete wrap-breaking
        // path, and the wrapped certificate cannot be replayed against this
        // graph.
        let unwrapped = mediated_graph(true);
        let PolicyEvaluation::Decided(PolicyOutcome::Violated {
            violation: Violation::Bypass { path },
        }) = evaluate_policy(&unwrapped, &call)
        else {
            panic!("unwrapped broad binding must violate")
        };
        let names: Vec<&str> = path
            .iter()
            .map(|index| unwrapped.instances()[*index as usize].name.as_str())
            .collect();
        assert_eq!(names, ["batch", "net"]);
        check_outcome(
            &unwrapped,
            &call,
            &PolicyOutcome::Violated {
                violation: Violation::Bypass { path },
            },
        )
        .expect("violation witness checks");

        let wrapped_reachable = match &outcome {
            PolicyOutcome::Satisfied {
                certificate: Certificate::OnlyVia { reachable, .. },
            } => reachable.clone(),
            _ => unreachable!(),
        };
        let forged = PolicyOutcome::Satisfied {
            certificate: Certificate::OnlyVia {
                path: vec![
                    unwrapped
                        .index_of(&InstanceName::new("batch").unwrap())
                        .unwrap(),
                    unwrapped
                        .index_of(&InstanceName::new("mux").unwrap())
                        .unwrap(),
                    unwrapped
                        .index_of(&InstanceName::new("net").unwrap())
                        .unwrap(),
                ],
                reachable: wrapped_reachable,
            },
        };
        let batch_index = unwrapped
            .index_of(&InstanceName::new("batch").unwrap())
            .unwrap();
        let net_index = unwrapped
            .index_of(&InstanceName::new("net").unwrap())
            .unwrap();
        assert_eq!(
            check_outcome(&unwrapped, &call, &forged),
            Err(CertificateRejection::NotClosed {
                from: batch_index,
                to: net_index,
            })
        );
    }
}
