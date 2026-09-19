//! Boundary loans preserve the fixed array's type, extent and exclusive access.

use crate::tests::{refresh_identity, refresh_node_derivatives};
use crate::validate_psi_optimization_unit;
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use terminal_psi::{StructuralAccess, StructuralTypeShape};

fn boundary_array_unit(nested: bool) -> PsiOptimizationUnit {
    let source = if nested {
        r#"
        pub data Bytes { prefix: u64; bytes: [u8; 8]; }
        pub data Buffer { prefix: u64; inner: Bytes; }
        boundary trait Sink { machine take(bytes: &mut [u8]) reaches Sink; }
        machine enter(buffer: &mut Buffer) reaches Sink {
            Sink::take(&mut buffer.inner.bytes);
        }
        "#
    } else {
        r#"
        pub data Buffer { prefix: u64; bytes: [u8; 8]; }
        boundary trait Sink { machine take(bytes: &mut [u8]) reaches Sink; }
        machine enter(buffer: &mut Buffer) reaches Sink { Sink::take(&mut buffer.bytes); }
        "#
    };
    source_unit(source)
}

fn source_unit(source: &str) -> PsiOptimizationUnit {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "enter").unwrap();
    let semantic = terminal_codec::encode_module(&terminal.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
            .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("independent Terminal verification accepts the exact fixed-array loan");
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
    .into_parts()
    .1
}

#[test]
fn boundary_fixed_byte_views_preserve_source_projection_and_reject_corruption() {
    for nested in [false, true] {
        let baseline = boundary_array_unit(nested);
        validate_psi_optimization_unit(&baseline)
            .expect("verified boundary array loans survive abstract reconstruction");
        let caller = baseline
            .functions
            .iter()
            .position(|function| function.machine == baseline.entry)
            .unwrap();
        let call = baseline.functions[caller].blocks[0]
            .nodes
            .iter()
            .position(|node| matches!(node.operation, AbstractOperation::BoundaryCall { .. }))
            .unwrap();
        for corruption in ["access", "path", "empty", "element", "alias"] {
            let mut changed = baseline.clone();
            let AbstractOperation::BoundaryCall {
                structural_arguments,
                ..
            } = &mut changed.functions[caller].blocks[0].nodes[call].operation
            else {
                panic!("boundary call")
            };
            match corruption {
                "access" => structural_arguments[0].access = StructuralAccess::SharedBorrow,
                "path" => structural_arguments[0]
                    .path
                    .push(terminal_psi::StructuralPathSegment::FixedIndex(0)),
                "alias" => {
                    structural_arguments.push(structural_arguments[0].clone());
                    let mut parameter =
                        changed.boundary_machines[0].structural_parameters[0].clone();
                    parameter.position = 1;
                    parameter.place = semantic_vocabulary::PlaceId::new(900).unwrap();
                    changed.boundary_machines[0]
                        .parameter_order
                        .push(terminal_psi::BoundaryParameterKind::Structural);
                    changed.boundary_machines[0]
                        .structural_parameters
                        .push(parameter);
                }
                "empty" | "element" => {
                    let mut types = changed.structural_types.to_vec();
                    let array = types
                        .iter_mut()
                        .find(|declaration| {
                            matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                        })
                        .unwrap();
                    let StructuralTypeShape::FixedArray { element, length } = &mut array.shape
                    else {
                        unreachable!()
                    };
                    if corruption == "empty" {
                        *length = 0;
                    } else {
                        let element_id = *element;
                        let element = types
                            .iter_mut()
                            .find(|declaration| declaration.id == element_id)
                            .unwrap();
                        element.shape = StructuralTypeShape::PrimitiveScalar(
                            semantic_vocabulary::ScalarType::Boolean,
                        );
                    }
                    changed.structural_types = types.into();
                }
                _ => unreachable!(),
            }
            refresh_node_derivatives(&mut changed, caller, 0, call);
            refresh_identity(&mut changed);
            assert!(
                validate_psi_optimization_unit(&changed).is_err(),
                "{corruption}, nested={nested}"
            );
        }
    }
}
