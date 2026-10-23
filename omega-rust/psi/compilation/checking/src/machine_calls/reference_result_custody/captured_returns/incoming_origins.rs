//! Join returned addresses to the exact stored ingress descriptor they use.
//!
//! A shared output lifetime conservatively depends on several input leaves.
//! Those dependencies do not make their addresses interchangeable. Select the
//! admitted incoming identity before comparing its referent-relative region;
//! otherwise the first same-typed sibling can mask the actual returning leaf.

use super::{Captures, TypeReferenceHandle, frame_origin_fits_loan};

pub(super) struct IncomingLeaf {
    pub(super) carrier: TypeReferenceHandle,
    pub(super) reference: super::super::DeclaredCarrierReference,
    pub(super) segments: Vec<facts::PlaceSegment>,
}

impl Captures<'_> {
    pub(super) fn incoming_leaf(
        &self,
        state: &typed_trees::state::State,
        loan: &checked_trees::BorrowReturnedLoanFact,
    ) -> Option<IncomingLeaf> {
        if !self
            .facts
            .borrow
            .incoming_references
            .is_valid(loan.incoming_reference)
        {
            return None;
        }
        let incoming = self
            .facts
            .borrow
            .incoming_references
            .get(loan.incoming_reference);
        let symbol = incoming.parameter_symbol();
        if incoming.state_symbol != state.symbol
            || incoming.machine_symbol != self.flow(state.symbol)?.machine_symbol
            || loan.root != facts::PlaceRoot::Symbol(symbol)
        {
            return None;
        }
        let parameter = self.program.state_parameter_by_symbol(state, symbol)?;
        let carrier = super::super::input_carrier_type(self.program, parameter.type_reference)?;
        if carrier != incoming.carrier_type {
            return None;
        }
        let owner = self.facts.borrow.owner_segments.span(incoming.owner_path)?;
        let reference = super::super::declared_carrier_references(self.program, carrier)?
            .into_iter()
            .find(|reference| {
                super::super::origin_owner_path(
                    self.program,
                    carrier,
                    reference.path.iter().cloned(),
                )
                .as_deref()
                    == Some(owner)
            })?;
        let segments = super::super::origin_place_segments(
            self.program,
            carrier,
            reference.path.iter().cloned(),
        )?;
        Some(IncomingLeaf {
            carrier,
            reference,
            segments,
        })
    }

    pub(super) fn origin_matches_returned_loan(
        &self,
        state: &typed_trees::state::State,
        loan: &checked_trees::BorrowReturnedLoanFact,
        origin: &crate::LocalWriteSource,
    ) -> bool {
        if loan.root != facts::PlaceRoot::Symbol(origin.root) {
            return false;
        }
        let Some(region) = self.facts.borrow.access_segments.span(loan.segments) else {
            return false;
        };
        if !loan.incoming_reference.is_valid() {
            return frame_origin_fits_loan(origin, region);
        }
        let Some(incoming) = self.incoming_leaf(state, loan) else {
            return false;
        };
        let (root, captured) = origin
            .recasts
            .first()
            .map_or((origin.root, origin.segments.as_slice()), |formation| {
                (formation.root, formation.segments.as_slice())
            });
        let Some(relative) = captured.strip_prefix(incoming.segments.as_slice()) else {
            return false;
        };
        if root != origin.root {
            return false;
        }
        // Region coordinates begin after the stored descriptor. Keep lifetime
        // dependencies intact while selecting only this actual address origin.
        let mut referent = origin.clone();
        referent.segments = relative.to_vec();
        referent.recasts.clear();
        frame_origin_fits_loan(&referent, region)
    }
}
