//! Completed values need their declared current claims, independently of the
//! returning edge's authored roster or the existence of an allocated home.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralMultiplicity, StructuralPathSegment};

impl TerminalExecution<'_> {
    pub(crate) fn validate_complete_owned_value_claims(
        &self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        region: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let root = self.structural_value(place)?;
        if root.structural_type != structural_type {
            return Err(malformed());
        }
        let types = self.structural_types();
        terminal_semantics::owned_structure::validate_complete_claim_frontier(
            |identity| types.get(&identity),
            structural_type,
            region,
            self.live_claims.values().filter_map(|claim| {
                (claim.place == Some(place)
                    && claim.multiplicity == Some(StructuralMultiplicity::Linear))
                .then_some(claim.path.as_slice())
            }),
            &|path, case| {
                crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
                    &types,
                    root,
                    self.structural_cases.as_map(),
                    path,
                ) == Some(case)
            },
        )
        .map_err(|reason| TerminalInterpretError::VerifiedOperationContractInvalid { reason })
    }
}
