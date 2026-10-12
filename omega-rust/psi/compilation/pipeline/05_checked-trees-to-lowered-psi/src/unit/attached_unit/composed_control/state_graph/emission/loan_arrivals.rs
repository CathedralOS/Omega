//! A completed descriptor can end before the next state renews its ingress loan.
//! The checked release and captured root establish that handoff; equal pointers,
//! reference shape and recursive position do not. Actual operands and selected
//! cleanup finish before the post-cleanup block resolves that recorded ingress.

use checked_trees::{
    BorrowLoanLineage, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan as Source, CheckedUnitStructuralPathSegment as Path,
};
use checked_trees::{CheckedComposedUnitControlStatePlan, CheckedTrees};

pub(super) struct ReleasedIngress {
    pub parameter: usize,
    pub path: Vec<Path>,
    pub descriptor_path: Vec<Path>,
}

/// Rejoin one selected descriptor leaf to its exact scheduled release and
/// complete captured loan group. A borrowed formal outlives this state; owned
/// input storage, local backing and unretained/reborrow lineage do not provide
/// an independent ingress after this descriptor ends.
pub(super) fn released_ingress(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &CheckedComposedUnitControlStatePlan,
    statement_index: u32,
    binding_ordinal: u32,
    path: &[Path],
    access: CheckedStructuralAccess,
) -> Option<ReleasedIngress> {
    let crossing = path
        .iter()
        .position(|segment| matches!(segment, Path::Referent))?;
    let descriptor_path = &path[..crossing];
    let mut bindings = state.operations.iter().filter_map(|operation| {
        operation
            .structural_result_binding()
            .filter(|binding| binding.binding_ordinal == binding_ordinal)
    });
    let binding = bindings.next()?;
    if bindings.next().is_some() || !binding.local.is_named() {
        return None;
    }
    let owner_path = validation::reference_result_custody::loan_owner_path(
        checked.as_ref(),
        binding.local.declared_type,
        descriptor_path,
    )?;
    let mut releases = state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ReleaseReference {
                statement_index: boundary,
                source:
                    Source::StructuralResult {
                        binding_ordinal: released,
                    },
                path,
                loans,
                defer_to_exit: true,
                ..
            } if *released == binding_ordinal
                && path == descriptor_path
                && *boundary == statement_index.checked_add(1)? =>
            {
                Some(*loans)
            }
            _ => None,
        });
    let loans = releases.next()?;
    if releases.next().is_some() || loans.is_empty() {
        return None;
    }
    let mut states = checked
        .facts
        .borrow
        .states
        .iter()
        .filter_map(|(_, borrowing)| {
            (borrowing.machine_symbol == machine && borrowing.state_symbol == state.state)
                .then_some(borrowing)
        });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let authored = checked.state_by_symbol(state.state)?;
    let parameters = checked.state_parameters(authored);
    let target_kind = access.borrow_access_kind()?;
    let mut origin = None;
    for (handle, loan) in checked.facts.borrow.loan_group(loans)? {
        if !checked.facts.borrow.state_owns_loan(borrowing, handle)
            || loan.lineage != BorrowLoanLineage::DirectRoot
            || loan.incoming_reference.is_valid()
            || loan.supplying_loan.is_valid()
            || loan.source_owner_symbol.is_valid()
            || loan.owner_symbol != binding.local.symbol
            || loan.storage_owner != facts::PlaceRoot::Symbol(binding.local.symbol)
            || checked.facts.borrow.loan_owner_path(loan) != owner_path
            || loan.statement_index != binding.statement_index as usize
            || validation::reference_result_custody::release_boundary(
                checked.as_ref(),
                &checked.facts,
                machine,
                state.state,
                handle,
            )? != statement_index.checked_add(1)?
            || loan.kind.direct_reborrow_effect(&target_kind).is_none()
        {
            return None;
        }
        let root = loan.root.as_symbol()?;
        let mut candidates =
            state
                .structural_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    parameters
                        .get(parameter.position as usize)
                        .is_some_and(|authored| authored.symbol == root)
                });
        let (dense, parameter) = candidates.next()?;
        if candidates.next().is_some()
            || parameter
                .access
                .borrow_access_kind()?
                .direct_reborrow_effect(&loan.kind)
                .is_none()
            || parameter.binding_carrier.is_some()
        {
            return None;
        }
        let declared = parameters.get(parameter.position as usize)?;
        validation::reference_shell(checked.as_ref(), declared.type_reference)?;
        let segments = checked.facts.borrow.loan_segments(loan);
        let source_path = recorded_path(checked, segments)?;
        // Source scoping, member membership and prepared generic views come
        // from the existing projection query, never an equal member spelling.
        if validation::reference_result_custody::loan_place_segments(
            checked.as_ref(),
            declared.type_reference,
            &source_path,
        )?
        .as_slice()
            != segments
        {
            return None;
        }
        let candidate = (dense, source_path);
        if origin
            .as_ref()
            .is_some_and(|previous| previous != &candidate)
        {
            return None;
        }
        origin = Some(candidate);
    }
    let (parameter, source_path) = origin?;
    Some(ReleasedIngress {
        parameter,
        path: source_path,
        descriptor_path: descriptor_path.to_vec(),
    })
}

