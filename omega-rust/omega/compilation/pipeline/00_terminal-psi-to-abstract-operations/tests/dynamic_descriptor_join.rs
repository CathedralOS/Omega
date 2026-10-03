use abstract_operations::{
    AbstractDynamicDescriptorSource, AbstractOperation, AbstractStructuralCallResult,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_unit::{
    recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed,
};
use optimization_unit_semantics::validate_psi_optimization_unit;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::FuelScheduleIdentity;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::StructuralPathSegment;
use terminal_psi_to_abstract_operations::lower_artifact;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> bool;
    }

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

#[test]
fn checked_descriptor_join_retains_both_predecessors_through_optimization_seed() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("joined dynamic source lowers to verified Terminal Psi");
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
    .expect("joined dynamic dispatch reaches target-neutral Omega");

    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("joined entry caller");
    assert_eq!(caller.block_entries.len(), 3);
    assert_eq!(
        caller
            .operations
            .iter()
            .filter(|operation| matches!(operation, AbstractOperation::Conditional { .. }))
            .count(),
        1,
    );
    let calls = caller
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::CallStructural {
                psi_operation,
                callee,
                dynamic_arguments,
                result: AbstractStructuralCallResult::Scalar(_),
                ..
            } if !dynamic_arguments.is_empty() => {
                Some((*psi_operation, *callee, dynamic_arguments))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [first, second] = calls.as_slice() else {
        panic!("two joined branch calls expected: {caller:#?}")
    };
    assert_eq!(first.1, second.1);
    assert_ne!(first.0, second.0);
    let [first_argument] = first.2.as_slice() else {
        panic!("first branch has one descriptor argument")
    };
    let [second_argument] = second.2.as_slice() else {
        panic!("second branch has one descriptor argument")
    };
    assert_eq!(first_argument.target, second_argument.target);
    assert!(first_argument.has_complete_custody(caller.machine, first.0, first.1));
    assert!(second_argument.has_complete_custody(caller.machine, second.0, second.1));
    let (
        AbstractDynamicDescriptorSource::Selection {
            selection: first_selection,
            application: first_application,
        },
        AbstractDynamicDescriptorSource::Selection {
            selection: second_selection,
            application: second_application,
        },
    ) = (&first_argument.source, &second_argument.source)
    else {
        panic!("both joined predecessors must retain exact selections")
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

    let helper = plan
        .functions
        .iter()
        .find(|function| function.machine == first.1)
        .expect("joined descriptor helper");
    assert!(helper.operations.iter().any(|operation| matches!(
        operation,
        AbstractOperation::CallDynamic { dispatch: abstract_operations::AbstractDynamicDispatch::Parameter(dynamic_dispatch), result: abstract_operations::AbstractDynamicCallResult::Scalar(_), .. } if dynamic_dispatch.parameter == first_argument.target
    )));

    let optimization = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("joined descriptor custody reconstructs into the optimizer");
    validate_psi_optimization_unit(&optimization)
        .expect("joined descriptor optimizer custody validates independently");

    let mut collapsed = optimization.clone();
    let first_source = collapsed
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .expect("joined optimization caller")
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::CallStructural {
                dynamic_arguments,
                result: AbstractStructuralCallResult::Scalar(_),
                ..
            } if !dynamic_arguments.is_empty() => Some(dynamic_arguments[0].source.clone()),
            _ => None,
        })
        .expect("first joined optimization source");
    let second_call = collapsed
        .functions
        .iter_mut()
        .find(|function| function.machine == caller.machine)
        .expect("joined optimization caller")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .filter_map(|node| match &mut node.operation {
            AbstractOperation::CallStructural {
                dynamic_arguments,
                result: AbstractStructuralCallResult::Scalar(_),
                ..
            } if !dynamic_arguments.is_empty() => Some(dynamic_arguments),
            _ => None,
        })
        .nth(1)
        .expect("second joined optimization call");
    second_call[0].source = first_source;
    collapsed.identity = recompute_psi_optimization_unit_identity(&collapsed);
    assert!(
        validate_psi_optimization_unit(&collapsed).is_err(),
        "one predecessor's source cannot replace the other"
    );

    // Scalar arguments beside forwarded descriptors are a shape stage 00
    // never produces for a scalar-result call. The unit contract refuses it
    // independently: the added scalar is the caller's own parameter, defined
    // on entry, so only the contract's empty-scalar-roster rule rejects it.
    let mut mixed = plan.clone();
    let caller = mixed
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("joined caller");
    let [choose_first] = caller.parameters.as_slice() else {
        panic!("joined caller declares one scalar parameter")
    };
    let choose_first = choose_first.value;
    let arguments = caller
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            AbstractOperation::CallStructural {
                arguments,
                dynamic_arguments,
                result: AbstractStructuralCallResult::Scalar(_),
                ..
            } if !dynamic_arguments.is_empty() => Some(arguments),
            _ => None,
        })
        .expect("joined caller retains a forwarding scalar call");
    assert!(arguments.is_empty());
    arguments.push(choose_first);
    let mixed = reconstruct_psi_optimization_unit_seed(
        &mixed,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("the mixed call still reconstructs");
    let caller = mixed
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("joined optimization caller");
    let (block, node) = caller
        .blocks
        .iter()
        .find_map(|block| {
            block
                .nodes
                .iter()
                .position(|node| {
                    matches!(&node.operation, AbstractOperation::CallStructural {
                        arguments,
                        dynamic_arguments,
                        result: AbstractStructuralCallResult::Scalar(_),
                        ..
                    } if !arguments.is_empty() && !dynamic_arguments.is_empty())
                })
                .map(|node| (block.id, node))
        })
        .expect("mixed call node");
    assert_eq!(
        validate_psi_optimization_unit(&mixed),
        Err(
            optimization_unit_semantics::OptimizationUnitValidationError::ScalarOperationContractMismatch {
                machine: caller.machine,
                block,
                node: u32::try_from(node).expect("node index fits u32"),
            }
        )
    );
}
