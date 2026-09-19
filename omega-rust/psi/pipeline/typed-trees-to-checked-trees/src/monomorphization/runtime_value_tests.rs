use super::{ExpressionHandle, ExpressionNode, ProofFact, StatementNode, TypedTrees};
use crate::monomorphization::collect_expression_tree;
use crate::monomorphization::collect_statement_expression_trees;
use crate::monomorphization::monomorphize_generic_machine_value_calls_with_nominal_uses;
use typed_trees::machine::Machine;
use typed_trees::typed_trees::MachineSpecialization;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

fn instance<'a>(program: &'a TypedTrees, receipt: &MachineSpecialization) -> &'a Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == receipt.instance)
        .expect("specialization instance")
}

fn call_expressions(program: &TypedTrees, machine: &Machine) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_expression_trees(program, statement, &mut roots);
        }
    }
    roots
        .into_iter()
        .filter(|handle| {
            matches!(
                program.expression_table.expression(*handle),
                ExpressionNode::Call(_)
            )
        })
        .collect()
}

#[test]
fn runtime_value_argument_realizes_an_ordinary_parameter() {
    let mut program = typed(
        "machine prefix_count<Count: u32>(base: u32) -> u32 { Count }
         machine main(base: u32) -> u32 { let n: u32 = 3; prefix_count<n>(base) }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("runtime subject specializes");
    let [receipt] = program.machine_specializations.as_slice() else {
        panic!("one runtime-carrier specialization");
    };
    let instance = instance(&program, receipt);
    let state = &program.machine_states(instance)[0];
    let parameters = program.state_parameters(state);
    // `base` plus the realized `Count` subject: the clone is an ordinary
    // two-parameter machine with no residual generic binders.
    assert_eq!(parameters.len(), 2);
    assert!(program.machine_type_parameters(instance).is_empty());
    // The rewritten call passes `base` and appends `n` as ordinary arguments.
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("caller");
    let calls = call_expressions(&program, caller);
    let [call] = calls.as_slice() else {
        panic!("one rewritten call");
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(*call) else {
        unreachable!();
    };
    assert_eq!(call.target_symbol, state.symbol);
    assert!(call.machine_arguments.is_empty());
    assert_eq!(
        program
            .expression_table
            .expression_handles(call.arguments)
            .len(),
        2
    );
}

#[test]
fn runtime_value_forwarding_appends_the_realized_parameter() {
    let mut program = typed(
        "machine prefix_count<Count: u32>(base: u32) -> u32 { Count }
         machine forward<K: u32>(base: u32) -> u32 { prefix_count<K>(base) }
         machine main(base: u32) -> u32 { let n: u32 = 3; forward<n>(base) }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("forwarded runtime subject specializes");
    assert_eq!(program.machine_specializations.len(), 2);
    let forward = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("forward template");
    let forward_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| receipt.template == forward.symbol)
        .expect("forward instance");
    let forward_instance = instance(&program, forward_receipt);
    let forward_state = &program.machine_states(forward_instance)[0];
    // `base` plus the realized `K`: the nested call forwards the realized
    // parameter's own symbol as the callee's ordinary runtime argument.
    assert_eq!(program.state_parameters(forward_state).len(), 2);
    let nested_calls = call_expressions(&program, forward_instance);
    let [nested] = nested_calls.as_slice() else {
        panic!("one nested call in the forward instance");
    };
    let ExpressionNode::Call(nested) = program.expression_table.expression(*nested) else {
        unreachable!();
    };
    assert!(nested.machine_arguments.is_empty());
    let nested_arguments = program
        .expression_table
        .expression_handles(nested.arguments);
    assert_eq!(nested_arguments.len(), 2);
    let ExpressionNode::Name(subject) = program.expression_table.expression(nested_arguments[1])
    else {
        panic!("the forwarded argument stays an ordinary subject");
    };
    let realized = program.state_parameters(forward_state)[1].symbol;
    assert_eq!(subject.symbol, realized);
    // The nested target is the prefix_count instance's entry state.
    let prefix = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "prefix_count")
        .expect("prefix template");
    let prefix_instance = instance(
        &program,
        program
            .machine_specializations
            .iter()
            .find(|receipt| receipt.template == prefix.symbol)
            .expect("prefix instance"),
    );
    assert_eq!(
        nested.target_symbol,
        program.machine_states(prefix_instance)[0].symbol
    );
}

#[test]
fn static_and_runtime_value_applications_share_one_template() {
    let mut program = typed(
        "machine prefix_count<Count: u32>(base: u32) -> u32 { Count }
         machine main(base: u32) -> u32 {
             let n: u32 = 3;
             let closed: u32 = prefix_count<4>(base);
             prefix_count<n>(base)
         }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("static and runtime tuples specialize");
    // The static literal and the runtime carrier are distinct tuples: the
    // literal specializes by value, the runtime subject by its declared
    // carrier, so each keeps its own instance.
    assert_eq!(program.machine_specializations.len(), 2);
    let parameter_counts = |receipt: &MachineSpecialization| {
        let instance = instance(&program, receipt);
        program
            .state_parameters(&program.machine_states(instance)[0])
            .len()
    };
    let static_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| parameter_counts(receipt) == 1)
        .expect("the literal tuple keeps the authored signature");
    let runtime_receipt = program
        .machine_specializations
        .iter()
        .find(|receipt| parameter_counts(receipt) == 2)
        .expect("the carrier tuple gains the realized parameter");
    assert_ne!(static_receipt.instance, runtime_receipt.instance);
    // `main`'s literal call targets the static instance; its runtime-subject
    // call targets the carrier instance with `n` appended as an argument.
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("caller");
    let calls = call_expressions(&program, caller);
    assert_eq!(calls.len(), 2);
    let targets: Vec<_> = calls
        .iter()
        .map(|handle| {
            let ExpressionNode::Call(call) = program.expression_table.expression(*handle) else {
                unreachable!();
            };
            (
                call.target_symbol,
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len(),
            )
        })
        .collect();
    let static_entry = program
        .machine_states(instance(&program, static_receipt))
        .first()
        .expect("static entry")
        .symbol;
    let runtime_entry = program
        .machine_states(instance(&program, runtime_receipt))
        .first()
        .expect("runtime entry")
        .symbol;
    assert!(targets.contains(&(static_entry, 1)));
    assert!(targets.contains(&(runtime_entry, 2)));
}

#[test]
fn const_binder_still_rejects_a_runtime_subject() {
    let program = typed(
        "machine prefix_count<const Count: u32>(base: u32) -> u32 { Count }
         machine main(base: u32) -> u32 { let n: u32 = 3; prefix_count<n>(base) }",
    );
    let error = crate::lower_typed_trees(program)
        .expect_err("a const binder cannot close over a runtime subject");
    assert!(error.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot derive a complete type/const/machine/conformance specialization")
    }));
}

