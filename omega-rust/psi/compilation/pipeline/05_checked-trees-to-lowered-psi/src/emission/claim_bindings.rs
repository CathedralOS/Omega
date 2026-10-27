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
        completed_values: Vec<CompletedValueClaim>,
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
    pub(crate) completed_values: &'f [CompletedValueClaim],
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

/// A destination-local name for one completed value join. This catalogue names
/// transport slots only; predecessor custody establishes which claims arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletedValueClaim {
    pub(crate) value: checked_trees::CheckedStructuralValueHandle,
    pub(crate) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(crate) claim: ClaimId,
}

/// The common checked frontier supplies supported slot geometry, not resources.
/// Existing arrival validation still requires every active slot's actual debt.
pub(crate) fn completed_value_paths(
    checked: &checked_trees::CheckedTrees,
    value: checked_trees::CheckedStructuralValueHandle,
) -> Result<Vec<Vec<checked_trees::CheckedUnitStructuralPathSegment>>, LoweringError> {
    let node = checked.facts.values.structural_values.nodes.get(value);
    validation::linear_claim_frontier(&checked.typed, node.type_reference)
        .iter()
        .map(|claim| {
            validation::structural_claim_path(&checked.typed, node.type_reference, &claim.path)
                .map_err(LoweringError::Unsupported)
        })
        .collect()
}

/// Seal one destination namespace after all private continuations have emitted.
/// A name cannot denote two block parameters or be declared twice.
pub(crate) fn seal_block_entry_claims(
    claims: &mut [terminal_psi::EntryClaim],
) -> Result<(), LoweringError> {
    claims.sort_by_key(|binding| binding.claim);
    if claims.windows(2).any(|pair| pair[0].claim == pair[1].claim) {
        return unsupported("block arrival namespace repeats a claim name");
    }
    Ok(())
}

/// Remove a redundant private rename only after every predecessor supplied
/// the exact same active name at that path. A missing/inactive edge contributes
/// nothing to the common roster and therefore keeps the receiving slot.
pub(crate) fn seal_receiving_claims(
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
    place: semantic_vocabulary::PlaceId,
    claims: &mut [terminal_psi::StructuralResultClaimBinding],
    common: Option<&[terminal_psi::StructuralResultClaimBinding]>,
) -> Result<(), LoweringError> {
    if !claims.is_empty() && common.is_none() {
        return unsupported("owned join has no completed predecessor custody");
    }
    for claim in claims.iter_mut() {
        if let Some(incoming) = common
            .unwrap_or(&[])
            .iter()
            .find(|incoming| incoming.path == claim.path)
            && incoming.claim != claim.claim
        {
            let old = claim.claim;
            if operations
                .block_entry_claims
                .iter()
                .filter(|row| row.claim == old && row.input == place && row.path == claim.path)
                .count()
                != 1
            {
                return unsupported("completed value name has no unique reserved join slot");
            }
            operations
                .block_entry_claims
                .retain(|row| !(row.claim == old && row.input == place && row.path == claim.path));
            claim.claim = incoming.claim;
        }
    }
    claims.sort();
    Ok(())
}

