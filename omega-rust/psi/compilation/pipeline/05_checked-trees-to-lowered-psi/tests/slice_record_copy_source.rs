//! A copied slice element keeps exact source custody and a live extent proof.

use checked_trees::{
    CheckedStructuralAccess, CheckedStructuralValueKind, CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::IntegerValue;
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionRequest,
    TerminalProductionTimings,
};
use terminal_psi::{OperationKind, StructuralPathSegment};

const SOURCE: &str = "
data Tag [copy] { case First; case Second; }
data Cell [copy] { tag: Tag; value: u64; }
data Bag { cells: [Cell; 2]; }
machine copy_selected(source: &Bag, destination: &mut Bag) {
    let input: &[Cell] = source.cells.as_slice();
    let output: &mut [Cell] = destination.cells.as_mut_slice();
    output[0] = input[1];
}";

#[test]
fn slice_copy_replays_exact_index_and_readable_access() {
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("copy_selected"),
    )
    .expect("copy a slice element with its runtime bounds obligation");
    for corrupt_access in [false, true] {
        let mut changed = checked.clone();
        let nodes = &mut changed.facts.values.structural_values.nodes;
        let copied = nodes.iter().find_map(|(handle, node)| {
            matches!(&node.kind, CheckedStructuralValueKind::CopiedStructuralPlace { source, .. }
                if source.path == [CheckedUnitStructuralPathSegment::FixedIndex(1)])
                .then_some(handle)
        }).expect("exact indexed record copy");
        let CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
            &mut nodes.get_mut(copied).kind
        else {
            unreachable!()
        };
        if corrupt_access {
            source.access = CheckedStructuralAccess::WriteOnlyBorrow;
        } else {
            source.path[0] = CheckedUnitStructuralPathSegment::FixedIndex(0);
        }
        checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("copy_selected"),
        )
        .expect_err("same-typed sibling or write-only source cannot replace the copied place");
    }
}

#[test]
fn slice_copy_index_obligation_replays_without_source() {
    let checked = crate::front_end::checked_program(SOURCE);
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("copy_selected"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("publish independently bounded slice copy")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("module");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("source-free copy bounds replay");
    let (source, selector) = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralLeafCopy { source, path } => match path.first() {
                Some(StructuralPathSegment::RuntimeIndex { index, .. }) => Some((*source, *index)),
                _ => None,
            },
            _ => None,
        })
        .expect("literal slice index retains a dynamic extent obligation");
    assert!(module.machines.iter().flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations).any(|operation|
            matches!(operation.kind, OperationKind::ElementViewLength { source: observed } if observed == source)),
        "bound observes the exact copied view");
    let mut past_end = module.clone();
    let index = past_end
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == selector)
        })
        .expect("literal selector producer");
    let OperationKind::IntegerConstant { value } = &mut index.kind else {
        panic!("literal index")
    };
    *value = IntegerValue::Unsigned(2);
    terminal_verifier::verify_module(
        &past_end,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect_err("original evidence cannot authorize an out-of-extent copy");
    let mut no_obligation = module.clone();
    for machine in &mut no_obligation.machines {
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                if let OperationKind::StructuralLeafCopy {
                    source: copied,
                    path,
                } = &mut operation.kind
                    && *copied == source
                {
                    path[0] = StructuralPathSegment::FixedIndex(1);
                }
            }
        }
    }
    terminal_verifier::validate_module(&no_obligation)
        .expect_err("static view index cannot erase the copy's bounds obligation");
    let mut no_length = module;
    for machine in &mut no_length.machines {
        for block in &mut machine.blocks {
            block.operations.retain(|operation| {
                !matches!(operation.kind,
                OperationKind::ElementViewLength { source: observed } if observed == source)
            });
        }
    }
    terminal_verifier::verify_module(
        &no_length,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect_err("copy bound cannot borrow a missing length observation");
}

#[test]
fn slice_copy_rejects_out_of_extent_source_selection() {
    crate::front_end::checked_program_result(&SOURCE.replace("input[1]", "input[2]"))
        .expect_err("a literal index does not establish that the slice contains it");
}

#[test]
fn slice_copy_retains_qualified_byte_fields() {
    let source = format!(
        "domain [u8; 8]::Utf8 requires valid_utf8(self);\n{}",
        SOURCE.replace(
            "tag: Tag; value: u64;",
            "label: [u8; 8] in Utf8; tag: Tag; value: u64;"
        )
    );
    let checked = crate::front_end::checked_program(&source);
    let _artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("copy_selected"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("whole slice record copy retains its qualified byte member");
}
