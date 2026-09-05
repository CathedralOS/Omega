use crate::{AssignmentError, assign_registers};
use abstract_operations_to_target_operations::lower_to_target_operations;
use assigned_target_operations::{AssignedOperation, AssignedUnitOperation};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{ScalarType, ValueId};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::{AbstractDynamicDescriptorSource, TargetOperation, TargetUnitOperation};
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::StructuralPathSegment;
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn target_plan(target: NativeTarget) -> target_operations::TargetOperationPlan {
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
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    lower_to_target_operations(&abstract_plan, target).expect("lower joined target control")
}

#[test]
fn assigns_joined_boolean_control_and_both_exact_descriptor_arguments() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = target_plan(target);
        let assigned = assign_registers(&target_plan).expect("assign joined descriptor control");
        let caller = assigned
            .functions
            .iter()
            .find(|function| function.machine == assigned.entry)
            .expect("joined entry caller");
        let AssignedOperation::UnitBody(body) = &caller.operation else {
            panic!("joined caller remains a Unit body")
        };
        let [
            AssignedUnitOperation::ConditionalBooleanParameter {
                condition,
                when_true,
                when_false,
                ..
            },
            AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                callee: first_callee,
                dynamic_arguments: first_arguments,
                ..
            },
            AssignedUnitOperation::Return { .. },
            AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
                callee: second_callee,
                dynamic_arguments: second_arguments,
                ..
            },
            AssignedUnitOperation::Return { .. },
        ] = body.operations.as_slice()
        else {
            panic!("assigned join keeps both physical arms: {body:#?}")
        };
        assert_eq!(condition.scalar_type, ScalarType::Boolean);
        assert_eq!(when_true.operation_ordinal, 1);
        assert_eq!(when_false.operation_ordinal, 3);
        assert_eq!(first_callee, second_callee);
        let ([first_argument], [second_argument]) =
            (first_arguments.as_slice(), second_arguments.as_slice())
        else {
            panic!("each assigned arm has one descriptor argument")
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
            panic!("assignment keeps the two selection sources")
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

        let mut corrupted = target_plan.clone();
        let drifted_value = {
            let caller = corrupted
                .functions
                .iter_mut()
                .find(|function| function.machine == corrupted.entry)
                .expect("joined target caller");
            let TargetOperation::UnitBody(body) = &mut caller.operation else {
                unreachable!("joined target caller is Unit")
            };
            let TargetUnitOperation::ConditionalBooleanParameter { condition, .. } =
                &mut body.operations[0]
            else {
                unreachable!("joined target starts with its guard")
            };
            condition.value = ValueId::new(condition.value.get() + 100).expect("drifted value");
            condition.value
        };
        assert_eq!(
            assign_registers(&corrupted),
            Err(AssignmentError::UnitScalarCallSourceMismatch(drifted_value))
        );
    }
}