fn recorded_path(checked: &CheckedTrees, segments: &[facts::PlaceSegment]) -> Option<Vec<Path>> {
    segments
        .iter()
        .map(|segment| {
            Some(match segment {
                facts::PlaceSegment::Field { symbol } => {
                    let mut selected = None;
                    for (_, definition) in checked.tables.data_definitions.iter() {
                        for field in checked
                            .data_member_fields(checked.data_members(definition))
                            .filter(|field| field.symbol == *symbol)
                        {
                            let identity = checked_trees::CheckedMemberIdentity::of_member(
                                field.identity,
                                field.name.clone(),
                            );
                            if selected
                                .as_ref()
                                .is_some_and(|previous| previous != &identity)
                            {
                                return None;
                            }
                            selected = Some(identity);
                        }
                    }
                    Path::Field(selected?)
                }
                facts::PlaceSegment::Case { variant } => {
                    let mut selected = None;
                    for (_, definition) in checked.tables.data_definitions.iter() {
                        if let Some(case) = checked.data_variant(definition, *variant) {
                            let identity = checked_trees::CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            );
                            if selected
                                .as_ref()
                                .is_some_and(|previous| previous != &identity)
                            {
                                return None;
                            }
                            selected = Some(identity);
                        }
                    }
                    Path::Case(selected?)
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    Path::FixedIndex(u64::try_from(*index).ok()?)
                }
                // A selector snapshot or collection extent needs its actual descriptor;
                // a fact's expression coordinate cannot reconstruct runtime geometry.
                facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedRange { .. } => {
                    return None;
                }
            })
        })
        .collect()
}

use super::{LoweringError, StateGraphEmission};
use semantic_vocabulary::{BlockId, ProofTerm, ScalarTerm, StructuralPlaceKind, ValueId};
use terminal_psi::{
    Block, StructuralArgument, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    TerminalAffineCleanupAction, TerminalNaturalRankComparison, Terminator, ValueDeclaration,
};

#[derive(Clone)]
pub(super) struct IngressRenewal {
    pub argument: usize,
    pub source: StructuralArgument,
    pub retirement: TerminalAffineCleanupAction,
    /// Existing path obligations whose exact source verdict survived this exit.
    pub preserved_bounds: Vec<semantic_vocabulary::ObligationId>,
    pub source_exit: checked_trees::FlowClaimJoinExit,
}

