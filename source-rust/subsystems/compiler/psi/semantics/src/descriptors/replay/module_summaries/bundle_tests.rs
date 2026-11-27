//! A `ModuleAnalyses` bundle shares borrowed-storage replay summaries only
//! between analyses over rows its own borrowed-storage family shares. The
//! module has two top machines that each call a middle machine, which calls a
//! leaf. In the dynamic twin the middle machine also calls an implementation
//! dynamically, so a closure that reaches it owns rows adapted against that
//! closure and must replay its callees alone.

use super::*;
use crate::descriptors::descriptor_results::{DescriptorAnalysis, GenericCallInputs};
use crate::module_analyses::{DescriptorRows, ModuleAnalyses};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, OperationId, ScalarQualificationSetId, ScalarType, ValueId,
};
use terminal_psi::{
    Block, BoundaryMachineResult, MachineContract, Operation, OperationResult,
    RegisterTargetApplicability, TerminalDynamicInterface, TerminalDynamicSlot,
    TerminalDynamicTable, TerminalIdentityIndex, TerminalMachine, TerminalMachineResult,
    Terminator, ValueDeclaration,
};

const TOP: u64 = 1;
const MIDDLE: u64 = 2;
const LEAF: u64 = 3;
const OTHER_TOP: u64 = 4;
const REALIZATION: u64 = 5;

fn id(number: u64) -> MachineId {
    MachineId::new(number).unwrap()
}

