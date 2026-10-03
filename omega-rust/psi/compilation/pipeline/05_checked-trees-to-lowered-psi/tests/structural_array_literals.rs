//! Fixed arrays assemble completed children in authored order. Each record or
//! primitive-array child retains its ordinary construction and range evidence;
//! the enclosing array does not require a universal zero.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{OperationKind, OperationResult, RecordFieldValue, TerminalModule};

fn verified_array(source: &str) -> TerminalModule {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("make"),
    )
    .unwrap();
    let module = terminal_codec::decode_module(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
    )
    .unwrap();
    let proof = terminal_codec::decode_proof_bundle(
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
    )
    .unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("completed children and their atomic array assembly verify independently");
    let mut arrays = module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishStructuralArray { elements } => Some(elements),
            _ => None,
        });
    let elements = arrays.next().expect("one completed array");
    assert_eq!(elements.len(), 2);
    assert_ne!(elements[0].place, elements[1].place);
    assert!(arrays.next().is_none());
    // Removing the retained bounded-field evidence cannot turn a completed
    // child into a valid unqualified initializer.
    let mut changed = module.clone();
    let obligation = changed.machines[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::EstablishRecord { fields } => Some(fields),
            _ => None,
        })
        .flatten()
        .find_map(|field| match &mut field.value {
            RecordFieldValue::Scalar {
                range_obligation, ..
            } if range_obligation.is_some() => Some(range_obligation),
            _ => None,
        });
    if let Some(obligation) = obligation {
        *obligation = None;
        assert!(
            terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default())
                .is_err()
        );
    }
    module
}

fn children(module: &TerminalModule) -> Vec<&[terminal_psi::RecordFieldInitializer]> {
    let operations = module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let elements = operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::EstablishStructuralArray { elements } => Some(elements),
            _ => None,
        })
        .unwrap();
    elements
        .iter()
        .map(|element| {
            assert!(element.path.is_empty());
            let operation = operations
                .iter()
                .find(|operation| {
                    matches!(&operation.result,
            OperationResult::Structural(result) if result.place == element.place)
                })
                .unwrap();
            let OperationKind::EstablishRecord { fields } = &operation.kind else {
                panic!("each authored record child completes before array assembly")
            };
            fields.as_slice()
        })
        .collect()
}

#[test]
fn record_leaves_store_in_authored_order_with_their_range_obligations() {
    let module = verified_array(
        "data Cell { tag: u8 [0..=100]; low: u8; }
         data Pack { items: [Cell; 2]; }
         machine make() -> Pack { Pack { items: [Cell { tag: 7, low: 1 }, Cell { tag: 9 }] } }",
    );
    let children = children(&module);
    for fields in &children {
        assert!(matches!(
            fields[0].value,
            RecordFieldValue::Scalar {
                range_obligation: Some(_),
                ..
            }
        ));
    }
    assert!(matches!(
        children[0][1].value,
        RecordFieldValue::Scalar {
            range_obligation: None,
            ..
        }
    ));
}

#[test]
fn nested_members_store_at_their_leaf_paths() {
    let module = verified_array(
        "data Span { low: u8; high: u8; }
         data Cell { span: Span; bytes: [u8; 2]; }
         data Pack { items: [Cell; 2]; }
         machine make() -> Pack {
             Pack { items: [Cell { span: Span { low: 1, high: 2 }, bytes: [3, 4] },
                            Cell { bytes: [8, 10] }] }
         }",
    );
    let children = children(&module);
    assert!(children.iter().all(|fields| {
        fields
            .iter()
            .all(|field| matches!(field.value, RecordFieldValue::Structural(_)))
    }));
    assert_eq!(module.machines[0].blocks.iter().flat_map(|block| &block.operations)
        .filter(|operation| matches!(&operation.kind, OperationKind::EstablishScalarArray { elements } if elements.len() == 2)).count(), 2);
}

#[test]
fn zeroless_shapes_assemble_completed_children() {
    let module = verified_array(
        "data Cell { tag: u8 [1..=8]; }
         data Pack { items: [Cell; 2]; }
         machine make() -> Pack { Pack { items: [Cell { tag: 7 }, Cell { tag: 8 }] } }",
    );
    assert!(children(&module).iter().all(|fields| matches!(
        fields[0].value,
        RecordFieldValue::Scalar {
            range_obligation: Some(_),
            ..
        }
    )));
}
