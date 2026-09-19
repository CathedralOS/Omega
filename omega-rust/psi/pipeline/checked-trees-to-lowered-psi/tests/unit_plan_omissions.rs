//! A missing checked Unit plan reports why the checked stage omitted it: the
//! lowering error carries the omission chain from the requested machine to
//! the machine whose own body failed local construction.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

const UNSUPPORTED_BODY: &str = r#"
    data Token { observed: bool; other: bool; }
    machine Token::drop(&mut self) {}
    data Main {}
    machine Main::main(token: Token, input: u64, enabled: bool) -> bool {
        let staged: bool = token.observed && ((input < 1u64) || enabled);
        staged
    }
"#;

#[test]
fn a_root_without_an_admitted_body_names_its_own_local_construction() {
    let checked = checked(UNSUPPORTED_BODY);
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("the mixed member/comparison body has no Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        reason,
        omission,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::main");
    assert_eq!(
        reason,
        "attached Unit closure is missing a checked transitive machine plan"
    );
    assert_eq!(
        omission.as_deref(),
        Some("`Main::main` has no admitted body (local construction stopped at completion)")
    );
}

#[test]
fn a_root_whose_callee_lacks_a_body_names_the_callee_chain() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Main {}
        machine Main::main(token: Token, input: u64, enabled: bool) {
            Main::relay(token, input, enabled);
        }
        machine Main::relay(token: Token, input: u64, enabled: bool) {
            Main::leaf(token, input, enabled);
        }
        machine Main::leaf(token: Token, input: u64, enabled: bool) {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
        }
    "#,
    );
    let plans = &checked.facts.flow.terminal_unit_effects;
    let named = |name: &str| {
        checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .symbol
    };
    let omission = |name: &str| {
        plans
            .omission_for_machine(named(name))
            .unwrap_or_else(|| panic!("{name} has an omission row"))
            .stage
    };
    assert!(matches!(
        omission("Main::leaf"),
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction { .. }
    ));
    assert_eq!(
        omission("Main::relay"),
        checked_trees::CheckedUnitPlanOmissionStage::UnavailableCallee {
            target: named("Main::leaf")
        }
    );
    assert_eq!(
        omission("Main::main"),
        checked_trees::CheckedUnitPlanOmissionStage::UnavailableCallee {
            target: named("Main::relay")
        }
    );
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("the root's transitive callee has no Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        omission,
        ..
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::main");
    assert_eq!(
        omission.as_deref(),
        Some(
            "`Main::main` calls `Main::relay`, which has no plan; \
             `Main::relay` calls `Main::leaf`, which has no plan; \
             `Main::leaf` has no admitted body (local construction stopped at completion)"
        )
    );
}

// TR3-TR8 pin: routed `TaskRuntime::start<Worker::run>` establishment and the
// ordinary generic-receiver consumer behind it. The source below is the
// smallest program that calls the generic boundary requirement with a
// concrete target machine through a runtime capability carried on `self`,
// then settles the returned `Task<Token>` through the generic attached
// `Task::settle<T>(self)`. Checking retains the `start` call's admitted
// specialization as a checked fact, and monomorphization retargets the
// `settle` edge at a receiver-specialized `Task::settle$specialized` clone
// whose retained owner application is the concrete `Task<Token>`: the
// specialized machine's signature, self argument, and claim path all replay
// that application instead of the open `Task<T>`. Both calls produce checked
// operations, and the routed receiver now clears provider attachment
// requirements: the `token` parameter stays an ordinary argument while the
// `self.runtime` receiver supplies the `start` requirement row. The
// remaining omission frontier is independent of the receiver specialization:
// the `TaskRuntime::start` requirement signature itself has no static
// boundary plan — `build_static_boundary_requirements` cannot telescope a
// `machine Target` binder that is neither a native callback entry nor a
// nominal use — so candidate closure drops `Main::probe` on its missing
// boundary target, and the owned-`self` `settle` bodies stop at entry claims
// because a linear owned receiver establishes no `StateEntry` claim event.

const ROUTED_TASK_START_DECLS: &str = r#"
    data Task<T> [linear] {
        provider: u64;
        activation: u64;
    }

    boundary trait TaskRuntime {
        machine start<T, Arguments, machine Target>(
            &self,
            arguments: Arguments
        ) -> Task<T>
        where machine Target(arguments: Arguments) -> T suspends; blocks;
        ensures true;
    }

    data CanaryTaskRuntime { }

    machine CanaryTaskRuntime::start<T, Arguments, machine Target>(
        &self,
        arguments: Arguments
    ) -> Task<T>
    where machine Target(arguments: Arguments) -> T suspends; blocks;
    satisfies TaskRuntime::start
    via Binding::CompilerIntrinsic;

    data Token {
        id: u64;
    }

    data Worker { }
    machine Worker::run(token: Token) -> Token suspends; {
        token
    }

    machine Task::settle<T>(self) { }
"#;

