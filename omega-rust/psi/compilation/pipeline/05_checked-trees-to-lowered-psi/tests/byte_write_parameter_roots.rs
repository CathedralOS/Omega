//! A byte-view write and a byte-field byte store name their borrowed root by
//! authored parameter position. A scalar formal before the root moves that
//! position past the root's dense structural index, so emission must resolve
//! it through the frame's authored-to-lowered parameter roster. Reading the
//! authored position as a dense index finds the next borrowed parameter, a
//! verified write to the wrong referent, or no parameter at all.

use checked_trees::{CheckedStoreRoot, CheckedTrees, CheckedUnitEffectOperationPlan};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralByteArrayValue, TerminalStructuralInputs,
    TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_psi::{OperationKind, StructuralPathSegment};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

/// Every byte-view write's and byte-field byte store's checked root, across
/// ordinary and composed plans.
fn byte_roots(checked: &CheckedTrees) -> Vec<CheckedStoreRoot> {
    let effects = &checked.facts.flow.terminal_unit_effects;
    effects
        .machines
        .iter()
        .flat_map(|plan| &plan.operations)
        .chain(
            effects
                .composed_machines
                .iter()
                .flat_map(|plan| &plan.states)
                .flat_map(|state| &state.operations),
        )
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) => Some(write.destination),
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                Some(store.destination)
            }
            _ => None,
        })
        .collect()
}

fn produce(
    checked: &CheckedTrees,
    machine: &'static str,
) -> terminal_codec::CanonicalTerminalArtifact {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name(machine),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the byte store publishes past its preceding formals")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &Default::default()).unwrap();
    artifact
}

fn complete(execution: &mut TerminalExecution) {
    let status = execution
        .resume(
            &mut terminal_fuel::TerminalFuelMeter::unbounded(),
            &mut AcceptTerminalEffects,
        )
        .unwrap();
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
}

const CELLS: &str = r#"
    domain [u8; 3]::Utf8 requires valid_utf8(self);
    data Cell { text: [u8; 3] in Utf8; }
"#;

/// `enter` over three cells and one index: its body fills each cell, then
/// replaces one byte of `root`.
fn byte_field_program(signature: &str, root: &str) -> CheckedTrees {
    crate::front_end::checked_program(&format!(
        r#"
        {CELLS}
        machine enter({signature}) {{
            before.text = "AAA";
            target.text = "BBB";
            after.text = "CCC";
            {root}.text[index] = 90;
        }}
        "#
    ))
}

/// Run `enter` with `index = 1` and return the three cells' live bytes.
fn byte_field_cells(checked: &CheckedTrees) -> [Vec<u8>; 3] {
    let artifact = produce(checked, "enter");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let field = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::StructuralByteSequenceFieldByteStore { field, .. } => Some(*field),
            _ => None,
        })
        .expect("retained byte store");
    let structural = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 70 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(structural.len(), 3);
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &Default::default(),
        &[unsigned(1)],
        TerminalStructuralInputs {
            arguments: &structural,
            ..Default::default()
        },
    )
    .unwrap();
    complete(&mut execution);
    [70, 71, 72].map(|identity| {
        execution
            .structural_byte_sequence_field(identity, &[], field)
            .expect("each cell holds its whole store")
            .to_vec()
    })
}

/// A byte-field byte store after a scalar formal replaces a byte of exactly
/// its authored root: `target` is authored position 2 but dense structural
/// index 1, and `after` is dense index 2.
#[test]
fn a_byte_field_store_after_a_scalar_formal_writes_its_authored_parameter() {
    let signature = "before: &mut Cell, index: u64 [0..=2], target: &mut Cell, after: &mut Cell";
    for (root, position, expected) in [
        ("before", 0, [b"AZA", b"BBB", b"CCC"]),
        ("target", 2, [b"AAA", b"BZB", b"CCC"]),
        ("after", 3, [b"AAA", b"BBB", b"CZC"]),
    ] {
        let checked = byte_field_program(signature, root);
        assert_eq!(
            byte_roots(&checked),
            [CheckedStoreRoot::Parameter { position }],
            "{root}: the checked root is the authored position"
        );
        assert_eq!(
            byte_field_cells(&checked),
            expected.map(|bytes| bytes.to_vec()),
            "{root}"
        );
    }
}

