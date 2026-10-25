//! A semantic register without physical requirements needs no manufactured home.

use std::collections::BTreeSet;

use crate::RegisterHomeError;
use register_homes::VirtualRegisterAllocationLegality;
use selected_instructions::{FunctionLiveRanges, VirtualLiveRange, VirtualRegisterId};

/// Collect all registers referenced in `ranges`'s relational tables once per
/// function, lazily on the first empty-span register, rather than rescanning
/// `interference` and sibling tables for every dead/spilled register.
pub(super) fn referenced_in_relations(ranges: &FunctionLiveRanges) -> BTreeSet<VirtualRegisterId> {
    let mut referenced = BTreeSet::new();
    for tie in &ranges.tied_pairs {
        referenced.insert(tie.use_virtual_register);
        referenced.insert(tie.def_virtual_register);
    }
    for edge in &ranges.edge_transfers {
        referenced.insert(edge.argument);
        referenced.insert(edge.parameter);
    }
    for affinity in &ranges.copy_affinities {
        referenced.insert(affinity.source);
        referenced.insert(affinity.destination);
    }
    for early in &ranges.early_clobbers {
        referenced.insert(early.def_virtual_register);
        for used in &early.uses {
            referenced.insert(used.virtual_register);
        }
    }
    for pair in &ranges.interference {
        referenced.insert(pair.lower);
        referenced.insert(pair.higher);
    }
    referenced
}

/// This checks facts, not placement. Producer and replay construct their own
/// home rosters after rejecting empty rows that still carry physical obligations.
pub(super) fn validate(
    function: usize,
    legality: &VirtualRegisterAllocationLegality,
    range: &VirtualLiveRange,
    ranges: &FunctionLiveRanges,
    referenced_cache: &mut Option<BTreeSet<VirtualRegisterId>>,
) -> Result<(), RegisterHomeError> {
    let register = legality.virtual_register;
    if register != range.virtual_register || legality.class != range.class {
        return Err(RegisterHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        });
    }
    if !legality.spans.is_empty() {
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
        || referenced_cache
            .get_or_insert_with(|| referenced_in_relations(ranges))
            .contains(&register)
    {
        return Err(RegisterHomeError::NoLivePoints {
            function,
            register: register.0,
        });
    }
    Ok(())
}
