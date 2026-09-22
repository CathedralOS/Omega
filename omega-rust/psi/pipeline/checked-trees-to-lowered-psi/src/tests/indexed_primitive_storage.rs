use super::{
    Lexer, ResolutionRequest, checked_source_with_core_service, lower_machine,
    lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees, resolve,
};
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, StructuralPathSegment};
use typed_trees_to_checked_trees::CheckingRequest;
#[test]
fn source_indexed_primitive_storage_composes_with_boundary_and_successors() {
    let source = r#"
        pub boundary trait Console { machine write_byte(byte: i32) reaches Console; }
        data Main { value: i32; bytes: [u8; 256]; console: Service<Console>; }
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
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        assert!(
            lower_typed_trees(typed, &CheckingRequest::settled()).is_err(),
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
                semantic_vocabulary::CanonicalStructuralPathSegment::Field(_),
                semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(255)
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
