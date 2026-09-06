//! Source-produced attached receiver stores through canonical Terminal admission.

use checked_trees::CheckedUnitEffectOperationPlan;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use terminal_psi::{
    OperationKind, OperationResult, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeShape, Terminator,
};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    data Pair { prefix: u8; value: u16; }
    data Inner { prefix: u8; value: u16; }
    data Outer { prefix: u8; inner: Inner; }
    data Cell [copy] { prefix: u8; value: u16; }
    data Matrix { prefix: u8; cells: [Cell; 3]; }

    machine Pair::direct(&mut self) {
        self.value = 17;
    }

    machine Outer::nested(&mut self) {
        self.inner.value = 19;
    }

    machine Matrix::indexed(&mut self) {
        self.cells[2].value = 29;
    }

    machine Pair::parameter(&mut self, replacement: u16) {
        self.value = replacement;
    }
"#;

fn typed_from_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize receiver store");
    let syntax = parse_syntax_trees(&tokens).expect("parse receiver store");
    let resolved = lower_syntax_trees(&syntax).expect("resolve receiver store");
    lower_symbol_resolved_trees(&resolved).expect("type receiver store")
}

fn assert_receiver_store(
    machine_name: &str,
    expected_path: &[StructuralPathSegment],
    expected_value: u128,
    from_parameter: bool,
) {
    assert_receiver_store_with_access(
        machine_name,
        expected_path,
        expected_value,
        from_parameter,
        StructuralAccess::MutableBorrow,
    );
}

fn assert_receiver_store_with_access(
    machine_name: &str,
    expected_path: &[StructuralPathSegment],
    expected_value: u128,
    from_parameter: bool,
    access: StructuralAccess,
) {
    let (source, checked_access) = match access {
        StructuralAccess::MutableBorrow => (
            SOURCE.to_owned(),
            checked_trees::CheckedStructuralAccess::MutableBorrow,
        ),
        StructuralAccess::WriteOnlyBorrow => (
            SOURCE.replace("&mut self", "&write self"),
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow,
        ),
        _ => panic!("store fixture requires writable borrowed access"),
    };
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed_from_source(&source))
        .expect("ordinary writable receiver stores check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("source machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .expect("receiver assignment retains a checked Unit plan");
    let [receiver] = plan.structural_parameters.as_slice() else {
        panic!("one checked receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.access, checked_access);
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::ReturnUnit { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("one source store followed by Unit return")
    };
    assert_eq!(store.destination_parameter_position, 0);
    assert_eq!(store.statement_index, 0);
    assert_eq!(store.field_identity, "value");

    let artifact = terminal_production::produce_terminal_artifact(&checked, machine_name)
        .expect("receiver store reaches canonical Terminal through production");
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("reload canonical receiver store semantics");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes())
        .expect("reload canonical receiver store proof");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    let profile = proof_admission::AdmissionProfile::default();
    let verified = terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("canonical receiver store independently verifies");
    let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry)
        .expect("one receiver store has fixed fuel");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("Terminal entry");
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("one Terminal receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.access, access);
    assert_eq!(receiver.multiplicity, StructuralMultiplicity::Unrestricted);
    assert!(receiver.qualifications.is_empty());
    assert!(receiver.projected_qualifications.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("ordinary one-state Unit body")
    };
    assert!(matches!(block.terminator, Terminator::ReturnUnit { .. }));
    assert_eq!(block.operations.len(), if from_parameter { 1 } else { 2 });
    let store = block.operations.last().expect("receiver store");
    let OperationKind::StructuralScalarFieldStore {
        destination,
        path,
        field,
        value,
    } = &store.kind
    else {
        panic!("last operation is the exact projected store")
    };
    assert_eq!(*destination, receiver.place);
    assert_eq!(path, expected_path);
    assert_eq!(store.result, OperationResult::Unit);

    let mut carrier_type = receiver.structural_type;
    for segment in expected_path {
        let carrier = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == carrier_type)
            .expect("declared carrier type");
        carrier_type = match (&carrier.shape, segment) {
            (StructuralTypeShape::Record { fields }, StructuralPathSegment::Field(identity)) => {
                let carrier_field = fields
                    .iter()
                    .find(|field| field.identity == *identity)
                    .expect("exact authored enclosing field");
                let StructuralFieldType::Structural(nested) = carrier_field.field_type else {
                    panic!("enclosing field has structural type")
                };
                nested
            }
            (
                StructuralTypeShape::FixedArray { element, length },
                StructuralPathSegment::FixedIndex(index),
            ) => {
                assert_eq!(*length, 3);
                assert!(*index < *length);
                *element
            }
            _ => panic!("authored carrier path retains its record/array shape"),
        };
    }
    let carrier = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier_type)
        .expect("final carrier type");
    let StructuralTypeShape::Record { fields } = &carrier.shape else {
        panic!("final scalar field belongs to a record")
    };
    let scalar_field = fields
        .iter()
        .find(|candidate| candidate.id == *field)
        .unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    assert_eq!(scalar_field.identity, "value");
    assert_eq!(
        scalar_field.field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer_type))
    );
    let scalar_arguments = if from_parameter {
        let [parameter] = entry.parameters.as_slice() else {
            panic!("one same-typed scalar parameter")
        };
        assert_eq!(*value, parameter.id);
        assert_eq!(parameter.scalar_type, ScalarType::Integer(integer_type));
        vec![TerminalScalarValue::Integer {
            scalar_type: integer_type,
            value: IntegerValue::Unsigned(expected_value),
        }]
    } else {
        assert!(entry.parameters.is_empty());
        let constant = &block.operations[0];
        assert_eq!(
            constant.kind,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(expected_value)
            }
        );
        let OperationResult::Scalar(result) = constant.result else {
            panic!("literal defines the typed stored value")
        };
        assert_eq!(*value, result.id);
        assert_eq!(result.scalar_type, ScalarType::Integer(integer_type));
        Vec::new()
    };

    // This is an opaque interpreter argument, not a native ProgramEntry pointer.
    // Projected field storage is private; completion does not assert a public
    // post-return value observation or native receiver provisioning.
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &scalar_arguments,
        &[TerminalStructuralValue {
            opaque_identity: 71,
            structural_type: receiver.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("canonical store accepts its supplied interpreter receiver");
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(certificate.ceiling_units());
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
    assert_eq!(
        meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(store.id))
            .unwrap()
            .executions(),
        1
    );
}

