//! Scalar helper catalogs embedded beneath an existing source-owned root.
use super::super::MachineId;
use super::{CheckedTrees, LoweringError, unsupported};
use crate::emission::operation_emission::calls::CallEmissionContext;

mod discovery;
pub(crate) use discovery::checked_scalar_call_closure_with_structural_roots;

pub(crate) struct EmbeddedScalarCalls {
    terminal_machine: MachineId,
    signatures: Vec<crate::unit::attached_unit::signatures::MachineSignature>,
    structural_types: Vec<terminal_psi::StructuralTypeDeclaration>,
    domain_ids: Vec<crate::terminal_identities::StructuralDomainBinding>,
    claim_bindings: Vec<(
        language_semantics::PermissionClaimIdentity,
        semantic_vocabulary::ClaimId,
    )>,
    unreceived_results: Vec<crate::emission::claim_bindings::UnreceivedResultClaim>,
    cleanup_receivers: Vec<(symbols::SymbolHandle, semantic_vocabulary::PlaceId)>,
    pub(crate) machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
    pub(crate) requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
    pub(crate) next_call_obligation: u64,
    /// The enclosing attached Unit closure's lowered boundary roster, which
    /// nested boundary applications in composed operands resolve against.
    /// A standalone helper catalog has no such closure and keeps it empty.
    boundaries: Vec<crate::unit::attached_unit::BoundaryParameters>,
}

impl EmbeddedScalarCalls {
    pub(crate) fn from_shared(
        terminal_machine: MachineId,
        machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
        requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
        next_call_obligation: u64,
    ) -> Self {
        Self {
            terminal_machine,
            signatures: Vec::new(),
            structural_types: Vec::new(),
            domain_ids: Vec::new(),
            claim_bindings: Vec::new(),
            unreceived_results: Vec::new(),
            cleanup_receivers: Vec::new(),
            machine_ids,
            requirement_counts,
            next_call_obligation,
            boundaries: Vec::new(),
        }
    }

    /// Attach the enclosing closure's lowered boundary roster.
    pub(crate) fn with_callable_namespace(
        mut self,
        boundaries: &[crate::unit::attached_unit::BoundaryParameters],
        domain_ids: &[crate::terminal_identities::StructuralDomainBinding],
        cleanup_receivers: Vec<(symbols::SymbolHandle, semantic_vocabulary::PlaceId)>,
        signatures: &[crate::unit::attached_unit::signatures::MachineSignature],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Self {
        self.signatures = signatures.to_vec();
        self.structural_types = structural_types.to_vec();
        self.boundaries = boundaries.to_vec();
        self.domain_ids = domain_ids.to_vec();
        self.cleanup_receivers = cleanup_receivers;
        self
    }

    pub(crate) fn emission_context(&self) -> CallEmissionContext<'_> {
        CallEmissionContext {
            terminal_machine: self.terminal_machine,
            domain_ids: &self.domain_ids,
            claims: crate::emission::claim_bindings::ClaimBindings::Fixed(
                crate::emission::claim_bindings::PublishedClaims {
                    bindings: &self.claim_bindings,
                    unreceived_results: &self.unreceived_results,
                },
            ),
            cleanup_receivers: &self.cleanup_receivers,
            machine_ids: &self.machine_ids,
            signatures: &self.signatures,
            structural_types: &self.structural_types,
            requirement_counts: &self.requirement_counts,
            boundaries: &self.boundaries,
            next_obligation_identity: self.next_call_obligation,
            obligation_limit: u64::MAX,
        }
    }

    /// Composed source graphs publish their claim roster before emitting any
    /// evaluation. Every temporary and ordinary call borrows that same roster.
    pub(crate) fn bind_claim_namespace(
        &mut self,
        published: crate::emission::claim_bindings::PublishedClaims<'_>,
    ) {
        self.claim_bindings.clear();
        self.claim_bindings.extend_from_slice(published.bindings);
        self.unreceived_results.clear();
        self.unreceived_results
            .extend_from_slice(published.unreceived_results);
    }
}
