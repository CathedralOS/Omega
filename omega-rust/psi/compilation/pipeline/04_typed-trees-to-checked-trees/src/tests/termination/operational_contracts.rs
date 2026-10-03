use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use crate::tests::termination::symbol_of_checked;

#[test]
fn symbol_resolved_service_reach_propagates_boundary_identity_and_parent_closure() {
    let source = r#"
    boundary trait Readable {
        machine read() -> u64;
    }

    boundary trait Filesystem: Readable {
    }

    data Worker<'s> { reader: &'s mut Readable; }
    machine Worker::run(&mut self) -> u64 reaches Filesystem {
        self.reader.read()
    }

    data Main { worker: Worker; }
    machine Main::main(&mut self) -> u64 {
        self.worker.run()
    }
    "#;

    let typed = typed_program(source);
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("service ceiling should admit the body");

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
fn qualification_facts_record_policy_commitments() {
    // STR4 checked plans, slice 2: a machine whose body casts under an
    // arithmetic policy COMMITS to that policy's fixed semantic identity;
    // a cast-free machine carries no entry.
    use language_semantics::SemanticDomainTable;

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

    let typed = typed_program(source);
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("checked lowering should succeed");

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
    // STR4 checked plans (wiki/spec/language/machines.md): the contract fingerprint
    // covers ONLY the published halves -- two machines with the same
    // published surface share it; a different derived service row changes it.
    // Service propagation is independent of prover heuristics.
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
    data Wrapper<'s> { device: &'s mut Device; value: u64; }
    machine Wrapper::boundary_call(&mut self) reaches Device {
        self.device.overwrite(&mut self.value);
    }
    machine Main::direct_named_loop(context: &mut Main) {
        context.left = 9;
        transition { _ -> Main::direct_named_loop(context) }
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

    let typed = typed_program(source);
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
    let direct_self_loop = symbol_of("Main::direct_named_loop");
    let branching = symbol_of("Main::branching");
    let call_bearing = symbol_of("Main::call_bearing");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("checked lowering should succeed");

    let plan = |symbol| {
        checked
            .facts
            .contract_plans
            .for_machine(symbol)
            .expect("contract plan")
    };
    // Same declared surface (different BODIES) -> same fingerprint.
    assert_eq!(
        plan(quiet_a).report_fingerprint,
        plan(quiet_b).report_fingerprint
    );
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
    assert_ne!(
        plan(quiet_a).report_fingerprint,
        plan(loud).report_fingerprint
    );
    // Slice 2: REQUIRES clause ORDER never enters the identity...
    let ab = symbol_of_checked(&checked, "bounded_ab");
    let ba = symbol_of_checked(&checked, "bounded_ba");
    let wider = symbol_of_checked(&checked, "bounded_wider");
    assert_eq!(plan(ab).report_fingerprint, plan(ba).report_fingerprint);
    // ...but a changed BOUND does.
    assert_ne!(plan(ab).report_fingerprint, plan(wider).report_fingerprint);
    // Parameter RENAMES normalize positionally -- identical contracts.
    let renamed = symbol_of_checked(&checked, "bounded_renamed");
    assert_eq!(
        plan(ab).report_fingerprint,
        plan(renamed).report_fingerprint
    );
}
