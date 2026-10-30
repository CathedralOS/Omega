//! Declaration catalog and assembly applicability wire checks.

use crate::{decode_module, encode_module};
use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
use terminal_psi::{Block, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator};

#[test]
fn declaration_catalog_round_trip_preserves_absent_default_entry() {
    let mut module = TerminalModule::declarations();
    for retain_body in [false, true] {
        if retain_body {
            module
                .machines
                .push(unit_machine(MachineId::new(1).unwrap()));
        }
        let bytes = encode_module(&module).unwrap();
        let decoded = decode_module(&bytes).unwrap();
        assert_eq!(decoded, module);
        assert!(decoded.entry.is_none());
        assert_eq!(encode_module(&decoded).unwrap(), bytes);
    }
}

pub(super) fn unit_machine(machine: MachineId) -> TerminalMachine {
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: machine,
        attachment: None,
        declaration: Some(terminal_psi::CheckedMachineDeclaration {
            semantic_declaration: Default::default(),
            parameter_types: Vec::new(),
            callable: Default::default(),
            result_type: Default::default(),
            machine: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            state: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(2)),
            contract: [7; 32],
            specialization: None,
        }),
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block {
            id: block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        }],
        contract: terminal_psi::MachineContract::empty(ContractId::new(1).unwrap()),
    }
}

/// A table whose first `count` real entries are distinct texts, so every
/// fixed identity index this file names below `count` resolves.
pub(super) fn identity_table(count: u32) -> terminal_psi::TerminalIdentityTable {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for position in 1..=count {
        identities.intern(&format!("test::identity{position}"));
    }
    identities.build()
}

#[test]
fn assembly_catalog_target_demand_survives_wire_and_changes_identity() {
    let machine = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine);
    module.machines.push(unit_machine(machine));
    let portable = encode_module(&module).unwrap();
    module.machines[0].assembly_target_requirement =
        terminal_psi::RegisterTargetApplicability::X86_64;
    let x86 = encode_module(&module).unwrap();
    assert_ne!(portable, x86);
    assert_eq!(decode_module(&x86).unwrap(), module);
    module.machines[0].assembly_target_requirement =
        terminal_psi::RegisterTargetApplicability::Aarch64;
    let arm = encode_module(&module).unwrap();
    assert_ne!(x86, arm);
    assert_eq!(decode_module(&arm).unwrap(), module);
    let mut old = portable;
    old[8..10].copy_from_slice(&164_u16.to_le_bytes());
    assert!(matches!(
        decode_module(&old),
        Err(CodecError::UnsupportedFormatMarker(164))
    ));
}