/// The control: with no scalar formal before any root, authored positions
/// and dense indexes coincide.
#[test]
fn a_byte_field_store_without_a_preceding_scalar_writes_its_authored_parameter() {
    let signature = "before: &mut Cell, target: &mut Cell, after: &mut Cell, index: u64 [0..=2]";
    for (root, expected) in [
        ("before", [b"AZA", b"BBB", b"CCC"]),
        ("target", [b"AAA", b"BZB", b"CCC"]),
        ("after", [b"AAA", b"BBB", b"CZC"]),
    ] {
        assert_eq!(
            byte_field_cells(&byte_field_program(signature, root)),
            expected.map(|bytes| bytes.to_vec()),
            "{root}"
        );
    }
}

/// Run `Bytes::run`, which lends three fixed byte arrays to `put`, and return
/// the arrays' bytes afterwards.
fn byte_view_arrays(put: &str) -> [Vec<u8>; 3] {
    let checked = crate::front_end::checked_program(&format!(
        r#"
        {put}
        data Bytes {{ before: [u8; 2]; target: [u8; 2]; after: [u8; 2]; }}
        machine Bytes::run(&mut self) {{
            put(&mut self.before, 1, &mut self.target, &mut self.after);
        }}
        "#
    ));
    let artifact = produce(&checked, "Bytes::run");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let paths =
        ["before", "target", "after"].map(|field| vec![StructuralPathSegment::Field(field.into())]);
    let arrays = paths
        .iter()
        .zip([10, 20, 30])
        .map(|(path, byte)| TerminalStructuralByteArrayValue {
            argument_index: 0,
            path: path.clone(),
            bytes: vec![byte; 2],
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &Default::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 73,
                structural_type: entry.structural_parameters[0].structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            byte_arrays: &arrays,
            ..Default::default()
        },
    )
    .unwrap();
    complete(&mut execution);
    paths.map(|path| execution.structural_byte_array(73, &path).unwrap().to_vec())
}

/// An ordinary machine body's byte-view write resolves its authored root in
/// the same roster. It cannot publish yet: its bound obligation does not see
/// the `requires` premise (as `byte_view_write_resolves_owned_local_element_value`
/// records), so every root stops at that proof, past its destination. The
/// dense lookup stopped `after` earlier, at "byte-view write destination is
/// absent", and resolved `target` to `after`'s place; the `indexed_write`
/// unit test pins the place each root selects.
#[test]
fn an_ordinary_byte_view_write_after_a_scalar_formal_reaches_its_bound_obligation() {
    for (root, position) in [("before", 0), ("target", 2), ("after", 3)] {
        let checked = crate::front_end::checked_program(&format!(
            "machine put(before: &mut [u8], index: u64, target: &mut [u8], after: &mut [u8])
             requires index < {root}.len
             {{ {root}[index] = 9; }}"
        ));
        assert_eq!(
            byte_roots(&checked),
            [CheckedStoreRoot::Parameter { position }],
            "{root}: the checked root is the authored position"
        );
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("put"),
        )
        .map(|_| ())
        .expect_err("the write's bound obligation has no premise");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::OperationProofUnavailable(_)
            ),
            "{root}: the destination resolves: {error:?}"
        );
    }
}

/// A state graph's byte-view write after a scalar formal writes its authored
/// view. The state's frame parameters already carried authored positions, so
/// this held under the dense lookup too; it pins that the state's authored
/// roster pairs each root with the same declaration.
#[test]
fn a_state_graph_byte_view_write_after_a_scalar_formal_writes_its_authored_parameter() {
    for (root, expected) in [
        ("before", [[10, 9], [20, 20], [30, 30]]),
        ("target", [[10, 10], [20, 9], [30, 30]]),
        ("after", [[10, 10], [20, 20], [30, 9]]),
    ] {
        let put = format!(
            r#"
            machine put(before: &mut [u8], index: u64, target: &mut [u8], after: &mut [u8]) {{
                transition index < {root}.len {{
                    true -> store(before, index, target, after)
                    false -> done()
                }}
                state store(before: &mut [u8], index: u64, target: &mut [u8], after: &mut [u8]) {{
                    {root}[index] = 9;
                }}
                state done() {{}}
            }}
            "#
        );
        assert_eq!(
            byte_view_arrays(&put),
            expected.map(|bytes| bytes.to_vec()),
            "{root}"
        );
    }
}
