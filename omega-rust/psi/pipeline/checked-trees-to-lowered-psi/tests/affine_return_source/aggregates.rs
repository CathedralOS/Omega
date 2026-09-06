use super::{assert_identity_execution, checked};
use checked_trees::{
    CheckedUnitStructuralFieldType, CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
};
use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    ClaimId, IeeeFloatFormat, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
};
use terminal_codec::{decode_module, encode_module};
use terminal_psi::{
    StructuralFieldType, StructuralTypeShape, TerminalMachineResult, TerminalModule, Terminator,
};

enum ExpectedShape<'a> {
    Record(Vec<(&'a str, ExpectedField<'a>)>),
    Array(&'a ExpectedShape<'a>, u64),
}

enum ExpectedField<'a> {
    Scalar(ScalarType),
    Structural(&'a ExpectedShape<'a>),
}

fn integer(sign: IntegerSign, bits: u16) -> ExpectedField<'static> {
    ExpectedField::Scalar(ScalarType::Integer(IntegerType::new(sign, bits).unwrap()))
}

fn assert_retained_shape(
    module: &TerminalModule,
    plans: &[CheckedUnitStructuralTypePlan],
    structural_type: StructuralTypeId,
    type_identity: &str,
    expected: &ExpectedShape<'_>,
    retained: &mut Vec<StructuralTypeId>,
) {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .expect("every recursively referenced type has a retained declaration");
    assert_eq!(declaration.identity, type_identity);
    let plan = plans
        .iter()
        .find(|plan| plan.identity == type_identity)
        .expect("retained type has its exact checked identity");
    if !retained.contains(&structural_type) {
        retained.push(structural_type);
    }
    match (expected, &plan.shape, &declaration.shape) {
        (
            ExpectedShape::Record(expected_fields),
            CheckedUnitStructuralTypeShape::Record {
                fields: checked_fields,
            },
            StructuralTypeShape::Record { fields },
        ) => {
            assert_eq!(fields.len(), expected_fields.len());
            assert_eq!(fields.len(), checked_fields.len());
            assert!(fields.windows(2).all(|pair| pair[0].id < pair[1].id));
            for ((field, checked_field), (identity, expected_field)) in
                fields.iter().zip(checked_fields).zip(expected_fields)
            {
                assert_eq!(field.identity, *identity);
                assert_eq!(field.identity, checked_field.identity);
                assert_eq!(field.relevance, checked_field.relevance);
                assert!(!field.relevance.is_erased());
                match (expected_field, &checked_field.field_type, &field.field_type) {
                    (
                        ExpectedField::Structural(expected),
                        CheckedUnitStructuralFieldType::Structural { type_identity },
                        StructuralFieldType::Structural(structural_type),
                    ) => assert_retained_shape(
                        module,
                        plans,
                        *structural_type,
                        type_identity,
                        expected,
                        retained,
                    ),
                    (
                        ExpectedField::Scalar(expected),
                        CheckedUnitStructuralFieldType::Scalar(_),
                        actual,
                    ) => {
                        let expected = match expected {
                            ScalarType::IeeeFloat(format) => {
                                StructuralFieldType::IeeeFloat(*format)
                            }
                            scalar => StructuralFieldType::Scalar(*scalar),
                        };
                        assert_eq!(*actual, expected, "{type_identity}::{identity}");
                    }
                    _ => panic!("unexpected retained field: {type_identity}::{identity}"),
                }
            }
        }
        (
            ExpectedShape::Array(expected_element, expected_length),
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity,
                length: checked_length,
            },
            StructuralTypeShape::FixedArray { element, length },
        ) => {
            assert_eq!(length, expected_length);
            assert_eq!(length, checked_length);
            assert_retained_shape(
                module,
                plans,
                *element,
                element_type_identity,
                expected_element,
                retained,
            );
        }
        _ => panic!("unexpected retained aggregate shape: {type_identity}"),
    }
}

