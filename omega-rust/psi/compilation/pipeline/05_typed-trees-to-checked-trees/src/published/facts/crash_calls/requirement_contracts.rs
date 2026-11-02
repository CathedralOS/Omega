//! A requirement call uses the complete signature retained for that call.
//! Different applications may share a declaration symbol, so that symbol
//! cannot recover the selected arguments or their crash conditions.

use checked_trees::{CallContractSignature, CrashContractCapsule, ProofFacts};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) struct RequirementCrashContract<'proof> {
    pub(super) source: &'proof CallContractSignature,
    pub(super) capsule: CrashContractCapsule,
}

impl RequirementCrashContract<'_> {
    fn location(&self) -> (SymbolHandle, SymbolHandle, usize, usize) {
        (
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
        operators: &checked_trees::CheckedOperatorFacts,
        content_conservation: &[validation::ContentConservationSourcePlan],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut rows = proof
            .call_contract_signatures
            .iter()
            .map(|(_, source)| {
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
        Ok(Self { rows })
    }

    pub(super) fn for_call(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        call: usize,
    ) -> Option<&RequirementCrashContract<'proof>> {
        self.rows
            .binary_search_by_key(&(machine, state, statement, call), |row| row.location())
            .ok()
            .map(|position| &self.rows[position])
    }
}
