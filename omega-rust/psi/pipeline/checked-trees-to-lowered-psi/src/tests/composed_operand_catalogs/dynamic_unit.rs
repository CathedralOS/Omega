//! Dynamic result continuations retain complete ordinary Unit closures.

use super::*;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralValue,
};

fn source(route: usize, qualified: bool, trailing: bool) -> String {
    let source = DYNAMIC_CONTINUATION_SOURCE
        .replace("console: Console; ", "")
        .replace(
            "data Main {",
            r#"
            data Empty {}
            machine consume(value: i32) reaches Console {
                Console::exit_process(helper(value));
                relay(helper(value));
            }
            machine relay(value: i32) reaches Console {
                let first: Empty = Empty {};
                let second: Empty = Empty {};
                Console::exit_process(helper(value));
            }
            data Main {
        "#,
        )
        .replace(
            "self.console.exit_process(helper(70i32));",
            "consume(helper(70i32));",
        )
        .replace(
            "self.console.exit_process(helper(71i32));",
            "consume(helper(71i32));",
        );
    let source = match route {
        0 => source.replace(
            "let result: i32 = forward(erased);",
            "let result: i32 = erased.measure();",
        ),
        1 => source
            .replace("selected: Item;", "decoy: Item; selected: Item;")
            .replace(
                "let erased: &dyn Measure = &self.selected as &dyn Item::Primary;",
                r#"
                let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
                erased = &self.selected as &dyn Item::Primary;
            "#,
            )
            .replace(
                "let result: i32 = forward(erased);",
                "let result: i32 = erased.measure();",
            ),
        2 => source
            .replace(
                "data Main {",
                "data Holder<'item> { handler: &'item dyn Measure; } data Main {",
            )
            .replace("Main::main(&mut self)", "Main::main<'item>(&mut self)")
            .replace("let erased: &dyn Measure", "let erased: &'item dyn Measure")
            .replace(
                "let result: i32 = forward(erased);",
                r#"
                let holder: Holder<'item> = Holder { handler: erased };
                let result: i32 = holder.handler.measure();
            "#,
            ),
        3 => source,
        _ => unreachable!(),
    };
    let source = if qualified {
        source
            .replace("data Empty {}", "data Empty {} data Relay {}")
            .replace("consume(", "Relay::consume(")
    } else {
        source
    };
    if trailing {
        source
            .replace("consume(helper(70i32));", "consume(helper(70i32))")
            .replace("consume(helper(71i32));", "consume(helper(71i32))")
    } else {
        source
    }
}

#[test]
fn dynamic_routes_share_ordinary_bodies_and_scalar_helpers() {
    for route in 0..4 {
        for qualified in [false, true] {
            for trailing in [false, true] {
                let source = source(route, qualified, trailing);
                let checked = checked_source(&source);
                let lowered = roundtrip(&checked);
                assert_closure(&checked, &lowered, route);
                assert_source_custody(&checked);
                let artifact =
                    terminal_production::produce_terminal_artifact(&checked, "Main::main").expect(
                        "dynamic ordinary Unit continuation publishes through the public producer",
                    );
                assert_eq!(
                    terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
                    lowered.semantic_module
                );
                assert_eq!(
                    terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
                    lowered.proof_bundle
                );
            }
        }
    }
}

#[test]
fn unused_root_provider_field_requires_an_authored_attachment_requirement() {
    for route in 0..4 {
        let source =
            source(route, true, true).replace("data Main {", "data Main { console: Console;");
        let checked = checked_source(&source);
        assert!(
            matches!(
                lower_machine(&checked, "Main::main"),
                Err(LoweringError::Unsupported(
                    "provider-backed attachment field lacks one complete specialization requirement set"
                ))
            ),
            "route={route}: unsupported unused provider storage rejects before Terminal construction"
        );
    }
}

fn assert_closure(checked: &CheckedTrees, lowered: &LoweredPsi, route: usize) {
    let module = &lowered.semantic_module;
    let dynamic = &module.dynamic_dispatch;
    assert_eq!(dynamic.rebound_descriptors.len(), usize::from(route == 1));
    assert_eq!(dynamic.stored_descriptors.len(), usize::from(route == 2));
    assert_eq!(dynamic.parameters.len(), if route == 3 { 2 } else { 0 });
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(
        !root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. })),
        "root provider reach is transitive; neither leaf needs a direct boundary workaround"
    );
    assert_eq!(
        root.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
            .count(),
        2
    );
    let helper = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "helper")
        .unwrap();
    let mut targets = std::collections::BTreeSet::new();
    let mut owners = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        if occurrence.source_target != helper.symbol {
            continue;
        }
        if !owners.contains(&occurrence.source_state) {
            owners.push(occurrence.source_state);
        }
        let operation = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == occurrence.terminal_operation)
            .unwrap();
        let OperationKind::Call { callee, .. } = operation.kind else {
            panic!("source helper occurrence");
        };
        targets.insert(callee);
    }
    assert_eq!(targets.len(), 1);
    assert_eq!(
        owners.len(),
        4,
        "good, bad, consume and relay share one source helper"
    );
    let cleanup = module
        .machines
        .iter()
        .find_map(|machine| {
            let established = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match operation.kind {
                    OperationKind::EstablishTrivialAffineLocal { destination } => Some(destination),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (established.len() == 2).then_some((machine, established))
        })
        .expect("transitive Unit callee retains its affine locals");
    assert!(
        cleanup
            .0
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator,
        Terminator::ReturnUnit { trivial_affine_discards, .. }
        if trivial_affine_discards.as_slice() == [cleanup.1[1], cleanup.1[0]]))
    );
}

