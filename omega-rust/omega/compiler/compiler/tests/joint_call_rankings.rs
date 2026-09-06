use compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const JOINT: &str = include_str!(
    "../../../../../tests/omega/pass/termination/joint_lexicographic_machine_call_cycle_compile/main.omg"
);
static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-joint-call-rankings-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test project");
        fs::write(path.join("main.omg"), source).expect("write source regression");
        Self(path)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test project");
    }
}

fn checks(source: &str) {
    let project = Project::new(source);
    compile_to_checked(&project.0.join("main.omg"), None).expect("joint ranking must check");
}

fn rejects_cycle(source: &str) {
    let project = Project::new(source);
    let diagnostics = match compile_to_checked(&project.0.join("main.omg"), None) {
        Ok(_) => panic!("unsound joint ranking was accepted"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("machine call cycle")),
        "expected call-cycle rejection: {diagnostics:#?}"
    );
}

#[test]
fn forwarding_then_lexicographic_descent_reaches_checked() {
    checks(JOINT);
}

#[test]
fn stalled_lexicographic_cycle_rejects() {
    rejects_cycle(&JOINT.replace("inner: progress.inner - 1", "inner: progress.inner"));
}

#[test]
fn unguarded_or_wrong_subject_descent_rejects() {
    for guard in [
        "progress.outer > 0",
        "progress.inner >= 0",
        "progress.inner == 0",
    ] {
        rejects_cycle(&JOINT.replace("progress.inner > 0", guard));
    }
}

#[test]
fn earlier_component_increase_cannot_borrow_later_descent() {
    rejects_cycle(&JOINT.replace("outer: progress.outer", "outer: progress.outer + 1"));
}

#[test]
fn earlier_lexicographic_descent_allows_later_reset() {
    checks(
        &JOINT
            .replace("progress.inner > 0", "progress.outer > 0")
            .replace("outer: progress.outer", "outer: progress.outer - 1")
            .replace("inner: progress.inner - 1", "inner: 4"),
    );
}

#[test]
fn false_arm_uses_only_its_own_guard_polarity() {
    checks(
        &JOINT
            .replace("progress.inner > 0", "progress.inner <= 0")
            .replace("true -> self.scan_", "false -> self.scan_")
            .replace("false -> 0", "true -> 0"),
    );
    rejects_cycle(&JOINT.replace("progress.inner > 0", "progress.inner <= 0"));
}

#[test]
fn distinct_view_declarations_do_not_share_a_ranking() {
    let source = JOINT.replace(
        "data Main {}",
        "measure Progress::Other lexicographic { inner, outer }\ndata Main {}",
    );
    let split = source.find("machine Main::scan_b").unwrap();
    let source = format!(
        "{}{}",
        &source[..split],
        source[split..].replace("Progress::Steps", "Progress::Other")
    );
    rejects_cycle(&source);
}

#[test]
fn runtime_non_tail_cycle_remains_rejected() {
    rejects_cycle(&JOINT.replace(
        "true -> self.scan_b(progress)",
        "true -> 1 + self.scan_b(progress)",
    ));
}

#[test]
fn typed_parameter_positions_are_substituted_at_each_site() {
    let source = JOINT
        .replace("self.scan_b(progress)", "self.scan_b(99, progress)")
        .replace(
            "Main::scan_b(&mut self, progress: Progress)",
            "Main::scan_b(&mut self, unrelated: u64, progress: Progress)",
        );
    checks(&source);
    rejects_cycle(&source.replace(
        "self.scan_b(99, progress)",
        "self.scan_b(99, Progress { outer: 1, inner: 4 })",
    ));
}

#[test]
fn scalar_forwarding_and_guarded_descent_reach_checked() {
    checks(
        r#"
data Main {}
machine Main::main(&mut self) -> u64 { transition { _ -> self.a(4) } }
machine Main::a(&mut self, remaining: u64)
terminates by remaining;
-> u64 { transition { _ -> self.b(remaining) } }
machine Main::b(&mut self, count: u64)
terminates by count -> Nat::Descending;
-> u64 { transition count { 0 -> 0 _ -> self.a(count - 1) } }
"#,
    );
}

#[test]
fn parallel_stalled_call_site_rejects() {
    let source = JOINT.replace("false -> 0", "false -> self.scan_a(progress)");
    rejects_cycle(&source);
}