fn machine(identity: u64, operations: Vec<Operation>) -> TerminalMachine {
    let entry = BlockId::new(1).unwrap();
    let mut block = Block::plain(
        entry,
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    block.operations = operations;
    TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: id(identity),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        call_formations: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![block],
        contract: MachineContract {
            result: TerminalMachineResult::Unit,
            ..MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}

fn call(operation: u64, callee: u64) -> Operation {
    Operation::unsuspended(
        OperationId::new(operation).unwrap(),
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee: id(callee),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    )
}

fn dynamic_call(first_operation: u64) -> [Operation; 2] {
    let selector = ValueId::new(21).unwrap();
    [
        Operation::unsuspended(
            OperationId::new(first_operation).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: selector,
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::SelectImplementation { table: 0 },
        ),
        Operation::unsuspended(
            OperationId::new(first_operation + 1).unwrap(),
            OperationResult::Unit,
            OperationKind::CallDynamic {
                selector,
                interface: 0,
                slot: 0,
                arguments: Vec::new(),
                family_actuals: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ),
    ]
}

fn module(dynamic: bool) -> TerminalModule {
    let mut module = TerminalModule::for_entry(id(TOP));
    let dispatch = &mut module.execution.dynamic_dispatch;
    dispatch.interfaces.push(TerminalDynamicInterface {
        id: 0,
        identity: TerminalIdentityIndex(7),
        slots: vec![TerminalDynamicSlot {
            slot: 0,
            declaring_trait_identity: TerminalIdentityIndex(7),
            requirement_identity: TerminalIdentityIndex(8),
            family_tuple: Vec::new(),
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            entry_claim_paths: Vec::new(),
            result: BoundaryMachineResult::Unit,
            result_claim_paths: Vec::new(),
            result_reference_regions: Vec::new(),
        }],
    });
    dispatch.tables.push(TerminalDynamicTable {
        id: 0,
        interface: 0,
        implementation_identity: TerminalIdentityIndex(9),
        rows: vec![Some(id(REALIZATION))],
    });
    let mut middle = vec![call(1, LEAF)];
    if dynamic {
        middle.extend(dynamic_call(2));
    }
    module.machines = vec![
        machine(LEAF, Vec::new()),
        machine(TOP, vec![call(4, MIDDLE)]),
        machine(MIDDLE, middle),
        machine(OTHER_TOP, vec![call(5, MIDDLE)]),
        machine(REALIZATION, Vec::new()),
    ];
    module
}

fn summary(analysis: &mut DescriptorAnalysis<'_>, machine: u64) -> Arc<Summary> {
    analysis
        .replay
        .summary(id(machine), 0)
        .expect("the machine has a borrowed-storage summary")
}

#[test]
fn bundle_analyses_over_its_borrowed_storage_family_share_one_summary() {
    let module = module(false);
    let bundle = ModuleAnalyses::new(&module).unwrap();
    let kind = DescriptorSlotKind::BorrowedStorage;
    let top = bundle.descriptor_closure(kind, [id(TOP)]).unwrap();
    let other_top = bundle.descriptor_closure(kind, [id(OTHER_TOP)]).unwrap();
    assert!(matches!(top, DescriptorRows::Shared { .. }));
    assert!(matches!(other_top, DescriptorRows::Shared { .. }));

    let mut first = bundle.analysis(kind, &top);
    assert!(first.replay.shared.is_some());
    let middle = summary(&mut first, MIDDLE);
    let leaf = summary(&mut first, LEAF);

    // Another closure of the same family adopts the summaries the first
    // analysis published: the same summaries, not copies.
    let mut second = bundle.analysis(kind, &other_top);
    assert!(second.replay.shared.is_some());
    assert!(Arc::ptr_eq(&middle, &summary(&mut second, MIDDLE)));
    assert!(Arc::ptr_eq(&leaf, &summary(&mut second, LEAF)));
    // A repeated hit in one analysis hands out the same summary again.
    assert!(Arc::ptr_eq(&middle, &summary(&mut second, MIDDLE)));
}

#[test]
fn bundle_analyses_share_nothing_with_rows_from_elsewhere_or_other_slot_kinds() {
    let module = module(false);
    let bundle = ModuleAnalyses::new(&module).unwrap();
    let kind = DescriptorSlotKind::BorrowedStorage;
    let top = bundle.descriptor_closure(kind, [id(TOP)]).unwrap();
    let mut first = bundle.analysis(kind, &top);
    summary(&mut first, MIDDLE);

    // Rows another bundle shares, even over the same module.
    let other_bundle = ModuleAnalyses::new(&module).unwrap();
    let foreign = other_bundle.descriptor_closure(kind, [id(TOP)]).unwrap();
    assert!(bundle.analysis(kind, &foreign).replay.shared.is_none());

    // Rows a caller built, with the same contents.
    let built = bundle
        .closure_rows(kind, [id(TOP)], &[], GenericCallInputs::default())
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let borrowed = DescriptorRows::Borrowed(&built);
    assert!(bundle.analysis(kind, &borrowed).replay.shared.is_none());

    // Another slot kind.
    let views = bundle
        .descriptor_closure(DescriptorSlotKind::Views, [id(TOP)])
        .unwrap();
    assert!(
        bundle
            .analysis(DescriptorSlotKind::Views, &views)
            .replay
            .shared
            .is_none()
    );
}

#[test]
fn closures_with_a_dynamic_call_replay_their_callees_alone() {
    let module = module(true);
    let bundle = ModuleAnalyses::new(&module).unwrap();
    let kind = DescriptorSlotKind::BorrowedStorage;
    let top = bundle.descriptor_closure(kind, [id(TOP)]).unwrap();
    let other_top = bundle.descriptor_closure(kind, [id(OTHER_TOP)]).unwrap();
    assert!(matches!(top, DescriptorRows::Owned(_)));
    assert!(matches!(other_top, DescriptorRows::Owned(_)));

    let mut first = bundle.analysis(kind, &top);
    let mut second = bundle.analysis(kind, &other_top);
    assert!(first.replay.shared.is_none());
    assert!(second.replay.shared.is_none());
    assert!(!Arc::ptr_eq(
        &summary(&mut first, LEAF),
        &summary(&mut second, LEAF)
    ));

    // Closures of the same module that reach no dynamic call still share.
    let leaf = bundle.descriptor_closure(kind, [id(LEAF)]).unwrap();
    assert!(matches!(leaf, DescriptorRows::Shared { .. }));
    let mut alone = bundle.analysis(kind, &leaf);
    assert!(alone.replay.shared.is_some());
    let leaf_summary = summary(&mut alone, LEAF);
    let mut again = bundle.analysis(kind, &leaf);
    assert!(Arc::ptr_eq(&leaf_summary, &summary(&mut again, LEAF)));
}
