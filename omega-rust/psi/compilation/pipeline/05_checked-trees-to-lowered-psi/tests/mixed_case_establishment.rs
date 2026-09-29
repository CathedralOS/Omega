//! A mixed-shape case literal establishes its runtime common fields, then the
//! selected payload, and zero-initializes every common field it leaves
//! unnamed (`checking::value_custody::data::validate_data_shape`).
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use terminal_psi::{OperationKind, RecordFieldValue, StructuralTypeShape};

const SOURCE: &str = r#"
    data Event {
        consumed: bool;
        bonus: i32;
        case Nothing;
        case Treasure(gold: i32);
    }
    data Main { event: Event; }
    machine Main::run(&mut self) {
        self.event = Event::Treasure { gold: 50, consumed: true };
    }
"#;

#[test]
fn mixed_case_literal_zero_initializes_unnamed_common_fields() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a mixed case literal lowers");
    let module = &lowered.semantic_module;
    let operations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let [establishment] = operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::EstablishStructuralCase { .. }))
        .collect::<Vec<_>>()[..]
    else {
        panic!("one mixed case establishment");
    };
    let OperationKind::EstablishStructuralCase { fields, .. } = &establishment.kind else {
        unreachable!()
    };
    let result = establishment.result.structural().expect("structural result");
    let StructuralTypeShape::Mixed { fields: common, cases } = &module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .expect("mixed declaration")
        .shape
    else {
        panic!("the literal establishes the mixed shape");
    };
    let treasure = cases
        .iter()
        .find(|case| case.identity.ends_with("Treasure"))
        .expect("treasure case");
    // Declaration order, not authored order: consumed, bonus, then gold.
    let expected = common
        .iter()
        .chain(&treasure.fields)
        .map(|field| field.id)
        .collect::<Vec<_>>();
    assert_eq!(
        fields.iter().map(|field| field.field).collect::<Vec<_>>(),
        expected
    );
    let value_of = |position: usize| {
        let RecordFieldValue::Scalar { value, .. } = fields[position].value else {
            panic!("scalar roster entry");
        };
        operations
            .iter()
            .find(|operation| operation.result.scalar().is_some_and(|result| result.id == value))
            .map(|operation| operation.kind.clone())
            .expect("defined roster value")
    };
    assert_eq!(
        value_of(1),
        OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Signed(0),
        },
        "the unnamed `bonus` common field establishes as zero"
    );
    terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the mixed establishment independently verifies");
    // Decoding replays the codec's own foundation validation of the roster.
    let bytes = terminal_codec::encode_module(module).expect("encode");
    assert_eq!(
        &terminal_codec::decode_module(&bytes).expect("decode the mixed roster"),
        module
    );
}

/// An erased payload member holds no runtime slot: the literal may omit it
/// when its unique nullary constructor determines it, and the establishment
/// binds only the runtime payload.
#[test]
fn case_literal_leaves_erased_payload_members_out_of_the_roster() {
    let checked = crate::front_end::checked_program(
        r#"
        data Evidence { case Only; }
        data Certified {
            case Pending;
            case Proven(value: i32, proof [erased]: Evidence);
        }
        data Main { record: Certified; }
        machine Main::run(&mut self) {
            self.record = Certified::Proven { value: 70 };
        }
    "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("a case with an erased payload member lowers");
    let module = &lowered.semantic_module;
    let [fields] = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::EstablishStructuralCase { fields, .. } => Some(fields),
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("one case establishment");
    };
    assert_eq!(fields.len(), 1, "only `value` takes a roster position");
    terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the erased-member establishment independently verifies");
    let bytes = terminal_codec::encode_module(module).expect("encode");
    assert_eq!(
        &terminal_codec::decode_module(&bytes).expect("decode the roster"),
        module
    );
}