#[test]
fn assignments_and_borrows_invalidate_entry_rank_lineage() {
    let source = JOINT.replace("progress: Progress)", "mut progress: Progress)");
    for action in [
        "progress.inner = 4;",
        "let alias: &mut Progress = &mut progress;",
    ] {
        let split = source.find("machine Main::scan_b").unwrap();
        let source = format!(
            "{}{}",
            &source[..split],
            source[split..].replace(
                "    transition progress.inner",
                &format!("    {action}\n    transition progress.inner")
            )
        );
        rejects_cycle(&source);
    }
}

#[test]
fn intervening_call_cannot_preserve_entry_rank_facts() {
    let source = JOINT.replace(
        "data Main {}",
        "data Main {}\nmachine Main::touch(&mut self) {}",
    );
    let split = source.find("machine Main::scan_b").unwrap();
    let source = format!(
        "{}{}",
        &source[..split],
        source[split..].replace(
            "    transition progress.inner",
            "    self.touch();\n    transition progress.inner"
        )
    );
    rejects_cycle(&source);
}

#[test]
fn every_cycle_in_three_machine_component_must_decrease() {
    let source = r#"
data Main {}
machine Main::main(&mut self) -> u64 { transition { _ -> self.a(4) } }
machine Main::a(&mut self, n: u64) terminates by n;
-> u64 { transition { _ -> self.b(n) } }
machine Main::b(&mut self, n: u64) terminates by n;
-> u64 { transition n { 0 -> 0 _ -> self.c(n) } }
machine Main::c(&mut self, n: u64) terminates by n;
-> u64 {
    transition n {
        0 -> 0
        1 -> self.a(n - 1)
        _ -> self.b(n - 1)
    }
}
"#;
    checks(source);
    rejects_cycle(&source.replace("_ -> self.b(n - 1)", "_ -> self.b(n)"));
}

#[test]
fn scalar_preserving_cross_edge_cannot_hide_behind_a_decreasing_dfs_cycle() {
    let source = r#"
data Main {}
machine Main::main(&mut self) -> u64 { transition { _ -> self.a(4) } }
machine Main::a(&mut self, n: u64) terminates by n;
-> u64 { transition n { 0 -> 0 1 -> self.b(n - 1) _ -> self.c(n - 1) } }
machine Main::b(&mut self, n: u64) terminates by n;
-> u64 { transition { _ -> self.c(n) } }
machine Main::c(&mut self, n: u64) terminates by n;
-> u64 { transition { _ -> self.a(n) } }
"#;
    checks(source);
    // DFS first visits a -> b -> c -> a, which decreases. Its later a -> c
    // edge reaches a finished node and hides the preserving a -> c -> a cycle.
    rejects_cycle(&source.replace("_ -> self.c(n - 1)", "_ -> self.c(n)"));
}

#[test]
fn scalar_parallel_preserving_site_cannot_borrow_another_sites_descent() {
    let source = r#"
data Main {}
machine Main::main(&mut self) -> u64 { transition { _ -> self.a(4) } }
machine Main::a(&mut self, n: u64) terminates by n;
-> u64 { transition n { 0 -> 0 1 -> self.b(n - 1) _ -> self.b(n - 1) } }
machine Main::b(&mut self, n: u64) terminates by n;
-> u64 { transition { _ -> self.a(n) } }
"#;
    checks(source);
    rejects_cycle(&source.replace("_ -> self.b(n - 1)", "_ -> self.b(n)"));
}

#[test]
fn joint_witness_does_not_publish_an_exported_guarantee() {
    let project = Project::new(
        &JOINT
            .replace("machine Main::scan_", "pub machine Main::scan_")
            .replace("data Main", "pub data Main")
            .replace("data Progress", "pub data Progress"),
    );
    let checked = compile_to_checked(&project.0.join("main.omg"), None)
        .expect("exported joint ranking must check")
        .into_program();
    for name in ["Main::scan_a", "Main::scan_b"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        assert!(machine.termination_plan.implementation_witness.is_some());
        let plan = checked
            .facts
            .termination
            .for_machine(machine.symbol)
            .expect("checked termination plan");
        assert!(
            plan.interface
                .published()
                .is_none_or(|guarantee| !guarantee.promises_termination())
        );
    }
}

#[test]
fn proof_only_structural_cycles_still_require_descent_on_every_edge() {
    let source = r#"
data ProofTree { case Leaf; case Branch(first: ProofTree, second: ProofTree); }
data Main {}
machine Main::main(&mut self) {}
machine left(n: ProofTree) terminates by n; -> ProofTree {
    transition n {
        ProofTree::Leaf -> ProofTree::Leaf
        ProofTree::Branch { first, second } -> ProofTree::Branch {
            first: right(first), second: right(second),
        }
    }
}
machine right(n: ProofTree) terminates by n; -> ProofTree {
    transition n {
        ProofTree::Leaf -> ProofTree::Leaf
        ProofTree::Branch { first, second } -> ProofTree::Branch {
            first: left(first), second: left(second),
        }
    }
}
"#;
    checks(source);
    rejects_cycle(&source.replace("right(first)", "right(n)"));
}