impl StateGraphEmission<'_, '_> {
    /// Preserve completed operands through the original selected cleanup. Only
    /// the already captured, checked ingress is resolved in the second jump.
    /// No original expression, initializer or selector is evaluated again.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_released_ingresses(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        target: BlockId,
        arguments: &mut Vec<ValueId>,
        structural_arguments: &mut Vec<StructuralArgument>,
        declarations: &[StructuralParameterDeclaration],
        values: &[ValueDeclaration],
        renewals: &[IngressRenewal],
        erased_arguments: &mut Vec<ScalarTerm>,
        erased_proof_arguments: &mut Vec<ProofTerm>,
        current_rank: Option<ValueId>,
        arriving_rank: Option<ValueId>,
        arriving_comparison: TerminalNaturalRankComparison,
        arrival_decided: bool,
        edge_blocks: &mut Vec<Block>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
    ) -> Result<BlockId, LoweringError> {
        let block = super::super::super::super::super::block_id(
            super::super::super::super::allocate_dense(next_block)?,
        );
        let final_edge = super::super::super::super::edge_id(
            super::super::super::super::allocate_dense(next_edge)?,
        );
        let original_arguments = arguments.clone();
        let mut inputs = Vec::new();
        for input in &original_arguments {
            if !inputs.contains(input) {
                inputs.push(*input);
            }
        }
        for rank in current_rank.iter().chain(arriving_rank.iter()) {
            if !inputs.contains(rank) {
                inputs.push(*rank);
            }
        }
        // A renewed path is consumed only after cleanup. Its selectors were
        // already evaluated with the other source operands and must cross the
        // staging edge as ordinary scalar parameters, too. The bound ID stays
        // on this one final carrying path; no certificate is copied or issued.
        for renewal in renewals {
            for segment in &renewal.source.path {
                if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment
                    && !inputs.contains(index)
                {
                    inputs.push(*index);
                }
            }
        }
        // Erased terms retain their source erased formals. Runtime snapshots
        // named by those terms enter the same actual scalar parameter roster.
        let mut visit = |term: &ScalarTerm| {
            term.visit_value_ids(|value| {
                if values.iter().any(|declaration| declaration.id == value)
                    && !inputs.contains(&value)
                {
                    inputs.push(value);
                }
                true
            });
        };
        for term in erased_arguments.iter() {
            visit(term);
        }
        let mut proof_terms = erased_proof_arguments.iter().collect::<Vec<_>>();
        while let Some(term) = proof_terms.pop() {
            match term {
                ProofTerm::Scalar(term) => visit(term),
                ProofTerm::Construction { fields, .. } => {
                    proof_terms.extend(fields.iter().map(|field| &field.term))
                }
                ProofTerm::Formal { .. } => {}
            }
        }
        let mut parameters = Vec::new();
        let mut scalar_map = Vec::new();
        for input in &inputs {
            // This is the retained actual value declaration, not a receiving
            // formal's carrier or an arithmetic-policy reconstruction.
            let original = values
                .iter()
                .find(|declaration| declaration.id == *input)
                .ok_or(LoweringError::Unsupported(
                    "loan handoff lost its completed scalar declaration",
                ))?;
            let mut parameter = original.clone();
            parameter.id = crate::terminal_identities::value_id(
                super::super::super::super::allocate_dense(next_value)?,
            );
            scalar_map.push((*input, parameter.id));
            parameters.push(parameter);
        }
        let remap = |value: ValueId| {
            scalar_map
                .iter()
                .find(|(source, _)| *source == value)
                .map_or(value, |(_, destination)| *destination)
        };
        let final_arguments = original_arguments.into_iter().map(remap).collect();
        let old_structural = std::mem::take(structural_arguments);
        if old_structural.len() != declarations.len() {
            return Err(LoweringError::Unsupported(
                "loan handoff lost its structural declaration roster",
            ));
        }
        let mut final_structural = Vec::new();
        let mut captured_structural = Vec::new();
        let mut structural_parameters = Vec::new();
        for (index, (argument, declaration)) in
            old_structural.into_iter().zip(declarations).enumerate()
        {
            if let Some(renewal) = renewals.iter().find(|renewal| renewal.argument == index) {
                let mut renewed = renewal.source.clone();
                for segment in &mut renewed.path {
                    if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment
                    {
                        *index = remap(*index);
                    }
                }
                final_structural.push(renewed);
            } else {
                let mut parameter = declaration.clone();
                parameter.place = super::super::super::super::place_id(
                    super::super::super::super::allocate_dense(&mut self.catalogs.next_place)?,
                );
                parameter.position = u32::try_from(structural_parameters.len()).map_err(|_| {
                    LoweringError::Unsupported("loan handoff structural roster exceeds u32")
                })?;
                parameter.access = argument.access;
                self.structural_places.push(StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block,
                        position: parameter.position,
                    },
                });
                final_structural.push(StructuralArgument {
                    place: parameter.place,
                    path: Vec::new(),
                    access: argument.access,
                });
                // All selected cleanup edges have already captured these
                // actuals. The actual whole block parameter carries this type;
                // an earlier projected source never donates a receiving type.
                if !argument.path.is_empty() {
                    return Err(LoweringError::Unsupported(
                        "loan handoff lost its completed whole capture",
                    ));
                }
                captured_structural.push(
                    crate::unit::attached_unit::argument_evaluation::cleanup_control::CapturedStructuralArgument {
                        source: argument.clone(), source_type: declaration.structural_type,
                        captured: parameter.clone(),
                    },
                );
                structural_parameters.push(parameter);
                structural_arguments.push(argument);
            }
        }
        let mut final_erased = std::mem::take(erased_arguments);
        let mut final_proofs = std::mem::take(erased_proof_arguments);
        let capture = crate::unit::attached_unit::argument_evaluation::cleanup_control::CapturedArgumentFrame {
            values: inputs.iter().zip(&parameters).map(|(input, parameter)| {
                let source = values.iter().find(|value| value.id == *input)
                    .expect("staging source declarations were retained above");
                (source.clone(), parameter.clone())
            }).collect(),
            structural: captured_structural,
        };
        let mut final_renewals = renewals.to_vec();
        transport_captured_arguments(
            &self.catalogs.structural_types,
            &capture,
            &mut [],
            &mut final_erased,
            &mut final_proofs,
            &mut final_renewals,
            &mut None,
            &mut None,
        )?;
        *arguments = inputs;
        *erased_arguments = self.state_erased[position]
            .iter()
            .map(|formal| ScalarTerm::value(formal.id, formal.scalar_type))
            .collect();
        *erased_proof_arguments = (0..self.state_erased_proof[position].len())
            .map(|position| {
                Ok(ProofTerm::Formal {
                    position: u32::try_from(position).map_err(|_| {
                        LoweringError::Unsupported("loan handoff proof roster exceeds u32")
                    })?,
                })
            })
            .collect::<Result<_, LoweringError>>()?;
        if let Some(rank) = current_rank {
            self.block_ranks.insert(block, remap(rank));
        }
        if let Some(rank) = arriving_rank {
            self.rank_edges
                .insert(final_edge, (remap(rank), arriving_comparison));
        }
        self.arrival_edges
            .entry(target)
            .or_default()
            .push(final_edge);
        if arrival_decided {
            self.decided_arrival_edges.insert(final_edge);
        }
        edge_blocks.push(Block {
            id: block,
            parameters,
            erased_scalar_formals: self.state_erased[position].clone(),
            erased_proof_formals:
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    identities,
                    self.checked,
                    &self.state_erased_proof[position],
                ),
            structural_parameters,
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: final_edge,
                target,
                arguments: final_arguments,
                structural_arguments: final_structural,
                erased_arguments: final_erased,
                erased_proof_arguments: final_proofs,
                cleanup_actions: Vec::new(),
            },
        });
        Ok(block)
    }
}

