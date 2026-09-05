use abstract_operations::{AbstractDynamicDescriptorSource, AbstractOperation};
use optimization_unit::{
    recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed,
};
use optimization_unit_semantics::validate_psi_optimization_unit;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{FuelScheduleIdentity, IntegerSign, IntegerType, ScalarType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::StructuralPathSegment;
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("stored dynamic source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
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
            AbstractOperation::CallStoredDynamicScalar {
                psi_operation,
                dynamic_dispatch,
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
            AbstractOperation::CallStoredDynamicScalar {
                dynamic_dispatch, ..
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
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
                AbstractOperation::CallStructuralScalar { callee, .. }
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("changed-conformance rebound lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
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
            AbstractOperation::CallDynamicScalar {
                dynamic_dispatch, ..
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
            AbstractOperation::CallDynamicScalar {
                dynamic_dispatch, ..
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
fn verified_forwarded_dynamic_parameter_retains_call_argument_and_helper_dispatch() {
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
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("forwarded dynamic source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified forwarded dispatch reaches target-neutral Omega");

    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let (callee, arguments) = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallStructuralScalarWithDynamicArguments {
                callee,
                dynamic_arguments,
                ..
            } if !dynamic_arguments.is_empty() => Some((*callee, dynamic_arguments)),
            _ => None,
        })
        .expect("caller retains one descriptor-bearing helper call");
    let [argument] = arguments.as_slice() else {
        panic!("one dynamic descriptor argument expected: {arguments:#?}")
    };
    assert_eq!(argument.argument.owner, caller.machine);
    assert_eq!(argument.target.owner, callee);
    let AbstractDynamicDescriptorSource::Rebound {
        initial,
        rebound,
        descriptor,
        initial_application,
        application,
    } = &argument.source
    else {
        panic!("the authored caller must supply its rebound descriptor")
    };
    assert_eq!(descriptor.owner, caller.machine);
    assert_eq!(initial.owner, caller.machine);
    assert_eq!(rebound.owner, caller.machine);
    assert_eq!(application.owner, caller.machine);
    assert_eq!(initial_application, application);
    assert_eq!(
        initial.conformance_application_commitment,
        application.commitment
    );
    assert_eq!(
        rebound.conformance_application_commitment,
        application.commitment
    );

    let helper = plan
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .expect("forward helper retained");
    let descriptor_parameters = helper
        .operations
        .iter()
        .take_while(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(descriptor_parameters.len(), 1);
    assert!(matches!(
        descriptor_parameters[0],
        AbstractOperation::DynamicDescriptorParameter { parameter }
            if parameter == &argument.target
    ));
    let parameter_calls = helper
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::CallDynamicParameterScalar {
                result,
                dynamic_dispatch,
                ..
            } => Some((result, dynamic_dispatch)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(result, dispatch)] = parameter_calls.as_slice() else {
        panic!("one helper parameter dispatch expected: {helper:#?}")
    };
    assert_eq!(dispatch.parameter, argument.target);
    assert_eq!(dispatch.dispatch.owner, helper.machine);
    assert_eq!(
        dispatch.dispatch.parameter_ordinal,
        dispatch.parameter.ordinal
    );
    assert_eq!(
        result.scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    );

    let optimization = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("forwarded descriptor custody reconstructs into the optimizer");
    validate_psi_optimization_unit(&optimization)
        .expect("forwarded descriptor optimizer custody validates independently");
    let mut missing_parameter = optimization.clone();
    let helper_function = missing_parameter
        .functions
        .iter_mut()
        .find(|function| function.machine == callee)
        .expect("mutated helper");
    let entry = helper_function
        .blocks
        .iter_mut()
        .find(|block| block.id == helper_function.entry)
        .expect("helper entry");
    assert!(matches!(
        entry.nodes.remove(0).operation,
        AbstractOperation::DynamicDescriptorParameter { .. }
    ));
    missing_parameter.identity = recompute_psi_optimization_unit_identity(&missing_parameter);
    assert!(
        validate_psi_optimization_unit(&missing_parameter).is_err(),
        "a forwarded call cannot outlive its callee's descriptor declaration"
    );
    let helper = optimization
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .expect("optimizer retains the helper");
    assert!(
        helper
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::CallDynamicParameterScalar { .. }
                ) && node
                    .definitions
                    .iter()
                    .any(|definition| definition.value == result.value)
            })
    );

    let mut missing_argument = optimization.clone();
    let caller_function = missing_argument
        .functions
        .iter_mut()
        .find(|function| function.machine == caller.machine)
        .expect("mutated caller");
    let call = caller_function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::CallStructuralScalarWithDynamicArguments { .. }
            )
        })
        .expect("descriptor-bearing call");
    let AbstractOperation::CallStructuralScalarWithDynamicArguments {
        dynamic_arguments, ..
    } = &mut call.operation
    else {
        unreachable!()
    };
    dynamic_arguments.clear();
    missing_argument.identity = recompute_psi_optimization_unit_identity(&missing_argument);
    assert!(validate_psi_optimization_unit(&missing_argument).is_err());
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("direct scalar forwarding lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
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
            AbstractOperation::CallStructuralScalarWithDynamicArguments {
                callee,
                result,
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
        AbstractOperation::CallDynamicParameterScalar {
            result: helper_result,
            dynamic_dispatch,
            ..
        } if helper_result.scalar_type == result.scalar_type
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