#[test]
fn runtime_bound_result_range_uses_the_realized_parameter() {
    // `-> u64[0..=Bound]` qualifies the realized subject, not a static
    // position: the clone keeps the range naming its realized trailing
    // parameter, and the caller's inferred result indexes on the captured
    // argument itself.
    // The tail call auto-hoists into an inferred local: its declared type is
    // exactly the substituted `u64[0..=n]`.
    let mut program = typed(
        "machine ranged<Bound: u64>() -> u64[0..=Bound] { Bound }
         machine main() -> u64 {
             let n: u64 = 7;
             ranged<n>()
         }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("a runtime-bound result range specializes");
    let [receipt] = program.machine_specializations.as_slice() else {
        panic!("one specialization for the runtime tuple");
    };
    let instance = instance(&program, receipt);
    let entry = &program.machine_states(instance)[0];
    let realized = program
        .state_parameters(entry)
        .iter()
        .find(|parameter| parameter.name.as_str() == "Bound")
        .expect("realized trailing parameter")
        .symbol;
    let scoped_maximum_symbol = |type_reference| {
        let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        let bound = program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                typed_trees::types::TypeConstraintNode::Range { maximum, .. } => {
                    typed_trees::dependent_ranges::scoped_name_bound(
                        &program.expression_table,
                        *maximum,
                    )
                }
                _ => None,
            })?;
        let ExpressionNode::Name(path) = program.expression_table.expression(bound.name) else {
            return None;
        };
        Some(path.symbol)
    };
    assert_eq!(
        scoped_maximum_symbol(entry.return_type),
        Some(realized),
        "the specialization's result range indexes the realized parameter"
    );
    // The caller's inferred hoist temporary carries the CAPTURED argument as
    // its index: `u64[0..=n]`, not the callee's realized parameter.
    let main_state = &program.machine_states(
        program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .expect("caller"),
    )[0];
    let mut n_symbol = None;
    let mut hoist_bound = None;
    for statement in program
        .statement_table
        .statements(main_state.statement_nodes)
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.name.as_str() == "n" {
            n_symbol = Some(local.symbol);
        }
        if local.type_is_inferred {
            hoist_bound = scoped_maximum_symbol(local.type_reference);
        }
    }
    assert_eq!(
        hoist_bound, n_symbol,
        "the inferred temporary's bound is the caller's argument"
    );
}

