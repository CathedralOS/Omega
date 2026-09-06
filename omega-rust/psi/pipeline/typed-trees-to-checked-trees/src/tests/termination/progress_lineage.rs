use super::*;
use checked_trees::CheckedTrees;
use language_semantics::{MachineTerminationPlan, TerminationGuarantee, TerminationInterface};

fn typed(source: &str) -> typed_trees::TypedTrees {
    let source = format!("data Main {{}} machine Main::run(&mut self) {{}} {source}");
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenize lineage fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse lineage fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve lineage fixture");
    lower_symbol_resolved_trees(&resolved).expect("type lineage fixture")
}

fn checked(source: &str) -> CheckedTrees {
    lower_typed_trees(typed(source)).unwrap_or_else(|diagnostics| {
        panic!("lineage fixture must reach checked trees: {diagnostics:#?}")
    })
}

fn plan<'program>(program: &'program CheckedTrees, name: &str) -> &'program MachineTerminationPlan {
    program
        .facts
        .termination
        .for_machine(symbol_of_checked(program, name))
        .unwrap_or_else(|| panic!("checked termination plan for {name}"))
}

#[test]
fn unused_growing_projection_finishes_checked_termination() {
    // Both edges strictly descend. The recursive reference is forwarded but
    // never used by a progress-premise-bearing operation. Its growing paths
    // must not prevent checking this unconditional termination guarantee.
    let program = checked(
        r#"
        data Node { next: &Node; }

        machine walk(node: &Node, remaining: u64)
        terminates by remaining -> Nat::Descending;
        -> u64 {
            transition remaining > 0 {
                true -> step(node.next, remaining - 1)
                false -> 0
            }

            state step(node: &Node, remaining: u64) -> u64 {
                transition remaining > 0 {
                    true -> walk(node.next, remaining - 1)
                    false -> 0
                }
            }
        }
        "#,
    );
    let plan = plan(&program, "walk");
    assert_eq!(plan.interface, TerminationInterface::InternalDerived);
    assert!(plan.implementation_witness.is_some());
    assert_eq!(
        plan.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new(),
        }
    );
}

// These are the owner-classified profile and conservative published operation
// contracts used by validation_integration.rs and private_joint_progress.rs.
// No call to grant establishes a local receipt in these fixtures.
const PROGRESS_PROFILE: &str = r#"
pub data SchedulerHandle {}
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#;

const CONTEXT_OPERATION: &str = r#"
pub data Context { scheduler: SchedulerHandle; }
pub machine wait_context(context: Context)
requires context.scheduler in WeakFair
terminates;
-> u64 { 0 }
"#;

fn assert_exact_subjects(
    program: &CheckedTrees,
    machine_name: &str,
    operation_name: &str,
    projection_path: &str,
    parameter_names: &[&str],
) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("subject-owning machine");
    let entry = &program.machine_states(machine)[0];
    let Some(TerminationGuarantee::Terminates {
        premises: operation_premises,
    }) = plan(program, operation_name).interface.published()
    else {
        panic!("operation must publish its exact progress premise")
    };
    let [operation_premise] = operation_premises.as_slice() else {
        panic!("operation must publish exactly one progress premise")
    };
    let [projection] = operation_premise.subject.projections.as_slice() else {
        panic!("operation must require exactly one scheduler field")
    };
    assert_eq!(
        program.symbols.display_path(*projection, "::"),
        projection_path
    );
    let TerminationGuarantee::Terminates { premises } =
        &plan(program, machine_name).checked_summary
    else {
        panic!("finite exact subjects must retain checked termination")
    };
    assert_eq!(premises.len(), parameter_names.len());
    for name in parameter_names {
        let parameter = program
            .state_parameters(entry)
            .iter()
            .find(|parameter| parameter.name.as_str() == *name)
            .expect("exact entry parameter");
        let mut expected = operation_premise.clone();
        expected.subject.root = parameter.symbol;
        assert!(
            premises.contains(&expected),
            "missing exact {name} subject: {premises:#?}"
        );
    }
}

