//! A semantic register without physical requirements needs no manufactured home.

use crate::{
    FunctionLiveRanges, RegisterHomeError, VirtualLiveRange, VirtualRegisterAllocationLegality,
};

/// This checks facts, not placement. Producer and replay construct their own
/// home rosters after rejecting empty rows that still carry physical obligations.
pub(super) fn validate(
    function: usize,
    legality: &VirtualRegisterAllocationLegality,
    range: &VirtualLiveRange,
    ranges: &FunctionLiveRanges,
) -> Result<(), RegisterHomeError> {
    let register = legality.virtual_register;
    if register != range.virtual_register || legality.class != range.class {
        return Err(RegisterHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        });
    }
    if !legality.points.is_empty() {
        return Ok(());
    }
    if !legality.entry_transitions.is_empty() {
        return Err(RegisterHomeError::UnresolvedEntryTransitions {
            function,
            register: register.0,
            count: legality.entry_transitions.len(),
        });
    }
    if !legality.early_clobber_points.is_empty()
        || !range.occurrences.is_empty()
        || !range.fragments.is_empty()
        || !range.edge_connectors.is_empty()
        || !range.fixed_constraints.is_empty()
        || ranges
            .tied_pairs
            .iter()
            .any(|tie| tie.use_virtual_register == register || tie.def_virtual_register == register)
        || ranges
            .edge_transfers
            .iter()
            .any(|edge| edge.argument == register || edge.parameter == register)
        || ranges.early_clobbers.iter().any(|early| {
            early.def_virtual_register == register
                || early
                    .uses
                    .iter()
                    .any(|used| used.virtual_register == register)
        })
        || ranges
            .interference
            .iter()
            .any(|pair| pair.lower == register || pair.higher == register)
    {
        return Err(RegisterHomeError::NoLivePoints {
            function,
            register: register.0,
        });
    }
    Ok(())
}