#[test]
fn runtime_value_in_a_static_length_position_rejects() {
    let mut program = typed(
        "machine sized<Count: u32>(witness: [u8; Count]) -> u32 { Count }
         machine main(witness: [u8; 4]) -> u32 {
             let n: u32 = 4;
             sized<n>(witness)
         }",
    );
    // The explicit runtime subject conflicts with the `4` length inferred from
    // `witness`: the call cannot close `Count` statically, so specialization
    // rejects it before any layout is realized.
    let error = monomorphize_generic_machine_value_calls_with_nominal_uses(
        &mut program,
        &mut Vec::new(),
        true,
    )
    .expect_err("a runtime subject cannot close a static layout");
    assert!(!error.is_empty());
}

#[test]
fn runtime_value_requires_rebases_onto_the_realized_parameter() {
    // `requires Count <= 10` is a caller obligation on the captured subject,
    // not a static use of the binder: the clone's contract must read the
    // realized trailing parameter so each rewritten call site owes the fact
    // on its own appended argument.
    let mut program = typed(
        "machine pick<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         machine main() -> u32 { let n: u32 = 3; pick<n>(7) }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("a requires contract on a runtime subject specializes");
    let [receipt] = program.machine_specializations.as_slice() else {
        panic!("one specialization for the runtime tuple");
    };
    let instance = instance(&program, receipt);
    let entry = &program.machine_states(instance)[0];
    let realized = program.state_parameters(entry)[1].symbol;
    let mut names = Vec::new();
    for contract in program.machine_contracts(instance) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            collect_expression_tree(&program, *expression, &mut names);
        }
    }
    assert!(names.iter().any(|handle| {
        matches!(
            program.expression_table.expression(*handle),
            ExpressionNode::Name(path) if path.symbol == realized
        )
    }));
}

