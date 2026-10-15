//! Invocation-owned operation identities and exact source-occurrence companions.
//!
//! Length observations belong to a control-flow scope, not to the append-only
//! operation roster. Successors can be staged before their guard is emitted;
//! restoring the whole observation scope prevents a guard or sibling from
//! borrowing a value defined only in one of those successors.

use crate::lowering_error::LoweringError;
use crate::terminal_identities::operation_id;
use lowered_psi::{
    LoweredSourceCallOccurrence, LoweredSourceCallSite, LoweredSourceFrontierBindings,
    LoweredSourceScalarBinding,
};
use semantic_vocabulary::{
    ObligationId, OperationId, PlaceId, ScalarType, StructuralFieldId, ValueId,
};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceCallCoordinate {
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement_index: usize,
    pub(crate) call_ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceQuotientCall {
    pub(crate) value: checked_trees::CheckedStructuralValueHandle,
    pub(crate) call: OperationId,
}

/// Evaluated operands of a domain-ending call, before its owner allocates the
/// terminating edge. Keeping this separate from operations prevents a Unit
/// result, normal completion receipts or implicit cleanup from being invented.
pub(crate) struct PendingExternalCompletion {
    pub(crate) coordinate: SourceCallCoordinate,
    pub(crate) source_site: Option<checked_trees::NominalMachineUseSite>,
    pub(crate) target: symbols::SymbolHandle,
    pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
    pub(crate) kind: semantic_vocabulary::ExternalCompletionKind,
    pub(crate) arguments: Vec<ValueId>,
    pub(crate) structural_arguments: Vec<terminal_psi::StructuralArgument>,
    pub(crate) requirement_obligations: Vec<ObligationId>,
}

/// One authored structural binding. Literal storage is established by a Unit
/// operation; it does not acquire a structural operation result or its claims.
/// A projected alias establishes no storage at all: transferring an owned
/// child consumes that subtree's custody and preserves its siblings
/// (`wiki/spec/terminal-psi/structural_access.md`), so the binding names a
/// path under an already established place rather than producing one.
#[derive(Debug, Clone)]
pub(crate) enum StructuralBinding {
    OperationResult(terminal_psi::StructuralOperationResult),
    ByteSequenceLiteral {
        place: PlaceId,
        structural_type: semantic_vocabulary::StructuralTypeId,
        declaration_ordinal: u32,
    },
    /// One authored binding of an owned subtree of a live root. The alias
    /// carries the composed path so every later use projects it at its own
    /// consuming operation, including state edges. The place and path are
    /// one value coordinate; forwarding the place alone moves its parent.
    ProjectedAlias {
        place: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        structural_type: semantic_vocabulary::StructuralTypeId,
        multiplicity: terminal_psi::StructuralMultiplicity,
        /// The claims the root already publishes under `path`, rebased onto
        /// the aliased leaf. The alias mints nothing; it names where the
        /// root's own bindings live so a consuming operation transfers
        /// exactly those.
        claims: Vec<terminal_psi::StructuralResultClaimBinding>,
    },
}

impl StructuralBinding {
    pub(crate) fn place(&self) -> PlaceId {
        match self {
            Self::OperationResult(result) => result.place,
            Self::ByteSequenceLiteral { place, .. } | Self::ProjectedAlias { place, .. } => *place,
        }
    }

    /// The path the binding names under `place`. Only a projected alias has
    /// one: an established value occupies its whole place.
    pub(crate) fn path(&self) -> &[terminal_psi::StructuralPathSegment] {
        match self {
            Self::OperationResult(_) | Self::ByteSequenceLiteral { .. } => &[],
            Self::ProjectedAlias { path, .. } => path,
        }
    }

    pub(crate) fn structural_type(&self) -> semantic_vocabulary::StructuralTypeId {
        match self {
            Self::OperationResult(result) => result.structural_type,
            Self::ByteSequenceLiteral {
                structural_type, ..
            }
            | Self::ProjectedAlias {
                structural_type, ..
            } => *structural_type,
        }
    }

    pub(crate) fn multiplicity(&self) -> terminal_psi::StructuralMultiplicity {
        match self {
            Self::OperationResult(result) => result.multiplicity,
            Self::ByteSequenceLiteral { .. } => terminal_psi::StructuralMultiplicity::Unrestricted,
            Self::ProjectedAlias { multiplicity, .. } => *multiplicity,
        }
    }

    pub(crate) fn claims(&self) -> &[terminal_psi::StructuralResultClaimBinding] {
        match self {
            Self::OperationResult(result) => &result.claims,
            Self::ByteSequenceLiteral { .. } => &[],
            // The alias mints no claim. These are the root's own bindings,
            // rebased onto the aliased leaf, so a consuming operation
            // transfers exactly the obligations the leaf carries.
            Self::ProjectedAlias { claims, .. } => claims,
        }
    }

    pub(crate) fn operation_result(
        &self,
    ) -> Result<&terminal_psi::StructuralOperationResult, LoweringError> {
        match self {
            Self::OperationResult(result) => Ok(result),
            Self::ByteSequenceLiteral { .. } => Err(LoweringError::Unsupported(
                "literal binding has no structural operation result",
            )),
            Self::ProjectedAlias { .. } => Err(LoweringError::Unsupported(
                "projected alias binding has no structural operation result",
            )),
        }
    }
}

impl From<terminal_psi::StructuralOperationResult> for StructuralBinding {
    fn from(result: terminal_psi::StructuralOperationResult) -> Self {
        Self::OperationResult(result)
    }
}

/// The scalar operation result most emitted operations produce: the declared
/// value carries no qualification membership.
pub(crate) fn scalar_result_declaration(
    id: ValueId,
    scalar_type: ScalarType,
) -> terminal_psi::OperationResult {
    terminal_psi::OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id,
        scalar_type,
    })
}

