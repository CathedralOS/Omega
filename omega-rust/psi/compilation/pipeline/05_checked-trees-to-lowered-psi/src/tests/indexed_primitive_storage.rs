use super::{checked_source_with_core_service, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralPathSegment};

fn source(primitive: &str, value: &str) -> String {
    format!(
        "data Buffer {{ bytes: [{primitive}; 256]; }}
        machine Buffer::update(&mut self) -> {primitive} {{
            self.bytes[255] = {value};
            self.bytes[255]
        }}"
    )
}

#[test]
fn source_indexed_primitive_storage_retains_canonical_leaf_paths() {
    for (primitive, value) in [("u8", "65"), ("i32", "65"), ("bool", "true")] {
        let checked = checked_source_with_core_service(&source(primitive, value));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("Buffer::update"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("indexed primitive source produces canonical Terminal")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let paths = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::PrimitiveScalarRead { path, .. }
                | OperationKind::WriteOnlyPrimitiveStore { path, .. } => Some(path),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], paths[1]);
        assert!(matches!(
            paths[0].as_slice(),
            [
                terminal_psi::StructuralPathSegment::Field(_),
                terminal_psi::StructuralPathSegment::FixedIndex(255)
            ]
        ));
    }
}

#[test]
fn source_indexed_primitive_store_and_read_share_serialized_backing() {
    let checked = checked_source_with_core_service(&source("u8", "65"));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Buffer::update"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let structural_type = entry.structural_parameters[0].structural_type;
    drop(checked);
    let path = vec![StructuralPathSegment::Field("bytes".into())];
    let mut bytes = vec![0; 256];
    bytes[254] = 17;
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 700,
                structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            byte_arrays: &[terminal_interpreter::TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: path.clone(),
                bytes,
            }],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        8
                    )
                    .unwrap(),
                    value: semantic_vocabulary::IntegerValue::Unsigned(65)
                }
            )
        )
    );
    let bytes = execution.structural_byte_array(700, &path).unwrap();
    assert_eq!(bytes[254], 17, "the neighboring cell is not overwritten");
    assert_eq!(bytes[255], 65);
}

const RUNTIME_ELEMENT_SOURCE: &str = "data Buffer { bytes: [u8; 8]; at: u64; }
    machine Buffer::update(&mut self) {
        self.at = 3;
        self.bytes[self.at] = 65;
    }";

/// A selector read from a stored field is the store's runtime element: the
/// Terminal store's path ends in `RuntimeIndex { index, obligation }` whose
/// index is the evaluated field read, verification re-proves `index < 8` from
/// the preceding store, and execution writes exactly the selected element.
#[test]
fn field_read_selector_stores_through_a_runtime_element() {
    let checked = checked_source_with_core_service(RUNTIME_ELEMENT_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Buffer::update"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("a field-read selector produces verified Terminal")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let stores = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::WriteOnlyPrimitiveStore { path, .. } => Some(path),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(
            stores.as_slice(),
            [path] if matches!(
                path.as_slice(),
                [
                    StructuralPathSegment::Field(_),
                    StructuralPathSegment::RuntimeIndex { .. }
                ]
            )
        ),
        "one element store over the runtime element: {stores:#?}"
    );
    let structural_type = entry.structural_parameters[0].structural_type;
    drop(checked);
    let path = vec![StructuralPathSegment::Field("bytes".into())];
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 700,
                structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            byte_arrays: &[terminal_interpreter::TerminalStructuralByteArrayValue {
                argument_index: 0,
                path: path.clone(),
                bytes: vec![7; 8],
            }],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
    assert_eq!(
        execution.structural_byte_array(700, &path).unwrap(),
        [7, 7, 7, 65, 7, 7, 7, 7],
        "only the selected element changes"
    );
}

/// The store's runtime element is custody, not a hint: substituting a literal
/// element, or a second runtime element, for the authored selector rejects.
#[test]
fn runtime_element_store_rejects_a_substituted_path() {
    let checked = checked_source_with_core_service(RUNTIME_ELEMENT_SOURCE);
    lower_machine(&checked, TerminalMachineSelection::Name("Buffer::update")).unwrap();
    for substitute in [
        vec![CheckedUnitStructuralPathSegment::FixedIndex(3)],
        vec![
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth: 0 },
            ),
            CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth: 0 },
            ),
        ],
        vec![CheckedUnitStructuralPathSegment::RuntimeIndex(
            checked_trees::CheckedRuntimeIndex::Parameter { position: 0 },
        )],
    ] {
        let mut changed = checked.clone();
        let path = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { path, .. } => Some(path),
                _ => None,
            })
            .expect("the element store is planned");
        assert!(matches!(
            path.as_slice(),
            [
                CheckedUnitStructuralPathSegment::Field(_),
                CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth: 0 }
                )
            ]
        ));
        path.truncate(1);
        path.extend(substitute.iter().cloned());
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Buffer::update")).is_err(),
            "substituted element path {substitute:?}"
        );
    }
}

/// `cli__algorithms__bubble_sort`'s swap: a runtime-selected element store
/// writes somewhere inside its array, not across its root, so the stored
/// `self.jp = self.j + 1` survives `self.nums[self.j] = self.b` and bounds the
/// next store's selector. Forgetting the whole root dropped that fact and the
/// second store's `jp < 5` obligation had no proof.
#[test]
fn a_runtime_element_store_keeps_sibling_field_facts() {
    let source = r#"
        data Main { nums: [u64; 5]; j: u64; jp: u64; a: u64; b: u64; }
        machine Main::main(&mut self) {
            self.j = 0;
            transition { _ -> inner() }
            state inner(&mut self) {
                transition self.j < 4 { true -> compare() _ -> done() }
            }
            state compare(&mut self) {
                self.jp = self.j + 1;
                self.a = self.nums[self.j];
                self.b = self.nums[self.jp];
                transition self.a > self.b { true -> swap() _ -> advance() }
            }
            state swap(&mut self) {
                self.jp = self.j + 1;
                self.nums[self.j] = self.b;
                self.nums[self.jp] = self.a;
                transition { _ -> advance() }
            }
            state advance(&mut self) {
                transition self.j < 3 { true -> bump() _ -> done() }
            }
            state bump(&mut self) {
                self.j = self.j + 1;
                transition { _ -> inner() }
            }
            state done(&mut self) {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the sibling field's stored bound survives the element store");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the swap verifies");
}

/// A domain-bearing `[u8; N]` field is a bounded byte sequence with a live
/// length, so `self.out[0]` reads byte zero through the bounds-checked byte
/// read, as `self.out[self.i]` does. Planning the literal index as a static
/// primitive projection reached lowering with a byte field where the path
/// expected a fixed array ("primitive projection requires a structural
/// carrier field"), which stopped `cli__text__caesar_cipher`'s
/// `self.out[0] == 75` guard.
#[test]
fn a_literal_index_into_a_bounded_byte_field_is_a_byte_read() {
    let source = r#"
        domain [u8; 5]::Tag;
        data Main { out: [u8; 5] in Tag; hits: u64; }
        machine Main::main(&mut self) {
            transition self.out.len > 0 && self.out[0] == 75 {
                true -> found()
                _ -> done()
            }
            state found(&mut self) {
                self.hits = 1;
                transition { _ -> done() }
            }
            state done(&mut self) {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("the literal-index byte read lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the byte read verifies");
}