#[test]
fn runtime_value_requires_accepts_a_guarded_or_known_subject() {
    for source in [
        // A literal initializer establishes the subject's bound outright.
        "machine pick<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         machine main() -> u32 { let n: u32 = 3; pick<n>(7) }",
        // A dominating transition guard establishes it on the same subject.
        "machine pick<Count: u32>(base: u32) -> u32
         requires
             Count == 3;
         { Count }
         data Main {}
         machine Main::run(n: u32) -> u32 {
             transition n == 3 {
                 true -> allowed(n)
                 false -> denied()
             }
             state allowed(&mut self, n: u32) -> u32 { pick<n>(7) }
             state denied(&mut self) -> u32 { 0 }
         }",
    ] {
        crate::lower_typed_trees(typed(source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn runtime_value_requires_rejects_unestablished_and_stale_subjects() {
    for source in [
        // The subject's declared carrier alone does not establish the bound.
        "machine pick<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         machine main(n: u32) -> u32 { pick<n>(7) }",
        // Reassignment before the call replaces the captured subject: the
        // earlier `3` initializer cannot stand in for the current `99`.
        "machine pick<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         data Main {}
         machine Main::run() -> u32 { let mut n: u32 = 3; n = 99; pick<n>(7) }",
        // A mismatched guard does not prove the subject's equality.
        "machine pick<Count: u32>(base: u32) -> u32
         requires
             Count == 3;
         { Count }
         data Main {}
         machine Main::run(n: u32) -> u32 {
             transition n == 4 {
                 true -> allowed(n)
                 false -> denied()
             }
             state allowed(&mut self, n: u32) -> u32 { pick<n>(7) }
             state denied(&mut self) -> u32 { 0 }
         }",
    ] {
        let error = crate::lower_typed_trees(typed(source))
            .expect_err("an unestablished subject must still owe the requirement");
        assert!(
            error.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("cannot prove requires contract")
            }),
            "{source}: {error:#?}"
        );
    }
}

#[test]
fn mixed_static_and_runtime_value_slots_keep_telescope_order() {
    let mut program = typed(
        "machine pick<Skip: u32, Keep: u32>(base: u32) -> u32 { Keep + Skip }
         machine main(base: u32) -> u32 {
             let n: u32 = 5;
             pick<2, n>(base)
         }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("a mixed static/runtime tuple specializes");
    let [receipt] = program.machine_specializations.as_slice() else {
        panic!("one specialization for the mixed tuple");
    };
    let instance = instance(&program, receipt);
    let state = &program.machine_states(instance)[0];
    // `base` plus exactly the realized `Keep` parameter: the static `Skip`
    // slot substitutes in place and adds no parameter.
    assert_eq!(program.state_parameters(state).len(), 2);
    let caller = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("caller");
    let calls = call_expressions(&program, caller);
    let [call] = calls.as_slice() else {
        panic!("one rewritten call");
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(*call) else {
        unreachable!();
    };
    assert!(call.machine_arguments.is_empty());
    let arguments = program.expression_table.expression_handles(call.arguments);
    // The appended runtime subject lands after the ordinary arguments in
    // telescope order, and it is the caller's own `n` subject.
    assert_eq!(arguments.len(), 2);
    let ExpressionNode::Name(subject) = program.expression_table.expression(arguments[1]) else {
        panic!("the runtime subject stays an ordinary argument");
    };
    let local = program
        .statement_table
        .statements(program.machine_states(caller)[0].statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "n" => Some(local.symbol),
            _ => None,
        })
        .expect("caller local n");
    assert_eq!(subject.symbol, local);
}

#[test]
fn specialized_value_machine_writes_its_own_attached_data() {
    // The specialized clone is a generated root, not an authored child of the
    // program root. Validation must still resolve the clone's own machine
    // symbol so `self.x` is owned data the `&mut self` state may write.
    crate::lower_typed_trees(typed(
        "data Main { x: u32; }
         machine Main::put<Count: u32>(&mut self, v: u32) { self.x = v; }
         machine Main::main(&mut self) { let n: u32 = 3; self.put<n>(10); }",
    ))
    .expect("a specialized clone keeps ownership of its attached data");
}

#[test]
fn runtime_value_indexes_an_attached_array_field_under_requires() {
    // `self.values[Count]` on a `Value` binder: the carrier `u32` proves the
    // lower bound and `requires Count <= 7` proves the length, on reads and
    // writes alike, after the source variable is reassigned to a new subject,
    // and for a second distinct subject sharing the one specialized body.
    crate::lower_typed_trees(typed(
        "data Main { values: [u32; 8]; }
         machine Main::at<Count: u32>(&self) -> u32
         requires
             Count <= 7;
         { self.values[Count] }
         machine Main::put<Count: u32>(&mut self, v: u32)
         requires
             Count <= 7;
         { self.values[Count] = v; }
         machine Main::main(&mut self) {
             let mut n: u32 = 3;
             self.put<n>(10);
             n = 5;
             self.put<n>(20);
             let a: u32 = self.at<n>();
             let m: u32 = 4;
             let b: u32 = self.at<m>();
             let s: u32 = self.at<6>();
         }",
    ))
    .expect("an established bound indexes attached array fields");
}

#[test]
fn runtime_value_index_without_a_bound_still_rejects() {
    // The carrier `u32` proves non-negativity but nothing proves `Count < 8`,
    // so the indexed read must still be refused.
    let error = crate::lower_typed_trees(typed(
        "data Main { values: [u32; 8]; }
         machine Main::at<Count: u32>(&self) -> u32 { self.values[Count] }
         machine Main::main(&mut self) { let n: u32 = 3; let a: u32 = self.at<n>(); }",
    ))
    .expect_err("an unproven index bound must still reject");
    assert!(
        error
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
        "{error:#?}"
    );
}

#[test]
fn generic_to_generic_requires_instantiates_against_the_forwarded_subject() {
    // `bounded<Count>` requires `Count <= 10`; `forward<K>` calls `bounded<K>`,
    // so the instantiated obligation is `K <= 10` on `forward`'s own binder.
    // `forward`'s requires contract establishes that fact for every caller.
    crate::lower_typed_trees(typed(
        "machine bounded<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         machine forward<K: u32>(base: u32) -> u32
         requires
             K <= 10;
         { bounded<K>(base) }
         machine main() -> u32 { let n: u32 = 3; forward<n>(7) }",
    ))
    .expect("the callee's requires discharges against the forwarded subject");
}

#[test]
fn generic_to_generic_requires_still_owes_the_unestablished_fact() {
    // Without `forward`'s own `requires K <= 10`, the instantiated obligation
    // names the forwarded `K` subject — not the callee's `Count` binder and
    // not a literal — and nothing establishes it.
    let error = crate::lower_typed_trees(typed(
        "machine bounded<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         machine forward<K: u32>(base: u32) -> u32 { bounded<K>(base) }
         machine main() -> u32 { let n: u32 = 3; forward<n>(7) }",
    ))
    .expect_err("an unestablished forwarded subject must still owe the requirement");
    assert!(
        error.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract")
                && diagnostic.message.contains("K <= 10")
        }),
        "{error:#?}"
    );
}

#[test]
fn runtime_value_subjects_reach_transition_targets_in_a_cloned_machine() {
    // A multi-state template cloned for a runtime `Value` subject carries the
    // realized subject as a trailing parameter on every cloned state, so a
    // `->` transition between them owes the target that appended subject
    // exactly as a rewritten call site does.
    let mut program = typed(
        "data Main { values: [u8; 8]; }
         machine Main::walk<Count: u8>(&mut self, n: u8) {
             transition n == 3 {
                 true -> allowed(n)
                 false -> denied()
             }
             state allowed(&mut self, n: u8) { self.values[3] = Count; }
             state denied(&mut self) { self.values[4] = Count; }
         }
         machine Main::main(&mut self) {
             let n: u8 = 3;
             self.walk<n>(n);
         }",
    );
    monomorphize_generic_machine_value_calls_with_nominal_uses(&mut program, &mut Vec::new(), true)
        .expect("a transitioned runtime subject specializes");
    let [receipt] = program.machine_specializations.as_slice() else {
        panic!("one specialization for the runtime tuple");
    };
    let instance = instance(&program, receipt);
    let states = program.machine_states(instance);
    assert_eq!(states.len(), 3, "entry plus the two transition targets");
    for state in states {
        let parameters = program.state_parameters(state);
        let last = parameters.last().expect("a trailing realized parameter");
        assert_eq!(
            last.name.as_str(),
            "Count",
            "every cloned state carries the realized subject parameter"
        );
    }
    // The entry state's transitions forward its own realized parameter to
    // each named sibling target, after the authored arguments. The guarded
    // `transition` form lowers to one statement per arm.
    let entry = &states[0];
    let clone_state_symbols: Vec<_> = states.iter().map(|state| state.symbol).collect();
    let transitions: Vec<_> = program
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::Transition(transition) => Some(transition),
            _ => None,
        })
        .collect();
    assert_eq!(
        transitions.len(),
        2,
        "both dispatch arms lower to transitions"
    );
    let entry_realized = program.state_parameters(entry).last().unwrap().symbol;
    let mut named_targets = 0;
    for transition in transitions {
        for target in [transition.target, transition.continuation] {
            let typed_trees::statement::TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(target)
            else {
                continue;
            };
            if !clone_state_symbols.contains(&path.symbol) {
                continue;
            }
            named_targets += 1;
            let arguments = program.statement_table.expression_handles(*arguments);
            let ExpressionNode::Name(subject) = program
                .expression_table
                .expression(*arguments.last().expect("an appended subject argument"))
            else {
                panic!("the trailing transition argument is the subject name");
            };
            assert_eq!(
                subject.symbol, entry_realized,
                "the transition forwards the containing state's realized subject"
            );
        }
    }
    assert_eq!(
        named_targets, 2,
        "both `allowed` and `denied` targets carry the forwarded subject"
    );
    // The clone remains an ordinary attached Unit machine: checked lowering
    // keeps its composed plan, and the entry conditional's edges record the
    // forwarded subject as an exact scalar argument per target.
    let clone_symbol = instance.symbol;
    let checked =
        crate::lower_typed_trees(program).expect("forwarded transition subjects validate");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(clone_symbol)
        .expect("the cloned multi-state body keeps its composed unit plan");
    assert_eq!(plan.states.len(), 3, "entry plus both transition targets");
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &plan.states[0].terminator
    else {
        panic!("the guarded dispatch stays a conditional terminator");
    };
    assert_eq!(
        when_true.scalar_arguments.len(),
        2,
        "`allowed` receives the authored argument and the forwarded subject"
    );
    assert_eq!(
        when_false.scalar_arguments.len(),
        1,
        "`denied` receives the forwarded subject alone"
    );
}