#[test]
fn direct_receiver_integer_literal_store_reaches_canonical_interpretation() {
    assert_receiver_store("Pair::direct", &[], 17, false);
}

#[test]
fn nested_receiver_integer_literal_store_retains_exact_carrier() {
    assert_receiver_store(
        "Outer::nested",
        &[StructuralPathSegment::Field("inner".into())],
        19,
        false,
    );
}

#[test]
fn indexed_receiver_integer_literal_store_retains_exact_element() {
    assert_receiver_store(
        "Matrix::indexed",
        &[
            StructuralPathSegment::Field("cells".into()),
            StructuralPathSegment::FixedIndex(2),
        ],
        29,
        false,
    );
}

#[test]
fn receiver_store_retains_same_typed_scalar_parameter() {
    assert_receiver_store("Pair::parameter", &[], 37, true);
}

#[test]
fn write_only_receiver_stores_retain_access_through_canonical_interpretation() {
    for (machine, path, value, from_parameter) in [
        ("Pair::direct", vec![], 17, false),
        (
            "Outer::nested",
            vec![StructuralPathSegment::Field("inner".into())],
            19,
            false,
        ),
        (
            "Matrix::indexed",
            vec![
                StructuralPathSegment::Field("cells".into()),
                StructuralPathSegment::FixedIndex(2),
            ],
            29,
            false,
        ),
        ("Pair::parameter", vec![], 37, true),
    ] {
        assert_receiver_store_with_access(
            machine,
            &path,
            value,
            from_parameter,
            StructuralAccess::WriteOnlyBorrow,
        );
    }
}

#[test]
fn shared_receiver_store_rejects_during_source_checking() {
    let source = r#"
        data Pair { prefix: u8; value: u16; }
        machine Pair::direct(&self) {
            self.value = 17;
        }
    "#;
    let diagnostics =
        match typed_trees_to_checked_trees::lower_typed_trees(typed_from_source(source)) {
            Ok(_) => panic!("a shared receiver store must fail source checking"),
            Err(diagnostics) => diagnostics,
        };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("assignment cannot write `value`")
            && diagnostic.message.contains("not mutable in this state")
    }));
}

#[test]
fn canonical_verifier_rejects_shared_access_substituted_for_mutable_receiver() {
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed_from_source(SOURCE)).unwrap();
    let artifact =
        terminal_production::produce_terminal_artifact(&checked, "Pair::direct").unwrap();
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    entry.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    assert!(matches!(
        terminal_verifier::validate_module(&module),
        Err(terminal_verifier::ModuleError::InvalidStructuralScalarFieldStore { .. })
    ));
}