fn assert_aggregate(source: &str, expected: &ExpectedShape<'_>) {
    // The shared helper verifies the semantic/proof codec roundtrip and the
    // interpreter's exact opaque result across repeated fuel exhaustion.
    assert_identity_execution(source, "forward", false, 0, &[], &[]);
    let checked = checked(source);
    let plans = &checked.facts.flow.terminal_structural_returns;
    let [plan] = plans.claim_free_affine_machines.as_slice() else {
        panic!("one aggregate affine identity plan")
    };
    let source_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .unwrap();
    let source_state = &checked.typed.machine_states(source_machine)[0];
    assert_eq!(
        plan.result.type_identity,
        checked
            .typed
            .normalized_type_identity(source_state.return_type)
            .as_str(),
        "the result identity rejoins the authored concrete return type"
    );
    let lowered = lower_machine(&checked, "forward").expect("aggregate identity lowers");
    let module = decode_module(&encode_module(&lowered.semantic_module).unwrap()).unwrap();
    assert_eq!(module, lowered.semantic_module);
    let result = module.machines[0].result.structural().unwrap();
    let mut retained = Vec::new();
    assert_retained_shape(
        &module,
        &plans.structural_types,
        result.structural_type,
        &plan.result.type_identity,
        expected,
        &mut retained,
    );
    assert_eq!(
        module.structural_types.len(),
        retained.len(),
        "the selected helper retains exactly its recursive result closure"
    );
}

#[test]
fn nested_records_retain_each_field_identity_and_shared_leaf_type() {
    let leaf = ExpectedShape::Record(vec![("number", integer(IntegerSign::Unsigned, 64))]);
    let middle = ExpectedShape::Record(vec![
        ("leaf", ExpectedField::Structural(&leaf)),
        ("count", integer(IntegerSign::Unsigned, 16)),
    ]);
    let outer = ExpectedShape::Record(vec![
        ("middle", ExpectedField::Structural(&middle)),
        ("other", ExpectedField::Structural(&leaf)),
        ("enabled", ExpectedField::Scalar(ScalarType::Boolean)),
    ]);
    assert_aggregate(
        "data Leaf { number: u64; }
         data Middle { leaf: Leaf; count: u16; }
         data Outer { middle: Middle; other: Leaf; enabled: bool; }
         data Unused { number: i32; }
         machine forward(value: Outer) -> Outer { value }",
        &outer,
    );
}

const RECORD_ARRAY: &str = "
    data Entry { number: u64; }
    machine forward(values: [Entry; 3]) -> [Entry; 3] { values }
";

#[test]
fn fixed_array_of_affine_records_retains_the_element_declaration() {
    // Fixed record-array syntax also appears in the layout corpus's
    // runtime_plan_laid_record_array_mutable_write_exit fixture.
    let entry = ExpectedShape::Record(vec![("number", integer(IntegerSign::Unsigned, 64))]);
    assert_aggregate(RECORD_ARRAY, &ExpectedShape::Array(&entry, 3));
}

#[test]
fn nested_fixed_arrays_retain_both_lengths_and_the_record_element() {
    // Nested array spelling follows runtime_plan_laid_nested_fixed_array_mutable_write_exit.
    let entry = ExpectedShape::Record(vec![("number", integer(IntegerSign::Signed, 32))]);
    let row = ExpectedShape::Array(&entry, 2);
    let matrix = ExpectedShape::Array(&row, 3);
    assert_aggregate(
        "data Entry { number: i32; }
         machine forward(values: [[Entry; 2]; 3]) -> [[Entry; 2]; 3] { values }",
        &matrix,
    );
    assert_aggregate(
        "data Entry { number: i32; }
         data Matrix { values: [[Entry; 2]; 3]; }
         machine forward(value: Matrix) -> Matrix { value }",
        &ExpectedShape::Record(vec![("values", ExpectedField::Structural(&matrix))]),
    );
}

