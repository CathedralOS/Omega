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

pub(crate) mod selections;

/// One exact producing operation defines this machine-local claim name.
/// Namespace reservation is not an introduction; forwarded claims retain the
/// incoming name when continuations specialize their producer occurrences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaimIntroduction {
    pub(crate) operation: semantic_vocabulary::OperationId,
    pub(crate) claim: ClaimId,
}

/// A lowering-time name on one control edge. Publication substitutes the
/// actual incoming claim throughout that continuation; no runtime resource
/// is introduced, moved, or renamed at its unchanged storage home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaimEdgeBinding {
    pub(crate) edge: semantic_vocabulary::EdgeId,
    pub(crate) destination: ClaimId,
    pub(crate) source: ClaimId,
}

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
        self.reserve_checked_binding(identity)
    }

    pub(crate) fn reserve_checked_binding(
        &mut self,
        identity: PermissionClaimIdentity,
    ) -> Result<ClaimId, LoweringError> {
        if identity == PermissionClaimIdentity::Unknown {
            return unsupported("receiving binding has no checked claim identity");
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

    /// Allocate a name for an authenticated establishment or checked join.
    /// Reserving a join name does not establish another resource obligation.
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
    let prepared = prepare_claim_transfers(identities, transfers, types, |_identities, index| {
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

/// Statement and computed boundary calls resolve the same captured caller
/// occurrences and prepared current paths as ordinary calls. A boundary's
/// completion disposition differs from transfer, not from custody geometry.
/// Lookup cannot manufacture a missing disposal obligation.
pub(crate) fn emit_completion_receipts(
    prepared: &[PreparedClaimTransfer],
    bindings: &[(PermissionClaimIdentity, ClaimId)],
) -> Result<Vec<terminal_psi::CompletionReceipt>, LoweringError> {
    let mut receipts = prepared
        .iter()
        .map(|receipt| {
            Ok(terminal_psi::CompletionReceipt {
                claim: lookup_claim_id(bindings, receipt.identity)?,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // Current field order can differ from the retained claim namespace after
    // reseating. Canonical receipt order follows claim identities, not the
    // argument's storage geometry; retain that geometry in each record.
    receipts.sort();
    Ok(receipts)
}

/// A result binding retains its checked path for the machine namespace and
/// its lowered path for the operation. No claim becomes live during preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedResultClaim {
    received: checked_trees::CheckedStructuralResultClaimPlan,
    path: Vec<terminal_psi::StructuralPathSegment>,
}

pub(crate) fn prepare_call_result_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    received: &[checked_trees::CheckedStructuralResultClaimPlan],
    structural_type: semantic_vocabulary::StructuralTypeId,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedResultClaim>, LoweringError> {
    received.iter().map(|received| {
        Ok(PreparedResultClaim {
            received: received.clone(),
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &received.path, structural_type, types)?,
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedClaimTransfer>, LoweringError> {
    prepare_claim_transfers(identities, transfers, types, |identities, index| {
        let parameter = parameters
            .get(index as usize)
            .ok_or(LoweringError::Unsupported(
                "claim transfer has no checked structural parameter",
            ))?;
        let identity = checked
            .type_identity_text(parameter.type_identity)
            .unwrap_or_default();
        let mut declarations = types
            .iter()
            .filter(|declaration| declaration.identity == identities.intern(&identity));
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "claim transfer has no lowered parameter type",
        ))?;
        if declarations.next().is_some() {
            return unsupported("claim transfer has ambiguous parameter type");
        }
        Ok(declaration.id)
    })
}

pub(crate) fn prepare_claim_transfers(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    types: &[terminal_psi::StructuralTypeDeclaration],
    mut argument_type: impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        u32,
    ) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError>,
) -> Result<Vec<PreparedClaimTransfer>, LoweringError> {
    transfers.iter().map(|transfer| {
        let root = argument_type(identities, transfer.argument_index)?;
        Ok(PreparedClaimTransfer {
            identity: transfer.claim_identity,
            argument_index: transfer.argument_index,
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &transfer.path, root, types)?,
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
    /// Direct establishment by this statement, rather than reservation for a
    /// completed child call whose claim this receiving owner only transports.
    direct_establishment: bool,
}

/// A completed operand can be authenticated at the place receiving it. The
/// checked flow graph retains both that exact destination and each constructed
/// child's relative path; no authored assignment or neighboring store is read.
fn receiving_claim_place(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    root: facts::PlaceRoot,
) -> Result<(facts::PlaceRoot, Vec<facts::PlaceSegment>), LoweringError> {
    let facts::PlaceRoot::Expression(expression) = root else {
        return Ok((root, Vec::new()));
    };
    let flow = &checked.facts.flow.control;
    let semantic = &checked.facts.semantic;
    let mut receiving = None;
    for (_, scope) in flow
        .states
        .iter()
        .filter(|(_, scope)| scope.machine_symbol == machine && scope.state_symbol == state)
    {
        for row in flow
            .statements
            .span_or_empty(scope.statements)
            .iter()
            .filter(|row| {
                row.statement_index == statement
                    && flow.operands.is_valid(row.stored_value)
                    && semantic.places.is_valid(row.stored_destination)
            })
        {
            let destination = semantic.places.get(row.stored_destination);
            let mut pending = vec![(
                row.stored_value,
                semantic
                    .place_segments
                    .span_or_empty(destination.segments)
                    .to_vec(),
            )];
            while let Some((handle, path)) = pending.pop() {
                let operand = flow.operands.get(handle);
                if operand.state_symbol != state || operand.statement_index != statement {
                    return unsupported(
                        "stored claim operand has a different evaluation coordinate",
                    );
                }
                if operand.expression == expression {
                    let candidate = (destination.root, path);
                    if receiving
                        .as_ref()
                        .is_some_and(|previous| previous != &candidate)
                    {
                        return unsupported(
                            "completed claim operand has ambiguous receiving storage",
                        );
                    }
                    receiving = Some(candidate);
                    continue;
                }
                for child in flow.operand_children.span_or_empty(operand.children) {
                    let mut child_path = path.clone();
                    child_path
                        .extend_from_slice(flow.operand_child_paths.span_or_empty(child.path));
                    pending.push((child.value, child_path));
                }
            }
        }
    }
    Ok(receiving.unwrap_or((root, Vec::new())))
}

/// Source checking has already joined moves and returns to their exact owner.
/// Retain that identity independently of storage or the producing operation's
/// spelling; only this body's authentic fresh establishment may grow custody.
pub(crate) fn prepare_establishment_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    root: facts::PlaceRoot,
    type_reference: typed_trees::types::TypeReferenceHandle,
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedClaim>, LoweringError> {
    // A computed RHS owns its result before the receiving store. Its selection
    // receipt binds that exact expression, independently of the borrowed or
    // projected place the following store will restore.
    let selection_result = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, statement as u32)
        .is_some_and(|(_, receipt)| {
            !receipt.destination.is_valid()
                && root == facts::PlaceRoot::Expression(receipt.expression)
        });
    let (root, receiving_path) = if selection_result {
        (root, Vec::new())
    } else {
        receiving_claim_place(checked, machine, state, statement, root)?
    };
    if let Some((_, receipt)) = checked
        .facts
        .flow
        .ownership
        .owned_selection_at(state, statement as u32)
        && receipt.machine == machine
        && root
            == if receipt.destination.is_valid() {
                facts::PlaceRoot::Symbol(receipt.destination)
            } else {
                facts::PlaceRoot::Expression(receipt.expression)
            }
    {
        return checked.facts.flow.ownership.selection_result_claims
            .span_or_empty(receipt.result_claims).iter().filter_map(|claim| {
                let path = checked.facts.flow.ownership.segments.span_or_empty(claim.path)
                    .strip_prefix(receiving_path.as_slice())?;
                Some((claim, path))
            }).map(|(claim, path)| {
                let path = validation::structural_claim_path(&checked.typed, type_reference, path)
                    .map_err(LoweringError::Unsupported)?;
                Ok(PreparedClaim {
                    identity: claim.claim_identity,
                    path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                        identities, &path, structural_type, structural_types,
                    )?,
                    // Only the exact checked selection owns these destination
                    // names. Block entry binds actual incoming obligations;
                    // no construction introduction may be emitted for them.
                    fresh: matches!((claim.claim_identity, claim.provenance),
                        (PermissionClaimIdentity::Established { machine_symbol, state_symbol,
                            source: language_semantics::PermissionEventSource::Statement { statement_index }, ordinal },
                        language_semantics::PermissionProvenance::Joined {
                            machine_symbol: source_machine, state_symbol: source_state,
                            source: language_semantics::PermissionEventSource::Statement { statement_index: source_statement },
                            ordinal: source_ordinal })
                        if machine_symbol == machine && state_symbol == state && statement_index == statement
                            && (machine_symbol, state_symbol, statement_index, ordinal)
                                == (source_machine, source_state, source_statement, source_ordinal)),
                    direct_establishment: false,
                })
            }).collect();
    }
    checked.facts.flow.ownership.permissions.iter().map(|(_, event)| event)
        .filter(|event| event.machine_symbol == machine && event.state_symbol == state
            && event.root == root
            && checked.facts.flow.ownership.segments.span_or_empty(event.segments).starts_with(&receiving_path)
            && event.kind == language_semantics::PermissionEventKind::Establish
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
                &checked.facts.flow.ownership.segments.span_or_empty(event.segments)[receiving_path.len()..])
                .map_err(
            LoweringError::Unsupported)?;
            Ok(PreparedClaim {
                identity: event.claim_identity,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &path, structural_type, structural_types)?,
                fresh: established_here,
                direct_establishment: matches!(event.source, language_semantics::PermissionEventSource::Statement { .. })
                    && event.provenance == language_semantics::PermissionProvenance::Established {
                        machine_symbol: machine, state_symbol: state, source: event.source,
                    }
                    && matches!(event.claim_identity, PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
                        if machine_symbol == machine && state_symbol == state && source == event.source),
            })
        }).collect()
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

/// The arm owns actual transferred claims and constructor births. Destination
/// join names are reserved separately and cannot authorize these introductions.
pub(crate) fn prepare_selection_arm_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    receipt: &checked_trees::FlowOwnedSelectionReceipt,
    arm: arena::Handle<typed_trees::expression::TableMatchArm>,
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<PreparedClaim>, LoweringError> {
    let ownership = &checked.facts.flow.ownership;
    let mut prepared = Vec::new();
    let mut append = |identity,
                      path: &[facts::PlaceSegment],
                      produced|
     -> Result<(), LoweringError> {
        let path = validation::structural_claim_path(&checked.typed, receipt.type_reference, path)
            .map_err(LoweringError::Unsupported)?;
        prepared.push(PreparedClaim {
            identity,
            path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities, &path, structural_type, structural_types,
            )?,
            fresh: produced,
            direct_establishment: produced,
        });
        Ok(())
    };
    for transfer in ownership
        .selection_transfers
        .span_or_empty(receipt.transfers)
        .iter()
        .filter(|transfer| transfer.source_arm == arm)
    {
        for claim in ownership
            .selection_transfer_claims
            .span_or_empty(transfer.claims)
        {
            let suffix = ownership
                .segments
                .span_or_empty(claim.path)
                .strip_prefix(ownership.segments.span_or_empty(transfer.path))
                .ok_or(LoweringError::Unsupported(
                    "selected claim is outside its moved source",
                ))?;
            let mut path = ownership
                .segments
                .span_or_empty(transfer.result_path)
                .to_vec();
            path.extend_from_slice(suffix);
            append(claim.claim_identity, &path, false)?;
        }
    }
    for produced in ownership
        .selection_produced_claims
        .span_or_empty(receipt.produced_claims)
        .iter()
        .filter(|produced| produced.source_arm == arm)
    {
        append(
            produced.claim_identity,
            ownership.segments.span_or_empty(produced.result_path),
            true,
        )?;
    }
    Ok(prepared)
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

/// Construction transports contained child claims. Only its directly checked
/// root establishment introduces a name. Borrowed extraction instead introduces
/// every directly established detached claim; owned extraction preserves origins.
#[derive(Clone, Copy)]
pub(crate) enum ClaimEstablishmentKind {
    Construction,
    DetachedValue,
}

/// Retain source-authenticated direct establishments while recursive literal
/// emission rebases their paths into each actual child producer.
pub(crate) fn direct_establishment_claims(
    prepared: &[PreparedClaim],
    bindings: &ClaimBindings<'_>,
) -> Result<Vec<ClaimId>, LoweringError> {
    prepared
        .iter()
        .filter(|claim| claim.direct_establishment)
        .map(|claim| lookup_claim_id(bindings.bindings(), claim.identity))
        .collect()
}

pub(crate) fn record_prepared_introductions(
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
    place: semantic_vocabulary::PlaceId,
    prepared: &[PreparedClaim],
    bindings: &ClaimBindings<'_>,
    kind: ClaimEstablishmentKind,
) -> Result<(), LoweringError> {
    let claims = prepared
        .iter()
        .filter(|claim| {
            claim.direct_establishment
                && (matches!(kind, ClaimEstablishmentKind::DetachedValue) || claim.path.is_empty())
        })
        .map(|claim| lookup_claim_id(bindings.bindings(), claim.identity))
        .collect::<Result<Vec<_>, _>>()?;
    if claims.is_empty() {
        return Ok(());
    }
    let mut producers = operations.operations.iter().filter(|operation| {
        operation
            .result
            .as_structural()
            .is_some_and(|result| result.place == place)
    });
    let producer = producers.next().ok_or(LoweringError::Unsupported(
        "claim introduction lost its exact producer",
    ))?;
    if producers.next().is_some() {
        return unsupported("claim introduction has ambiguous producer");
    }
    let operation = producer.id;
    if claims.iter().any(|claim| {
        !producer
            .result
            .as_structural()
            .expect("matched structural result")
            .claims
            .iter()
            .any(|binding| binding.claim == *claim)
    }) {
        return unsupported("claim introduction is absent from its completed result");
    }
    for claim in claims {
        operations
            .claim_introductions
            .push(ClaimIntroduction { operation, claim });
    }
    Ok(())
}
