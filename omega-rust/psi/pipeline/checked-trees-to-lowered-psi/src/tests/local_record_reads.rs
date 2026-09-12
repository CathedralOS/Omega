//! Stored records use exact field identities and current ownership places.

use super::*;
use checked_trees::CheckedScalarComputationKind;

const SOURCE: &str = "
    data Record { prefix: u8; payload: u64; }
    data Choice { case Empty; case Some(value: u32); }
    machine observe(selected: bool, payload: u64) -> u64 {
        let retained: Record = Record { payload: payload, prefix: 3 };
        let other: Record = Record { prefix: 4, payload: 7 };
        retained.payload
    }
    machine joined(selected: bool, payload: u64) -> u64 {
        let retained: Record = Record { payload: payload, prefix: 3 };
        let left: Choice = Choice::Some { value: 37 };
        let right: Choice = Choice::Empty;
        let result: Choice = match selected { true -> left, false -> right };
        retained.payload
    }
";

#[test]
fn local_record_reads_publish_direct_and_transported_places() {
    let checked = checked_source(SOURCE);
    for name in ["observe", "joined"] {
        let artifact = produce_terminal_artifact(&checked, name).expect("record read publishes");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let reads = entry
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation.kind {
                OperationKind::IntegerStructuralField { source, field } => Some((source, field)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 1);
        assert!(
            entry.structural_parameters.is_empty(),
            "an owned local is not an incoming parameter"
        );
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn local_record_reads_reject_changed_field_source_and_carrier() {
    let checked = checked_source(SOURCE);
    let _artifact =
        produce_terminal_artifact(&checked, "observe").expect("uncorrupted source publishes");
    let handle = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::StructuralField { .. }
            )
            .then_some(handle)
        })
        .unwrap();
    for corruption in 0..4 {
        let mut changed = checked.clone();
        let node = changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle);
        let CheckedScalarComputationKind::StructuralField {
            source_expression,
            subject,
            field,
        } = &mut node.kind
        else {
            panic!("field read")
        };
        match corruption {
            0 => *field = SymbolHandle::invalid(),
            1 => {
                subject.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: SymbolHandle::invalid(),
                    }
            }
            2 => *source_expression = checked_trees::expression::ExpressionHandle::invalid(),
            3 => node.primitive_type = PrimitiveType::U8,
            _ => unreachable!(),
        }
        assert!(
            produce_terminal_artifact(&changed, "observe").is_err(),
            "corruption {corruption} must reject"
        );
    }
}

#[test]
fn shared_record_getter_keeps_receiver_custody_separate_from_arguments() {
    let checked = checked_source(
        "
        data Record { payload: u64; }
        machine Record::get_payload(&self) -> u64 { self.payload }
        machine observe(payload: u64) -> u64 {
            let retained: Record = Record { payload: payload };
            retained.get_payload()
        }",
    );
    let artifact =
        produce_terminal_artifact(&checked, "observe").expect("shared local getter publishes");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let constructed = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            matches!(operation.kind, OperationKind::EstablishScalarRecord { .. })
                .then(|| operation.result.structural().unwrap().place)
        })
        .unwrap();
    let arguments = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .expect("getter retains a structural call");
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments[0].place, constructed);
    assert_eq!(arguments[0].access, StructuralAccess::SharedBorrow);
    assert!(arguments[0].path.is_empty());

    let mut changed = checked.clone();
    let plans = &mut changed.facts.values.scalar_computations;
    let span = plans
        .nodes
        .iter()
        .find_map(|(_, node)| match node.kind {
            CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } if !structural_arguments.is_empty() => Some(structural_arguments),
            _ => None,
        })
        .unwrap();
    let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
        &mut plans.structural_arguments.span_mut(span).unwrap()[0]
    else {
        panic!("local receiver")
    };
    argument.access = checked_trees::CheckedStructuralAccess::Owned;
    assert!(
        produce_terminal_artifact(&changed, "observe").is_err(),
        "a shared receiver cannot become an owned transfer"
    );
}

#[test]
fn local_record_reads_compose_with_calls_and_selective_booleans() {
    for source in [
        "data Record { prefix: u8; payload: u64; }
         machine identity(value: u64) -> u64 { value }
         machine observe(payload: u64) -> u64 {
             let left: Record = Record { prefix: 1, payload: payload };
             let right: Record = Record { payload: 7, prefix: 2 };
             identity(left.payload) ^ right.payload
         }",
        "data Record { prefix: u64; selected: bool; }
         machine identity(value: bool) -> bool { value }
         machine observe(selected: bool, other: bool) -> bool {
             let record: Record = Record { prefix: 19, selected: selected };
             record.selected && identity(other)
         }",
    ] {
        let checked = checked_source(source);
        let artifact = produce_terminal_artifact(&checked, "observe")
            .expect("record reads compose in ordinary expressions");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn local_record_reads_cannot_swap_same_typed_operand_occurrences() {
    let checked = checked_source(
        "
        data Record { first: u64; second: u64; }
        machine observe(value: u64) -> u64 {
            let retained: Record = Record { first: value, second: 17 };
            retained.first ^ retained.second
        }",
    );
    let _artifact =
        produce_terminal_artifact(&checked, "observe").expect("distinct fields publish");
    let fields = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::StructuralField { .. }
            )
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    let replacement = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(fields[1])
        .kind
        .clone();
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(fields[0])
        .kind = replacement;
    assert!(
        produce_terminal_artifact(&changed, "observe").is_err(),
        "another valid same-typed read cannot replace this operand"
    );
}