/// Substitute only coordinates produced by the actual capture. Static
/// observations use the same canonical source-prefix projection as ordinary
/// copied operands; runtime reads remain completed scalar snapshots.
#[allow(clippy::too_many_arguments)]
pub(super) fn transport_captured_arguments(
    types: &[terminal_psi::StructuralTypeDeclaration],
    capture: &crate::unit::attached_unit::argument_evaluation::cleanup_control::CapturedArgumentFrame,
    arguments: &mut [ValueId],
    erased: &mut [ScalarTerm],
    proofs: &mut [ProofTerm],
    renewals: &mut [IngressRenewal],
    current_rank: &mut Option<ValueId>,
    arriving_rank: &mut Option<ValueId>,
) -> Result<(), LoweringError> {
    let mut values = Vec::with_capacity(capture.values.len());
    for (source, destination) in &capture.values {
        if source.value_type() != destination.value_type() {
            return Err(LoweringError::Unsupported(
                "cleanup continuation changes its captured scalar carrier",
            ));
        }
        values.push((source.id, destination.id));
    }
    let remap = |value: &mut ValueId| {
        if let Some((_, destination)) = values.iter().find(|(source, _)| source == value) {
            *value = *destination;
        }
    };
    for argument in arguments {
        remap(argument);
    }
    if let Some(rank) = current_rank {
        remap(rank);
    }
    if let Some(rank) = arriving_rank {
        remap(rank);
    }
    for renewal in renewals.iter_mut() {
        for segment in &mut renewal.source.path {
            if let terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                remap(index);
            }
        }
    }
    let mut terms = erased.iter_mut().collect::<Vec<_>>();
    let mut pending = proofs.iter_mut().collect::<Vec<_>>();
    while let Some(proof) = pending.pop() {
        match proof {
            ProofTerm::Scalar(term) => terms.push(term),
            ProofTerm::Construction { fields, .. } => {
                pending.extend(fields.iter_mut().map(|field| &mut field.term))
            }
            ProofTerm::Formal { .. } => {}
        }
    }
    while let Some(term) = terms.pop() {
        let observed = match &*term {
            ScalarTerm::StructuralRead { root, .. }
            | ScalarTerm::BooleanField { root, .. }
            | ScalarTerm::IntegerField { root, .. }
            | ScalarTerm::IeeeFloatField { root, .. }
            | ScalarTerm::ViewExtent { root, .. } => Some(*root),
            _ => None,
        };
        if observed.is_some_and(|root| {
            renewals
                .iter()
                .any(|renewal| renewal.retirement.place() == root)
        }) {
            return Err(LoweringError::Unsupported(
                "loan handoff erased observation lacks a completed snapshot",
            ));
        }
        match term {
            ScalarTerm::Value { id, .. } => remap(id),
            ScalarTerm::StructuralRead { root, path, .. } => {
                for captured in &capture.structural {
                    if captured.source.place != *root {
                        continue;
                    }
                    let Some(prefix) = captured_read_prefix(types, captured, &capture.values)?
                    else {
                        continue;
                    };
                    if let Some(relative) = path.strip_prefix(prefix.as_slice()) {
                        *path = relative.to_vec();
                        *root = captured.captured.place;
                        break;
                    }
                }
            }
            ScalarTerm::BooleanField { root, path, .. }
            | ScalarTerm::IntegerField { root, path, .. }
            | ScalarTerm::IeeeFloatField { root, path, .. }
            | ScalarTerm::ViewExtent { root, path, .. } => {
                for captured in &capture.structural {
                    if captured.source.place != *root {
                        continue;
                    }
                    let Some((prefix, selected)) =
                        terminal_semantics::canonical_observation_projection(
                            types.iter(),
                            captured.source_type,
                            &captured.source.path,
                        )
                    else {
                        continue;
                    };
                    if selected.id != captured.captured.structural_type {
                        return Err(LoweringError::Unsupported(
                            "cleanup capture changes its observed source carrier",
                        ));
                    }
                    if let Some(relative) = path.strip_prefix(prefix.as_slice()) {
                        *path = relative.to_vec();
                        *root = captured.captured.place;
                        break;
                    }
                }
            }
            _ => {}
        }
        let whole = match &*term {
            ScalarTerm::BooleanField { root, path } if path.is_empty() => {
                Some((*root, semantic_vocabulary::ScalarType::Boolean))
            }
            ScalarTerm::IntegerField {
                root,
                path,
                scalar_type,
            } if path.is_empty() => Some((
                *root,
                semantic_vocabulary::ScalarType::Integer(*scalar_type),
            )),
            ScalarTerm::IeeeFloatField { root, path, format } if path.is_empty() => {
                Some((*root, semantic_vocabulary::ScalarType::IeeeFloat(*format)))
            }
            _ => None,
        };
        if let Some((root, scalar_type)) = whole {
            *term = ScalarTerm::StructuralRead {
                root,
                path: Vec::new(),
                scalar_type,
            };
        }
        terms.extend(term.operands_mut());
    }
    Ok(())
}

