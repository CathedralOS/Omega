//! Exact mutable and write-only receiver subloans from plain record fields.

use super::*;
use checked_trees::CheckedUnitStructuralPathSegment;
use terminal_psi::StructuralPathSegment;

#[path = "projections/harness.rs"]
mod harness;
use harness::{assert_projected_receiver, projected_source};

#[path = "projections/rejections.rs"]
mod rejections;

#[test]
fn mutable_parameter_field_receiver_retains_exact_subloan() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
        false,
        false,
        false,
    );
}

#[test]
fn mutable_parameter_nested_receiver_retains_ordered_subloan() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
        true,
        false,
        false,
    );
}

#[test]
fn mutable_self_field_receiver_retains_container_root() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
        false,
        true,
        false,
    );
}

#[test]
fn mutable_self_nested_receiver_retains_container_root() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::MutableBorrow,
        true,
        true,
        false,
    );
}

#[test]
fn write_only_parameter_field_receiver_retains_exact_subloan() {
    assert_projected_receiver(
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::WriteOnlyBorrow,
        false,
        false,
        false,
    );
}

#[test]
fn write_only_parameter_nested_receiver_retains_ordered_subloan() {
    assert_projected_receiver(
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::WriteOnlyBorrow,
        true,
        false,
        false,
    );
}

#[test]
fn write_only_self_field_receiver_retains_container_root() {
    assert_projected_receiver(
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::WriteOnlyBorrow,
        false,
        true,
        false,
    );
}

#[test]
fn write_only_self_nested_receiver_retains_container_root() {
    assert_projected_receiver(
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::WriteOnlyBorrow,
        true,
        true,
        false,
    );
}

#[test]
fn bare_attached_fields_retain_exclusive_receiver_paths() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        for nested in [false, true] {
            assert_projected_receiver(access, access, nested, true, true);
        }
    }
}

#[test]
fn corrupted_projected_receiver_path_type_and_access_reject() {
    assert_corrupted_projected_receiver("mut");
}

#[test]
fn corrupted_attenuated_receiver_path_type_and_access_reject() {
    assert_corrupted_projected_receiver("write");
}

fn assert_corrupted_projected_receiver(callee_borrow: &str) {
    use terminal_verifier::{ModuleError, VerificationError};

    // A scalar-free caller reaches the precise access diagnostic rather than
    // the earlier mixed-signature gate after the exclusive operand is corrupted.
    let (source, caller_name, _) =
        projected_source("mut", callee_borrow, true, false, false, false, false);
    let checked = checked_from_source(&source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, caller_name).unwrap();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("uncorrupted source artifact verifies");
    for corruption in [
        "unknown field",
        "reordered path",
        "erased path",
        "target type",
        "argument access",
        "source access",
    ]
    .into_iter()
    .chain((callee_borrow == "write").then_some("upgraded argument"))
    {
        let mut corrupted = module.clone();
        let caller = corrupted
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut caller.blocks[0].operations[0].kind
        else {
            panic!("one projected receiver call")
        };
        let argument = &mut structural_arguments[0];
        match corruption {
            "unknown field" => argument.path[0] = StructuralPathSegment::Field("missing".into()),
            "reordered path" => argument.path.reverse(),
            "erased path" => argument.path.clear(),
            "argument access" => argument.access = StructuralAccess::SharedBorrow,
            "upgraded argument" => argument.access = StructuralAccess::MutableBorrow,
            "source access" => {
                caller.structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            "target type" => {
                let root_type = caller.structural_parameters[0].structural_type;
                let root = corrupted
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == root_type)
                    .unwrap();
                let StructuralTypeShape::Record { fields } = &mut root.shape else {
                    panic!("container root")
                };
                // The same field identity now resolves to a scalar instead of Inner.
                fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                ));
            }
            _ => unreachable!(),
        }
        let Err(error) = terminal_verifier::verify_module(&corrupted, &proof, &profile) else {
            panic!("independent verification must reject {corruption}")
        };
        match corruption {
            "source access" => assert!(matches!(
                error,
                VerificationError::Module(ModuleError::StructuralArgumentAccessExceedsSource {
                    source: StructuralAccess::SharedBorrow,
                    ..
                })
            )),
            "argument access" | "upgraded argument" => assert!(matches!(
                error,
                VerificationError::Module(ModuleError::StructuralArgumentAccessMismatch { .. })
            )),
            _ => {}
        }
    }
}

#[test]
fn mutable_parameter_field_receiver_attenuates_to_write_only() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        false,
        false,
        false,
    );
}

#[test]
fn mutable_parameter_nested_receiver_attenuates_to_write_only() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        true,
        false,
        false,
    );
}

#[test]
fn mutable_self_field_receiver_attenuates_to_write_only() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        false,
        true,
        false,
    );
}

#[test]
fn mutable_self_nested_receiver_attenuates_to_write_only() {
    assert_projected_receiver(
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        true,
        true,
        false,
    );
}

#[test]
fn mutable_bare_attached_fields_attenuate_to_write_only() {
    for nested in [false, true] {
        assert_projected_receiver(
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
            nested,
            true,
            true,
        );
    }
}
