use psi_core::Proposition;
use psi_symbols::SymbolHandle;

use crate::MachineContractCommitment;

/// Kernel-checked discharge of one exact contract-entailment stand-down by
/// citing an authored machine `requires` assumption.
///
/// The propositions are source-independent kernel terms. The machine and
/// contract coordinates remain compiler-private joins to the retained typed
/// program, while the strong contract commitment prevents a valid certificate
/// from being replayed against changed callable semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedContractEntailmentAssumptionDischarge {
    machine_symbol: SymbolHandle,
    contract_position: u32,
    fact_position: u32,
    machine_contract_commitment: MachineContractCommitment,
    assumptions: Vec<Proposition>,
    goal: Proposition,
    selected_assumption_position: u32,
}

impl CheckedContractEntailmentAssumptionDischarge {
    pub fn new(
        machine_symbol: SymbolHandle,
        contract_position: u32,
        fact_position: u32,
        machine_contract_commitment: MachineContractCommitment,
        assumptions: Vec<Proposition>,
        goal: Proposition,
        selected_assumption_position: u32,
    ) -> Result<Self, &'static str> {
        if !machine_symbol.is_valid() {
            return Err("contract-entailment discharge requires a valid machine symbol");
        }
        if machine_contract_commitment.is_zero() {
            return Err("contract-entailment discharge requires a nonzero contract commitment");
        }
        if usize::try_from(selected_assumption_position)
            .ok()
            .is_none_or(|position| position >= assumptions.len())
        {
            return Err("contract-entailment discharge selects a missing assumption");
        }
        Ok(Self {
            machine_symbol,
            contract_position,
            fact_position,
            machine_contract_commitment,
            assumptions,
            goal,
            selected_assumption_position,
        })
    }

    pub const fn machine_symbol(&self) -> SymbolHandle {
        self.machine_symbol
    }

    pub const fn contract_position(&self) -> u32 {
        self.contract_position
    }

    pub const fn fact_position(&self) -> u32 {
        self.fact_position
    }

    pub const fn machine_contract_commitment(&self) -> MachineContractCommitment {
        self.machine_contract_commitment
    }

    pub fn assumptions(&self) -> &[Proposition] {
        &self.assumptions
    }

    pub const fn goal(&self) -> &Proposition {
        &self.goal
    }

    pub const fn selected_assumption_position(&self) -> u32 {
        self.selected_assumption_position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_rejects_zero_contract_commitment() {
        let machine = SymbolHandle::from_parts(1, 1);
        let result = CheckedContractEntailmentAssumptionDischarge::new(
            machine,
            0,
            0,
            MachineContractCommitment::from_digest([0; 32]),
            vec![Proposition::Truth],
            Proposition::Truth,
            0,
        );
        assert_eq!(
            result.expect_err("zero commitment must reject"),
            "contract-entailment discharge requires a nonzero contract commitment"
        );
    }
}
