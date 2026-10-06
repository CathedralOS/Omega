//! Invocation-owned operation identities and exact source-occurrence companions.

use crate::lowering_error::LoweringError;
use crate::terminal_identities::operation_id;
use lowered_psi::LoweredSourceCallOccurrence;
use semantic_vocabulary::{
    ObligationId, OperationId, PlaceId, ScalarType, StructuralFieldId, ValueId,
};
use terminal_psi::{Operation, ValueDeclaration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceCallCoordinate {
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement_index: usize,
    pub(crate) call_ordinal: usize,
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
    /// consuming operation, which is the lane Terminal Psi admits for a
    /// partial move; the Jump lane refuses a projected owned linear leaf.
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

/// Module-wide operation identities for one machine namespace. Machine zero
/// uses the historical one-based range; additional machines receive disjoint
/// ranges when source call-closure production composes them.
pub(crate) struct OperationBuffer {
    /// Authored structural bindings may complete at a control join rather than
    /// a call operation. Keep those places in the same state-local namespace.
    pub(crate) structural_values: Vec<(u32, StructuralBinding)>,
    /// Value-establishment obligations accompany the exact private arrival.
    pub(crate) scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
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
    /// Temporary observations available on the current emission path only.
    pub(crate) byte_lengths: Vec<(PlaceId, ValueId)>,
    pub(crate) element_lengths: Vec<(PlaceId, ValueId)>,
    /// Field-length observations reuse their dominating value so an
    /// index bound proven against an earlier `.len` discharges the read's
    /// certificate. A replacement or call may change the extent, so the
    /// verifier independently re-checks that the observation is current.
    pub(crate) field_byte_lengths: Vec<(
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        StructuralFieldId,
        ValueId,
    )>,
    pub(crate) source_calls: Vec<LoweredSourceCallOccurrence>,
}

impl OperationBuffer {
    pub(crate) fn new(identity_base: u64) -> Self {
        Self {
            structural_values: Vec::new(),
            scalar_block_invariants: Vec::new(),
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
            byte_lengths: Vec::new(),
            element_lengths: Vec::new(),
            field_byte_lengths: Vec::new(),
            source_calls: Vec::new(),
            selected_ieee_float_comparisons: Vec::new(),
            selected_integer_comparisons: Vec::new(),
            source_checked_obligations: Vec::new(),
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

    /// Drop field-length observations a field subtree store or move makes
    /// stale: every cached carrier at or under `path` on `place` loses its
    /// dominating length (over-clears sibling fields on the same carrier).
    pub(crate) fn invalidate_field_lengths_under(
        &mut self,
        place: PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
    ) {
        self.field_byte_lengths.retain(|(root, carrier, _, _)| {
            *root != place || !terminal_semantics::structural_paths_may_overlap(carrier, path)
        });
    }

    pub(crate) fn record_source_call(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        self.record_source_call_with_values(coordinate, source_site, operation, target, &[])
    }

    pub(crate) fn record_source_call_with_values(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
        source_values_before_call: &[ValueDeclaration],
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
            terminal_operation: operation,
            source_target: target,
            source_values_before_call: source_values_before_call.to_vec(),
        });
        Ok(())
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