#[test]
fn instantiated_generic_record_retains_the_concrete_nested_field() {
    // Generic data syntax is pinned by generics/generic_data_instantiation
    // and generics/runtime_nested_generic_instantiations_exit.
    let entry = ExpectedShape::Record(vec![("number", integer(IntegerSign::Unsigned, 32))]);
    assert_aggregate(
        "data Optional<T> { has_value: bool; value: T; }
         data Entry { number: u32; }
         machine forward(value: Optional<Entry>) -> Optional<Entry> { value }",
        &ExpectedShape::Record(vec![
            ("has_value", ExpectedField::Scalar(ScalarType::Boolean)),
            ("value", ExpectedField::Structural(&entry)),
        ]),
    );
}

#[test]
fn generic_array_fields_and_repeated_generic_owners_preserve_results() {
    for source in [
        "data Entry { number: u64; } data Buffer<T> { entries: [T; 3]; }
         machine forward(value: Buffer<Entry>) -> Buffer<Entry> { value }",
        "data Entry { number: u64; } data Wrapper<T> { value: T; }
         machine forward(value: Wrapper<Wrapper<Entry>>) -> Wrapper<Wrapper<Entry>> { value }",
        "data Values { words: [u32; 3]; matrix: [[i16; 2]; 3]; bytes: [u8; 5]; }
         machine forward(value: Values) -> Values { value }",
    ] {
        assert_identity_execution(source, "forward", false, 0, &[], &[]);
    }
}

#[test]
fn whole_sum_payloads_and_common_fields_preserve_owned_results() {
    for fields in [
        "case Empty; case Filled(entry: Entry);",
        "number: u32; case Empty; case Filled(entry: Entry);",
    ] {
        assert_identity_execution(
            &format!(
                "data Entry {{ number: u64; }} data Choice {{ {fields} }}
                      machine forward(value: Choice) -> Choice {{ value }}"
            ),
            "forward",
            false,
            0,
            &[],
            &[],
        );
    }
}

#[test]
fn concrete_generic_arguments_cannot_hide_reference_or_linear_fields() {
    for declarations in [
        "data Entry { reference: &u64; }",
        "data Entry [linear] { number: u64; }",
        "data Entry { number: u64; } machine Entry::drop(&mut self) {}",
    ] {
        for carrier in ["Wrapper<Entry>", "Wrapper<Wrapper<Entry>>", "Buffer<Entry>"] {
            assert_no_affine_plan(&format!(
                "{declarations} data Wrapper<T> {{ value: T; }}
                 data Buffer<T> {{ values: [T; 1]; }}
                 machine forward(value: {carrier}) -> {carrier} {{ value }}"
            ));
        }
    }
}

#[test]
fn empty_arrays_retain_the_existing_terminal_carrier_fence() {
    assert_no_affine_plan(
        "data Entry { number: u64; }
         machine forward(value: [Entry; 0]) -> [Entry; 0] { value }",
    );
}

#[test]
fn primitive_field_widths_boolean_and_ieee_formats_survive_whole_return() {
    // Float fields use the source form in float/named_provider_negate_is_nan_exit;
    // this fixture performs no arithmetic and needs no provider imports.
    assert_aggregate(
        "data Scalars {
             unsigned_byte: u8; unsigned_short: u16; unsigned_word: u32; unsigned_long: u64;
             signed_byte: i8; signed_short: i16; signed_word: i32; signed_long: i64;
             enabled: bool; single: f32; double: f64;
         }
         machine forward(value: Scalars) -> Scalars { value }",
        &ExpectedShape::Record(vec![
            ("unsigned_byte", integer(IntegerSign::Unsigned, 8)),
            ("unsigned_short", integer(IntegerSign::Unsigned, 16)),
            ("unsigned_word", integer(IntegerSign::Unsigned, 32)),
            ("unsigned_long", integer(IntegerSign::Unsigned, 64)),
            ("signed_byte", integer(IntegerSign::Signed, 8)),
            ("signed_short", integer(IntegerSign::Signed, 16)),
            ("signed_word", integer(IntegerSign::Signed, 32)),
            ("signed_long", integer(IntegerSign::Signed, 64)),
            ("enabled", ExpectedField::Scalar(ScalarType::Boolean)),
            (
                "single",
                ExpectedField::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
            ),
            (
                "double",
                ExpectedField::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
            ),
        ]),
    );
}

