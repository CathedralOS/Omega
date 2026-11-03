//! A requirement call uses the complete signature retained for that call.
//! Calls with different arguments can name the same declaration, so its symbol
//! alone cannot recover those arguments or their crash conditions.

use checked_trees::{CallContractSignature, CrashContractCapsule, ProofFacts};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) struct RequirementCrashContract<'proof> {
    pub(super) source: &'proof CallContractSignature,
    pub(super) capsule: CrashContractCapsule,
}

impl RequirementCrashContract<'_> {
    fn location(&self) -> (u32, u32, u32, u32, usize, usize) {
        call_location(
            self.source.caller_machine_symbol,
            self.source.caller_state_symbol,
            self.source.statement_index,
            self.source.call_ordinal,
        )
    }
}

pub(super) struct RequirementCrashContracts<'proof> {
    rows: Vec<RequirementCrashContract<'proof>>,
}

impl<'proof> RequirementCrashContracts<'proof> {
    pub(super) fn prepare(
        program: &dyn validation::CheckingInput,
        proof: &'proof ProofFacts,
        flow: &checked_trees::FlowFacts,
        operators: &checked_trees::CheckedOperatorFacts,
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut applications = Vec::new();
        for (_, call) in proof.contract_calls.iter() {
            if !call.signature.is_valid() {
                continue;
            }
            if !proof.call_contract_signatures.is_valid(call.signature) {
                return Err(vec![Diagnostic::error(
                    "a requirement call lost its retained public signature",
                )]);
            }
            let source = proof.call_contract_signatures.get(call.signature);
            if source.caller_machine_symbol != call.caller_machine_symbol
                || source.caller_state_symbol != call.caller_state_symbol
                || source.statement_index != call.statement_index
                || source.call_ordinal != call.call_ordinal
            {
                return Err(vec![Diagnostic::error(
                    "a requirement call retained another call's public signature",
                )]);
            }
            applications.push(call.signature);
        }
        applications.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
        applications.dedup();
        let mut rows = applications
            .into_iter()
            .map(|application| {
                let source = proof.call_contract_signatures.get(application);
                let capsule = super::super::crash_plan_facts::build_signature_crash_capsule(
                    program,
                    source.declaring_trait,
                    source.signature.symbol,
                    &source.signature,
                    content_conservation,
                    operators,
                )?;
                Ok(RequirementCrashContract { source, capsule })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()
            .map_err(|diagnostic| vec![diagnostic])?;
        rows.sort_unstable_by_key(RequirementCrashContract::location);
        if rows
            .windows(2)
            .any(|pair| pair[0].location() == pair[1].location())
        {
            return Err(vec![Diagnostic::error(
                "a source call retained conflicting public requirement contracts",
            )]);
        }
        let prepared = Self { rows };
        for (_, state) in flow.control.states.iter() {
            for call in flow.control.calls.span_or_empty(state.calls) {
                let Some(site) = validation::find_call_site(
                    program,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let is_requirement = site.static_requirement_dispatch().is_some()
                    || validation::selected_requirement_signature(
                        program,
                        site.target_symbol(program),
                    )
                    .is_some();
                if is_requirement
                    && prepared
                        .for_call(
                            state.machine_symbol,
                            state.state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                        )
                        .is_none()
                {
                    return Err(vec![Diagnostic::error(
                        "a source requirement call has no retained public contract",
                    )]);
                }
            }
        }
        Ok(prepared)
    }

    pub(super) fn for_call(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        call: usize,
    ) -> Option<&RequirementCrashContract<'proof>> {
        self.rows
            .binary_search_by_key(&call_location(machine, state, statement, call), |row| {
                row.location()
            })
            .ok()
            .map(|position| &self.rows[position])
    }
}

fn call_location(
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    call: usize,
) -> (u32, u32, u32, u32, usize, usize) {
    (
        machine.arena_index(),
        machine.generation(),
        state.arena_index(),
        state.generation(),
        statement,
        call,
    )
}