#[test]
fn proof_receiver_dependency_cannot_hide_an_unmeasured_cycle() {
    rejects_cycle(
        r#"
data ProofNat { case Zero; case Succ(previous: ProofNat); }
data Main {}
machine Main::main(&mut self) {}
machine left(n: ProofNat) -> ProofNat { n.right() }
machine ProofNat::right(self) -> ProofNat { left(self) }
"#,
    );
}

#[test]
fn parallel_proof_receiver_call_requires_its_own_structural_certificate() {
    let source = r#"
data ProofTree { case Leaf; case Branch(first: ProofTree, second: ProofTree); }
data Main {}
machine Main::main(&mut self) {}
machine ProofTree::left(self, n: ProofTree) terminates by n; -> ProofTree {
    transition n {
        ProofTree::Leaf -> ProofTree::Leaf
        ProofTree::Branch { first, second } -> ProofTree::Branch {
            first: self.right(first), second: second,
        }
    }
}
machine ProofTree::right(self, n: ProofTree) terminates by n; -> ProofTree {
    transition n {
        ProofTree::Leaf -> ProofTree::Leaf
        ProofTree::Branch { first, second } -> ProofTree::Branch {
            first: self.left(first), second: second,
        }
    }
}
"#;
    checks(source);
    // The SCC and its existing strict edges stay the same. The extra call
    // selects right through a non-self receiver and passes the unchanged n.
    for receiver in ["n", "second"] {
        rejects_cycle(&source.replacen(
            "second: second,",
            &format!("second: {receiver}.right(n),"),
            1,
        ));
    }
}

#[test]
fn acyclic_pattern_payload_method_retains_its_selected_callee() {
    let source = r#"
data ProofNat { case Zero; case Succ(previous: ProofNat); }
data Other { case Zero; case Succ(previous: Other); }
data Main {}
machine Main::main(&mut self) {}
machine ProofNat::finish(self) -> ProofNat { self }
machine Other::finish(self) -> Other { self }
machine finish() -> u64 { 99 }
machine peel(n: ProofNat, payload: Other) -> ProofNat {
    transition n {
        ProofNat::Zero -> ProofNat::Zero
        ProofNat::Succ { previous as payload } -> payload.finish()
    }
}
"#;
    let project = Project::new(source);
    let checked = compile_to_checked(&project.0.join("main.omg"), None)
        .expect("acyclic proof payload method must check")
        .into_program();
    let callee = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ProofNat::finish")
        .expect("proof payload method");
    let expected = checked.machine_states(callee)[0].symbol;
    let calls = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "finish" =>
            {
                Some(call)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !calls.is_empty(),
        "payload call must remain in the typed program"
    );
    for call in calls {
        assert_eq!(call.target_symbol, expected);
        let typed_trees::expression::ExpressionNode::Member(member) =
            checked.expression_table.expression(call.receiver)
        else {
            panic!("pattern binding must retain its payload projection");
        };
        assert_eq!(member.member.as_str(), "previous");
        assert_eq!(member.case_variant.as_ref().unwrap().as_str(), "Succ");
    }
}

#[test]
fn self_pattern_payload_method_does_not_select_the_enclosing_state() {
    checks(
        "data Cell {} data Tree { case Branch(right: Cell); }
        data Main {} machine Main::main(&mut self) {}
        machine Cell::read(self) -> u64 { 1 }
        machine Tree::right(self) -> u64 {
            transition self { Tree::Branch { right } -> right.read() }
        }",
    );
}

#[test]
fn authored_joint_termination_guarantees_seed_progress_summaries() {
    for authored_count in [1, 2] {
        let source = JOINT.replacen(
            "terminates by progress",
            "terminates;\nterminates by progress",
            authored_count,
        );
        let project = Project::new(&source);
        let checked = compile_to_checked(&project.0.join("main.omg"), None)
            .expect("ranked SCC with authored termination must check")
            .into_program();
        for name in ["Main::scan_a", "Main::scan_b"] {
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            assert!(
                checked
                    .facts
                    .termination
                    .for_machine(machine.symbol)
                    .unwrap()
                    .checked_summary
                    .promises_termination()
            );
        }
    }
}
