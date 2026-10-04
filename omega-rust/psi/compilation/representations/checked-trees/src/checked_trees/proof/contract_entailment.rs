use semantic_vocabulary::Proposition;
use symbols::SymbolHandle;

use crate::MachineContractCommitment;

/// One normal exit covered by a checked live-context membership judgment.
/// These source coordinates are local custody, not portable proof identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedContractMembershipExit {
    state_symbol: SymbolHandle,
    statement_index: usize,
}

impl CheckedContractMembershipExit {
    pub const fn new(state_symbol: SymbolHandle, statement_index: usize) -> Self {
        Self {
            state_symbol,
            statement_index,
        }
    }

    pub const fn state_symbol(&self) -> SymbolHandle {
        self.state_symbol
    }
    pub const fn statement_index(&self) -> usize {
        self.statement_index
    }
}

/// A local reconstruction result for an exact membership postcondition.
/// Construction alone grants no authority: consumers must rederive every exit
/// against compiler-issued checked facts and rejoin the exact source question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedContractEntailmentExitMembershipDischarge {
    machine_symbol: SymbolHandle,
    contract_position: u32,
    fact_position: u32,
    machine_contract_commitment: MachineContractCommitment,
    covered_exits: Vec<CheckedContractMembershipExit>,
}

impl CheckedContractEntailmentExitMembershipDischarge {
    pub fn new(
        machine_symbol: SymbolHandle,
        contract_position: u32,
        fact_position: u32,
        machine_contract_commitment: MachineContractCommitment,
        covered_exits: Vec<CheckedContractMembershipExit>,
    ) -> Result<Self, &'static str> {
        if !machine_symbol.is_valid()
            || machine_contract_commitment.is_zero()
            || covered_exits.is_empty()
            || covered_exits
                .iter()
                .any(|exit| !exit.state_symbol.is_valid())
        {
            return Err("exit membership discharge requires exact contract and nonempty exits");
        }
        Ok(Self {
            machine_symbol,
            contract_position,
            fact_position,
            machine_contract_commitment,
            covered_exits,
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
    pub fn covered_exits(&self) -> &[CheckedContractMembershipExit] {
        &self.covered_exits
    }
}

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
