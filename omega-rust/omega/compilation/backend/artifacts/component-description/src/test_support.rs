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

/// A canonical Terminal module with one Unit entry machine and nothing else:
/// it describes a component that exports no realization.
pub fn bare_module() -> TerminalModule {
    TerminalModule {
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            nominal_cleanup_subject: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("block identity"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: BlockId::new(1).expect("block identity"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).expect("edge identity"),
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: terminal_psi::MachineContract::empty(
                ContractId::new(1).expect("contract identity"),
            ),
        }],
        ..TerminalModule::for_entry(machine_id(1))
    }
}
/// The Terminal Psi identity a compilation of `module` observes: the exact
/// subject a consumer must supply to admit the module's description.
pub fn module_subject(module: &TerminalModule) -> TerminalPsiIdentity {
    terminal_codec::terminal_psi_identity(module).expect("module identity")
}

/// Describe one component module the way a producing compilation would:
/// canonical artifact, identity optimization record, no sealed requirement
/// of its own, and no native realization facts. Returns the encoded
/// description bytes a consumer verifies.
pub fn describe_module(module: &TerminalModule) -> Vec<u8> {
    let record = terminal_codec::build_identity_optimization_execution_record(module)
        .expect("identity optimization record");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_parts(module.clone(), record, None)
            .expect("canonical artifact");
    let selected = SelectedProviderPlanFacts::from_selected_plans(Vec::new())
        .expect("a provider component seals no requirement of its own");
    let description = describe_component_facts(ComponentDescriptionFacts {
        artifact: &artifact,
        selected_provider_plans: &selected,
        component_progress: None,
        stack_demand: None,
        realization_identity: None,
    })
    .expect("component description");
    encode_component_description(&description)
}