/// Read vocabulary retains evaluated selectors, while canonical field facts
/// only name static owned paths. Follow actual declaration geometry at each
/// segment so a captured runtime-selected child has an exact read prefix;
/// reference crossings have no owned scalar-read spelling in this vocabulary.
fn captured_read_prefix(
    types: &[terminal_psi::StructuralTypeDeclaration],
    captured: &crate::unit::attached_unit::argument_evaluation::cleanup_control::CapturedStructuralArgument,
    values: &[(ValueDeclaration, ValueDeclaration)],
) -> Result<Option<Vec<semantic_vocabulary::ScalarReadProjection>>, LoweringError> {
    use semantic_vocabulary::ScalarReadProjection;
    use terminal_psi::StructuralPathSegment;
    let selected = terminal_semantics::runtime_structural_path_tip(
        types.iter(),
        captured.source_type,
        &captured.source.path,
    )
    .ok_or(LoweringError::Unsupported(
        "cleanup capture lost its actual observed projection",
    ))?;
    if selected.id != captured.captured.structural_type {
        return Err(LoweringError::Unsupported(
            "cleanup capture changes its observed source carrier",
        ));
    }
    let mut prefix = Vec::new();
    let mut carrier = captured.source_type;
    for segment in &captured.source.path {
        match segment {
            StructuralPathSegment::RuntimeIndex { index, .. } => {
                let declaration = values
                    .iter()
                    .find(|(source, _)| source.id == *index)
                    .map(|(source, _)| source)
                    .ok_or(LoweringError::Unsupported(
                        "cleanup capture lost its completed selector declaration",
                    ))?;
                prefix.push(ScalarReadProjection::Index(Box::new(ScalarTerm::value(
                    *index,
                    declaration.scalar_type,
                ))));
            }
            StructuralPathSegment::Referent | StructuralPathSegment::FixedByteRange { .. } => {
                return Ok(None);
            }
            _ => {
                let Some((steps, _)) = terminal_semantics::canonical_observation_projection(
                    types.iter(),
                    carrier,
                    std::slice::from_ref(segment),
                ) else {
                    return Ok(None);
                };
                prefix.extend(steps.into_iter().map(ScalarReadProjection::Static));
            }
        }
        carrier = terminal_semantics::runtime_structural_path_tip(
            types.iter(),
            carrier,
            std::slice::from_ref(segment),
        )
        .ok_or(LoweringError::Unsupported(
            "cleanup capture lost its selected declaration",
        ))?
        .id;
    }
    Ok(Some(prefix))
}

