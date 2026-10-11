//! A selected body's region application is constrained by inputs alone.
//! Captured coordinates remain rigid. Fresh body binders may instantiate at
//! captured requirement operands; the remaining invocation telescope stays
//! positional and universal. Results only consume the established application.

use super::carried_lifetimes::RegionIdentity;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Default)]
pub(super) struct InputRegionApplication {
    fresh: Vec<(usize, RegionIdentity)>,
    pub(super) elided: Vec<(usize, TypeReferenceHandle, RegionIdentity)>,
}

// Constraints form a relation, not a trace of which domain term was tried
// first. Equivalent successful alternatives may encounter them in any order.
impl PartialEq for InputRegionApplication {
    fn eq(&self, other: &Self) -> bool {
        self.fresh.len() == other.fresh.len()
            && self
                .fresh
                .iter()
                .all(|binding| other.fresh.contains(binding))
            && self.elided.len() == other.elided.len()
            && self
                .elided
                .iter()
                .all(|binding| other.elided.contains(binding))
    }
}

impl InputRegionApplication {
    pub(super) fn constrain(
        &mut self,
        parameter: usize,
        reference: TypeReferenceHandle,
        actual: Option<RegionIdentity>,
        required: RegionIdentity,
    ) -> bool {
        match actual {
            Some(RegionIdentity::Fresh(ordinal)) => {
                if let Some((_, region)) = self.fresh.iter().find(|(source, _)| *source == ordinal)
                {
                    *region == required
                } else {
                    self.fresh.push((ordinal, required));
                    true
                }
            }
            Some(actual) => actual == required,
            None => {
                if let Some((_, _, region)) =
                    self.elided.iter().find(|(input_parameter, input, _)| {
                        *input_parameter == parameter && *input == reference
                    })
                {
                    *region == required
                } else {
                    self.elided.push((parameter, reference, required));
                    true
                }
            }
        }
    }

    pub(super) fn finish(
        &mut self,
        actual_count: usize,
        required_count: usize,
    ) -> Result<(), &'static str> {
        let mut invocation_count = 0;
        for source in 0..actual_count {
            if self.fresh.iter().any(|(ordinal, region)| {
                *ordinal == source && matches!(region, RegionIdentity::Captured(_, _))
            }) {
                continue;
            }
            let expected = RegionIdentity::Fresh(invocation_count);
            if let Some((_, region)) = self.fresh.iter().find(|(ordinal, _)| *ordinal == source) {
                if *region != expected {
                    return Err("fresh invocation lifetime telescope is not universally refined");
                }
            } else {
                self.fresh.push((source, expected));
            }
            invocation_count += 1;
        }
        // Elided input regions can supply a universally fresh input without
        // manufacturing a captured environment or an output-source relation.
        let mut implicit = Vec::new();
        for (_, _, region) in &self.elided {
            if let RegionIdentity::Fresh(ordinal) = region
                && *ordinal >= invocation_count
                && !implicit.contains(ordinal)
            {
                implicit.push(*ordinal);
            }
        }
        if required_count != 0 && invocation_count + implicit.len() != required_count {
            if self
                .fresh
                .iter()
                .any(|(_, region)| matches!(region, RegionIdentity::Captured(_, _)))
            {
                // A binder used by a captured Type operand is not available
                // as an independent universal invocation binder as well.
                return Err("callable type does not preserve its region identity");
            }
            return Err("fresh invocation lifetime telescope is not universally refined");
        }
        Ok(())
    }

    pub(super) fn apply(&self, region: &mut RegionIdentity) {
        if let RegionIdentity::Fresh(ordinal) = region
            && let Some((_, target)) = self.fresh.iter().find(|(source, _)| source == ordinal)
        {
            *region = target.clone();
        }
    }
}
