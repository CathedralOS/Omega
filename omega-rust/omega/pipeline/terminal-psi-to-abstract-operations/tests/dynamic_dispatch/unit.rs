//! Unit-result dynamic dispatch and forwarded descriptor custody.

use abstract_operations::{AbstractDynamicDescriptorSource, AbstractOperation};
use optimization_unit::{
    recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed,
};
use optimization_validation::validate_psi_optimization_unit;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::FuelScheduleIdentity;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn verified_forwarded_dynamic_unit_retains_argument_and_parameter_custody() {
    let rebound = r#"
        trait Touch {
            machine touch(&self);
        }

        data Item { value: i32; }

        Primary: Item satisfies Touch {
            machine touch(&self) {}
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            forward(erased);
        }

        machine forward(erased: &dyn Touch) {
            erased.touch();
        }
    "#;
    let direct = r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        data Main { selected: Item; }
        machine Main::run(&mut self) {
            let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
            forward(erased);
        }
        machine forward(erased: &dyn Touch) { erased.touch(); }
    "#;
    for (source, expects_rebound) in [(rebound, true), (direct, false)] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize source");
        let syntax = parse_syntax_trees(&tokens).expect("parse source");
        let resolved = lower_syntax_trees(&syntax).expect("resolve source");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
        let checked = lower_typed_trees(typed).expect("check source");
        let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
            .expect("dynamic Unit source lowers to verified Terminal Psi");
        let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
        let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
        let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("verified dynamic Unit dispatch reaches target-neutral Omega");

        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let (helper_id, dynamic_arguments) = caller
            .operations
            .iter()
            .find_map(|operation| match operation {
                AbstractOperation::CallUnitWithDynamicArguments {
                    callee,
                    dynamic_arguments,
                    ..
                } => Some((*callee, dynamic_arguments)),
                _ => None,
            })
            .expect("caller retains the forwarded Unit descriptor argument");
        let [argument] = dynamic_arguments.as_slice() else {
            panic!("one forwarded Unit descriptor expected: {dynamic_arguments:#?}")
        };
        assert!(argument.has_complete_custody(
            caller.machine,
            argument.argument.operation,
            helper_id
        ));
        let application = match (&argument.source, expects_rebound) {
            (AbstractDynamicDescriptorSource::Rebound { application, .. }, true)
            | (AbstractDynamicDescriptorSource::Selection { application, .. }, false) => {
                application
            }
            (source, _) => {
                panic!("forwarded Unit descriptor retained the wrong source: {source:#?}")
            }
        };
        assert!(application.realization_callables.iter().any(|callable| {
            callable.result == terminal_psi::ClosedConformanceCallableResult::Unit
        }));

        let helper = plan
            .functions
            .iter()
            .find(|function| function.machine == helper_id)
            .expect("forward helper retained");
        let parameter_dispatch = helper
            .operations
            .iter()
            .find_map(|operation| match operation {
                AbstractOperation::CallDynamicParameterUnit {
                    psi_operation,
                    dynamic_dispatch,
                    ..
                } => Some((*psi_operation, dynamic_dispatch)),
                _ => None,
            })
            .expect("helper retains one Unit parameter dispatch");
        assert_eq!(parameter_dispatch.1.parameter, argument.target);
        assert_eq!(parameter_dispatch.1.dispatch.owner, helper.machine);
        assert_eq!(
            parameter_dispatch.1.dispatch.operation,
            parameter_dispatch.0
        );
        assert_eq!(
            parameter_dispatch.1.parameter.requirements[0].result,
            terminal_psi::ClosedConformanceCallableResult::Unit
        );

        let optimization = reconstruct_psi_optimization_unit_seed(
            &plan,
            FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
        )
        .expect("dynamic Unit custody reconstructs into the optimizer");
        validate_psi_optimization_unit(&optimization)
            .expect("dynamic Unit optimizer custody validates independently");
        for node in optimization
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.nodes)
            .filter(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::CallDynamicUnit { .. }
                        | AbstractOperation::CallDynamicParameterUnit { .. }
                        | AbstractOperation::CallUnitWithDynamicArguments { .. }
                )
            })
        {
            assert!(
                node.definitions.is_empty(),
                "Unit calls must define no scalar value"
            );
        }
    }
}

#[test]
fn verified_rebound_dynamic_unit_retains_exact_indirect_custody() {
    let source = r#"
        trait Touch {
            machine touch(&self);
        }

        data Item { value: i32; }

        Primary: Item satisfies Touch {
            machine touch(&self) {}
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            erased.touch();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("rebound dynamic Unit source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified rebound Unit dispatch reaches target-neutral Omega");
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let (operation, dispatch) = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamicUnit {
                psi_operation,
                dynamic_dispatch,
                ..
            } => Some((*psi_operation, dynamic_dispatch)),
            _ => None,
        })
        .expect("caller retains one rebound Unit dispatch");
    assert!(dispatch.has_complete_application_custody(caller.machine, operation));
    assert!(
        dispatch
            .application
            .realization_callables
            .iter()
            .any(|callable| {
                callable.machine == dispatch.dispatch.realization
                    && callable.result == terminal_psi::ClosedConformanceCallableResult::Unit
            })
    );
}

#[test]
fn verified_changed_conformance_unit_retains_both_applications() {
    let source = r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        Secondary: Item satisfies Touch { machine touch(&self) {} }
        data Main { decoy: Item; selected: Item; }
        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Secondary;
            erased.touch();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("changed-conformance Unit source lowers to verified Terminal Psi");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified changed-conformance Unit dispatch reaches target-neutral Omega");
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let (operation, dynamic) = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallDynamicUnit {
                psi_operation,
                dynamic_dispatch,
                ..
            } => Some((*psi_operation, dynamic_dispatch)),
            _ => None,
        })
        .expect("changed-conformance Unit dispatch");
    assert!(dynamic.has_complete_application_custody(caller.machine, operation));
    assert_ne!(
        dynamic.initial_application.commitment,
        dynamic.application.commitment
    );
    assert!(dynamic.initial_application.realization_callables.is_empty());
    assert_eq!(dynamic.application.realization_callables.len(), 1);

    let optimization = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero test fuel schedule"),
    )
    .expect("changed-conformance Unit custody reconstructs into the optimizer");
    validate_psi_optimization_unit(&optimization)
        .expect("changed-conformance Unit optimizer custody validates independently");
    let mut collapsed = optimization.clone();
    let dynamic = collapsed
        .functions
        .iter_mut()
        .find(|function| function.machine == collapsed.entry)
        .expect("entry optimization function")
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallDynamicUnit {
                dynamic_dispatch, ..
            } => Some(dynamic_dispatch),
            _ => None,
        })
        .expect("changed-conformance Unit optimization dispatch");
    dynamic.initial_application = dynamic.application.clone();
    collapsed.identity = recompute_psi_optimization_unit_identity(&collapsed);
    assert!(
        validate_psi_optimization_unit(&collapsed).is_err(),
        "collapsing the initial Unit conformance into the latest must reject"
    );
}
