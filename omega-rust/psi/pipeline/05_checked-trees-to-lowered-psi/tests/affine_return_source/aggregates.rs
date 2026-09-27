use super::{assert_identity_execution, checked};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ClaimId;
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalMachineResult, Terminator};

const RECORD_ARRAY: &str = "
    data Entry { number: u64; }
    machine forward(values: [Entry; 3]) -> [Entry; 3] { values }
";

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
fn empty_arrays_retain_the_existing_terminal_carrier_fence() {
    assert_no_affine_plan(
        "data Entry { number: u64; }
         machine forward(value: [Entry; 0]) -> [Entry; 0] { value }",
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
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("forward")).is_err(),
        "{source}"
    );
}

#[test]
fn reference_and_slice_fields_have_no_affine_whole_result_plan() {
    // Reference fields follow expressions/borrow_carrying_data_field_exit;
    // slice fields follow domains/utf8_field_write_from_param.
    for field_type in ["&mut Entry", "&[u8]", "&mut [Entry]"] {
        assert_no_affine_plan(&format!(
            "data Entry {{ number: u64; }}
             data Inner {{ field: {field_type}; }}
             data Outer {{ inner: Inner; }}
             machine forward(value: Outer) -> Outer {{ value }}"
        ));
    }
    // A shared reference field leaves the record unrestricted, so its
    // identity return lowers through the ordinary route without a
    // claim-free affine producer.
    let shared = checked(
        "data Entry { number: u64; }
         data Inner { field: &Entry; }
         data Outer { inner: Inner; }
         machine forward(value: Outer) -> Outer { value }",
    );
    assert!(
        shared
            .facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_machines
            .is_empty()
    );
    lower_machine(&shared, TerminalMachineSelection::Name("forward"))
        .expect("a shared reference field's identity return lowers");
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
    let lowered = lower_machine(
        &checked(RECORD_ARRAY),
        TerminalMachineSelection::Name("forward"),
    )
    .expect("array identity lowers");
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
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    },
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