fn assert_no_affine_plan(source: &str) {
    let checked = checked(source);
    assert!(
        checked
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines
            .is_empty(),
        "unsupported aggregate must retain its source obligations: {source}"
    );
    assert!(lower_machine(&checked, "forward").is_err(), "{source}");
}

#[test]
fn reference_and_slice_fields_have_no_affine_whole_result_plan() {
    // Reference fields follow expressions/borrow_carrying_data_field_exit;
    // slice fields follow domains/utf8_field_write_from_param.
    for field_type in ["&Entry", "&mut Entry", "&[u8]", "&mut [Entry]"] {
        assert_no_affine_plan(&format!(
            "data Entry {{ number: u64; }}
             data Inner {{ field: {field_type}; }}
             data Outer {{ inner: Inner; }}
             machine forward(value: Outer) -> Outer {{ value }}"
        ));
    }
}

#[test]
fn nested_nominal_cleanup_and_projected_linear_claims_have_no_affine_plan() {
    for obligation in [
        "data Entry { number: u64; } machine Entry::drop(&mut self) {}",
        "data Entry [linear] { number: u64; }",
    ] {
        for field_type in ["Entry", "[Entry; 2]", "[[Entry; 2]; 3]"] {
            assert_no_affine_plan(&format!(
                "{obligation}
                 data Inner {{ field: {field_type}; }}
                 data Outer {{ inner: Inner; }}
                 machine forward(value: Outer) -> Outer {{ value }}"
            ));
        }
    }
}

#[test]
fn nested_field_qualifications_have_no_affine_plan() {
    // Vacuous domain declarations and qualifications are pinned by
    // domains/vacuous_domain_qualification; ranged fields by
    // wire/runtime_wire_decode_ranged_field_exit.
    for field_type in ["i64 in Km", "i64 [0..=100]"] {
        assert_no_affine_plan(&format!(
            "domain i64::Km;
             data Inner {{ number: {field_type}; }}
             data Outer {{ inner: Inner; }}
             machine forward(value: Outer) -> Outer {{ value }}"
        ));
    }
}

#[test]
fn corrupted_array_declarations_result_type_and_returned_claims_reject() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        MissingElementDeclaration,
        WrongResultType,
        DuplicatedReturnedClaim,
        BorrowedElementField,
    }
    let lowered = lower_machine(&checked(RECORD_ARRAY), "forward").expect("array identity lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("unmodified array identity independently verifies");
    let result_type = lowered.semantic_module.machines[0]
        .result
        .structural()
        .unwrap()
        .structural_type;
    let StructuralTypeShape::FixedArray { element, .. } = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result_type)
        .unwrap()
        .shape
    else {
        panic!("array result declaration")
    };
    for drift in [
        Drift::MissingElementDeclaration,
        Drift::WrongResultType,
        Drift::DuplicatedReturnedClaim,
        Drift::BorrowedElementField,
    ] {
        let mut module = lowered.semantic_module.clone();
        match drift {
            Drift::MissingElementDeclaration => {
                module
                    .structural_types
                    .retain(|declaration| declaration.id != element);
            }
            Drift::BorrowedElementField => {
                let declaration = module
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == element)
                    .unwrap();
                let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
                    panic!("record element");
                };
                fields[0].field_type = StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                );
            }
            Drift::WrongResultType => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[0].result
                else {
                    panic!("structural result")
                };
                result.structural_type = element;
            }
            Drift::DuplicatedReturnedClaim => {
                let Terminator::ReturnStructural {
                    returned_claims, ..
                } = &mut module.machines[0].blocks[0].terminator
                else {
                    panic!("structural return")
                };
                // There is no valid claim in this affine slice. Duplicating an
                // invented claim must not turn the empty transfer into custody.
                returned_claims.extend([ClaimId::new(1).unwrap(); 2]);
            }
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "{drift:?} must fail independent aggregate verification"
        );
    }
}