#[test]
fn a_routed_task_start_call_plans_and_the_missing_boundary_plan_stops_probe() {
    let checked = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{
             runtime: TaskRuntime;
         }}
         machine Main::probe(&mut self, token: Token) reaches TaskRuntime {{
             let task: Task<Token> = self.runtime.start<Worker::run>(token);
             Task::settle(task);
         }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    // The routed call survived checking with its exact static target: the
    // requirement symbol and the selected `Worker::run` entry are both
    // retained on the checked call node and the flow call fact.
    let start_requirement = checked
        .traits()
        .iter()
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .find(|signature| signature.name.as_str() == "start")
        .expect("TaskRuntime::start requirement is retained")
        .symbol;
    let worker_entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .and_then(|machine| checked.typed.machine_states(machine).first())
        .expect("Worker::run entry is retained")
        .symbol;
    let call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| {
            let checked_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            (call.target_symbol == start_requirement).then_some(call)
        })
        .expect("the start call is retained in the checked expression table");
    let [target] = call.machine_arguments.as_ref() else {
        panic!("start<Worker::run> retains exactly one static machine argument");
    };
    assert_eq!(target.symbol, worker_entry);
    let probe_state = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.machine_symbol == probe).then_some(state))
        .expect("Main::probe has a checked flow state");
    assert!(
        checked
            .facts
            .flow
            .control
            .calls
            .span_or_empty(probe_state.calls)
            .iter()
            .any(|call| call.target_symbol == start_requirement),
        "the start call is retained as a checked flow call fact"
    );
    // Both calls in `Main::probe` plan, and the provider roster now admits
    // the routed receiver: `token` is an ordinary structural argument beside
    // the `self.runtime` provider field, so the checked requirement row names
    // `TaskRuntime::start` on `runtime`. `probe` therefore leaves local
    // construction and is pruned by candidate closure instead: the `start`
    // requirement signature is generic over a `machine Target` binder, which
    // `build_static_boundary_requirements` cannot telescope into a static
    // boundary plan, so the boundary call's target has no plan to call into.
    let omission = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(probe)
        .expect("Main::probe has an omission row");
    assert_eq!(
        omission.stage,
        checked_trees::CheckedUnitPlanOmissionStage::MissingBoundaryTarget {
            target: start_requirement,
        }
    );
    // The specialized `settle` edge is real — checking emitted
    // `Task::settle$specialized` for the `Task<Token>` receiver — but an owned
    // `self` on `[linear]` data establishes no `StateEntry` claim event, so
    // both `settle` bodies still stop at entry claims. That wall is about
    // owned linear receivers in general, not the receiver specialization.
    let settle_omission_phase = |name: &str| {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .symbol;
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine)
            .unwrap_or_else(|| panic!("{name} has an omission row"))
            .stage
    };
    assert!(matches!(
        settle_omission_phase("Task::settle"),
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "entry claims",
            ..
        }
    ));
    let specialized = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            machine
                .name
                .as_str()
                .starts_with("Task::settle$specialized$")
        })
        .expect("the Task<Token> receiver specialization is emitted")
        .symbol;
    assert!(matches!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(specialized)
            .expect("the specialized settle body has an omission row")
            .stage,
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "entry claims",
            ..
        }
    ));
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::probe")
        .expect_err("the provider-attached receiver has no checked Unit plan");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        reason,
        omission,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::probe");
    assert_eq!(
        reason,
        "attached Unit closure is missing a checked transitive machine plan"
    );
    assert_eq!(
        omission.as_deref(),
        Some("`Main::probe` calls boundary `TaskRuntime::start`, which has no boundary plan")
    );
}

#[test]
fn a_provider_carrying_argument_still_stops_at_provider_attachment_requirements() {
    // `token: Token` is an ordinary routed argument, but a structural
    // parameter whose own carrier holds a provider-backed field is a second
    // provider surface: the requirement roster names `self.<field>`
    // receivers only, so `carrier` cannot cross as an unspecialized
    // argument and the receiver still stops at provider attachment
    // requirements.
    let checked = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Carrier {{
             runtime: TaskRuntime;
         }}
         data Main {{
             runtime: TaskRuntime;
         }}
         machine Main::probe(&mut self, carrier: Carrier) {{ }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    let omission = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(probe)
        .expect("Main::probe has an omission row");
    assert!(matches!(
        omission.stage,
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "provider attachment requirements",
            ..
        }
    ));
}

#[test]
fn a_shared_task_runtime_place_stops_at_signature_construction() {
    // The authored canary shape carries `&TaskRuntime`: a shared borrow of a
    // selected provider capability. Checked structural signatures admit an
    // owned boundary-trait field but not a borrowed one, so construction
    // stops one stage earlier, before any call planning is reached.
    let checked = checked(&format!(
        "{ROUTED_TASK_START_DECLS}
         data Main {{
             runtime: &TaskRuntime;
         }}
         machine Main::probe(&mut self, token: Token) reaches TaskRuntime {{
             let task: Task<Token> = self.runtime.start<Worker::run>(token);
             Task::settle(task);
         }}
         machine Main::main(&mut self) {{ }}"
    ));
    let probe = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::probe")
        .expect("Main::probe is declared")
        .symbol;
    let omission = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(probe)
        .expect("Main::probe has an omission row");
    assert!(matches!(
        omission.stage,
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "signature",
            ..
        }
    ));
    let error = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::probe")
        .expect_err("a shared TaskRuntime place has no admitted checked signature");
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        omission,
        ..
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(machine, "Main::probe");
    assert_eq!(
        omission.as_deref(),
        Some("`Main::probe` has no admitted body (local construction stopped at signature)")
    );
}