impl ClaimBindings<'_> {
    pub(crate) fn new(bindings: Vec<(PermissionClaimIdentity, ClaimId)>, next_claim: u64) -> Self {
        Self::Growable {
            bindings,
            next_claim,
            completed_values: Vec::new(),
        }
    }

    pub(crate) fn bindings(&self) -> &[(PermissionClaimIdentity, ClaimId)] {
        match self {
            Self::Growable { bindings, .. } => bindings,
            Self::Fixed(published) => published.bindings,
        }
    }

    /// Reserve a join's destination name independently of incoming alternatives.
    /// It enters no checked identity table and introduces no resource.
    pub(crate) fn receive_completed_value_claim(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Result<ClaimId, LoweringError> {
        let (rows, next_claim) = match self {
            Self::Growable {
                completed_values,
                next_claim,
                ..
            } => (completed_values, next_claim),
            Self::Fixed(published) => {
                return published
                    .completed_values
                    .iter()
                    .find(|row| row.value == value && row.path == path)
                    .map(|row| row.claim)
                    .ok_or(LoweringError::Unsupported(
                        "published graph omits completed value claim slot",
                    ));
            }
        };
        if let Some(row) = rows
            .iter()
            .find(|row| row.value == value && row.path == path)
        {
            return Ok(row.claim);
        }
        let claim = claim_id(allocate_dense(next_claim)?);
        rows.push(CompletedValueClaim {
            value,
            path: path.to_vec(),
            claim,
        });
        Ok(claim)
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
            ..
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
    arguments: &[terminal_psi::StructuralArgument],
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
    let prepared = prepare_claim_transfers(identities, transfers, types, |_identities, index| {
        parameters
            .get(index as usize)
            .map(|parameter| parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "claim transfer has no structural argument type",
            ))
    })?;
    emit_prepared_claim_transfers(&prepared, claim_bindings, arguments, current_claims)
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
    arguments: &[terminal_psi::StructuralArgument],
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<Vec<terminal_psi::CompletionReceipt>, LoweringError> {
    let mut receipts = prepared
        .iter()
        .map(|receipt| {
            Ok(terminal_psi::CompletionReceipt {
                claim: resolve_argument_claim(receipt, bindings, arguments, current_claims)?,
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
    arguments: &[terminal_psi::StructuralArgument],
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<Vec<terminal_psi::ClaimTransfer>, LoweringError> {
    let mut emitted = transfers
        .iter()
        .map(|transfer| {
            Ok(terminal_psi::ClaimTransfer {
                claim: resolve_argument_claim(transfer, claim_bindings, arguments, current_claims)?,
                argument_index: transfer.argument_index,
                path: transfer.path.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    emitted.sort();
    Ok(emitted)
}

/// A consuming checked event grants transfer authority. A selected source has
/// no invariant incoming identity; its actual completed home supplies only the
/// current destination-local name at the exact selected path.
pub(crate) fn resolve_consumed_claim(
    identity: PermissionClaimIdentity,
    path: &[terminal_psi::StructuralPathSegment],
    bindings: &[(PermissionClaimIdentity, ClaimId)],
    completed: Option<&[terminal_psi::StructuralResultClaimBinding]>,
) -> Result<ClaimId, LoweringError> {
    if identity != PermissionClaimIdentity::Unknown {
        return lookup_claim_id(bindings, identity);
    }
    let mut matching = completed
        .into_iter()
        .flatten()
        .filter(|claim| claim.path == path);
    let selected = matching.next().ok_or(LoweringError::Unsupported(
        "selected transfer has no materialized source claim",
    ))?;
    if matching.next().is_some() {
        return unsupported("selected transfer has ambiguous materialized source claim");
    }
    Ok(selected.claim)
}

fn resolve_argument_claim(
    transfer: &PreparedClaimTransfer,
    bindings: &[(PermissionClaimIdentity, ClaimId)],
    arguments: &[terminal_psi::StructuralArgument],
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<ClaimId, LoweringError> {
    let argument =
        arguments
            .get(transfer.argument_index as usize)
            .ok_or(LoweringError::Unsupported(
                "selected transfer has no actual argument",
            ))?;
    if argument.access != terminal_psi::StructuralAccess::Owned {
        return unsupported("selected transfer requires owned argument custody");
    }
    let completed = current_claims.source(argument)?;
    resolve_consumed_claim(
        transfer.identity,
        &transfer.path,
        bindings,
        completed.as_deref(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedClaim {
    pub(crate) identity: PermissionClaimIdentity,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
    provenance: language_semantics::PermissionProvenance,
    fresh: bool,
    /// Direct establishment by this statement, rather than reservation for a
    /// completed child call whose claim this receiving owner only transports.
    direct_establishment: bool,
}

/// A completed operand can have several proved construction/receiving
/// coordinates. The checked flow graph retains their exact typed child paths;
/// claim preparation reconciles identities instead of choosing an ancestor.
/// No authored assignment or neighboring store is read.
fn receiving_claim_places(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    root: facts::PlaceRoot,
) -> Result<Vec<(facts::PlaceRoot, Vec<facts::PlaceSegment>)>, LoweringError> {
    let facts::PlaceRoot::Expression(expression) = root else {
        return Ok(Vec::new());
    };
    let flow = &checked.facts.flow.control;
    let semantic = &checked.facts.semantic;
    let mut roots = Vec::new();
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
            roots.push((
                row.stored_value,
                destination.root,
                semantic
                    .place_segments
                    .span_or_empty(destination.segments)
                    .to_vec(),
            ));
        }
    }
    // Computed storage completes before an address borrows it. Flow retains
    // that actual construction even when no local receives the backing. Its
    // source-owned establishment authenticates the root; the construction's
    // typed child paths supply only correspondence, never new claim authority.
    let mut established_constructions = Vec::new();
    let state_permissions = checked
        .facts
        .flow
        .ownership
        .with_state_permissions(state, |handles| handles.to_vec());
    for handle in state_permissions {
        let event = checked.facts.flow.ownership.permissions.get(handle);
        let facts::PlaceRoot::Expression(construction) = event.root else {
            continue;
        };
        if event.machine_symbol == machine
            && event.state_symbol == state
            && event.expression == construction
            && event.kind == language_semantics::PermissionEventKind::Establish
            && event.access == language_semantics::PermissionAccess::Owned
            && event.multiplicity == language_semantics::Multiplicity::Linear
            && event.obligation_live
            && event.claim_identity != PermissionClaimIdentity::Unknown
            && matches!(event.source,
                language_semantics::PermissionEventSource::Statement { statement_index }
                | language_semantics::PermissionEventSource::Call { statement_index, .. }
                if statement_index == statement)
            && !established_constructions.contains(&construction)
        {
            established_constructions.push(construction);
        }
    }
    for (handle, operand) in flow.operands.iter().filter(|(_, operand)| {
        operand.state_symbol == state
            && operand.statement_index == statement
            && operand.expression != expression
            && !operand.constructed_children().is_empty()
            && established_constructions.contains(&operand.expression)
    }) {
        roots.push((
            handle,
            facts::PlaceRoot::Expression(operand.expression),
            Vec::new(),
        ));
    }
    let mut receiving = Vec::new();
    for (handle, destination, path) in roots {
        let mut pending = vec![(handle, path)];
        while let Some((handle, path)) = pending.pop() {
            let operand = flow.operands.get(handle);
            if operand.state_symbol != state || operand.statement_index != statement {
                return unsupported("stored claim operand has a different evaluation coordinate");
            }
            if operand.expression == expression {
                let candidate = (destination, path);
                if !receiving.contains(&candidate) {
                    receiving.push(candidate);
                }
                continue;
            }
            for child in flow
                .operand_children
                .span_or_empty(operand.constructed_children())
            {
                let mut child_path = path.clone();
                child_path.extend_from_slice(flow.operand_child_paths.span_or_empty(child.path));
                pending.push((child.value, child_path));
            }
        }
    }
    Ok(receiving)
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
    let completed_expression = root.as_expression();
    let receiving = receiving_claim_places(checked, machine, state, statement, root)?;
    // Completion and its proved construction/receiving storage are coordinates
    // of one value. A containing source root authenticates only the exact typed
    // child prefix found above; it never replaces the child's actual producer.
    // Transparent receivers retain that same claim at their exact typed path.
    let mut coordinates = vec![(root, Vec::new())];
    for coordinate in receiving {
        if !coordinates.contains(&coordinate) {
            coordinates.push(coordinate);
        }
    }
    let mut prepared: Vec<PreparedClaim> = Vec::new();
    let state_permissions = checked
        .facts
        .flow
        .ownership
        .with_state_permissions(state, |handles| handles.to_vec());
    for (root, receiving_path) in coordinates {
        let mut completed_evaluation = None;
        if completed_expression.is_some() {
            if let Some(construction) = root.as_expression() {
                for &handle in &state_permissions {
                    let event = checked.facts.flow.ownership.permissions.get(handle);
                    if event.machine_symbol != machine
                        || event.state_symbol != state
                        || event.root != root
                        || event.expression != construction
                        || event.kind != language_semantics::PermissionEventKind::Establish
                        || !matches!(event.source,
                            language_semantics::PermissionEventSource::Statement { statement_index }
                            | language_semantics::PermissionEventSource::Call { statement_index, .. }
                            if statement_index == statement)
                    {
                        continue;
                    }
                    if event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
                        || completed_evaluation.is_some_and(|lane| lane != event.evaluation)
                    {
                        return unsupported(
                            "completed construction has ambiguous evaluation custody",
                        );
                    }
                    completed_evaluation = Some(event.evaluation);
                }
            } else {
                completed_evaluation = Some(checked_trees::FlowPermissionEvaluation::Statement);
            }
        }
        let at_completion = |event: &checked_trees::FlowPermissionEventFact| {
            completed_expression.is_none_or(|_| {
                completed_evaluation == Some(event.evaluation)
                    && root
                        .as_expression()
                        .is_none_or(|owner| event.expression == owner)
            })
        };
        for &handle in &state_permissions {
            let event = checked.facts.flow.ownership.permissions.get(handle);
            if !(event.machine_symbol == machine && event.state_symbol == state
                && at_completion(event) && event.root == root
                && checked.facts.flow.ownership.segments.span_or_empty(event.segments).starts_with(&receiving_path)
                && event.kind == language_semantics::PermissionEventKind::Establish
                // Alternative incoming IDs without an invariant name remain
                // unavailable; completion never manufactures a replacement.
                && event.claim_identity != PermissionClaimIdentity::Unknown
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == language_semantics::Multiplicity::Linear && event.obligation_live
                && matches!(event.source, language_semantics::PermissionEventSource::Statement { statement_index }
                    | language_semantics::PermissionEventSource::Call { statement_index, .. } if statement_index == statement))
            {
                continue;
            }
            // Reservation follows the authenticated first completion even when
            // this same evaluation subsequently transports the completed child.
            let established_here = state_permissions.iter().copied().any(|source_handle| {
                let source = checked.facts.flow.ownership.permissions.get(source_handle);
                source.evaluation == event.evaluation
                    && source.claim_identity == event.claim_identity
                    && claim_established_here(machine, state, statement, source)
            });
            let path = validation::structural_claim_path(&checked.typed, type_reference,
                &checked.facts.flow.ownership.segments.span_or_empty(event.segments)[receiving_path.len()..])
                .map_err(LoweringError::Unsupported)?;
            let claim = PreparedClaim {
                identity: event.claim_identity,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, &path, structural_type, structural_types)?,
                provenance: event.provenance,
                fresh: established_here,
                direct_establishment: matches!(event.source, language_semantics::PermissionEventSource::Statement { .. })
                    && event.provenance == language_semantics::PermissionProvenance::Established {
                        machine_symbol: machine, state_symbol: state, source: event.source,
                    }
                    && matches!(event.claim_identity, PermissionClaimIdentity::Established { machine_symbol, state_symbol, source, .. }
                        if machine_symbol == machine && state_symbol == state && source == event.source),
            };
            if let Some(previous) = prepared.iter().find(|previous| previous.path == claim.path) {
                if previous != &claim {
                    return unsupported("completed construction has conflicting claim correspondences");
                }
            } else {
                prepared.push(claim);
            }
        }
    }
    Ok(prepared)
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

/// Transport the exact checked consuming occurrence through its current source names.
/// Selected custody authenticates those same events before current-name resolution.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_owned_operand_claims(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    expected_evaluation: Option<checked_trees::FlowPermissionEvaluation>,
    expected_source: Option<language_semantics::PermissionEventSource>,
    bindings: &[(PermissionClaimIdentity, ClaimId)],
    completed_claims: Option<&[terminal_psi::StructuralResultClaimBinding]>,
    structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
    if !expression.is_valid() {
        return unsupported("owned operand has no exact consuming expression");
    }
    let statement = usize::try_from(statement_index)
        .map_err(|_| LoweringError::Unsupported("structural return coordinate exceeds usize"))?;
    let expression =
        validation::ownership_source_expression(&checked.typed, state, statement, expression);
    // Source transfer discovery and selection receipts use ownership places.
    // A noncopy saved subject observes its original owning home; its proof/read
    // anchor is not another source of claims. Copy subjects retain their own
    // captured home through that same shared query.
    let place = validation::ownership_place_from_expression_in_state(
        &checked.typed,
        state,
        statement,
        expression,
    )
    .ok_or(LoweringError::Unsupported(
        "owned operand has no exact owning source",
    ))?;
    let root = place.root;
    let prefix = place.segments.as_slice();
    let evaluated_transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(_, event)| {
            (event.machine_symbol == machine
                && event.state_symbol == state
                && event.root == root
                && event.expression == expression
                && event.kind == language_semantics::PermissionEventKind::Transfer
                && event.access == language_semantics::PermissionAccess::Owned
                && event.multiplicity == language_semantics::Multiplicity::Linear
                && event.obligation_live)
                .then_some(event)
        })
        .collect::<Vec<_>>();
    let ownership = &checked.facts.flow.ownership;
    let mut selected_transfer = None;
    let state_selections =
        ownership.with_state_owned_selections(state, |handles| handles.to_vec());
    for handle in state_selections {
        let receipt = ownership.owned_selections.get(handle);
        if !(receipt.machine == machine
            && receipt.state == state
            && receipt.statement_ordinal == statement_index
            && expected_evaluation.is_none_or(|evaluation| receipt.evaluation == evaluation))
        {
            continue;
        }
        let transfers = ownership
            .selection_transfers
            .span(receipt.transfers)
            .ok_or(LoweringError::Unsupported(
                "owned operand selection transfers are stale",
            ))?;
        for transfer in transfers
            .iter()
            .filter(|transfer| transfer.expression == expression)
        {
            if selected_transfer.replace((receipt, transfer)).is_some() {
                return unsupported("owned operand has ambiguous selected custody");
            }
        }
    }
    // Evaluated events own the transfer authority. A selection receipt must
    // authenticate that same frontier, including an empty linear frontier;
    // it cannot supply an alternate stream or another arm's resource claims.
    if evaluated_transfers.is_empty() && selected_transfer.is_none() {
        return Ok(Vec::new());
    }
    let root_type = validation::canonical_place_type_reference(
        &checked.typed,
        state,
        statement,
        &validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        },
    )
    .ok_or(LoweringError::Unsupported(
        "returned owner has no exact carrier",
    ))?;
    let (prefix, _) = validation::structural_owned_path(&checked.typed, root_type, prefix)
        .map_err(LoweringError::Unsupported)?;
    let mut claims = Vec::new();
    let mut consumer = expected_source;
    let mut evaluation = expected_evaluation;
    for event in &evaluated_transfers {
        if event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
            || evaluation.is_some_and(|lane| lane != event.evaluation)
        {
            return unsupported("owned operand has ambiguous consuming evaluation");
        }
        evaluation = Some(event.evaluation);
        match event.source {
            language_semantics::PermissionEventSource::Statement { statement_index }
            | language_semantics::PermissionEventSource::Call {
                statement_index, ..
            } if statement_index == statement => {}
            _ => return unsupported("owned operand transfer has another consumer statement"),
        }
        if consumer.is_some_and(|source| source != event.source) {
            return unsupported("owned operand has multiple consuming coordinates");
        }
        consumer = Some(event.source);
        let path = validation::structural_claim_path(
            &checked.typed,
            root_type,
            checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments),
        )
        .map_err(LoweringError::Unsupported)?;
        let Some(relative) = path.strip_prefix(prefix.as_slice()) else {
            continue;
        };
        let path =
            crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities,
                relative,
                structural_type,
                structural_types,
            )?;
        // The consuming event owns the exact current logical claim. A
        // completed source supplies its current transport name, including
        // known claims renamed by an owned arrival; it supplies no lineage.
        let claim = crate::emission::claim_bindings::resolve_consumed_claim(
            event.claim_identity,
            &path,
            bindings,
            completed_claims,
        )?;
        claims.push(terminal_psi::StructuralResultClaimBinding { claim, path });
    }
    if let Some((receipt, transfer)) = selected_transfer {
        if receipt.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
            || evaluation.is_some_and(|evaluation| evaluation != receipt.evaluation)
        {
            return unsupported("owned operand has ambiguous consuming evaluation");
        }
        let source_path = validation::structural_owned_path(
            &checked.typed,
            root_type,
            ownership.segments.span_or_empty(transfer.path),
        )
        .map_err(LoweringError::Unsupported)?
        .0;
        if transfer.root != root || prefix != source_path {
            return unsupported("selected custody names another operand source");
        }
        // Named source rows coordinate residual cleanup only. Anonymous
        // completed producers retain their own expression root and need no
        // fictional named cleanup candidate or invariant source identity.
        if transfer.source.is_valid() {
            if !ownership.selection_sources.is_valid(transfer.source)
                || !receipt
                    .sources
                    .handles()
                    .any(|source| source == transfer.source)
            {
                return unsupported("selected owned operand has stale source custody");
            }
            let source = ownership.selection_sources.get(transfer.source);
            if root != facts::PlaceRoot::Symbol(source.symbol) {
                return unsupported("selected custody names another operand source");
            }
        } else if !matches!(root, facts::PlaceRoot::Expression(_)) {
            let source_type = validation::project_type_reference_from_segments(
                &checked.typed,
                root_type,
                ownership.segments.span_or_empty(transfer.path),
            )
            .ok_or(LoweringError::Unsupported(
                "selected copy has no typed source path",
            ))?;
            if !transfer.claims.is_empty()
                || !evaluated_transfers.is_empty()
                || !validation::reference_result_custody::source_value_is_copied(
                    &checked.typed,
                    source_type,
                )
                || !validation::reference_result_custody::source_value_is_copied(
                    &checked.typed,
                    transfer.source_type_reference,
                )
            {
                return unsupported("selected owned operand has no named source custody");
            }
        }
        let selected = ownership
            .selection_transfer_claims
            .span(transfer.claims)
            .ok_or(LoweringError::Unsupported(
                "owned operand selection claims are stale",
            ))?;
        let mut selected_claims = Vec::new();
        for claim in selected {
            // Authenticate logical identity before resolving the current
            // transport name: equal physical paths alone prove no lineage.
            if !evaluated_transfers.iter().any(|event| {
                event.claim_identity == claim.claim_identity
                    && event.provenance == claim.provenance
                    && ownership.segments.span_or_empty(event.segments)
                        == ownership.segments.span_or_empty(claim.path)
            }) {
                return unsupported("selected claim has no matching evaluated transfer");
            }
            let path = validation::structural_claim_path(
                &checked.typed,
                root_type,
                ownership.segments.span_or_empty(claim.path),
            )
            .map_err(LoweringError::Unsupported)?;
            let relative =
                path.strip_prefix(prefix.as_slice())
                    .ok_or(LoweringError::Unsupported(
                        "selected claim is outside its operand",
                    ))?;
            let path =
                crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities,
                    relative,
                    structural_type,
                    structural_types,
                )?;
            let claim =
                resolve_consumed_claim(claim.claim_identity, &path, bindings, completed_claims)?;
            selected_claims.push(terminal_psi::StructuralResultClaimBinding { claim, path });
        }
        claims.sort();
        selected_claims.sort();
        if claims != selected_claims {
            return unsupported("selected custody differs from its evaluated claim frontier");
        }
    }
    claims.sort();
    if claims.iter().enumerate().any(|(index, claim)| {
        claims[..index]
            .iter()
            .any(|prior| prior.claim == claim.claim || prior.path == claim.path)
    }) {
        return unsupported("owned operand repeats a claim identity or path");
    }
    Ok(claims)
}