#[test]
fn runtime_bound_result_range_discharges_through_checking() {
    // The whole spine: a declared local bound `u64[0..=n]` admits a runtime
    // subject in its maximum, and the call's result proves `Bound <= n`
    // through the captured-argument binding. The authored total keeps its
    // declared carrier; the relation rides on the inferred temporary.
    crate::lower_typed_trees(typed(
        "machine ranged<Bound: u64>() -> u64[0..=Bound] { Bound }
         machine main() -> u64 {
             let n: u64 = 7;
             let captured: u64[0..=n] = ranged<n>();
             let total: u64 = ranged<n>();
             captured
         }",
    ))
    .expect("a runtime-bound result range checks end to end");
}

#[test]
fn runtime_bound_result_range_still_rejects_an_unproven_subject() {
    // The same relation is enforced in the caller's scope: a bound naming
    // `n` does not admit a result captured against a different argument `m`.
    let diagnostics = crate::lower_typed_trees(typed(
        "machine ranged<Bound: u64>() -> u64[0..=Bound] { Bound }
         machine main() -> u64 {
             let n: u64 = 7;
             let m: u64 = 3;
             let wrong: u64[0..=n] = ranged<m>();
             wrong
         }",
    ))
    .expect_err("the bound names a different subject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("not provably within its declared symbolic const range")
    }));
}
