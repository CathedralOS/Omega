use super::engine::{EffectObservations, Replay, ResultObservations, StorageFrame, Summary};
use super::*;

impl Replay<'_> {
    pub(crate) fn empty_interface_summary(
        &self,
        machine: &DescriptorMachine,
    ) -> Option<Summary> {
        if machine.returns.is_some() {
            return None;
        }
        for row in &machine.places {
            let DescriptorDefinition::Parameter {
                structural_type,
                access,
                ..
            } = row.definition
            else {
                continue;
            };
            if !self
                .slot_kind
                .writable_slots(self.types, row.place, structural_type, access)?
                .is_empty()
                || self.slot_kind.may_write_indirect_contents(
                    self.types,
                    structural_type,
                    &StructuralArgument {
                        place: row.place,
                        path: Vec::new(),
                        access,
                    },
                )
            {
                return None;
            }
        }
        let mut pending = vec![machine.id];
        let mut visited = BTreeSet::new();
        let mut invalidates_contents = false;
        while let Some(identity) = pending.pop() {
            if !visited.insert(identity) {
                continue;
            }
            let body = self.machine(identity)?;
            super::case_flow::validate_graph(self, body)?;
            invalidates_contents |= !body.unresolved_contents_writes.is_empty();
            for (_, call) in body.known_invocations() {
                invalidates_contents |= call.argument_contents.iter().any(Option::is_none);
                match &call.target {
                    DescriptorInvocationTarget::Machine(callee) => pending.push(*callee),
                    DescriptorInvocationTarget::Provider {
                        invalidates_contents: effect,
                        ..
                    } => {
                        invalidates_contents |= effect;
                    }
                    DescriptorInvocationTarget::Public {
                        effects_machine,
                        realizations,
                        invalidates_contents: effect,
                        ..
                    } => {
                        invalidates_contents |= effect;
                        pending.extend(effects_machine);
                        pending.extend(realizations);
                    }
                }
            }
        }
        Some(Summary {
            result: ResultObservations::Observed {
                slots: None,
                contents: Vec::new(),
                unknown: Vec::new(),
            },
            result_cases: Default::default(),
            effects: EffectObservations::Observed(Vec::new()),
            invalidates_contents,
            storage_frame: StorageFrame::default(),
        })
    }
}