/// Value IDs denote completed scalar captures. Structural observations keep
/// live storage meaning after place transport, even inside nested arithmetic.
pub(super) fn erased_scalar_observes_storage(term: &semantic_vocabulary::ScalarTerm) -> bool {
    let mut pending = vec![term];
    while let Some(term) = pending.pop() {
        if matches!(
            term,
            semantic_vocabulary::ScalarTerm::StructuralRead { .. }
                | semantic_vocabulary::ScalarTerm::BooleanField { .. }
                | semantic_vocabulary::ScalarTerm::IntegerField { .. }
                | semantic_vocabulary::ScalarTerm::IeeeFloatField { .. }
                | semantic_vocabulary::ScalarTerm::ViewExtent { .. }
        ) {
            return true;
        }
        pending.extend(term.operands());
    }
    false
}

pub(super) fn erased_proof_observes_storage(term: &semantic_vocabulary::ProofTerm) -> bool {
    let mut pending = vec![term];
    while let Some(term) = pending.pop() {
        match term {
            semantic_vocabulary::ProofTerm::Scalar(term)
                if erased_scalar_observes_storage(term) =>
            {
                return true;
            }
            semantic_vocabulary::ProofTerm::Construction { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.term))
            }
            // Formal admission may include reference-bearing proof carriers.
            // It is not an immutable-content certificate merely by erasure.
            semantic_vocabulary::ProofTerm::Formal { .. } => return true,
            semantic_vocabulary::ProofTerm::Scalar(_) => {}
        }
    }
    false
}
