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
fn verified_local_array_store_retains_its_owned_producer() {
    let source = r#"
        machine fill() {
            let mut values: [i32; 2];
            values[1] = 42;
            transition values[1] == 42 {
                true -> done()
                false -> done()
            }
            state done() {}
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("fill"),
    )
    .expect("local array state graph");
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
    .expect("owned projected store survives artifact admission")
    .into_plan();
    let function = plan
        .functions
        .iter()
        .find(|function| {
            function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::WriteOnlyPrimitiveStore { .. })
            })
        })
        .expect("store function");
    let (producer, destination) = function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::WriteOnlyPrimitiveStore {
                destination:
                    abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                        producer,
                        result,
                    },
                ..
            } => Some((*producer, result)),
            _ => None,
        })
        .expect("owned destination, not a fabricated ABI parameter");
    assert!(function.structural_parameters.is_empty());
    assert!(
        function
            .operations
            .iter()
            .any(|operation| matches!(operation,
                AbstractOperation::EstablishZeroedStructural { psi_operation, result }
                if *psi_operation == producer && result == destination
            ))
    );
}

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
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
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
    let destination = destination
        .parameter()
        .expect("borrowed parameter destination");
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
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
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
    let destination = destination
        .parameter()
        .expect("borrowed parameter destination");
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
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
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
    let destination = store.0.parameter().expect("borrowed array parameter");
    assert_eq!(destination, &function.structural_parameters[0]);
    assert_eq!(destination.access, StructuralAccess::MutableBorrow);
    assert_eq!(
        destination.multiplicity,
        StructuralMultiplicity::Unrestricted
    );
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
fn verified_owned_array_read_retains_exact_parameter_and_projection() {
    let source = r#"
        data Output { value: u64; }
        machine Output::from_array(values: [u64; 2]) -> Output {
            Output { value: values[1] }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Output::from_array"),
    )
    .expect("owned array projection reaches verified Terminal Psi");
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
    .expect("owned projection reaches native abstract operations");
    let (function, read) = plan
        .functions
        .iter()
        .find_map(|function| {
            function
                .operations
                .iter()
                .find(|operation| {
                    matches!(operation, AbstractOperation::PrimitiveScalarRead { .. })
                })
                .map(|read| (function, read))
        })
        .expect("projected read");
    let AbstractOperation::PrimitiveScalarRead {
        source,
        path,
        result,
        ..
    } = read
    else {
        unreachable!();
    };
    let parameter = &function.structural_parameters[0];
    assert_eq!(*source, parameter.place);
    assert_eq!(parameter.access, StructuralAccess::Owned);
    assert_eq!(parameter.multiplicity, StructuralMultiplicity::Unrestricted);
    assert_eq!(path, &[terminal_psi::StructuralPathSegment::FixedIndex(1)]);
    assert_eq!(
        result.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    );
}

/// `board.cells[index].hp` read as a store value: Terminal spells one
/// primitive read whose path selects the element at run time and ends at the
/// element's `hp` field, and target-neutral Omega keeps that exact path. A
/// substituted final field (`tag`, a `u8`, for an `i32` read) or a field the
/// element does not declare is refused before any abstract operation exists.
#[test]
fn verified_runtime_element_field_read_retains_its_exact_path() {
    let source = r#"
        data Cell { tag: u8; hp: i32; }
        data Board { cells: [Cell; 4]; scratch: i32; }
        machine copy(board: &mut Board, index: u64 [0..=3]) {
            board.scratch = board.cells[index].hp;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("copy"),
    )
    .expect("runtime element field read lowers to verified Terminal Psi");
    fn element_path(leaf: &str, path: &[terminal_psi::StructuralPathSegment]) -> bool {
        matches!(path, [
            terminal_psi::StructuralPathSegment::Field(array),
            terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
            terminal_psi::StructuralPathSegment::Field(field),
        ] if array == "cells" && field == leaf)
    }
    // The codec's module check, Terminal verification and stage 00 each
    // rebuild the read's leaf; `None` is a refusal by any of them.
    let lower = |module: &terminal_psi::TerminalModule| {
        let semantic = encode_module(module).ok()?;
        let proof = encode_proof_section(module, &terminal.proof_bundle).ok()?;
        lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &semantic,
                proof_bytes: &proof,
                obligation_ledger_bytes: None,
            },
            &AdmissionProfile::default(),
        )
        .ok()
        .map(|admitted| admitted.into_plan())
    };
    let plan = lower(&terminal.semantic_module)
        .expect("verified runtime element field read reaches target-neutral Omega");
    let read = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .find_map(|operation| match operation {
            AbstractOperation::PrimitiveScalarRead { path, result, .. } => Some((path, result)),
            _ => None,
        })
        .expect("one runtime element field read");
    assert!(element_path("hp", read.0), "{:?}", read.0);
    assert_eq!(
        read.1.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    );

    for leaf in ["tag", "missing"] {
        let mut changed = terminal.semantic_module.clone();
        let path = changed
            .machines
            .iter_mut()
            .flat_map(|machine| machine.blocks.iter_mut())
            .flat_map(|block| block.operations.iter_mut())
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::PrimitiveScalarRead { path, .. }
                    if element_path("hp", path) =>
                {
                    Some(path)
                }
                _ => None,
            })
            .expect("the Terminal read");
        *path.last_mut().unwrap() = terminal_psi::StructuralPathSegment::Field(leaf.into());
        assert!(lower(&changed).is_none(), "a substituted {leaf} field");
    }
}
