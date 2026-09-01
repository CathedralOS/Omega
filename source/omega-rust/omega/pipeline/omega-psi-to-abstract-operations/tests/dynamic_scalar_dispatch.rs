use omega_abstract_operations::AbstractOperation;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use psi_core::{IntegerSign, IntegerType, ScalarType};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::StructuralPathSegment;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn verified_rebound_dynamic_call_retains_versions_and_indirect_row() {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.measure();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("rebound dynamic source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified rebound dispatch reaches target-neutral Omega");

    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let dynamic_calls = caller
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::CallDynamicScalar {
                result,
                dynamic_dispatch,
                ..
            } => Some((result, dynamic_dispatch)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(result, dynamic)] = dynamic_calls.as_slice() else {
        panic!("one abstract rebound dynamic call expected: {caller:#?}")
    };

    assert_eq!(
        result.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    );
    assert_eq!(dynamic.descriptor.owner, caller.machine);
    assert_eq!(dynamic.dispatch.owner, caller.machine);
    assert_eq!(dynamic.dispatch.descriptor_ordinal, dynamic.descriptor.ordinal);
    assert_eq!(
        dynamic.initial.ordinal,
        dynamic.descriptor.initial_selection_ordinal
    );
    assert_eq!(
        dynamic.rebound.ordinal,
        dynamic.descriptor.rebound_selection_ordinal
    );
    assert_eq!(
        dynamic.initial.conformance_application_commitment,
        dynamic.rebound.conformance_application_commitment
    );
    assert_eq!(
        dynamic.initial.source.path,
        vec![StructuralPathSegment::Field("decoy".into())]
    );
    assert_eq!(
        dynamic.rebound.source.path,
        vec![StructuralPathSegment::Field("selected".into())]
    );
    assert!(
        plan.functions
            .iter()
            .any(|function| function.machine == dynamic.dispatch.realization),
        "private table row must resolve to one retained realization function"
    );
    assert!(
        !caller.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::CallStructuralScalar { callee, .. }
                    if *callee == dynamic.dispatch.realization
            )
        }),
        "rebound dispatch must not be substituted with a direct structural call"
    );
}
