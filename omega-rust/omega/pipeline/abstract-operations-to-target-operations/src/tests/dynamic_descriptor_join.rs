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
    source_plan(source)
}

fn source_plan(source: &str) -> abstract_operations::AbstractOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::run")
        .expect("lower joined dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact")
}

#[test]
fn dynamic_calls_compose_with_definitions_and_reordered_blocks() {
    let mut source = joined_plan();
    let caller = source
        .functions
        .iter_mut()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let AbstractOperation::CallStructuralScalarWithDynamicArguments { result, .. } =
        &caller.operations[1]
    else {
        panic!("first dynamic call");
    };
    let result = result.value;
    caller.operations.insert(
        2,
        AbstractOperation::BooleanNot {
            psi_operation: OperationId::new(90001).unwrap(),
            result: ValueId::new(90001).unwrap(),
            operand: result,
        },
    );
    caller.operations.insert(
        1,
        AbstractOperation::BooleanConstant {
            psi_operation: OperationId::new(90000).unwrap(),
            result: ValueId::new(90000).unwrap(),
            value: false,
        },
    );
    caller.block_entries[2].operation_offset += 2;
    let second_offset = caller.block_entries[2].operation_offset;
    caller.operations[1..].rotate_left(second_offset - 1);
    caller.block_entries.swap(1, 2);
    caller.block_entries[1].operation_offset = 1;
    caller.block_entries[2].operation_offset = 3;
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered =
            lower_to_target_operations(&source, target).expect("ordinary operation sequencing");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap();
        let TargetOperation::ControlGraph(graph) = &caller.operation else {
            panic!("ordinary graph")
        };
        assert_eq!(graph.blocks[2].operations.len(), 3);
        assert!(matches!(
            graph.blocks[2].operations[1],
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments { .. }
        ));
        assert!(
            caller
                .provenance
                .operations
                .contains(&OperationId::new(90001).unwrap())
        );
    }
}

#[test]
fn dynamic_graph_calls_reject_wrong_custody_and_duplicate_results() {
    for mutation in 0..4 {
        let mut source = joined_plan();
        let caller = source
            .functions
            .iter_mut()
            .find(|function| function.machine == source.entry)
            .unwrap();
        let parameter = caller.parameters[0].value;
        let AbstractOperation::CallStructuralScalarWithDynamicArguments {
            dynamic_arguments,
            result,
            ..
        } = &mut caller.operations[1]
        else {
            panic!("dynamic call")
        };
        match mutation {
            0 => dynamic_arguments[0].argument.operation = OperationId::new(90000).unwrap(),
            1 => dynamic_arguments[0].target.owner = caller.machine,
            2 => dynamic_arguments[0].argument.parameter_ordinal += 1,
            3 => result.value = parameter,
            _ => unreachable!(),
        }
        assert!(lower_to_target_operations(&source, NativeTarget::linux_x64()).is_err());
    }
}

#[test]
fn dynamic_unit_calls_keep_resultless_abi_in_ordinary_branches() {
    let source = source_plan(
        r#"
        trait Touch { machine touch(&self); }
        data Item [copy] { marker: bool; }
        Implementation: Item satisfies Touch { machine touch(&self) {} }
        data Main [copy] { first: Item; second: Item; }
        machine Main::run(&self, choose_first: bool) {
            transition choose_first { true -> first() _ -> second() }
            state first(&self) {
                let selected: &dyn Touch = &self.first as &dyn Item::Implementation;
                finish(selected);
            }
            state second(&self) {
                let selected: &dyn Touch = &self.second as &dyn Item::Implementation;
                finish(selected);
            }
        }
        machine finish(erased: &dyn Touch) { erased.touch(); }
    "#,
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered =
            lower_to_target_operations(&source, target).expect("resultless descriptor calls");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == source.entry)
            .unwrap();
        let TargetOperation::ControlGraph(graph) = &caller.operation else {
            panic!("ordinary graph")
        };
        for branch in &graph.blocks[1..] {
            let [
                TargetUnitOperation::StructuralUnitCallWithDynamicArguments {
                    call_plan,
                    dynamic_arguments,
                    ..
                },
            ] = branch.operations.as_slice()
            else {
                panic!("resultless dynamic call")
            };
            assert!(call_plan.result.is_none());
            assert_eq!(dynamic_arguments.len(), 1);
            assert_eq!(call_plan.parameters.len(), 2);
        }
    }
}

#[test]
fn lowers_joined_descriptor_predecessors_without_a_representative_table() {
    let source = joined_plan();
    let authored = source
        .functions
        .iter()
        .find(|function| function.machine == source.entry)
        .unwrap();
    let expected_operations = authored
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::CallStructuralScalarWithDynamicArguments {
                psi_operation, ..
            } => Some(*psi_operation),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_edges = authored
        .operations
        .iter()
        .flat_map(|operation| match operation {
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.psi_edge, when_false.psi_edge],
            AbstractOperation::ReturnUnit { psi_edge, .. } => vec![*psi_edge],
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let lowered = lower_to_target_operations(&source, target)
            .expect("joined descriptor control reaches target operations");
        let caller = lowered
            .functions
            .iter()
            .find(|function| function.machine == lowered.entry)
            .expect("joined entry caller");
        assert_eq!(caller.provenance.operations, expected_operations);
        assert_eq!(caller.provenance.edges, expected_edges);
        let TargetOperation::ControlGraph(graph) = &caller.operation else {
            panic!("joined caller uses ordinary control blocks")
        };
        let [condition] = graph.scalar_parameters.as_slice() else {
            panic!("joined caller has one Boolean ABI parameter")
        };
        assert_eq!(condition.scalar_type, ScalarType::Boolean);
        assert_eq!(condition.placement.shape, ValueShape::integer(1, 1));
        let [entry, first, second] = graph.blocks.as_slice() else {
            panic!("three authored blocks");
        };
        let target_operations::TargetControlTerminator::Conditional {
            condition_source,
            when_true,
            when_false,
            ..
        } = &entry.terminator
        else {
            panic!("ordinary Boolean branch");
        };
        let [
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                callee: first_callee,
                dynamic_arguments: first_arguments,
                ..
            },
        ] = first.operations.as_slice()
        else {
            panic!("first descriptor call");
        };
        let [
            TargetUnitOperation::StructuralScalarCallWithDynamicArguments {
                callee: second_callee,
                dynamic_arguments: second_arguments,
                ..
            },
        ] = second.operations.as_slice()
        else {
            panic!("second descriptor call");
        };
        assert_eq!(*condition_source, condition.value);
        assert_eq!(when_true.target, first.block);
        assert_eq!(when_false.target, second.block);
        for branch in [first, second] {
            assert!(
                matches!(branch.terminator, target_operations::TargetControlTerminator::Return { ref cleanup_actions, .. } if cleanup_actions.is_empty())
            );
        }
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
