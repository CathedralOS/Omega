//! One machine-local claim namespace for Unit and scalar-producing operations.
//! Held identities resolve before fresh establishment is considered. Source
//! moves cannot mint replacement identities; published graph namespaces are
//! immutable while straight-line emission reserves checked completion names.
//! Namespace reservation is not live custody: the producing operation alone
//! establishes or receives the resource when execution completes.
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{allocate_dense, claim_id, lookup_claim_id};
use language_semantics::PermissionClaimIdentity;
use semantic_vocabulary::ClaimId;

/// The caller's claim namespace.
pub(crate) enum ClaimBindings<'f> {
    /// The body's entry claims and its checked producer completion identities.
    /// Scalar and Unit operations share this dense namespace.
    Growable {
        bindings: Vec<(PermissionClaimIdentity, ClaimId)>,
        next_claim: u64,
    },
    /// A composed state's machine-wide table. The graph publishes its claim
    /// roster before any state emits, so a state cannot mint a binding.
    Fixed(PublishedClaims<'f>),
}

/// A composed graph's claim roster, published before any state emits.
#[derive(Clone, Copy)]
pub(crate) struct PublishedClaims<'f> {
    pub(crate) bindings: &'f [(PermissionClaimIdentity, ClaimId)],
    /// Claims of call results no source place receives (a terminal call
    /// returned at once). They have no checked identity, so the call's exact
    /// coordinate and result path name them.
    pub(crate) unreceived_results: &'f [UnreceivedResultClaim],
}

/// One claim a call result carries at `path` when no source place receives
/// the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnreceivedResultClaim {
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) coordinate: checked_trees::CheckedUnitCallCoordinate,
    pub(crate) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(crate) claim: ClaimId,
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
            Self::Fixed(published) => published.bindings,
        }
    }

    /// Bind a source-checked normal-loan completion in this caller's namespace.
    pub(crate) fn receive_loan_completion_claim(
        &mut self,
        identity: PermissionClaimIdentity,
    ) -> Result<ClaimId, LoweringError> {
        if identity == PermissionClaimIdentity::Unknown {
            return unsupported("loan completion has no checked claim identity");
        }
        match lookup_claim_id(self.bindings(), identity) {
            Ok(claim) => Ok(claim),
            Err(_) => self.mint(identity),
        }
    }

    /// Bind one claim a structural call result carries. An identity this
    /// body already holds continues (an input returned on every exit keeps
    /// its transferred identity); any other identity is established in this
    /// body by the call's completion. A result no source place receives has
    /// no identity: its claim is named only by the result and the edge that
    /// returns it, so it enters no lookup table.
    pub(crate) fn receive_call_result_claim(
        &mut self,
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        received: &checked_trees::CheckedStructuralResultClaimPlan,
    ) -> Result<ClaimId, LoweringError> {
        if received.claim == PermissionClaimIdentity::Unknown {
            return match self {
                Self::Growable { next_claim, .. } => Ok(claim_id(allocate_dense(next_claim)?)),
                Self::Fixed(published) => {
                    let mut matching = published.unreceived_results.iter().filter(|claim| {
                        claim.state == state
                            && claim.coordinate == coordinate
                            && claim.path == received.path
                    });
                    let claim = matching.next().ok_or(LoweringError::Unsupported(
                        "published graph omits an unreceived call result claim",
                    ))?;
                    if matching.next().is_some() {
                        return unsupported(
                            "published graph repeats an unreceived call result claim",
                        );
                    }
                    Ok(claim.claim)
                }
            };
        }
        match lookup_claim_id(self.bindings(), received.claim) {
            Ok(bound) => Ok(bound),
            Err(_) => self.mint(received.claim),
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

/// Resolve every checked transfer through the caller's exact claim table.
pub(crate) fn emitted_claim_transfers(
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
    let prepared = prepare_claim_transfers(transfers, types, |index| {
        parameters
            .get(index as usize)
            .map(|parameter| parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "claim transfer has no structural argument type",
            ))
    })?;
    emit_prepared_claim_transfers(&prepared, claim_bindings)
}

/// Preparation resolves named Case guards against the exact argument type,
/// while emission resolves permission identities in the body's live namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedClaimTransfer {
    pub(crate) identity: PermissionClaimIdentity,
    pub(crate) argument_index: u32,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
}

/// A boundary's normal-completion disposition names caller custody, not a
/// provider-private ingress identity. Its argument supplies the selected path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedCompletionReceipt {
    identity: PermissionClaimIdentity,
    argument_index: u32,
}

pub(crate) fn prepare_completion_receipts(
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
) -> Vec<PreparedCompletionReceipt> {
    transfers
        .iter()
        .map(|transfer| PreparedCompletionReceipt {
            identity: transfer.claim_identity,
            argument_index: transfer.argument_index,
        })
        .collect()
}

/// Statement and computed boundary calls resolve the same captured caller
/// occurrences. Lookup cannot manufacture a missing disposal obligation.
pub(crate) fn emit_completion_receipts(
    prepared: &[PreparedCompletionReceipt],
    bindings: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<terminal_psi::CompletionReceipt>, LoweringError> {
    prepared
        .iter()
        .map(|receipt| {
            Ok(terminal_psi::CompletionReceipt {
                claim: lookup_claim_id(bindings, receipt.identity)?,
                argument_index: receipt.argument_index,
            })
        })
        .collect()
}

/// A result binding retains its checked path for the machine namespace and
/// its lowered path for the operation. No claim becomes live during preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedResultClaim {
    received: checked_trees::CheckedStructuralResultClaimPlan,
    path: Vec<terminal_psi::StructuralPathSegment>,
}

