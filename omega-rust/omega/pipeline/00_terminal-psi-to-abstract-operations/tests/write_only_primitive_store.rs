use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};
use terminal_psi_to_abstract_operations::lower_artifact;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Harness::exercise"),
    )
    .expect("mutable source store lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
        .expect("encode proof");
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("write-only Boolean source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
        .expect("encode proof");
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("write-only IEEE float source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
        .expect("encode proof");
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
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
fn verified_runtime_indexed_store_retains_index_value_and_bounds_obligation() {
    let source = r#"
        machine forward(values: &mut [u16; 4], index: u64 [0..=3]) {
            values[index] = 17;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("forward"),
    )
    .expect("declared-range runtime index store lowers to verified Terminal Psi");
    assert!(
        terminal
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .any(|operation| matches!(
                &operation.kind,
                terminal_psi::OperationKind::WriteOnlyPrimitiveStore { path, .. }
                    if !terminal_psi::is_static_structural_path(path)
            )),
        "Terminal Psi retains the runtime-indexed store"
    );
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
        .expect("encode proof");
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
    .expect("verified runtime-indexed store reaches target-neutral Omega");

    let [function] = plan.functions.as_slice() else {
        panic!("one store function")
    };
    let u64_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let u16_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    let [index_parameter] = function.parameters.as_slice() else {
        panic!("one runtime scalar index")
    };
    assert_eq!(index_parameter.scalar_type, u64_type);
    // The runtime element stays the path's own segment: one store, whose
    // selector and verified obligation ride in the path.
    let store = function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::WriteOnlyPrimitiveStore {
                destination,
                path,
                value,
                ..
            } => Some((destination, path, value)),
            _ => None,
        })
        .expect("one runtime-indexed primitive store");
    assert_eq!(store.0, &function.structural_parameters[0]);
    assert_eq!(store.0.access, StructuralAccess::MutableBorrow);
    assert_eq!(store.0.multiplicity, StructuralMultiplicity::Unrestricted);
    let [terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. }] = store.1.as_slice()
    else {
        panic!("the store path is one runtime element: {:?}", store.1)
    };
    assert_eq!(*index, index_parameter.value);
    assert_eq!(store.2.scalar_type, u16_type);
    assert!(
        function.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::IntegerConstant {
                    result,
                    scalar_type,
                    value: IntegerValue::Unsigned(17),
                    ..
                } if *result == store.2.value && *scalar_type == u16_type
            )
        }),
        "the stored scalar rejoins its exact preceding definition"
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
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Sink::fill"),
    )
    .expect("write-only fixed-integer parameter store lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
        .expect("encode proof");
    let plan = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
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

/// Lower one machine of `source` through checked trees and verified,
/// serialized Terminal Psi to the target-neutral Omega plan.
pub(super) fn verified_plan(
    source: &str,
    machine: &str,
) -> abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(machine),
    )
    .expect("the store lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
        .expect("encode proof");
    lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &AdmissionProfile::default(),
    )
    .map(|admitted| admitted.into_plan())
    .expect("the verified store reaches target-neutral Omega")
}

#[test]
fn verified_double_indexed_store_is_one_store_over_both_runtime_elements() {
    // `grid[row][column]` has no single trailing element: both selectors ride
    // as path segments of the one primitive store, each with the obligation
    // the verifier discharged, where the retired indexed store took one.
    let plan = verified_plan(
        r#"
        machine forward(grid: &mut [[u16; 4]; 3], row: u64 [0..=2], column: u64 [0..=3]) {
            grid[row][column] = 17;
        }
        "#,
        "forward",
    );
    let [function] = plan.functions.as_slice() else {
        panic!("one store function")
    };
    let [row, column] = function.parameters.as_slice() else {
        panic!("two runtime selectors")
    };
    let path = function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::WriteOnlyPrimitiveStore { path, .. } => Some(path),
            _ => None,
        })
        .expect("one primitive store");
    let [
        terminal_psi::StructuralPathSegment::RuntimeIndex {
            index: outer,
            obligation: outer_bound,
        },
        terminal_psi::StructuralPathSegment::RuntimeIndex {
            index: inner,
            obligation: inner_bound,
        },
    ] = path.as_slice()
    else {
        panic!("the store path is two runtime elements: {path:?}")
    };
    assert_eq!((*outer, *inner), (row.value, column.value));
    assert_ne!(outer_bound, inner_bound, "each element owns its obligation");
}
