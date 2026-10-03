use abstract_operations::{
    AbstractDynamicDescriptorSource, AbstractOperation, AbstractStructuralCallResult,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_unit::{
    recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed,
};
use optimization_unit_semantics::validate_psi_optimization_unit;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{FuelScheduleIdentity, IntegerSign, IntegerType, ScalarType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::StructuralPathSegment;
use terminal_psi_to_abstract_operations::lower_artifact;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[path = "dynamic_dispatch/unit.rs"]
mod unit;

#[test]
fn verified_stored_dynamic_descriptor_retains_aggregate_custody_through_optimization_seed() {
    let source = r#"
        trait Measure { machine measure(&self) -> bool; }
        data Item [copy] { value: bool; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.value } }
        }
        data Holder<'item> { handler: &'item dyn Measure; }
        data Main [copy] { item: Item; }
        machine Main::run<'item>(&self) {
            let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: bool = holder.handler.measure();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("stored dynamic source lowers to verified Terminal Psi");
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
    .expect("stored descriptor reaches target-neutral Omega");
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let store = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::StoreDynamicDescriptor {
                psi_operation,
                stored,
            } => Some((*psi_operation, stored)),
            _ => None,
        })
        .expect("one target-neutral descriptor store");
    let call = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamic {
                psi_operation,
                dispatch: abstract_operations::AbstractDynamicDispatch::Stored(dynamic_dispatch),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                ..
            } => Some((*psi_operation, dynamic_dispatch)),
            _ => None,
        })
        .expect("one target-neutral stored descriptor call");
    assert!(store.1.has_complete_custody(caller.machine, store.0));
    assert!(call.1.has_complete_custody(caller.machine, call.0));
    assert_eq!(&call.1.stored, store.1);
    assert!(
        store
            .1
            .descriptor
            .aggregate_type_identity
            .contains("Holder")
    );
    assert_eq!(store.1.descriptor.field_identity, "handler");

    let optimization = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("stored descriptor custody reconstructs into the optimizer");
    validate_psi_optimization_unit(&optimization)
        .expect("stored descriptor optimizer custody validates independently");

    let mut drifted = optimization.clone();
    let dynamic = drifted
        .functions
        .iter_mut()
        .find(|function| function.machine == drifted.entry)
        .expect("entry optimization function")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallDynamic {
                dispatch: abstract_operations::AbstractDynamicDispatch::Stored(dynamic_dispatch),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                ..
            } => Some(dynamic_dispatch),
            _ => None,
        })
        .expect("stored descriptor optimization dispatch");
    dynamic.stored.descriptor.field_identity = "other".into();
    drifted.identity = recompute_psi_optimization_unit_identity(&drifted);
    assert!(
        validate_psi_optimization_unit(&drifted).is_err(),
        "a call may not drift from the exact preceding descriptor store"
    );
}

#[test]
fn verified_rebound_dynamic_call_retains_versions_and_indirect_row() {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
            machine alternate(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }

            machine alternate(&self) -> i32 {
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
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("rebound dynamic source lowers to verified Terminal Psi");
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
            AbstractOperation::CallDynamic {
                dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(result),
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
    assert_eq!(dynamic.application.owner, caller.machine);
    assert_eq!(dynamic.application.rows.len(), 2);
    assert!(dynamic.has_complete_application_custody(caller.machine, dynamic.dispatch.operation));
    assert_eq!(
        dynamic
            .application
            .rows
            .iter()
            .filter(|row| {
                row.public_requirement_identity == dynamic.dispatch.public_requirement_identity
                    && row.realization_callable_identity.as_deref()
                        == Some(dynamic.dispatch.realization_callable_identity.as_str())
            })
            .count(),
        1,
        "the selected row must remain one exact member of the complete two-row application"
    );
    assert_eq!(dynamic.dispatch.owner, caller.machine);
    assert_eq!(
        dynamic.dispatch.descriptor_ordinal,
        dynamic.descriptor.ordinal
    );
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
                AbstractOperation::CallStructural {
                    callee,
                    result: AbstractStructuralCallResult::Scalar(_),
                    ..
                }
                    if *callee == dynamic.dispatch.realization
            )
        }),
        "rebound dispatch must not be substituted with a direct structural call"
    );
}