#[test]
fn finite_predecessor_alternatives_retain_both_exact_projected_subjects() {
    let source = format!(
        r#"{PROGRESS_PROFILE}
        {CONTEXT_OPERATION}
        machine process(first: Context, second: Context, choose_first: bool)
        requires first.scheduler in WeakFair
        requires second.scheduler in WeakFair
        terminates;
        -> u64 {{
            transition choose_first {{
                true -> waiting(first)
                false -> waiting(second)
            }}

            state waiting(context: Context) -> u64
            requires context.scheduler in WeakFair
            {{ wait_context(context) }}
        }}
        "#
    );
    let program = checked(&source);
    assert_exact_subjects(
        &program,
        "process",
        "wait_context",
        "Context::scheduler",
        &["first", "second"],
    );
}

#[test]
fn identity_cycle_swapping_entry_parameters_retains_both_subjects() {
    // A complete local cycle exchanges first and second. Neither edge adds a
    // projection, so repeated visits produce two exact roots, not ambiguity.
    let source = format!(
        r#"{PROGRESS_PROFILE}
        {CONTEXT_OPERATION}
        machine process(first: Context, second: Context, remaining: u64)
        requires first.scheduler in WeakFair
        requires second.scheduler in WeakFair
        terminates by remaining -> Nat::Descending;
        -> u64 {{
            transition remaining > 0 {{
                true -> step(second, first, remaining - 1)
                false -> wait_context(first)
            }}

            state step(first: Context, second: Context, remaining: u64) -> u64 {{
                transition remaining > 0 {{
                    true -> process(first, second, remaining - 1)
                    false -> 0
                }}
            }}
        }}
        "#
    );
    let program = checked(&source);
    assert_eq!(
        plan(&program, "process").interface,
        TerminationInterface::InternalDerived
    );
    assert!(plan(&program, "process").implementation_witness.is_some());
    assert_exact_subjects(
        &program,
        "process",
        "wait_context",
        "Context::scheduler",
        &["first", "second"],
    );
}

#[test]
fn unused_growing_projection_preserves_an_independent_exact_premise() {
    let source = format!(
        r#"{PROGRESS_PROFILE}
        {CONTEXT_OPERATION}
        data Node {{ next: &Node; }}
        machine walk(node: &Node, context: Context, remaining: u64)
        requires context.scheduler in WeakFair
        terminates by remaining -> Nat::Descending;
        -> u64 {{
            transition remaining > 0 {{
                true -> step(node.next, context, remaining - 1)
                false -> wait_context(context)
            }}

            state step(node: &Node, context: Context, remaining: u64) -> u64 {{
                transition remaining > 0 {{
                    true -> walk(node.next, context, remaining - 1)
                    false -> 0
                }}
            }}
        }}
        "#
    );
    let program = checked(&source);
    assert_eq!(
        plan(&program, "walk").interface,
        TerminationInterface::InternalDerived
    );
    assert!(plan(&program, "walk").implementation_witness.is_some());
    assert_exact_subjects(
        &program,
        "walk",
        "wait_context",
        "Context::scheduler",
        &["context"],
    );
}

const NODE_CYCLE: &str = r#"
pub data Node { scheduler: SchedulerHandle; next: &Node; }
pub machine wait_node(node: &Node)
requires node.scheduler in WeakFair
terminates;
-> u64 { 0 }

machine walk(node: &Node, remaining: u64)
requires node.scheduler in WeakFair
terminates by remaining -> Nat::Descending;
-> u64 {
    transition remaining > 0 {
        true -> step(node, remaining - 1)
        false -> wait_node(node)
    }

    state step(node: &Node, remaining: u64) -> u64 {
        transition remaining > 0 {
            true -> walk(node.next, remaining - 1)
            false -> 0
        }
    }
}
"#;

fn node_cycle(published: bool, growing: bool) -> String {
    let mut source = format!("{PROGRESS_PROFILE}\n{NODE_CYCLE}");
    if published {
        source = source
            .replace("machine walk(", "pub machine walk(")
            .replace(
                "terminates by remaining",
                "terminates;\nterminates by remaining",
            );
    }
    if !growing {
        source = source.replace("walk(node.next,", "walk(node,");
    }
    source
}

