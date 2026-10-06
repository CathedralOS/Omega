//! One machine-local claim namespace for Unit and scalar-producing operations.
//! Held identities resolve before fresh establishment is considered. Source
//! moves cannot mint replacement identities; published graph namespaces are
//! immutable while straight-line bodies grow at actual value completion.
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, claim_id, lookup_claim_id};
use language_semantics::PermissionClaimIdentity;
use semantic_vocabulary::ClaimId;

/// The caller's claim namespace.
pub(crate) enum ClaimBindings<'f> {
    /// The body's entry claims and every identity its completed producers
    /// establish. Scalar and Unit operations share this dense namespace.
    Growable {
        bindings: Vec<(PermissionClaimIdentity, ClaimId)>,
        next_claim: u64,
    },
    /// A composed state's machine-wide table. The graph publishes its claim
    /// roster before any state emits, so a state cannot mint a binding.
    Fixed(&'f [(PermissionClaimIdentity, ClaimId)]),
}

impl ClaimBindings<'_> {
    pub(crate) fn new(bindings: Vec<(PermissionClaimIdentity, ClaimId)>, next_claim: u64) -> Self {
        Self::Growable {
            bindings,
            next_claim,
        }
    }

    pub(crate) fn bindings(&self) -> &[(PermissionClaimIdentity, ClaimId)] {
        match self {
            Self::Growable { bindings, .. } => bindings,
            Self::Fixed(bindings) => bindings,
        }
    }

    /// Mint only an authenticated new establishment in this body.
    fn mint(&mut self, identity: PermissionClaimIdentity) -> Result<ClaimId, LoweringError> {
        let Self::Growable {
            bindings,
            next_claim,
        } = self
        else {
            return unsupported("published graph cannot mint another claim binding");
        };
        let claim = claim_id(allocate_dense(next_claim)?);
        bindings.push((identity, claim));
        Ok(claim)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedClaim {
    pub(crate) identity: PermissionClaimIdentity,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    fresh: bool,
}

pub(crate) struct PreparedClaimDisposition {
    pub(crate) identity: PermissionClaimIdentity,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
}

/// Translate the selected occurrence once; hooks later select their exact
/// owned path without rescanning permissions or consuming a containing owner.
pub(crate) fn prepare_claim_dispositions(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    source: language_semantics::PermissionEventSource,
    root: facts::PlaceRoot,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<Vec<PreparedClaimDisposition>, LoweringError> {
    checked.facts.flow.ownership.permissions.iter().map(|(_, event)| event)
        .filter(|event| event.machine_symbol == machine && event.state_symbol == state && event.root == root
            && event.source == source && event.kind == language_semantics::PermissionEventKind::Consume
            && event.access == language_semantics::PermissionAccess::Owned)
        .map(|event| {
            let path = validation::structural_claim_path(&checked.typed, type_reference,
                checked.facts.flow.ownership.segments.span_or_empty(event.segments))
                .map_err(LoweringError::Unsupported)?;
            Ok(PreparedClaimDisposition { identity: event.claim_identity,
                path: crate::expression_preparation::bindings::structural_paths::lower_structural_path(&path)?,
            })
        }).collect()
}

/// Source checking has already joined moves and returns to their exact owner.
/// Retain that identity independently of storage or the producing operation's
/// spelling; only this body's authentic fresh establishment may grow custody.
pub(crate) fn prepare_establishment_claims(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    root: facts::PlaceRoot,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<Vec<PreparedClaim>, LoweringError> {
    checked.facts.flow.ownership.permissions.iter().map(|(_, event)| event)
        .filter(|event| event.machine_symbol == machine && event.state_symbol == state
            && event.root == root && event.kind == language_semantics::PermissionEventKind::Establish
            && event.access == language_semantics::PermissionAccess::Owned
            && event.multiplicity == language_semantics::Multiplicity::Linear && event.obligation_live
            && matches!(event.source, language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call { statement_index, .. } if statement_index == statement))
        .map(|event| {
            let transferred = checked.facts.flow.ownership.permissions.iter().any(|(_, source)| {
                source.machine_symbol == machine && source.state_symbol == state && source.evaluation == event.evaluation
                    && source.kind == language_semantics::PermissionEventKind::Transfer
                    && source.access == language_semantics::PermissionAccess::Owned
                    && source.claim_identity == event.claim_identity && source.obligation_live
                    && matches!(source.source, language_semantics::PermissionEventSource::Statement { statement_index }
                        | language_semantics::PermissionEventSource::Call { statement_index, .. } if statement_index == statement)
            });
            let path = validation::structural_claim_path(&checked.typed, type_reference,
                checked.facts.flow.ownership.segments.span_or_empty(event.segments))
                .map_err(LoweringError::Unsupported)?;
            Ok(PreparedClaim {
                identity: event.claim_identity,
                path: crate::expression_preparation::bindings::structural_paths::lower_structural_path(&path)?,
                fresh: !transferred && claim_established_here(machine, state, statement, event),
            })
        }).collect()
}

pub(crate) fn claims_below_path(
    claims: &[PreparedClaim],
    path: &[terminal_psi::StructuralPathSegment],
) -> Vec<PreparedClaim> {
    claims
        .iter()
        .filter_map(|claim| {
            claim.path.strip_prefix(path).map(|suffix| PreparedClaim {
                identity: claim.identity,
                fresh: claim.fresh,
                path: suffix.to_vec(),
            })
        })
        .collect()
}

pub(crate) fn emit_establishment_claims(
    claims: &[PreparedClaim],
    bindings: &mut ClaimBindings<'_>,
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    claims
        .iter()
        .map(|claim| {
            let resolved = match lookup_claim_id(bindings.bindings(), claim.identity) {
                Ok(resolved) => resolved,
                Err(_) if claim.fresh => bindings.mint(claim.identity)?,
                Err(error) => return Err(error),
            };
            Ok(terminal_psi::StructuralResultClaimBinding {
                claim: resolved,
                path: claim.path.clone(),
            })
        })
        .collect()
}

/// Rebase one completed-establishment claim occurrence onto this body's claim
/// namespace. An identity the caller already holds — an entry claim, or a
/// binding an earlier establishment minted — resolves through the grown
/// table. A claim the checker established at this body's own statement mints
/// its caller binding here instead: the identity's establishment coordinate
/// is exactly that statement, so the minted `ClaimId` is that claim's only
/// caller binding, and every later receipt, transfer and custody join
/// resolves it the same way. Any other identity is custody this caller never
/// held — the body cannot mint a binding for a claim some other point owns.
pub(crate) fn resolve_established_claim(
    claims: &mut ClaimBindings<'_>,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    event: &checked_trees::FlowPermissionEventFact,
) -> Result<ClaimId, LoweringError> {
    if let Ok(bound) = lookup_claim_id(claims.bindings(), event.claim_identity) {
        return Ok(bound);
    }
    if !claim_established_here(machine, state, statement_index, event) {
        return lookup_claim_id(claims.bindings(), event.claim_identity);
    }
    claims.mint(event.claim_identity)
}

/// Authenticate a fresh claim at its actual establishment coordinate. Graph
/// publication and ordinary emission share this minting boundary; transported
/// claims instead resolve the identity their original establishment published.
pub(crate) fn claim_established_here(
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    event: &checked_trees::FlowPermissionEventFact,
) -> bool {
    event.machine_symbol == machine
        && event.state_symbol == state
        && matches!(event.source, language_semantics::PermissionEventSource::Statement { statement_index: index }
            | language_semantics::PermissionEventSource::Call { statement_index: index, .. } if index == statement_index)
        && event.kind == language_semantics::PermissionEventKind::Establish
        && event.access == language_semantics::PermissionAccess::Owned
        && event.multiplicity == language_semantics::Multiplicity::Linear
        && event.obligation_live
        && matches!(event.claim_identity, PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
            if machine_symbol == machine && state_symbol == state && source == event.source)
        && event.provenance
            == language_semantics::PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: event.source,
            }
}
