//! Source-free target-family wire coverage; membership does not authorize selection.

use crate::sections::semantic_module::module_wire::encode_raw;
use crate::{CodecError, decode_module, encode_module};
use semantic_vocabulary::{BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId};
use terminal_psi::{
    Block, BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryRequirementKind, TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember,
    TargetMachineSupply, TerminalIdentityIndex, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator,
};

#[test]
fn target_family_round_trip_preserves_exact_tags_callables_and_supplies() {
    let machine = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine);
    module.machines.push(unit_machine(machine));
    module.identities = identity_table(10);
    let mut members = vec![TargetMachineFamilyMember {
        target: "freestanding_custom".into(),
        callable: TargetMachineCallable::Machine(machine),
        supply: TargetMachineSupply::CheckedBody,
    }];
    for (boundary_position, (target, supply)) in [
        ("linux_x86_64", TargetMachineSupply::Requirement),
        ("macos_arm64", TargetMachineSupply::TopLevelRequirement),
        ("opaque:custom", TargetMachineSupply::Boundary),
        ("target.with.dots", TargetMachineSupply::AdmissionClaim),
        ("windows_x86_64", TargetMachineSupply::ExternalRealization),
    ]
    .into_iter()
    .enumerate()
    {
        let boundary = BoundaryMachineId::new(boundary_position as u64 + 1).unwrap();
        module.boundary_machines.push(BoundaryMachineDeclaration {
            declaration: None,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
            id: boundary,
            kind: BoundaryRequirementKind::Machine,
            completion: BoundaryCompletion::NormalReturn,
            unconditional_termination: false,
            identity: TerminalIdentityIndex(
                u32::try_from(boundary.get()).expect("boundary identity"),
            ),
            attachment: None,
            owner: terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            reference_outputs: Vec::new(),
            fixed_service_reach: Vec::new(),
            owning_service: None,
            published_service_ceiling: Vec::new(),
        });
        members.push(TargetMachineFamilyMember {
            target: target.into(),
            callable: TargetMachineCallable::Boundary(boundary),
            supply,
        });
    }
    module.target_machine_families.push(TargetMachineFamily {
        canonical: members[0].callable,
        head_contract: [1; 32],
        members,
    });
    let owner = semantic_vocabulary::ServiceId::new(1).unwrap();
    module.services.push(terminal_psi::ServiceDeclaration {
        id: owner,
        identity: TerminalIdentityIndex(10),
        parents: Vec::new(),
    });
    module.boundary_machines[0].kind = BoundaryRequirementKind::Trait;
    module.boundary_machines[0].owning_service = Some(owner);
    for canonical in [
        TargetMachineCallable::Machine(machine),
        TargetMachineCallable::Boundary(BoundaryMachineId::new(1).unwrap()),
    ] {
        module.target_machine_families[0].canonical = canonical;
        let encoded_bytes = encode_module(&module).unwrap();
        let decoded_module = decode_module(&encoded_bytes).unwrap();
        assert_eq!(decoded_module, module);
        assert_eq!(encode_module(&decoded_module).unwrap(), encoded_bytes);
    }
}

#[test]
fn target_family_wire_rejects_duplicate_tags_and_missing_callables() {
    let machine = TargetMachineCallable::Machine(MachineId::new(1).unwrap());
    let boundary = TargetMachineCallable::Boundary(BoundaryMachineId::new(1).unwrap());
    for (case, members, expected_error) in [
        (
            "duplicate deployment tag",
            vec![
                TargetMachineFamilyMember {
                    target: "custom_target".into(),
                    callable: machine,
                    supply: TargetMachineSupply::CheckedBody,
                },
                TargetMachineFamilyMember {
                    target: "custom_target".into(),
                    callable: boundary,
                    supply: TargetMachineSupply::Requirement,
                },
            ],
            CodecError::NonCanonicalOrder("target-family members by deployment tag"),
        ),
        (
            "missing machine",
            vec![TargetMachineFamilyMember {
                target: "custom_target".into(),
                callable: machine,
                supply: TargetMachineSupply::CheckedBody,
            }],
            CodecError::MalformedStructuralFoundation(
                "target-family checked member has no matching body supply",
            ),
        ),
        (
            "missing boundary",
            vec![TargetMachineFamilyMember {
                target: "custom_target".into(),
                callable: boundary,
                supply: TargetMachineSupply::Requirement,
            }],
            CodecError::MalformedStructuralFoundation(
                "target-family opaque member has no matching declaration supply",
            ),
        ),
    ] {
        // The entry is a present machine outside the family, so the only
        // defect is the family member the case names.
        let entry = MachineId::new(2).unwrap();
        let mut module = TerminalModule::for_entry(entry);
        module.machines.push(unit_machine(entry));
        module.identities = identity_table(10);
        module.target_machine_families.push(TargetMachineFamily {
            canonical: members[0].callable,
            head_contract: [1; 32],
            members,
        });
        let encoded_bytes = encode_raw(&module).unwrap();
        if matches!(expected_error, CodecError::NonCanonicalOrder(_)) {
            assert_eq!(
                encode_module(&module),
                Err(expected_error.clone()),
                "{case}"
            );
        } else {
            // Encoding and identity describe the producer's module; they do not
            // admit it. External ingress must still reject missing supplies.
            assert_eq!(encode_module(&module).unwrap(), encoded_bytes, "{case}");
            assert_eq!(
                crate::semantic_fingerprint(&module).unwrap(),
                super::fingerprint_bytes(&encoded_bytes),
                "{case}"
            );
        }
        assert_eq!(decode_module(&encoded_bytes), Err(expected_error), "{case}");
    }
}

/// One resultless checked machine whose single block returns at once.
fn unit_machine(machine: MachineId) -> TerminalMachine {
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
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
fn identity_table(count: u32) -> terminal_psi::TerminalIdentityTable {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for position in 1..=count {
        identities.intern(&format!("test::identity{position}"));
    }
    identities.build()
}
