//! A call result's backing union from its exact pre-call contents snapshot.
//! Keep reconstruction refusals visible to consumers that report diagnostics.
use super::*;

impl DescriptorAnalysis<'_> {
    pub fn call_origins(
        &mut self,
        identity: MachineId,
        result: PlaceId,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        self.try_call_origins(identity, result).ok()
    }

    pub fn try_call_origins(
        &mut self,
        identity: MachineId,
        result: PlaceId,
    ) -> Result<Vec<DescriptorCallOrigin>, &'static str> {
        self.replay
            .prepare_states(identity)
            .ok_or("caller descriptor snapshots could not be reconstructed")?;
        let machine = self
            .replay
            .machine(identity)
            .ok_or("caller descriptor graph is absent")?;
        let mut calls = machine.calls.iter().filter(|(_, call)| {
            call.result
                .as_ref()
                .is_some_and(|location| *location == DescriptorLocation::whole(result))
        });
        let (position, call) = calls
            .next()
            .ok_or("descriptor result has no producing call")?;
        if calls.next().is_some() {
            return Err("descriptor result has multiple producing calls");
        }
        if call.argument_contents.iter().any(Option::is_none) {
            return Err("descriptor call has an unresolved argument contents coordinate");
        }
        let states = self
            .replay
            .snapshots
            .get(&identity)
            .and_then(Option::as_ref)
            .ok_or("caller descriptor snapshots disappeared")?;
        let summary = self
            .replay
            .cached_invocation_summary(call)
            .ok_or("callee descriptor summary could not be reconstructed")?;
        let mut origins = BTreeSet::new();
        // Every owned descriptor slot contributes backing, including fields
        // inside records, cases and arrays. A carrier home is not its backing.
        for slot in summary
            .result
            .as_ref()
            .ok_or("callee has no normal descriptor result summary")?
        {
            for origin in &slot.origins {
                origins.extend(
                    self.replay
                        .substitute_result(
                            machine,
                            states
                                .get(position)
                                .ok_or("descriptor call has no reachable pre-call snapshot")?,
                            call,
                            origin,
                        )
                        .ok_or("descriptor result origin could not be substituted at the call")?,
                );
            }
        }
        self.replay
            .resolved(machine, origins)
            .ok_or("descriptor result backing union could not be resolved")
    }
}
