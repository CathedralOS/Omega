use super::super::activation::{Bindings, Destination};
use super::super::storage::Stored;
use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, PlaceId, StructuralCaseId, StructuralTypeId,
};
use terminal_psi::{
    Block, MachineContract, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, TerminalMachine,
};

#[test]
fn cleanup_keeps_the_captured_value_and_the_borrowed_home() {
    let source = PlaceId::new(1).unwrap();
    let owned = PlaceId::new(2).unwrap();
    let borrowed = PlaceId::new(3).unwrap();
    let entry = BlockId::new(1).unwrap();
    let next = BlockId::new(2).unwrap();
    let jump = Terminator::Jump {
        edge: EdgeId::new(1).unwrap(),
        target: next,
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![
            StructuralArgument::whole(source, StructuralAccess::Owned),
            StructuralArgument::whole(source, StructuralAccess::SharedBorrow),
        ],
        cleanup_actions: vec![TerminalAffineCleanupAction::InvokeOwned(
            terminal_psi::OwnedCleanupInvocation {
                subject: StructuralArgument::whole(
                    PlaceId::new(4).unwrap(),
                    StructuralAccess::Owned,
                ),
                structural_type: StructuralTypeId::new(2).unwrap(),
                cleanup_machine: MachineId::new(2).unwrap(),
                claim_transfers: vec![],
                requirement_obligations: vec![],
            },
        )],
    };
    let mut successor = Block::plain(
        next,
        vec![],
        Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: vec![],
        },
    );
    for (position, (place, access)) in [
        (owned, StructuralAccess::Owned),
        (borrowed, StructuralAccess::SharedBorrow),
    ]
    .into_iter()
    .enumerate()
    {
        successor
            .structural_parameters
            .push(StructuralParameterDeclaration {
                position: position as u32,
                place,
                access,
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: vec![],
            });
    }
    let machine = TerminalMachine {
        id: MachineId::new(1).unwrap(),
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        entry,
        blocks: vec![Block::plain(entry, vec![], jump), successor],
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
        attachment: None,
        ranked_scc: None,
        structural_places: vec![],
        block_entry_claims: vec![],
        call_formations: vec![],
        loan_completions: vec![],
        content_identity_reshuffles: vec![],
        content_partition_compositions: vec![],
    };
    let module = TerminalModule::declarations();
    let mut memory = Storage::default();
    let before = StructuralCaseId::new(1).unwrap();
    let after = StructuralCaseId::new(2).unwrap();
    let input = memory.allocate(Stored::variant(before));
    let mut bindings = Bindings::new(&module, &machine);
    bindings.places.insert(source, input.clone());
    let mut frame = Frame::enter(&machine, bindings, Destination::Unit).unwrap();

    assert!(matches!(
        prepare(&module, &mut frame, &mut memory),
        Some(Selection::Ready([_]))
    ));
    // Model a write by a selected cleanup between argument preparation and
    // the destination block. Re-entry must reuse the prepared edge.
    memory.set(&input, Stored::variant(after));
    assert!(matches!(
        prepare(&module, &mut frame, &mut memory),
        Some(Selection::Ready([_]))
    ));
    frame.arrive().unwrap();
    assert_eq!(
        memory.get(frame.bindings.places.get(&owned).unwrap()).tag(),
        Some(before)
    );
    assert_eq!(
        memory
            .get(frame.bindings.places.get(&borrowed).unwrap())
            .tag(),
        Some(after)
    );
}