fn assert_source_custody(checked: &CheckedTrees) {
    let consumer = checked
        .typed
        .machines()
        .iter()
        .find(|machine| matches!(machine.name.as_str(), "consume" | "Relay::consume"))
        .unwrap();
    let target = checked.typed.machine_states(consumer)[0].symbol;
    let (handle, _) = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, call)| call.target_symbol == target)
        .expect("captured leaf Unit call");
    let mut changed = checked.clone();
    changed
        .facts
        .flow
        .control
        .calls
        .get_mut(handle)
        .target_symbol = symbols::SymbolHandle::invalid();
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "captured ordinary target drift rejects"
    );
    let (handle, _) = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find(|(_, root)| {
            matches!(
                root.role,
                checked_trees::CheckedScalarExpressionRole::UnitCallArgument { .. }
            )
        })
        .expect("computed ordinary argument");
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_computations
        .roots
        .get_mut(handle)
        .statement_ordinal += 1;
    assert!(
        lower_machine(&changed, "Main::main").is_err(),
        "computed argument coordinate drift rejects"
    );
}

#[derive(Default)]
struct Observe(Vec<Vec<TerminalScalarValue>>);

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("observable Console call");
        };
        self.0.push(arguments.clone());
        Ok(())
    }
}

#[test]
fn dynamic_boolean_result_executes_only_the_selected_ordinary_unit_leaf() {
    for route in 0..4 {
        let source = source(route, true, true)
            .replace(
                "machine measure(&self) -> i32",
                "machine measure(&self) -> bool",
            )
            .replace("value: i32;", "value: bool;")
            .replace(
                "forward(erased: &dyn Measure) -> i32",
                "forward(erased: &dyn Measure) -> bool",
            )
            .replace(
                "finish(erased: &dyn Measure) -> i32",
                "finish(erased: &dyn Measure) -> bool",
            )
            .replace("let result: i32", "let result: bool")
            .replace("transition result == 0", "transition result");
        let checked = checked_source(&source);
        let lowered = roundtrip(&checked);
        let module = &lowered.semantic_module;
        let semantic = terminal_codec::encode_module(module).unwrap();
        let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let [parameter] = entry.structural_parameters.as_slice() else {
            panic!("one self argument");
        };
        let field = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::BooleanStructuralField { field, .. } => Some(field),
                _ => None,
            })
            .expect("dynamic realization Boolean field");
        for selected in [false, true] {
            let argument = TerminalStructuralValue {
                opaque_identity: 1,
                structural_type: parameter.structural_type,
                qualifications: parameter.qualifications.clone(),
                path: Vec::new(),
            };
            let fields = module
                .dynamic_dispatch
                .selections
                .iter()
                .map(|selection| TerminalStructuralBooleanFieldValue {
                    argument_index: 0,
                    path: selection.source.path.clone(),
                    field,
                    value: if route == 1 && selection.ordinal == 0 {
                        !selected
                    } else {
                        selected
                    },
                })
                .collect::<Vec<_>>();
            let mut execution =
                TerminalExecution::start_artifact_with_structural_arguments_and_boolean_fields(
                    &semantic,
                    &evidence,
                    &proof_admission::AdmissionProfile::default(),
                    &[],
                    &[argument],
                    &fields,
                )
                .unwrap();
            let mut observer = Observe::default();
            assert_eq!(
                execution
                    .resume_with_effect_handler(
                        &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                        &mut observer
                    )
                    .unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let value = TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    32,
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Signed(if selected { 70 } else { 71 }),
            };
            assert_eq!(
                observer.0,
                vec![vec![value], vec![value]],
                "route={route}, selected={selected}"
            );
        }
    }
}
