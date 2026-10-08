//! Canonical component-module fixtures for consumers of verified
//! descriptions.
//!
//! A consumer test needs a real described-and-verifiable component whose
//! provider-candidate catalog names exact selected-plan coordinates. These
//! builders produce such modules directly in Terminal Psi so a build route
//! can describe them with `describe_component_facts` and admit them with
//! `verify_component`; nothing here is a hand-authored inventory.

use effects::SelectedProviderPlanFacts;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId,
};
use terminal_psi::{
    Block, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalPsiIdentity, Terminator,
};

use crate::{
    ComponentDescriptionFacts, describe_component_facts, encode_component_description,
};

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine identity")
}


