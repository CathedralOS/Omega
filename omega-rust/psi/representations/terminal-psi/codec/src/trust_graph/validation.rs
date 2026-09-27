//! Canonical structural admission for terminal trust graphs.

use super::{
    TrustAcceptingPolicy, TrustDependencyKind, TrustDependencyNode, TrustDependencyStatus,
    TrustGraphError, ValidatedTerminalTrustGraph, graph_identity,
};
use std::collections::{BTreeMap, BTreeSet};

/// Validate a complete terminal-Psi trust graph. The input is already an
/// artifact surface, so the validator rejects alternate ordering rather than
/// silently normalizing it.
pub fn validate_terminal_trust_graph(
    entry: impl Into<String>,
    nodes: Vec<TrustDependencyNode>,
) -> Result<ValidatedTerminalTrustGraph, TrustGraphError> {
    let entry = entry.into();
    if entry.is_empty() {
        return Err(TrustGraphError::EmptyEntry);
    }
    if nodes
        .windows(2)
        .any(|pair| pair[0].identity >= pair[1].identity)
    {
        if let Some(identity) = duplicate_identity(&nodes) {
            return Err(TrustGraphError::DuplicateNode(identity));
        }
        return Err(TrustGraphError::NonCanonicalNodeOrder);
    }

    let by_identity = nodes
        .iter()
        .map(|node| (node.identity.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    if !by_identity.contains_key(entry.as_str()) {
        return Err(TrustGraphError::UnknownEntry(entry));
    }

    for node in &nodes {
        validate_nonempty(node)?;
        if node.dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(TrustGraphError::NonCanonicalDependencyOrder {
                node: node.identity.clone(),
            });
        }
        if node
            .dependencies
            .iter()
            .any(|dependency| dependency == &node.identity)
        {
            return Err(TrustGraphError::SelfDependency(node.identity.clone()));
        }
        match node.kind {
            TrustDependencyKind::RegisteredRoot => {
                if !node.dependencies.is_empty() {
                    return Err(TrustGraphError::RootHasDependencies(node.identity.clone()));
                }
                if node.status != TrustDependencyStatus::Registered {
                    return Err(TrustGraphError::RootHasInvalidStatus(node.identity.clone()));
                }
                if !matches!(
                    node.accepting_policy,
                    TrustAcceptingPolicy::RegisteredSemanticFoundation
                        | TrustAcceptingPolicy::ExplicitMigrationTrust
                ) {
                    return Err(TrustGraphError::RootHasInvalidPolicy(node.identity.clone()));
                }
            }
            _ => {
                if node.dependencies.is_empty() {
                    return Err(TrustGraphError::NonRootHasNoDependencies(
                        node.identity.clone(),
                    ));
                }
                if node.status == TrustDependencyStatus::Registered {
                    return Err(TrustGraphError::NonRootHasRegisteredStatus(
                        node.identity.clone(),
                    ));
                }
            }
        }
        for dependency in &node.dependencies {
            if !by_identity.contains_key(dependency.as_str()) {
                return Err(TrustGraphError::UnknownDependency {
                    node: node.identity.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    visit(entry.as_str(), &by_identity, &mut visiting, &mut visited)?;
    if let Some(node) = nodes
        .iter()
        .find(|node| !visited.contains(node.identity.as_str()))
    {
        return Err(TrustGraphError::UnreachableNode(node.identity.clone()));
    }

    let identity = graph_identity(&entry, &nodes);
    let fully_derived = nodes.iter().all(|node| {
        node.kind == TrustDependencyKind::RegisteredRoot
            || node.status == TrustDependencyStatus::FullyDerived
    });
    Ok(ValidatedTerminalTrustGraph {
        entry,
        nodes,
        identity,
        fully_derived,
    })
}

fn validate_nonempty(node: &TrustDependencyNode) -> Result<(), TrustGraphError> {
    for (field, value) in [
        ("identity", node.identity.as_str()),
        ("semantic_subject", node.semantic_subject.as_str()),
        ("version", node.version.as_str()),
        ("owner", node.owner.as_str()),
        ("scope", node.scope.as_str()),
        ("rationale", node.rationale.as_str()),
    ] {
        if value.is_empty() {
            return Err(TrustGraphError::EmptyField {
                node: node.identity.clone(),
                field,
            });
        }
    }
    Ok(())
}

fn duplicate_identity(nodes: &[TrustDependencyNode]) -> Option<String> {
    let mut seen = BTreeSet::new();
    nodes
        .iter()
        .find_map(|node| (!seen.insert(node.identity.as_str())).then(|| node.identity.clone()))
}

fn visit<'graph>(
    identity: &'graph str,
    nodes: &BTreeMap<&'graph str, &'graph TrustDependencyNode>,
    visiting: &mut BTreeSet<&'graph str>,
    visited: &mut BTreeSet<&'graph str>,
) -> Result<(), TrustGraphError> {
    if visited.contains(identity) {
        return Ok(());
    }
    if !visiting.insert(identity) {
        return Err(TrustGraphError::DependencyCycle(identity.to_owned()));
    }
    let node = nodes
        .get(identity)
        .expect("all dependency identities were validated before traversal");
    for dependency in &node.dependencies {
        visit(dependency, nodes, visiting, visited)?;
    }
    visiting.remove(identity);
    visited.insert(identity);
    Ok(())
}