#[test]
fn verified_changed_conformance_rebound_retains_both_applications() {
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

        Secondary: Item satisfies Measure {
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
            erased = &self.selected as &dyn Item::Secondary;
            let result: i32 = erased.measure();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("changed-conformance rebound lowers to verified Terminal Psi");
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
    .expect("verified changed-conformance rebound reaches target-neutral Omega");

    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let dynamic = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamic {
                dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                ..
            } => Some(dynamic_dispatch),
            _ => None,
        })
        .expect("one changed-conformance rebound dispatch");

    assert!(dynamic.has_complete_application_custody(caller.machine, dynamic.dispatch.operation));
    assert_ne!(
        dynamic.initial_application.commitment, dynamic.application.commitment,
        "the initializer and rebound conformance must remain distinct"
    );
    assert_ne!(
        dynamic.initial_application.declaration_identity,
        dynamic.application.declaration_identity
    );
    assert_eq!(
        dynamic.initial.conformance_application_commitment,
        dynamic.initial_application.commitment
    );
    assert_eq!(
        dynamic.rebound.conformance_application_commitment,
        dynamic.application.commitment
    );
    assert!(dynamic.initial_application.realization_callables.is_empty());
    assert_eq!(dynamic.application.realization_callables.len(), 1);

    let optimization = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("changed-conformance custody reconstructs into the optimizer");
    validate_psi_optimization_unit(&optimization)
        .expect("changed-conformance optimizer custody validates independently");

    let mut collapsed = optimization.clone();
    let collapsed_dispatch = collapsed
        .functions
        .iter_mut()
        .find(|function| function.machine == collapsed.entry)
        .expect("entry optimization function")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallDynamic {
                dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
                result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
                ..
            } => Some(dynamic_dispatch),
            _ => None,
        })
        .expect("changed-conformance optimization dispatch");
    collapsed_dispatch.initial_application = collapsed_dispatch.application.clone();
    collapsed.identity = recompute_psi_optimization_unit_identity(&collapsed);
    assert!(
        validate_psi_optimization_unit(&collapsed).is_err(),
        "collapsing the initializer into the latest application must invalidate custody"
    );
}

#[test]
fn verified_direct_scalar_forwarding_retains_selection_and_result_custody() {
    let source = r#"
        trait Measure { machine measure(&self) -> i32; }
        data Item { value: i32; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 { transition { _ -> self.value } }
        }
        data Main { selected: Item; }
        machine Main::run(&mut self) {
            let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }
        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(syntax)).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check source");
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("direct scalar forwarding lowers to verified Terminal Psi");
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
    .expect("direct scalar forwarding reaches target-neutral Omega");

    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let (callee, result, arguments) = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallStructural {
                callee,
                result: AbstractStructuralCallResult::Scalar(result),
                dynamic_arguments,
                ..
            } if !dynamic_arguments.is_empty() => Some((*callee, *result, dynamic_arguments)),
            _ => None,
        })
        .expect("caller retains one descriptor-bearing scalar helper call");
    assert_eq!(
        result.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    );
    let [argument] = arguments.as_slice() else {
        panic!("one direct-selection descriptor argument expected: {arguments:#?}")
    };
    assert!(argument.has_complete_custody(caller.machine, argument.argument.operation, callee));
    let AbstractDynamicDescriptorSource::Selection {
        selection,
        application,
    } = &argument.source
    else {
        panic!("the direct caller must supply its exact selection")
    };
    assert_eq!(selection.owner, caller.machine);
    assert_eq!(application.owner, caller.machine);
    assert_eq!(
        selection.conformance_application_commitment,
        application.commitment
    );

    let helper = plan
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .expect("forward helper retained");
    assert!(helper.operations.iter().any(|operation| matches!(
        operation,
        AbstractOperation::CallDynamic { dispatch: abstract_operations::AbstractDynamicDispatch::Parameter(dynamic_dispatch), result: abstract_operations::AbstractDynamicCallResult::Scalar(helper_result), .. } if helper_result.scalar_type == result.scalar_type
            && dynamic_dispatch.parameter == argument.target
    )));

    let optimization = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("direct descriptor custody reconstructs into the optimizer");
    validate_psi_optimization_unit(&optimization)
        .expect("direct descriptor optimizer custody validates independently");
}
