use abstract_operations::AbstractOperation;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn verified_source_store_retains_exact_mutable_parameter_and_preceding_value() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &mut i32) {
            destination = 2;
        }

        data Harness {}
        machine Harness::exercise(root: &mut i32) {
            let parent: &mut i32 = &mut root;
            let child: &write i32 = &write parent;
            Sink::fill(parent);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Harness::exercise")
        .expect("mutable source store lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified write-only store reaches target-neutral Omega");

    let store_function = plan
        .functions
        .iter()
        .find(|function| {
            function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })
        })
        .expect("one function retains the non-observing store");
    let store_index = store_function
        .operations
        .iter()
        .position(|operation| {
            matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
        })
        .expect("store operation index");
    let AbstractOperation::WriteOnlyPrimitiveStore {
        destination, value, ..
    } = &store_function.operations[store_index]
    else {
        unreachable!("store index selects the store")
    };
    assert_eq!(destination, &store_function.structural_parameters[0]);
    assert_eq!(destination.position, 0);
    assert!(!destination.is_self);
    assert_eq!(destination.access, StructuralAccess::MutableBorrow);
    assert_eq!(
        destination.multiplicity,
        StructuralMultiplicity::Unrestricted
    );
    assert!(destination.qualifications.is_empty());
    let i32_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    assert_eq!(value.scalar_type, i32_type);
    assert!(
        store_function.operations[..store_index]
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    AbstractOperation::IntegerConstant {
                        result,
                        scalar_type,
                        value: IntegerValue::Signed(2),
                        ..
                    } if *result == value.value && *scalar_type == i32_type
                )
            })
    );
}

#[test]
fn verified_boolean_store_retains_exact_write_only_parameter_and_preceding_value() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write bool) {
            destination = true;
        }

        data Root {}
        machine Root::enter(destination: &mut bool) {
            Sink::fill(&write destination);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("write-only Boolean source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified Boolean store reaches target-neutral Omega");

    let store_function = plan
        .functions
        .iter()
        .find(|function| {
            function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })
        })
        .expect("one function retains the Boolean store");
    let store_index = store_function
        .operations
        .iter()
        .position(|operation| {
            matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
        })
        .expect("store operation index");
    let AbstractOperation::WriteOnlyPrimitiveStore {
        destination, value, ..
    } = &store_function.operations[store_index]
    else {
        unreachable!("store index selects the store")
    };
    assert_eq!(destination, &store_function.structural_parameters[0]);
    assert_eq!(destination.access, StructuralAccess::WriteOnlyBorrow);
    assert_eq!(value.scalar_type, ScalarType::Boolean);
    assert!(
        store_function.operations[..store_index]
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    AbstractOperation::BooleanConstant {
                        result,
                        value: true,
                        ..
                    } if *result == value.value
                )
            })
    );
}

#[test]
fn verified_ieee_float_store_retains_exact_write_only_parameter_and_preceding_value() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write f32) {
            destination = 1.25f32;
        }

        data Root {}
        machine Root::enter(destination: &mut f32) {
            Sink::fill(&write destination);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("write-only IEEE float source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified IEEE float store reaches target-neutral Omega");

    let store_function = plan
        .functions
        .iter()
        .find(|function| {
            function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })
        })
        .expect("one function retains the IEEE float store");
    let store_index = store_function
        .operations
        .iter()
        .position(|operation| {
            matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
        })
        .expect("store operation index");
    let AbstractOperation::WriteOnlyPrimitiveStore {
        destination, value, ..
    } = &store_function.operations[store_index]
    else {
        unreachable!("store index selects the store")
    };
    assert_eq!(destination, &store_function.structural_parameters[0]);
    assert_eq!(destination.access, StructuralAccess::WriteOnlyBorrow);
    assert_eq!(
        value.scalar_type,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)
    );
    assert!(
        store_function.operations[..store_index]
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    AbstractOperation::IeeeFloatConstant {
                        result,
                        value: IeeeFloatValue::Binary32(0x3fa0_0000),
                        ..
                    } if *result == value.value
                )
            })
    );
}

#[test]
fn verified_fixed_integer_parameter_store_retains_exact_runtime_source() {
    let source = r#"
        data Sink {}
        machine Sink::fill(destination: &write i32, replacement: i32) {
            destination = replacement;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Sink::fill")
        .expect("write-only fixed-integer parameter store lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified parameter store reaches target-neutral Omega");

    let [function] = plan.functions.as_slice() else {
        panic!("one store function")
    };
    let [replacement] = function.parameters.as_slice() else {
        panic!("one runtime scalar source")
    };
    let store = function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::WriteOnlyPrimitiveStore {
                destination, value, ..
            } => Some((destination, value)),
            _ => None,
        })
        .expect("one parameter-sourced primitive store");
    assert_eq!(store.0, &function.structural_parameters[0]);
    assert_eq!(store.0.access, StructuralAccess::WriteOnlyBorrow);
    assert_eq!(store.1.value, replacement.value);
    assert_eq!(store.1.scalar_type, replacement.scalar_type);
    assert_eq!(
        replacement.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    );
    assert!(!function.operations.iter().any(|operation| matches!(
        operation,
        AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::IeeeFloatConstant { .. }
    )));
}
