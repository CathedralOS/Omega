//! Domain-separated canonical identities for trust nodes and graphs.

use super::{
    TerminalTrustGraphIdentity, TrustAcceptingPolicy, TrustDependencyDigest, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus,
};
use sha2::{Digest, Sha256};

const NODE_DIGEST_DOMAIN: &[u8] = b"psi-terminal-trust-node\0";
const GRAPH_DIGEST_DOMAIN: &[u8] = b"psi-terminal-trust-graph\0";

#[allow(clippy::too_many_arguments)]
pub(super) fn dependency_digest(
    identity: &str,
    kind: TrustDependencyKind,
    status: TrustDependencyStatus,
    semantic_subject: &str,
    version: &str,
    owner: &str,
    scope: &str,
    rationale: &str,
    accepting_policy: TrustAcceptingPolicy,
    exact_sources: &[(&str, &[u8])],
) -> TrustDependencyDigest {
    let mut digest = Sha256::new();
    digest.update(NODE_DIGEST_DOMAIN);
    hash_string(&mut digest, identity);
    digest.update([kind as u8, status as u8, accepting_policy as u8]);
    for value in [semantic_subject, version, owner, scope, rationale] {
        hash_string(&mut digest, value);
    }
    hash_len(&mut digest, exact_sources.len());
    for (label, source) in exact_sources {
        hash_string(&mut digest, label);
        hash_bytes(&mut digest, source);
    }
    TrustDependencyDigest(digest.finalize().into())
}

pub(super) fn graph_identity(
    entry: &str,
    nodes: &[TrustDependencyNode],
) -> TerminalTrustGraphIdentity {
    let mut digest = Sha256::new();
    digest.update(GRAPH_DIGEST_DOMAIN);
    hash_string(&mut digest, entry);
    hash_len(&mut digest, nodes.len());
    for node in nodes {
        hash_string(&mut digest, &node.identity);
        digest.update([
            node.kind as u8,
            node.status as u8,
            node.accepting_policy as u8,
        ]);
        hash_string(&mut digest, &node.semantic_subject);
        digest.update(node.digest.as_bytes());
        hash_string(&mut digest, &node.version);
        hash_string(&mut digest, &node.owner);
        hash_string(&mut digest, &node.scope);
        hash_string(&mut digest, &node.rationale);
        hash_len(&mut digest, node.dependencies.len());
        for dependency in &node.dependencies {
            hash_string(&mut digest, dependency);
        }
    }
    TerminalTrustGraphIdentity(digest.finalize().into())
}

fn hash_string(digest: &mut Sha256, value: &str) {
    hash_bytes(digest, value.as_bytes());
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    hash_len(digest, bytes.len());
    digest.update(bytes);
}

fn hash_len(digest: &mut Sha256, len: usize) {
    let len = u64::try_from(len).expect("trust-graph data fits u64");
    digest.update(len.to_le_bytes());
}
