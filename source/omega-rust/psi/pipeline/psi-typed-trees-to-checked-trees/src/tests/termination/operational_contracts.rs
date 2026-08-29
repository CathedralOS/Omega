use super::*;

#[test]
fn symbol_resolved_service_reach_propagates_boundary_identity_and_parent_closure() {
    let source = r#"
    boundary trait Readable {
        machine read() -> u64;
    }

    boundary trait Filesystem: Readable {
    }

    data Worker { reader: Readable; }
    machine Worker::run(&mut self) -> u64 reaches Filesystem {
        self.reader.read()
    }

    data Main { worker: Worker; }
    machine Main::main(&mut self) -> u64 {
        self.worker.run()
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let worker = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("worker")
        .symbol;
    let main = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main")
        .symbol;
    let checked = lower_typed_trees(typed).expect("service ceiling should admit the body");

    let worker_reach = checked
        .facts
        .service_reaches
        .for_machine(worker)
        .expect("worker service facts");
    let published = checked
        .facts
        .service_reaches
        .rows
        .services(worker_reach.published_ceiling);
    let published_names = published
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(published_names, ["Filesystem", "Readable"]);

    let main_reach = checked
        .facts
        .service_reaches
        .for_machine(main)
        .expect("main service facts");
    assert_eq!(
        checked
            .facts
            .service_reaches
            .rows
            .services(main_reach.inferred_transitive),
        published,
        "an internal caller consumes the callee's published service ceiling",
    );
}

#[test]
fn symbol_resolved_service_ceiling_rejects_undeclared_boundary_reach() {
    let source = r#"
    boundary trait Readable { machine read() -> u64; }
    boundary trait Queryable { }

    data Main { reader: Readable; }
    machine Main::run(&mut self) -> u64 reaches Queryable {
        self.reader.read()
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("service ceiling must reject widening");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("reaches undeclared service `Readable`")
        }),
        "expected symbol-resolved service ceiling diagnostic, got {diagnostics:#?}",
    );
}

