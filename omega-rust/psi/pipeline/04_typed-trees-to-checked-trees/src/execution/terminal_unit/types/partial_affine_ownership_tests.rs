use super::DataMember;
use crate::execution::terminal_unit::types::partial_affine_source_contents_are_owned;
use crate::tests::front_end::typed_program;

fn field_is_owned(spelling: &str) -> bool {
    let source = format!("data Carrier {{ value: {spelling}; }}");
    let program = typed_program(&source);
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Carrier")
        .unwrap();
    let [DataMember::Field(field)] = program.data_members(data) else {
        panic!("one source field")
    };
    partial_affine_source_contents_are_owned(&program, field.type_reference, &mut Vec::new())
}

#[test]
fn scalar_ranges_and_policies_retain_owned_storage_classification() {
    for spelling in [
        "u64",
        "f64",
        "u64 [0..=100]",
        "i8 [-10..=10]",
        "u8 in Wrapping",
        "i8 in Saturating",
    ] {
        assert!(
            field_is_owned(spelling),
            "{spelling} is an owned scalar, not a reference"
        );
    }
}

#[test]
fn scalar_constraints_do_not_erase_reference_access() {
    for spelling in [
        "&u64",
        "&mut u64",
        "&write u64",
        "&u64 [0..=100]",
        "&mut u8 in Wrapping",
    ] {
        assert!(
            !field_is_owned(spelling),
            "{spelling} must not become owned through its scalar referent"
        );
    }
}
