use super::{ExpressionHandle, ExpressionNode, StatementNode, TypedTrees};
use crate::monomorphization::collect_statement_expression_trees;
use crate::monomorphization::monomorphize_generic_machine_value_calls_with_selections;
use crate::tests::front_end::typed_program;
use typed_trees::machine::Machine;
use typed_trees::typed_trees::MachineSpecialization;

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
fn runtime_bound_result_range_uses_the_realized_parameter() {
    // `-> u64[0..=Bound]` qualifies the realized subject, not a static
    // position: the clone keeps the range naming its realized trailing
    // parameter, and the caller's inferred result indexes on the captured
    // argument itself.
    // The tail call auto-hoists into an inferred local: its declared type is
    // exactly the substituted `u64[0..=n]`.
    let mut program = typed_program(
        "machine ranged<Bound: u64>() -> u64[0..=Bound] { Bound }
         machine main() -> u64 {
             let n: u64 = 7;
             ranged<n>()
         }",
    );
    monomorphize_generic_machine_value_calls_with_selections(
        &mut program,
        &mut Default::default(),
        true,
    )
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
                    typed_trees::dependent_ranges::scoped_name_bound(program, *maximum)
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
fn mixed_static_and_runtime_value_slots_keep_telescope_order() {
    let mut program = typed_program(
        "machine pick<Skip: u32, Keep: u32>(base: u32) -> u32 { Keep + Skip }
         machine main(base: u32) -> u32 {
             let n: u32 = 5;
             pick<2, n>(base)
         }",
    );
    monomorphize_generic_machine_value_calls_with_selections(
        &mut program,
        &mut Default::default(),
        true,
    )
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
fn runtime_value_indexes_an_attached_array_field_under_requires() {
    // `self.values[Count]` on a `Value` binder: the carrier `u32` proves the
    // lower bound and `requires Count <= 7` proves the length, on reads and
    // writes alike, after the source variable is reassigned to a new subject,
    // and for a second distinct subject sharing the one specialized body.
    crate::lower_typed_trees(
        typed_program(
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
        ),
        &crate::CheckingRequest::settled(),
    )
    .expect("an established bound indexes attached array fields");
}

#[test]
fn generic_to_generic_requires_instantiates_against_the_forwarded_subject() {
    // `bounded<Count>` requires `Count <= 10`; `forward<K>` calls `bounded<K>`,
    // so the instantiated obligation is `K <= 10` on `forward`'s own binder.
    // `forward`'s requires contract establishes that fact for every caller.
    crate::lower_typed_trees(
        typed_program(
            "machine bounded<Count: u32>(base: u32) -> u32
         requires
             Count <= 10;
         { Count }
         machine forward<K: u32>(base: u32) -> u32
         requires
             K <= 10;
         { bounded<K>(base) }
         machine main() -> u32 { let n: u32 = 3; forward<n>(7) }",
        ),
        &crate::CheckingRequest::settled(),
    )
    .expect("the callee's requires discharges against the forwarded subject");
}

#[test]
fn runtime_value_subjects_reach_transition_targets_in_a_cloned_machine() {
    // A multi-state template cloned for a runtime `Value` subject carries the
    // realized subject as a trailing parameter on every cloned state, so a
    // `->` transition between them owes the target that appended subject
    // exactly as a rewritten call site does.
    let mut program = typed_program(
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
    monomorphize_generic_machine_value_calls_with_selections(
        &mut program,
        &mut Default::default(),
        true,
    )
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
    let checked = crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
        .expect("forwarded transition subjects validate");
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
fn runtime_bound_result_range_still_rejects_an_unproven_subject() {
    // The same relation is enforced in the caller's scope: a bound naming
    // `n` does not admit a result captured against a different argument `m`.
    let diagnostics = crate::lower_typed_trees(
        typed_program(
            "machine ranged<Bound: u64>() -> u64[0..=Bound] { Bound }
         machine main() -> u64 {
             let n: u64 = 7;
             let m: u64 = 3;
             let wrong: u64[0..=n] = ranged<m>();
             wrong
         }",
        ),
        &crate::CheckingRequest::settled(),
    )
    .expect_err("the bound names a different subject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("not provably within its declared symbolic const range")
    }));
}