#[test]
fn operational_plans_are_independent_from_service_reach_rows() {
    use psi_language_semantics::{
        BlockingInterface, ServiceReachInterface, ServiceReachRowTable, SuspensionInterface,
    };

    let source = r#"
    boundary trait Clock { machine read(); }

    data Sleeper { clock: Clock; }
    machine Sleeper::wait(&mut self) reaches Clock suspends; blocks; {}

    data Main { sleeper: Sleeper; }
    machine Main::run(&mut self) {
        self.sleeper.wait();
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let wait = symbol_of("Sleeper::wait");
    let run = symbol_of("Main::run");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let wait_suspension = checked
        .facts
        .suspensions
        .for_machine(wait)
        .expect("published callee suspension plan");
    let wait_blocking = checked
        .facts
        .blocking
        .for_machine(wait)
        .expect("published callee blocking plan");
    assert_eq!(
        wait_suspension.interface,
        SuspensionInterface::PublishedMaySuspend(true)
    );
    assert_eq!(
        wait_blocking.interface,
        BlockingInterface::PublishedMayBlock(true)
    );
    assert!(!wait_suspension.checked_may_suspend);
    assert!(!wait_blocking.checked_may_block);
    let wait_reach = checked
        .facts
        .service_reaches
        .plan_for_machine(wait)
        .expect("published callee service plan");
    let ServiceReachInterface::PublishedCeiling(wait_ceiling) = wait_reach.interface else {
        panic!("wait should publish its authored service ceiling");
    };
    let clock = checked
        .facts
        .service_reaches
        .services
        .id_for_name("Clock")
        .expect("Clock service");
    assert_eq!(
        checked.facts.service_reaches.rows.services(wait_ceiling),
        &[clock]
    );
    assert_eq!(wait_reach.checked_inferred, ServiceReachRowTable::EMPTY_ROW);

    let run_suspension = checked
        .facts
        .suspensions
        .for_machine(run)
        .expect("caller suspension plan");
    let run_blocking = checked
        .facts
        .blocking
        .for_machine(run)
        .expect("caller blocking plan");
    assert_eq!(
        run_suspension.interface,
        SuspensionInterface::InternalInferred
    );
    assert_eq!(run_blocking.interface, BlockingInterface::InternalInferred);
    // Local calls to checked bodies consume the honest checked summary, not
    // the callee's authored ceiling. `wait` is quiet, so the private caller
    // remains quiet even though `wait` publishes room to suspend and block.
    assert!(!run_suspension.checked_may_suspend);
    assert!(!run_blocking.checked_may_block);

    let run_reach = checked
        .facts
        .service_reaches
        .for_machine(run)
        .expect("caller service row");
    assert_eq!(
        checked
            .facts
            .service_reaches
            .rows
            .services(run_reach.inferred_transitive),
        &[clock],
        "the authored service ceiling remains the modular caller contract",
    );
}

#[test]
fn checked_machine_operational_facts_keep_suspension_and_blocking_independent() {
    use psi_language_semantics::{BlockingInterface, SuspensionInterface};

    let source = r#"
    boundary trait Sleeper {
        machine sleep() suspends;
    }

    boundary trait Waiter {
        machine wait() blocks;
    }

    data Harness { sleeper: Sleeper; waiter: Waiter; }

    machine Harness::suspend_only(&mut self) suspends; {
        suspend self.sleeper.sleep();
    }

    machine Harness::block_only(&mut self) blocks; {
        block self.waiter.wait();
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let suspend_only = symbol_of("Harness::suspend_only");
    let block_only = symbol_of("Harness::block_only");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let suspension = |machine| {
        checked
            .facts
            .suspensions
            .for_machine(machine)
            .expect("machine suspension fact")
    };
    let blocking = |machine| {
        checked
            .facts
            .blocking
            .for_machine(machine)
            .expect("machine blocking fact")
    };

    assert_eq!(
        suspension(suspend_only).interface,
        SuspensionInterface::PublishedMaySuspend(true)
    );
    assert_eq!(
        blocking(suspend_only).interface,
        BlockingInterface::InternalInferred
    );
    assert!(suspension(suspend_only).checked_may_suspend);
    assert!(!blocking(suspend_only).checked_may_block);

    assert_eq!(
        suspension(block_only).interface,
        SuspensionInterface::InternalInferred
    );
    assert_eq!(
        blocking(block_only).interface,
        BlockingInterface::PublishedMayBlock(true)
    );
    assert!(!suspension(block_only).checked_may_suspend);
    assert!(blocking(block_only).checked_may_block);
}

#[test]
fn qualification_facts_record_policy_commitments() {
    // STR4 checked plans, slice 2: a machine whose body casts under an
    // arithmetic policy COMMITS to that policy's fixed semantic identity;
    // a cast-free machine carries no entry.
    use psi_language_semantics::SemanticDomainTable;

    let source = r#"
    data Main {}

    domain i64::Km
    requires
        self >= 0;

    machine Main::clamped(&mut self, value: u64) -> u8 {
        let squeezed: u8 in Saturating = value as u8 in Saturating;
        squeezed as u8
    }

    machine Main::minted(&mut self) -> i64 {
        let distance: i64 in Km = 5 as i64 in Km;
        distance as i64
    }

    machine Main::main(&mut self) -> u64 {
        7
    }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let clamped_symbol = symbol_of("Main::clamped");
    let minted_symbol = symbol_of("Main::minted");
    let main_symbol = symbol_of("Main::main");
    let km_id = typed
        .semantic_domains
        .lookup("i64::Km")
        .expect("Km interned");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let clamped = checked
        .facts
        .qualifications
        .for_machine(clamped_symbol)
        .expect("clamped's qualification fact");
    assert_eq!(
        clamped.body_committed,
        vec![SemanticDomainTable::SATURATING],
        "the saturating cast commits to the fixed Saturating identity"
    );
    // The MINT commits to the DECLARED domain's interned identity.
    let minted = checked
        .facts
        .qualifications
        .for_machine(minted_symbol)
        .expect("minted's qualification fact");
    assert_eq!(minted.body_committed, vec![km_id]);
    assert!(
        checked
            .facts
            .qualifications
            .for_machine(main_symbol)
            .is_none(),
        "a cast-free machine carries no qualification entry"
    );
}

#[test]
fn contract_plans_fingerprint_published_halves() {
    // STR4 checked plans (machine_taxonomy.md): the contract fingerprint
    // covers ONLY the published halves -- two machines with the same
    // declared surface share it; a different `reaches` clause changes it;
    // inferred rows never enter (prover-independence by construction).
    let source = r#"
    boundary trait Filesystem {}
    boundary trait Network {}

    data Main {
        left: u64;
        right: u64;
    }
    data FrameCell { value: u64; }
    data FrameOwner {
        first: FrameCell;
        second: FrameCell;
    }

    machine Main::quiet_a(&mut self) -> u64 reaches Filesystem {
        self.left = 1;
        1
    }
    machine Main::quiet_b(&mut self) -> u64 reaches Filesystem {
        self.right = 2;
        2
    }
    machine Main::loud(&mut self) -> u64 reaches Network { 3 }
    machine bounded_ab(x: u64, y: u64) -> u64
    requires
        x >= 1;
        y >= 2
    { x }
    machine bounded_ba(x: u64, y: u64) -> u64
    requires
        y >= 2;
        x >= 1
    { x }
    machine bounded_wider(x: u64, y: u64) -> u64
    requires
        x >= 1;
        y >= 3
    { x }
    machine bounded_renamed(alpha: u64, beta: u64) -> u64
    requires
        alpha >= 1;
        beta >= 2
    { alpha }
    machine write_alpha(alpha: &mut u64) {
        alpha = 1;
    }
    machine write_beta(beta: &mut u64) {
        beta = 2;
    }
    machine Main::transitioning(&mut self) {
        transition { _ -> write_left() }
        state write_left(&mut self) {
            self.left = 3;
            transition { _ -> finished() }
        }
        state finished(&mut self) { }
    }
    machine write_through_transition(value: &mut u64) {
        transition { _ -> write(value) }
        state write(slot: &mut u64) { slot = 4; }
    }
    machine Main::cyclic(&mut self) {
        transition { _ -> cycle() }
        state cycle(&mut self) { transition { _ -> cycle() } }
    }
    machine reordered_cycle(first: u64, second: u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: u64, right: u64) {
            transition { _ -> cycle(right, left) }
        }
    }
    machine reordered_shared_cycle(first: &u64, second: &u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &u64, right: &u64) {
            transition { _ -> cycle(right, left) }
        }
    }
    machine reordered_mut_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            transition { _ -> cycle(right, left) }
        }
    }
    machine rotating_mut_cycle(first: &mut u64, second: &mut u64) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut u64, right: &mut u64) {
            left = 11;
            transition { _ -> cycle(right, left) }
        }
    }
    machine rotating_mut_scc(first: &mut u64, second: &mut u64) {
        transition { _ -> write(first, second) }
        state write(left: &mut u64, right: &mut u64) {
            left = 12;
            transition { _ -> forward(right, left) }
        }
        state forward(left: &mut u64, right: &mut u64) {
            transition { _ -> write(right, left) }
        }
    }
    machine call_rotating_mut_cycle(first: &mut u64, second: &mut u64) {
        rotating_mut_cycle(first, second);
    }
    machine call_rotating_mut_scc(first: &mut u64, second: &mut u64) {
        rotating_mut_scc(first, second);
    }
    machine rotating_mut_fields(first: &mut FrameCell, second: &mut FrameCell) {
        transition { _ -> cycle(first, second) }
        state cycle(left: &mut FrameCell, right: &mut FrameCell) {
            left.value = 13;
            transition { _ -> cycle(right, left) }
        }
    }
    machine FrameOwner::call_rotating_mut_fields(&mut self) {
        rotating_mut_fields(&mut self.first, &mut self.second);
    }
    boundary trait Device {
        machine overwrite(value: &mut u64);
    }
    data Wrapper { device: Device; value: u64; }
    machine Wrapper::boundary_call(&mut self) {
        self.device.overwrite(&mut self.value);
    }
    machine Main::direct_self_loop(&mut self) {
        self.left = 9;
        transition { _ -> self }
    }
    machine Main::branching(&mut self) {
        transition self.left == 0 {
            true -> write_left_branch()
            false -> write_right_branch()
        }
        state write_left_branch(&mut self) {
            self.left = 5;
            transition { _ -> branch_done() }
        }
        state write_right_branch(&mut self) {
            self.right = 6;
            transition { _ -> branch_done() }
        }
        state branch_done(&mut self) { }
    }
    machine Main::touch_right_value(&mut self) -> bool {
        self.right = 8;
        true
    }
    machine Main::call_bearing(&mut self) -> bool {
        let seed: bool = self.touch_right_value();
        transition self.touch_right_value() == seed {
            true -> call_bearing_done(self.touch_right_value())
            false -> call_bearing_done(seed)
        }
        state call_bearing_done(&mut self, value: bool) -> bool {
            let answer: bool = self.touch_right_value();
            answer
        }
    }
    machine Main::main(&mut self) -> u64 { 7 }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let symbol_of = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
            .symbol
    };
    let quiet_a = symbol_of("Main::quiet_a");
    let quiet_b = symbol_of("Main::quiet_b");
    let loud = symbol_of("Main::loud");
    let write_alpha = symbol_of("write_alpha");
    let write_beta = symbol_of("write_beta");
    let transitioning = symbol_of("Main::transitioning");
    let write_through_transition = symbol_of("write_through_transition");
    let cyclic = symbol_of("Main::cyclic");
    let reordered_cycle = symbol_of("reordered_cycle");
    let reordered_shared_cycle = symbol_of("reordered_shared_cycle");
    let reordered_mut_cycle = symbol_of("reordered_mut_cycle");
    let rotating_mut_cycle = symbol_of("rotating_mut_cycle");
    let rotating_mut_scc = symbol_of("rotating_mut_scc");
    let call_rotating_mut_cycle = symbol_of("call_rotating_mut_cycle");
    let call_rotating_mut_scc = symbol_of("call_rotating_mut_scc");
    let rotating_mut_fields = symbol_of("rotating_mut_fields");
    let call_rotating_mut_fields = symbol_of("FrameOwner::call_rotating_mut_fields");
    let boundary_call = symbol_of("Wrapper::boundary_call");
    let direct_self_loop = symbol_of("Main::direct_self_loop");
    let branching = symbol_of("Main::branching");
    let call_bearing = symbol_of("Main::call_bearing");
    let checked = lower_typed_trees(typed).expect("checked lowering should succeed");

    let plan = |symbol| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol)
            .expect("contract plan")
    };
    // Same declared surface (different BODIES) -> same fingerprint.
    assert_eq!(plan(quiet_a).fingerprint, plan(quiet_b).fingerprint);
    let frame = |symbol| {
        &checked
            .facts
            .mutation
            .for_machine(symbol)
            .expect("mutation facts")
            .state_write_frames
            .first()
            .expect("entry-state frame")
            .frame
    };
    assert_eq!(frame(quiet_a).paths(), &["self.left".to_owned()]);
    assert_eq!(frame(quiet_b).paths(), &["self.right".to_owned()]);
    assert_ne!(
        frame(quiet_a).compatibility_report_fingerprint(),
        frame(quiet_b).compatibility_report_fingerprint()
    );
    assert_eq!(frame(write_alpha).paths(), &["$P0".to_owned()]);
    assert_eq!(frame(write_alpha), frame(write_beta));
    assert_eq!(frame(transitioning).paths(), &["self.left".to_owned()]);
    assert_eq!(frame(write_through_transition).paths(), &["$P0".to_owned()]);
    assert_eq!(
        frame(cyclic).complete_paths(),
        Some([].as_slice()),
        "an argument-free named state cycle preserves its complete empty namespace"
    );
    assert_eq!(
        frame(reordered_cycle).complete_paths(),
        Some([].as_slice()),
        "reordering read-only scalar parameters cannot redirect a caller-visible write"
    );
    assert_eq!(
        frame(reordered_shared_cycle).complete_paths(),
        Some([].as_slice()),
        "reordering shared-reference parameters cannot redirect a caller-visible write"
    );
    assert_eq!(
        frame(reordered_mut_cycle).complete_paths(),
        Some([].as_slice()),
        "an exact exclusive-parameter permutation with no writes has a complete empty frame"
    );
    assert_eq!(
        frame(rotating_mut_cycle).paths(),
        &["$P0".to_owned(), "$P1".to_owned()],
        "a write rotating through an exclusive-parameter permutation reaches the complete finite orbit"
    );
    assert_eq!(
        frame(rotating_mut_scc).paths(),
        &["$P0".to_owned()],
        "a multi-state SCC composes its exact permutations before publishing the entry frame"
    );
    assert_eq!(
        frame(call_rotating_mut_cycle).paths(),
        &["$P0".to_owned(), "$P1".to_owned()],
        "a resolved caller instantiates the complete permutation-orbit frame"
    );
    assert_eq!(
        frame(call_rotating_mut_scc).paths(),
        &["$P0".to_owned()],
        "a resolved caller preserves the multi-state SCC's exact positional frame"
    );
    assert_eq!(
        frame(rotating_mut_fields).paths(),
        &["$P0.value".to_owned(), "$P1.value".to_owned()],
        "permutation closure preserves written member suffixes"
    );
    assert_eq!(
        frame(call_rotating_mut_fields).paths(),
        &[
            "self.first.value".to_owned(),
            "self.second.value".to_owned(),
        ],
        "caller instantiation preserves member arguments and written suffixes"
    );
    assert_eq!(
        frame(boundary_call).paths(),
        &["self.device".to_owned(), "self.value".to_owned()],
        "checked frames retain exact nested boundary receiver and out-argument writes"
    );
    assert_eq!(
        frame(direct_self_loop).paths(),
        &["self.left".to_owned()],
        "a direct self target repeats the same finite may-write frame"
    );
    assert_eq!(
        frame(branching).paths(),
        &["self.left".to_owned(), "self.right".to_owned()],
        "both conditional arms compose and may share one memoized tail state"
    );
    assert_eq!(
        frame(call_bearing).paths(),
        &["self.right".to_owned()],
        "value calls in locals, guards, jump arguments, and terminal results compose"
    );
    // A different `reaches` clause -> a different fingerprint.
    assert_ne!(plan(quiet_a).fingerprint, plan(loud).fingerprint);
    // Slice 2: REQUIRES clause ORDER never enters the identity...
    let ab = symbol_of_checked(&checked, "bounded_ab");
    let ba = symbol_of_checked(&checked, "bounded_ba");
    let wider = symbol_of_checked(&checked, "bounded_wider");
    assert_eq!(plan(ab).fingerprint, plan(ba).fingerprint);
    // ...but a changed BOUND does.
    assert_ne!(plan(ab).fingerprint, plan(wider).fingerprint);
    // Parameter RENAMES normalize positionally -- identical contracts.
    let renamed = symbol_of_checked(&checked, "bounded_renamed");
    assert_eq!(plan(ab).fingerprint, plan(renamed).fingerprint);
}