#[test]
fn projected_entry_into_an_identity_loop_retains_the_finite_subject() {
    let source = format!(
        r#"{PROGRESS_PROFILE}
        pub data Node {{ scheduler: SchedulerHandle; next: &Node; }}
        pub machine wait_node(node: &Node)
        requires node.scheduler in WeakFair
        terminates;
        -> u64 {{ 0 }}

        machine walk(node: &Node, remaining: u64)
        requires node.next.scheduler in WeakFair
        terminates by remaining -> Nat::Descending;
        -> u64 {{
            transition remaining > 0 {{
                true -> step(node.next, remaining - 1)
                false -> 0
            }}
            state step(node: &Node, remaining: u64) -> u64 {{
                transition remaining > 0 {{
                    true -> step(node, remaining - 1)
                    false -> wait_node(node)
                }}
            }}
        }}"#
    );
    let program = checked(&source);
    let TerminationGuarantee::Terminates { premises } = &plan(&program, "walk").checked_summary
    else {
        panic!("one projected entry must stay exact through an identity cycle");
    };
    let [premise] = premises.as_slice() else {
        panic!("the cycle needs exactly one projected caller premise");
    };
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap();
    assert_eq!(
        premise.subject.root,
        program.state_parameters(&program.machine_states(machine)[0])[0].symbol
    );
    assert_eq!(
        premise
            .subject
            .projections
            .iter()
            .map(|projection| program.symbols.display_path(*projection, "::"))
            .collect::<Vec<_>>(),
        ["Node::next", "Node::scheduler"]
    );
}

#[test]
fn demanded_identity_forwarding_retains_the_single_entry_subject() {
    // This source control changes only the growing back-edge to identity.
    // Both the private and published forms must finish with the exact premise.
    for published in [false, true] {
        let program = checked(&node_cycle(published, false));
        assert_exact_subjects(&program, "walk", "wait_node", "Node::scheduler", &["node"]);
        let plan = plan(&program, "walk");
        assert!(plan.implementation_witness.is_some());
        if published {
            assert_eq!(plan.interface.published(), Some(&plan.checked_summary));
        } else {
            assert_eq!(plan.interface, TerminationInterface::InternalDerived);
        }
    }
}

#[test]
fn demanded_growing_projection_has_no_private_checked_guarantee() {
    // A valid ranking alone cannot cover node.next.scheduler, then
    // node.next.next.scheduler, with the entry node.scheduler premise.
    // Private inference reports NoGuarantee; it must not reject valid source
    // merely because the machine has no published completion promise.
    let program = checked(&node_cycle(false, true));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk machine");
    assert!(
        crate::checks::termination::infer_machine_checked_summary(&program.typed, machine)
            .promises_termination(),
        "the independent local ranking must succeed"
    );
    let plan = plan(&program, "walk");
    assert_eq!(plan.interface, TerminationInterface::InternalDerived);
    assert!(plan.implementation_witness.is_some());
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn demanded_growing_projection_rejects_the_finite_published_premise() {
    let program = typed(&node_cycle(true, true));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk machine");
    assert!(
        crate::checks::termination::infer_machine_checked_summary(&program, machine)
            .promises_termination(),
        "the rejection must concern progress coverage, not the ranking"
    );
    let Some(TerminationGuarantee::Terminates { premises }) =
        machine.termination_plan.interface.published()
    else {
        panic!("walk must publish its finite entry premise")
    };
    assert_eq!(premises.len(), 1);
    assert_eq!(premises[0].subject.projections.len(), 1);
    let entry = &program.machine_states(machine)[0];
    assert_eq!(
        premises[0].subject.root,
        program.state_parameters(entry)[0].symbol
    );
    assert_eq!(
        program
            .symbols
            .display_path(premises[0].subject.projections[0], "::"),
        "Node::scheduler"
    );

    let diagnostics = match lower_typed_trees(program) {
        Ok(_) => panic!("a finite entry premise cannot cover the growing dependency"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove published termination for machine `walk`")),
        "expected published-progress rejection: {diagnostics:#?}"
    );
}
