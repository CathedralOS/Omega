use abstract_operations::AbstractDynamicDescriptorSource;
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

use super::prelude::*;
use crate::lower_to_target_operations;

fn joined_plan() -> abstract_operations::AbstractOperationPlan {
    let source = r#"
        trait Measure { machine measure(&self) -> bool; }
        data Item [copy] { marker: bool; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.marker } }
        }
        Secondary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.marker } }
        }
        data Main [copy] { first: Item; second: Item; }
        machine Main::run(&self, choose_first: bool) {
            transition choose_first {
                true -> take_first()
                _ -> take_second()
            }
            state take_first(&self) {
                let selected: &dyn Measure = &self.first as &dyn Item::Primary;
                let result: bool = finish(selected);
            }
            state take_second(&self) {
                let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
                let result: bool = finish(selected);
            }
        }
        machine finish(erased: &dyn Measure) -> bool {
            let result: bool = erased.measure();
            transition { _ -> result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("lower joined dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

#[test]
fn lowers_joined_descriptor_predecessors_without_a_representative_table() {
    let source = joined_plan();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower_to_target_operations(&source, target)
            .expect("joined descriptor control reaches target operations");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == lowered.entry)
            .expect("joined entry caller");
        let TargetOperation::UnitBody(body) = &caller.operation else {
            panic!("joined caller must remain an attached Unit body")
        };
        let [condition] = body.scalar_parameters.as_slice() else {
            panic!("joined caller has one Boolean ABI parameter")
        };
        assert_eq!(condition.scalar_type, ScalarType::Boolean);
        assert_eq!(condition.placement.shape, ValueShape::integer(1, 1));
        let [
            TargetUnitOperation::ConditionalBooleanParameter {
                condition: branch_condition,
                when_true,
                when_false,
            },
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                callee: first_callee,
                dynamic_arguments: first_arguments,
                ..
            },
            TargetUnitOperation::Return { .. },
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                callee: second_callee,
                dynamic_arguments: second_arguments,
                ..
            },
            TargetUnitOperation::Return { .. },
        ] = body.operations.as_slice()
        else {
            panic!("joined caller preserves its conditional branch bodies: {body:#?}")
        };
        assert_eq!(branch_condition, condition);
        assert_eq!(when_true.operation_ordinal, 1);
        assert_eq!(when_false.operation_ordinal, 3);
        assert_eq!(first_callee, second_callee);
        let ([first_argument], [second_argument]) =
            (first_arguments.as_slice(), second_arguments.as_slice())
        else {
            panic!("each predecessor supplies one descriptor")
        };
        assert_eq!(
            first_argument.custody.target,
            second_argument.custody.target
        );
        let (
            AbstractDynamicDescriptorSource::Selection {
                selection: first_selection,
                application: first_application,
            },
            AbstractDynamicDescriptorSource::Selection {
                selection: second_selection,
                application: second_application,
            },
        ) = (
            &first_argument.custody.source,
            &second_argument.custody.source,
        )
        else {
            panic!("target custody keeps both exact predecessor selections")
        };
        assert_eq!(
            first_selection.source.path,
            [StructuralPathSegment::Field("first".into())]
        );
        assert_eq!(
            second_selection.source.path,
            [StructuralPathSegment::Field("second".into())]
        );
        assert_ne!(first_application.commitment, second_application.commitment);
    }
}
