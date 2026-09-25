use super::{checked_source_with_core_service, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralPathSegment};
#[test]
fn source_indexed_primitive_storage_composes_with_boundary_and_successors() {
    let source = r#"
        pub boundary trait Console { machine write_byte(byte: i32) reaches Console; }
        data Main { value: i32; bytes: [u8; 256]; console: Binding<Console>; }
        machine Main::main(&mut self) reaches Console {
            transition self.value == 0 { true -> initialized() false -> failed() }
            state initialized(&mut self) {
                self.bytes[255] = 65;
                transition self.bytes[255] == 65 && self.bytes[254] == 0 { true -> observed() false -> failed() }
            }
            state observed(&mut self) { self.console.write_byte(self.bytes[255] as i32); }
            state failed(&mut self) { self.console.write_byte(70); }
        }
    "#;
    let checked = checked_source_with_core_service(source);
    lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("indexed source retains its composed attachment");
}

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
fn indexed_primitive_source_rejects_out_of_bounds_and_shared_writes() {
    for source in [
        source("u8", "65").replace("[255]", "[256]"),
        source("u8", "65").replace("&mut self", "&self"),
    ] {
        assert!(
            crate::front_end::checked_program_result(&source).is_err(),
            "invalid primitive access: {source}"
        );
    }
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

#[test]
fn indexed_primitive_store_receiver_rejects_changed_path_and_missing_store() {
    let checked = checked_source_with_core_service(&source("u8", "65"));
    lower_machine(&checked, TerminalMachineSelection::Name("Buffer::update")).unwrap();
    for index in [254, 256] {
        let mut changed = checked.clone();
        let operation = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                )
            })
            .unwrap();
        let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { path, .. } = operation else {
            unreachable!()
        };
        *path.last_mut().unwrap() = CheckedUnitStructuralPathSegment::FixedIndex(index);
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Buffer::update")).is_err(),
            "index substitution {index}"
        );
    }
    let mut changed = checked.clone();
    for plan in &mut changed.facts.flow.terminal_unit_effects.machines {
        plan.operations.retain(|operation| {
            !matches!(
                operation,
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            )
        });
    }
    assert!(lower_machine(&changed, TerminalMachineSelection::Name("Buffer::update")).is_err());
}

#[test]
fn indexed_primitive_read_receiver_rejects_changed_source_path() {
    let checked = checked_source_with_core_service(&source("u8", "65"));
    let plan = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|plan| {
            matches!(
                plan.expression,
                CheckedScalarExpression::StructuralParameterField { .. }
            )
        })
        .unwrap();
    let (binding, value) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(plan.state, plan.statement_ordinal, plan.role)
        .unwrap();
    for index in [254, 256] {
        let mut changed = value.clone();
        let CheckedScalarExpression::StructuralParameterField { path, .. } = &mut changed else {
            unreachable!()
        };
        *path.last_mut().unwrap() =
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index);
        assert!(
            crate::expression_preparation::source_custody::validate_storage_read_expression(
                &checked,
                binding.state,
                binding.statement_ordinal,
                binding.expression,
                &changed
            )
            .is_err()
        );
    }
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

/// Produce `Buffer::update`'s verified Terminal entry machine and return the
/// projected stores (the selector fields' own root-level stores excluded),
/// each flagged whether it is a scalar field store.
fn store_paths(source: &str) -> Vec<(bool, Vec<StructuralPathSegment>)> {
    let checked = checked_source_with_core_service(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Buffer::update"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("verified Terminal for {source}: {error:?}"))
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::WriteOnlyPrimitiveStore { path, .. } => Some((false, path.clone())),
            OperationKind::StructuralScalarFieldStore { path, .. } if !path.is_empty() => {
                Some((true, path.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Every selector of a target is its own runtime element, numbered by depth
/// from the target: `self.grid[self.i][self.j]` stores through two runtime
/// elements, each with its own obligation, in one primitive store.
#[test]
fn nested_selectors_store_through_two_runtime_elements() {
    let stores = store_paths(
        "data Buffer { grid: [[u8; 4]; 3]; i: u64; j: u64; }
        machine Buffer::update(&mut self) {
            self.i = 2;
            self.j = 1;
            self.grid[self.i][self.j] = 65;
        }",
    );
    let [(false, path)] = stores.as_slice() else {
        panic!("one element store: {stores:#?}");
    };
    let [
        StructuralPathSegment::Field(_),
        StructuralPathSegment::RuntimeIndex {
            obligation: outer, ..
        },
        StructuralPathSegment::RuntimeIndex {
            obligation: inner, ..
        },
    ] = path.as_slice()
    else {
        panic!("field, then two runtime elements: {path:#?}");
    };
    assert_ne!(outer, inner, "each element owns its obligation");
}

/// A record field below a runtime element is one scalar field store over a
/// carrier that ends in the runtime element, and a literal element followed
/// by fields is the same carrier grammar.
#[test]
fn field_after_runtime_and_literal_elements_is_one_field_store() {
    let stores = store_paths(
        "data Point { x: u8; y: u8; }
        data Entity { pos: Point; hp: u8; }
        data Buffer { ents: [Entity; 3]; i: u64; }
        machine Buffer::update(&mut self) {
            self.i = 1;
            self.ents[self.i].hp = 7;
            self.ents[0].pos.x = 9;
        }",
    );
    let [(true, runtime), (true, literal)] = stores.as_slice() else {
        panic!("two field stores: {stores:#?}");
    };
    assert!(
        matches!(
            runtime.as_slice(),
            [
                StructuralPathSegment::Field(_),
                StructuralPathSegment::RuntimeIndex { .. }
            ]
        ),
        "`ents[i]` carries `hp`: {runtime:#?}"
    );
    assert!(
        matches!(
            literal.as_slice(),
            [
                StructuralPathSegment::Field(_),
                StructuralPathSegment::FixedIndex(0),
                StructuralPathSegment::Field(_)
            ]
        ),
        "`ents[0].pos` carries `x`: {literal:#?}"
    );
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
