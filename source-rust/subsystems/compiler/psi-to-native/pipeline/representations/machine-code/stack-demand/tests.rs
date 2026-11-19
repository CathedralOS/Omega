use super::*;
use semantic_vocabulary::{MachineId, OperationId};
use target::NativeTarget;
use target_vocabulary::CallSiteOwner;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

type Error = StackDemandError;

fn machine(id: u64) -> MachineId {
    MachineId::new(id).unwrap()
}

fn program() -> PhysicalStackProgram {
    PhysicalStackProgram {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([5; 32]),
        },
        target: NativeTarget::linux_x64(),
        functions: vec![
            PhysicalStackFunction {
                machine: machine(1),
                local: LocalStackExtent::Finite(32),
                calls: vec![PhysicalStackCall {
                    target: machine(2),
                    owner: CallSiteOwner::Operation(OperationId::new(3).unwrap()),
                    caller_live_bytes: 48,
                }],
                foreign_calls: vec![],
            },
            PhysicalStackFunction {
                machine: machine(2),
                local: LocalStackExtent::Finite(64),
                calls: vec![],
                foreign_calls: vec![],
            },
        ],
    }
}

#[test]
fn physical_frames_compose_before_bytes_exist_and_hosted_demand_remains_unbounded() {
    let mut program = program();
    let demand = derive_physical_stack_demand(&program, machine(1), vec![]).unwrap();
    assert_eq!(demand.ceiling_bytes(), Some(112));
    assert_eq!(
        demand
            .contributing_machines()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [machine(1), machine(2)]
    );
    let entry = demand.with_entry_cost(24, 32).unwrap();
    assert_eq!(entry.ceiling_bytes(), Some(136));
    assert_eq!(entry.stack_alignment(), 32);
    program.functions[1]
        .foreign_calls
        .push(PhysicalForeignStackCall {
            owner: CallSiteOwner::Operation(OperationId::new(4).unwrap()),
            caller_live_bytes: 80,
            extent: ForeignStackExtent::Hosted {
                receipt: Some([7; 32]),
            },
            invoked_callbacks: vec![],
        });
    let hosted = derive_physical_stack_demand(&program, machine(1), vec![]).unwrap();
    assert_eq!(hosted.known_stack_bytes(), 128);
    assert_eq!(hosted.ceiling_bytes(), None);
    assert!(hosted.hosted_native_bindings().contains(&[7; 32]));
    program.functions[1].foreign_calls[0].extent =
        ForeignStackExtent::CompilerImport(target_vocabulary::CompilerImport::WindowsProcessExit);
    assert_eq!(
        derive_physical_stack_demand(&program, machine(1), vec![]),
        Err(Error::ConflictingTerminalStackEvidence(machine(1)))
    );
    program.target = target::TargetProfile::WindowsX64.native_target();
    let compiler_import = derive_physical_stack_demand(&program, machine(1), vec![]).unwrap();
    assert_eq!(compiler_import.ceiling_bytes(), None);
    assert!(compiler_import.hosted_native_bindings().is_empty());
    program.functions[1].foreign_calls[0].extent = ForeignStackExtent::Hosted { receipt: None };
    assert_eq!(
        derive_physical_stack_demand(&program, machine(1), vec![]),
        Err(Error::InvalidPrivateFunctionIdentity)
    );
}

#[test]
fn missing_cyclic_and_unaccounted_calls_never_become_zero_demand() {
    let original = program();
    let mut changed = original.clone();
    changed.functions[1].local = LocalStackExtent::Unaccounted;
    assert_eq!(
        derive_physical_stack_demand(&changed, machine(1), vec![]),
        Err(Error::UnaccountedTerminalStack(machine(2)))
    );
    changed = original.clone();
    changed.functions[1].calls = vec![PhysicalStackCall {
        target: machine(1),
        owner: CallSiteOwner::Operation(OperationId::new(5).unwrap()),
        caller_live_bytes: 64,
    }];
    assert_eq!(
        derive_physical_stack_demand(&changed, machine(1), vec![]),
        Err(Error::TerminalStackCycle(machine(1)))
    );
    changed = original;
    changed.functions.pop();
    assert_eq!(
        derive_physical_stack_demand(&changed, machine(1), vec![]),
        Err(Error::UnknownInternalCallTarget {
            caller: machine(1),
            target: machine(2)
        })
    );
}
