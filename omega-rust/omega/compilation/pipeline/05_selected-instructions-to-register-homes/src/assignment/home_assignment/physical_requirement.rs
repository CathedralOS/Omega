//! A semantic register without physical requirements needs no manufactured home.

use crate::RegisterHomeError;
use register_homes::VirtualRegisterAllocationLegality;
use selected_instructions::{FunctionLiveRanges, VirtualLiveRange, VirtualRegisterId};

pub(super) struct ReferencedRegisters(Vec<u64>);

impl ReferencedRegisters {
    #[inline]
    fn insert(&mut self, reg: VirtualRegisterId) {
        let word = (reg.0 as usize) >> 6;
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1u64 << (reg.0 & 63);
    }

    #[inline]
    pub(super) fn contains(&self, reg: VirtualRegisterId) -> bool {
        let word = (reg.0 as usize) >> 6;
        self.0
            .get(word)
            .is_some_and(|&bits| (bits & (1u64 << (reg.0 & 63))) != 0)
    }
}

/// Collect all registers referenced in `ranges`'s relational tables once per
/// function, lazily on the first empty-span register, rather than rescanning
/// `interference` and sibling tables for every dead/spilled register.
pub(super) fn referenced_in_relations(ranges: &FunctionLiveRanges) -> ReferencedRegisters {
    let max_reg = ranges
        .virtual_registers
        .iter()
        .map(|r| r.virtual_register.0 as usize)
        .max()
        .unwrap_or(0);
    let mut referenced = ReferencedRegisters(vec![0u64; (max_reg >> 6) + 1]);
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
    referenced_cache: &mut Option<ReferencedRegisters>,
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
            .contains(register)
    {
        return Err(RegisterHomeError::NoLivePoints {
            function,
            register: register.0,
        });
    }
    Ok(())
}