pub(crate) fn prepare_call_result_claims(
    received: &[checked_trees::CheckedStructuralResultClaimPlan],
    structural_type: semantic_vocabulary::StructuralTypeId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedResultClaim>, LoweringError> {
    received.iter().map(|received| {
        Ok(PreparedResultClaim {
            received: received.clone(),
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                &received.path, structural_type, types,
            )?,
        })
    }).collect()
}

/// Ordinary and expression calls receive custody at the same successful
/// completion, never by assigning an enclosing constructor's type to a result.
pub(crate) fn emit_call_result_claims(
    prepared: &[PreparedResultClaim],
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    bindings: &mut ClaimBindings<'_>,
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    let mut received = prepared
        .iter()
        .map(|claim| {
            Ok(terminal_psi::StructuralResultClaimBinding {
                claim: bindings.receive_call_result_claim(state, coordinate, &claim.received)?,
                path: claim.path.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    received.sort();
    Ok(received)
}

pub(crate) fn prepare_checked_claim_transfers(
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedClaimTransfer>, LoweringError> {
    prepare_claim_transfers(transfers, types, |index| {
        let parameter = parameters
            .get(index as usize)
            .ok_or(LoweringError::Unsupported(
                "claim transfer has no checked structural parameter",
            ))?;
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.identity == parameter.type_identity);
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "claim transfer has no lowered parameter type",
        ))?;
        if declarations.next().is_some() {
            return unsupported("claim transfer has ambiguous parameter type");
        }
        Ok(declaration.id)
    })
}

fn prepare_claim_transfers(
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    types: &[terminal_psi::StructuralTypeDeclaration],
    argument_type: impl Fn(u32) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>,
) -> Result<Vec<PreparedClaimTransfer>, LoweringError> {
    transfers.iter().map(|transfer| {
        let root = argument_type(transfer.argument_index)?;
        Ok(PreparedClaimTransfer {
            identity: transfer.claim_identity,
            argument_index: transfer.argument_index,
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                &transfer.path, root, types,
            )?,
        })
    }).collect()
}

pub(crate) fn emit_prepared_claim_transfers(
    transfers: &[PreparedClaimTransfer],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
    let mut emitted = transfers
        .iter()
        .map(|transfer| {
            Ok(terminal_psi::ClaimTransfer {
                claim: lookup_claim_id(claim_bindings, transfer.identity)?,
                argument_index: transfer.argument_index,
                path: transfer.path.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    emitted.sort();
    Ok(emitted)
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
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
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
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(&path, structural_type, structural_types)?,
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
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedClaim>, LoweringError> {
    checked.facts.flow.ownership.permissions.iter().map(|(_, event)| event)
        .filter(|event| event.machine_symbol == machine && event.state_symbol == state
            && event.root == root && event.kind == language_semantics::PermissionEventKind::Establish
            && event.access == language_semantics::PermissionAccess::Owned
            && event.multiplicity == language_semantics::Multiplicity::Linear && event.obligation_live
            && matches!(event.source, language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call { statement_index, .. } if statement_index == statement))
        .map(|event| {
            // Reserving a name is not establishing live custody. A completed
            // child can transfer through another call before this aggregate
            // receives it, all within one evaluation. Its authenticated first
            // completion still permits namespace reservation before operand
            // emission; an intervening transfer does not erase that origin.
            // A claim from an earlier statement or invocation instead resolves
            // its existing binding and cannot acquire a replacement identity.
            let established_here = checked.facts.flow.ownership.permissions.iter().any(|(_, source)| {
                source.evaluation == event.evaluation
                    && source.claim_identity == event.claim_identity
                    && claim_established_here(machine, state, statement, source)
            });
            let path = validation::structural_claim_path(&checked.typed, type_reference,
                checked.facts.flow.ownership.segments.span_or_empty(event.segments))
                .map_err(LoweringError::Unsupported)?;
            Ok(PreparedClaim {
                identity: event.claim_identity,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(&path, structural_type, structural_types)?,
                fresh: established_here,
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
    let mut emitted = claims
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
        .collect::<Result<Vec<_>, LoweringError>>()?;
    emitted.sort();
    Ok(emitted)
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

/// Reserve the namespace identity authenticated at this completion. A fresh
/// construction has establishment provenance; a returned child has a checked
/// call-join provenance with the same occurrence ordinal. Both reserve names,
/// not resources: only the actual producing operation establishes live custody.
/// A constructor may reserve its children's names before emitting their calls,
/// and those calls resolve the same bindings when they complete. Transported
/// claims instead resolve their original establishment's existing identity.
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
        && matches!(
            event.kind,
            language_semantics::PermissionEventKind::Establish
                | language_semantics::PermissionEventKind::Restore
        )
        && event.access == language_semantics::PermissionAccess::Owned
        && event.multiplicity == language_semantics::Multiplicity::Linear
        && event.obligation_live
        && matches!(event.claim_identity, PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
            if machine_symbol == machine && state_symbol == state && source == event.source)
        && (event.provenance
            == language_semantics::PermissionProvenance::Established {
                machine_symbol: machine,
                state_symbol: state,
                source: event.source,
            }
            || matches!((event.provenance, event.claim_identity),
                (language_semantics::PermissionProvenance::Joined {
                    machine_symbol, state_symbol,
                    source: language_semantics::PermissionEventSource::Call { statement_index: received_at, .. },
                    ordinal,
                }, PermissionClaimIdentity::Established { ordinal: identity_ordinal, .. })
                    if machine_symbol == machine && state_symbol == state
                        && received_at == statement_index && ordinal == identity_ordinal))
}