/// The structural operation result an establishing operation produces:
/// declared memberships arrive through qualification establishments and
/// claims, so both rosters plus the projected list start empty.
pub(crate) fn structural_result_declaration(
    place: PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
) -> terminal_psi::OperationResult {
    terminal_psi::OperationResult::Structural(terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type,
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

/// Length values available on one emission path. Carrier kinds retain their
/// distinct lookup and mutation rules, but branch entry and exit carry this
/// state as a whole: every reused value must dominate its consumer.
#[derive(Clone, Default)]
pub(crate) struct CollectionLengthObservations {
    pub(crate) bytes: Vec<(PlaceId, ValueId)>,
    pub(crate) elements: Vec<(terminal_psi::StructuralArgument, ValueId)>,
    pub(crate) field_bytes: Vec<(
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        StructuralFieldId,
        ValueId,
    )>,
}

impl CollectionLengthObservations {
    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
        self.elements.clear();
        self.field_bytes.clear();
    }
}

/// Module-wide operation identities for one machine namespace. Machine zero
/// uses the historical one-based range; additional machines receive disjoint
/// ranges when source call-closure production composes them.
/// Source occurrence paired with the ordinary copy actually emitted for it.
#[derive(Debug, Clone)]
pub(crate) struct SourceQuotientAdaptation {
    pub(crate) value: checked_trees::CheckedStructuralValueHandle,
    pub(crate) copy: OperationId,
    pub(crate) source: PlaceId,
    pub(crate) result: PlaceId,
    pub(crate) source_type: semantic_vocabulary::StructuralTypeId,
    pub(crate) result_type: semantic_vocabulary::StructuralTypeId,
}

pub(crate) struct OperationBuffer {
    pub(crate) claim_introductions: Vec<crate::emission::claim_bindings::ClaimIntroduction>,
    pub(crate) selected_cleanup_groups:
        Vec<crate::emission::selected_continuations::SelectedCleanupGroup>,
    pub(crate) external_completion: Option<PendingExternalCompletion>,
    /// Authored structural bindings may complete at a control join rather than
    /// a call operation. Keep those places in the same state-local namespace.
    pub(crate) structural_values: Vec<(u32, StructuralBinding)>,
    /// Physical homes in establishment order, including anonymous operands and
    /// completed joins. Cleanup uses this chronology, never local declarations
    /// alone; each selected exit still supplies its own disposal authority.
    pub(crate) structural_establishments: Vec<PlaceId>,
    /// Literal operations have Unit results, so their emitting owner retains
    /// the carrier here rather than recovering it from a source-call shape.
    pub(crate) literal_types: Vec<(PlaceId, semantic_vocabulary::StructuralTypeId)>,
    /// Value-establishment obligations accompany the exact private arrival.
    pub(crate) scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
    /// Source-proved singleton values keyed by their exact selector occurrence
    /// and immutable SSA value. These refine overlap only, not bounds or authority.
    pub(crate) scalar_index_coordinates: Vec<(ObligationId, ValueId, u64)>,
    pub(crate) selected_ieee_float_comparisons:
        Vec<lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(crate) selected_integer_comparisons:
        Vec<lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
    /// Formation obligations of operations whose source occurrence checking
    /// decided. The module's proof bundle carries each as source-checked
    /// evidence; a route that drops them leaves certificate search to prove
    /// those obligations instead.
    pub(crate) source_checked_obligations: Vec<ObligationId>,
    pub(crate) next_identity: u64,
    pub(crate) operations: Vec<Operation>,
    pub(crate) length_observations: CollectionLengthObservations,
    pub(crate) source_calls: Vec<LoweredSourceCallOccurrence>,
    pub(crate) quotient_adaptations: Vec<SourceQuotientAdaptation>,
    pub(crate) quotient_calls: Vec<SourceQuotientCall>,
    /// Source-only associations created by the actual reference emitter.
    /// The source operand belongs to its formation epoch, not current storage.
    pub(crate) loan_captures: Vec<lowered_psi::LoweredSourceLoanCapture>,
    pub(crate) loan_completions: Vec<terminal_psi::LoanCompletion>,
    /// The module's selector interfaces and tables, by identity.
    pub(crate) dynamic_dispatch:
        std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
}

impl OperationBuffer {
    /// Calls introduce the returned caller-local names except exact input
    /// claims their checked transfer roster forwards through this invocation.
    pub(crate) fn push(&mut self, operation: Operation) {
        use crate::emission::claim_bindings::ClaimIntroduction;
        let transferred = match &operation.kind {
            OperationKind::CallDynamic {
                claim_transfers, ..
            }
            | OperationKind::CallUnit {
                claim_transfers, ..
            }
            | OperationKind::CallStructuralScalar {
                claim_transfers, ..
            }
            | OperationKind::CallStructural {
                claim_transfers, ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                claim_transfers, ..
            } => Some(claim_transfers.as_slice()),
            OperationKind::Call { .. } | OperationKind::BoundaryCall { .. } => Some(&[][..]),
            _ => None,
        };
        if let (Some(transferred), Some(result)) = (transferred, operation.result.as_structural()) {
            self.claim_introductions.extend(
                result
                    .claims
                    .iter()
                    .filter(|binding| !transferred.iter().any(|input| input.claim == binding.claim))
                    .map(|binding| ClaimIntroduction {
                        operation: operation.id,
                        claim: binding.claim,
                    }),
            );
        }
        if let Some(result) = operation.result.structural() {
            self.structural_establishments.push(result.place);
        } else if let OperationKind::EstablishTrivialAffineLocal { destination } = &operation.kind {
            // This owner has no operation result, but its actual establishment
            // still belongs to the same cleanup chronology. Checked completion
            // supplies its disposal authority independently.
            self.structural_establishments.push(*destination);
        }
        self.operations.push(operation);
    }

    pub(crate) fn bind_structural_value(&mut self, ordinal: u32, value: StructuralBinding) {
        // A control join or literal can complete without producing an operation.
        // Aliases name existing storage and do not establish another home.
        if value.path().is_empty() && !self.structural_establishments.contains(&value.place()) {
            self.structural_establishments.push(value.place());
        }
        self.structural_values.push((ordinal, value));
    }

    /// Publish the claims transported by completed constructor operands.
    /// The operation owns the result home; its receiving binding is separate.
    pub(crate) fn set_structural_result_claims(
        &mut self,
        place: PlaceId,
        claims: Vec<terminal_psi::StructuralResultClaimBinding>,
    ) -> Result<(), LoweringError> {
        let result = self
            .operations
            .iter_mut()
            .rev()
            .find_map(|operation| match &mut operation.result {
                OperationResult::Structural(result) if result.place == place => Some(result),
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "constructed value lost its result claim home",
            ))?;
        result.claims = claims;
        Ok(())
    }

    /// Allocate the read obligation that canonically certifies the next
    /// operation's bounds, recording it when `bound_decided` requires proof.
    pub(crate) fn push_read_obligation(
        &mut self,
        bound_decided: bool,
    ) -> semantic_vocabulary::ObligationId {
        let obligation = crate::terminal_identities::obligation_id(
            self.next_identity
                .checked_add(1)
                .expect("read obligation follows its operation identity"),
        );
        if bound_decided {
            self.source_checked_obligations.push(obligation);
        }
        obligation
    }

    pub(crate) fn new(
        identity_base: u64,
        dynamic_dispatch: std::sync::Arc<crate::unit::dynamic_dispatch::DynamicDispatchIndex>,
    ) -> Self {
        Self {
            external_completion: None,
            dynamic_dispatch,
            structural_values: Vec::new(),
            structural_establishments: Vec::new(),
            literal_types: Vec::new(),
            scalar_block_invariants: Vec::new(),
            scalar_index_coordinates: Vec::new(),
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
            length_observations: CollectionLengthObservations::default(),
            source_calls: Vec::new(),
            quotient_adaptations: Vec::new(),
            quotient_calls: Vec::new(),
            loan_captures: Vec::new(),
            loan_completions: Vec::new(),
            selected_ieee_float_comparisons: Vec::new(),
            selected_integer_comparisons: Vec::new(),
            source_checked_obligations: Vec::new(),
            claim_introductions: Vec::new(),
            selected_cleanup_groups: Vec::new(),
        }
    }

    pub(crate) fn allocate(&mut self) -> OperationId {
        let id = operation_id(self.next_identity);
        self.next_identity = self
            .next_identity
            .checked_add(1)
            .expect("terminal operation identities advance");
        id
    }

    /// Allocate and push an operation carrying no reach binding or
    /// suspension demand.
    pub(crate) fn push_leaf(
        &mut self,
        result: OperationResult,
        kind: OperationKind,
    ) -> OperationId {
        let id = self.allocate();
        self.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result,
            kind,
        });
        id
    }

    /// Drop live storage observations a subtree store or move makes stale.
    /// Whole immutable byte-view extents remain captured values; field lengths
    /// and element descriptors may have changed beneath the written path.
    pub(crate) fn invalidate_field_lengths_under(
        &mut self,
        place: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
    ) {
        self.length_observations.elements.retain(|(source, _)| {
            source.place != place
                || !terminal_semantics::structural_paths_may_overlap(&source.path, path)
        });
        self.length_observations
            .field_bytes
            .retain(|(root, carrier, _, _)| {
                *root != place || !terminal_semantics::structural_paths_may_overlap(carrier, path)
            });
    }

    pub(crate) fn record_source_call(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
        frontier_before_call: &LoweredSourceFrontierBindings,
    ) -> Result<(), LoweringError> {
        self.record_source_call_site(
            coordinate,
            source_site,
            LoweredSourceCallSite::Operation(operation),
            target,
            frontier_before_call,
        )
    }

    pub(crate) fn record_source_call_with_values(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
        source_values_before_call: &[ValueDeclaration],
    ) -> Result<(), LoweringError> {
        let bindings = source_values_before_call
            .iter()
            .enumerate()
            .map(|(source_position, value)| LoweredSourceScalarBinding {
                source_position,
                value: *value,
            })
            .collect::<Vec<_>>();
        self.record_source_call(
            coordinate,
            source_site,
            operation,
            target,
            &LoweredSourceFrontierBindings {
                scalars: bindings,
                ..Default::default()
            },
        )
    }

    fn record_source_call_site(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        terminal_site: LoweredSourceCallSite,
        target: symbols::SymbolHandle,
        frontier_before_call: &LoweredSourceFrontierBindings,
    ) -> Result<(), LoweringError> {
        if self.source_calls.iter().any(|existing| {
            existing.source_state == coordinate.state
                && existing.statement_index == coordinate.statement_index
                && existing.call_ordinal == coordinate.call_ordinal
        }) {
            return Err(LoweringError::DuplicateContentPartitionProducerCoordinate);
        }
        self.source_calls.push(LoweredSourceCallOccurrence {
            source_site,
            source_state: coordinate.state,
            statement_index: coordinate.statement_index,
            call_ordinal: coordinate.call_ordinal,
            terminal_site,
            source_target: target,
            frontier_before_call: frontier_before_call.clone(),
        });
        Ok(())
    }

    /// Seal abandonment on the current evaluation path. The empty audit lower
    /// bound does not assert an empty frontier: verification reconstructs live
    /// custody, and runtime domain delegation is a separate admission demand.
    pub(crate) fn take_external_completion(
        &mut self,
        next_edge: &mut u64,
    ) -> Result<Option<terminal_psi::Terminator>, LoweringError> {
        let Some(completion) = self.external_completion.take() else {
            return Ok(None);
        };
        let edge = crate::terminal_identities::edge_id(crate::terminal_identities::allocate_dense(
            next_edge,
        )?);
        self.record_source_call_site(
            completion.coordinate,
            completion.source_site,
            LoweredSourceCallSite::ExternalCompletion(edge),
            completion.target,
            &LoweredSourceFrontierBindings::default(),
        )?;
        Ok(Some(terminal_psi::Terminator::ExternalCompletion {
            edge,
            boundary: completion.boundary,
            kind: completion.kind,
            arguments: completion.arguments,
            structural_arguments: completion.structural_arguments,
            requirement_obligations: completion.requirement_obligations,
            frontier_lower_bound: Vec::new(),
        }))
    }
}

impl std::ops::Deref for OperationBuffer {
    type Target = Vec<Operation>;

    fn deref(&self) -> &Self::Target {
        &self.operations
    }
}

impl std::ops::DerefMut for OperationBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.operations
    }
}
