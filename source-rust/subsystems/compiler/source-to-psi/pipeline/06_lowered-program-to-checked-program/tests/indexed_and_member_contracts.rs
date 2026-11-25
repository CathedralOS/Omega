//! The checker must judge a clause that reads an array element, a record
//! field or a case payload at the exact place the clause names, and a call
//! through a trait member or a boundary by the contract of what was selected.
//! Each test pairs a program the checker accepts with a twin that changes
//! one input, so a checker that read a sibling element, field or case, or
//! another member's contract, would give the twin the wrong verdict.
//!
//! The spec rules: a clause reads the place it names
//! (`wiki/spec/language/counts_and_addresses.md`, "Bounds", for indexes;
//! `wiki/spec/language/data_and_literals.md`, "Case constraints", for
//! payloads); a call owes the requirement of the declaration it selects
//! (`wiki/spec/language/machines.md`), and a machine's reach covers every
//! service its calls reach (`wiki/spec/language/effects.md`).

#[path = "support/source_handoff.rs"]
mod program;

use program::Program;

fn verdict_of(source: &str) -> Result<(), String> {
    match Program::try_of(&[("product/main.omg", source)]) {
        Err(refusal) => Err(format!(
            "{}: {} at `{}`",
            refusal.stage, refusal.message, refusal.at
        )),
        Ok(program) => match program.judged(|_| {}) {
            program::Verdict::Accepted => Ok(()),
            other => Err(format!("{other:?}")),
        },
    }
}

/// The checker rejected the program for an obligation it could not prove.
fn unproved(reason: &str) -> bool {
    reason.starts_with("Rejected(") && reason.contains("unproved: [\"")
}

#[track_caller]
fn accepted(source: &str) {
    if let Err(reason) = verdict_of(source) {
        panic!("{source}\n{reason}")
    }
}

#[track_caller]
fn rejected(source: &str) -> String {
    match verdict_of(source) {
        Ok(()) => panic!("the program must be refused: {source}"),
        Err(reason) => reason,
    }
}

/// `requires a[0] < a[3]` compares the first and last elements; a caller
/// whose array breaks only that pair is rejected, and one that breaks a
/// neighbouring pair is not. The same holds for `m[1][2] == m[0][2]` in a
/// matrix, where only that column of the two rows is compared.
#[test]
fn a_fixed_index_clause_reads_the_elements_it_names() {
    let first_last = "machine f(a: [u8; 4], b: u8) -> u8 requires a[0] < a[3] { b }";
    accepted(&format!(
        "{first_last} machine call() -> u8 {{ f([1u8, 9u8, 0u8, 2u8], 0u8) }}"
    ));
    let twin = rejected(&format!(
        "{first_last} machine call() -> u8 {{ f([2u8, 0u8, 9u8, 1u8], 0u8) }}"
    ));
    assert!(twin.contains("CallRequirement"), "{twin}");

    let column = "machine g(m: [[u8; 3]; 2], b: u8) -> u8 requires m[1][2] == m[0][2] { b }";
    accepted(&format!(
        "{column} machine call() -> u8 {{ g([[1u8, 2u8, 7u8], [4u8, 5u8, 7u8]], 0u8) }}"
    ));
    let twin = rejected(&format!(
        "{column} machine call() -> u8 {{ g([[1u8, 7u8, 7u8], [4u8, 7u8, 6u8]], 0u8) }}"
    ));
    assert!(twin.contains("CallRequirement"), "{twin}");
}

/// `requires a[i] < 5` reads the element the caller's `i` selects: index 0
/// of `[1, 9, 1, 1]` meets it and index 1 does not. An index past a fixed
/// extent is refused at the index.
#[test]
fn a_runtime_index_clause_reads_the_element_its_selector_chooses() {
    let selector =
        "machine f(a: [u8; 4], i: u64, b: u8) -> u8 requires i < 4 requires a[i] < 5 { b }";
    accepted(&format!(
        "{selector} machine call() -> u8 {{ f([1u8, 9u8, 1u8, 1u8], 0u64, 0u8) }}"
    ));
    let twin = rejected(&format!(
        "{selector} machine call() -> u8 {{ f([1u8, 9u8, 1u8, 1u8], 1u64, 0u8) }}"
    ));
    assert!(twin.contains("CallRequirement"), "{twin}");

    let outside = rejected("machine f(a: [u8; 4], b: u8) -> u8 requires a[4] < 5 { b }");
    assert_eq!(
        outside,
        "lowering: Violation: contract array index 4 is outside its fixed extent 4 while \
         reading the signature of `f` at `4`"
    );
}

/// A data clause over a field array, `self.cells[0] <= self.cells[2]`, is
/// judged on the elements written for those cells.
#[test]
fn a_data_clause_reads_the_field_element_it_names() {
    let window = "data Window requires self.cells[0] <= self.cells[2] { cells: [u8; 3]; }";
    accepted(&format!(
        "{window} machine make() -> Window {{ Window {{ cells: [1u8, 0u8, 2u8] }} }}"
    ));
    let twin = rejected(&format!(
        "{window} machine make() -> Window {{ Window {{ cells: [3u8, 9u8, 2u8] }} }}"
    ));
    assert!(unproved(&twin), "{twin}");
}

/// `Beta::Same` declares `v` before `w`, while `Alpha::Same` declares them
/// the other way round. `Beta`'s clause `self.v <= self.w` reads `Beta`'s
/// own fields by name, so `{ v: 1, w: 2 }` is accepted and `{ v: 3, w: 2 }`
/// rejected; `Pair::Both` must hold equal fields, and a case that is not
/// `Both` breaks the clause.
#[test]
fn a_case_clause_reads_the_payload_of_its_own_sum_and_case() {
    let sums = "data Alpha { case Same(w: u8, v: u8); case Other; } \
        data Beta requires (self in Beta::Same) && self.v <= self.w { case Empty; case Same(v: u8, w: u8); }";
    accepted(&format!(
        "{sums} machine make() -> Beta {{ Beta::Same {{ v: 1u8, w: 2u8 }} }}"
    ));
    let twin = rejected(&format!(
        "{sums} machine make() -> Beta {{ Beta::Same {{ v: 3u8, w: 2u8 }} }}"
    ));
    assert!(unproved(&twin), "{twin}");

    let pair = "data Pair requires (self in Pair::Both) && self.left == self.right \
        { case Neither; case Both(left: u8, right: u8); }";
    accepted(&format!(
        "{pair} machine make() -> Pair {{ Pair::Both {{ left: 3u8, right: 3u8 }} }}"
    ));
    for twin in [
        rejected(&format!(
            "{pair} machine make() -> Pair {{ Pair::Both {{ left: 3u8, right: 4u8 }} }}"
        )),
        rejected(&format!(
            "{pair} machine make() -> Pair {{ Pair::Neither }}"
        )),
    ] {
        assert!(unproved(&twin), "{twin}");
    }
}

/// `use_clock` calls `E::tick(n)`, and `Clock::tick` requires `v != 0`. The
/// requirement's own clause is owed at the call even though the selected
/// implementation `loose` asks for nothing, so `use_clock` without its own
/// `requires n != 0` is rejected, and with it accepted.
#[test]
fn a_member_call_owes_the_requirement_of_the_trait_member() {
    let clock = |requires: &str| {
        format!(
            "trait Clock {{ machine tick(v: u8) -> bool requires v != 0; }} \
             machine loose(v: u8) -> bool {{ true }} \
             Local satisfies Clock {{ Clock::tick = loose; }} \
             machine use_clock<const E: Clock>(n: u8) -> bool {requires} {{ E::tick(n) }} \
             machine call(n: u8) -> bool {requires} {{ use_clock::<Local>(n) }}"
        )
    };
    let unproved = rejected(&clock(""));
    assert!(unproved.contains("CallRequirement"), "{unproved}");
    accepted(&clock("requires n != 0"));
}

/// A machine's reach must cover every service its calls reach. `run` calls
/// `B::ping`, which reaches `B`: `reaches A + B` is accepted and `reaches
/// A` alone is not. A generic boundary member called at two applications,
/// `Codec::pass::<u8>` and `Codec::pass::<bool>`, and a const-indexed one,
/// `Ring::slot::<4>` and `::<9>`, are each judged at their own call.
#[test]
fn a_boundary_call_is_judged_by_the_selected_members_reach() {
    let services =
        "boundary trait A {} boundary trait B { machine ping() reaches B crashes none; }";
    accepted(&format!(
        "{services} machine run() reaches A + B {{ B::ping(); }}"
    ));
    let outside = rejected(&format!(
        "{services} machine run() reaches A {{ B::ping(); }}"
    ));
    assert!(
        outside.contains("exceeded: [\"machine `run` reaches `")
            && outside.contains("contributes `trait-application(declaration(nominal")
            && outside.contains("main.omg::B::ping")
            && outside.contains("main.omg::A"),
        "{outside}"
    );

    accepted(
        "boundary trait Codec { machine pass<T>(x: T) -> T reaches Codec crashes none; } \
         machine small(x: u8) -> u8 reaches Codec { Codec::pass::<u8>(x) } \
         machine flag(x: bool) -> bool reaches Codec { Codec::pass::<bool>(x) }",
    );
    accepted(
        "boundary trait Ring { machine slot<const N: u64>(x: u8) -> u8 reaches Ring crashes none; } \
         machine low(x: u8) -> u8 reaches Ring { Ring::slot::<4>(x) } \
         machine high(x: u8) -> u8 reaches Ring { Ring::slot::<9>(x) }",
    );
    accepted(
        "boundary trait Device<T> {} machine pass<T>(x: T) -> T reaches Device<T> { x } \
         machine small(x: u8) -> u8 { pass::<u8>(x) } machine flag(x: bool) -> bool { pass::<bool>(x) }",
    );
}

/// The callee `maybe_zero` crashes exactly when the element its caller
/// selects is zero (`crashes Trap items[index] == 0`). A public caller that
/// publishes no crash route must show the call never takes it: its own
/// `requires items[index] == 7` says so, and without that requirement the
/// call is refused. The guard reads the caller's array at the call, as a
/// `requires` does (calls_and_outcomes.md, "Call contracts": "Declared
/// requirements and published crash routes describe invocation entry").
#[test]
fn a_crash_guard_reads_the_element_its_caller_selects() {
    let callee = "machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
        requires index < 2 crashes Trap items[index] == 0 \
        { transition items[index] == 0 { true -> bad(), false -> (items[index]) } \
          machine bad() -> u64 { crash Trap; } }";
    let caller = |requires: &str| {
        format!(
            "{callee} public machine read_seven(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 {requires} {{ maybe_zero(items, index) }}"
        )
    };
    accepted(&caller("requires items[index] == 7"));
    let twin = rejected(&caller(""));
    assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");

    // A guard describes invocation entry, so it still cannot name `result`.
    let result =
        rejected("machine f(items: @[u64; 2]) -> u64 crashes Trap result == 0 { items[0] }");
    assert!(
        result.contains("contract result in an entry clause"),
        "{result}"
    );
}

/// A crash guard over a field, `p.a == 0`, reads that field of the lent
/// record as it is when the call is made. Writing the sibling field `b`
/// leaves `a` at 7 and the call cannot crash; writing `a` itself makes the
/// route reachable, and the public caller is refused (AGENTS.md rule 3:
/// facts follow each field path).
#[test]
fn a_crash_guard_reads_the_field_version_at_the_call() {
    let program = |write: &str| {
        format!(
            "public data Pair {{ a: u64; b: u64; }} \
             machine maybe_zero(p: @Pair) -> u64 crashes Trap p.a == 0 \
             {{ transition p.a == 0 {{ true -> bad(), false -> (p.a) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             public machine run() -> u64 {{ \
                let mutable q: Pair = Pair {{ a: 7, b: 11 }}; {write} = 0; maybe_zero(@q) }}"
        )
    };
    accepted(&program("q.b"));
    let twin = rejected(&program("q.a"));
    assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");
}

/// `forward` publishes the same element guard as `maybe_zero` and never
/// reads the element itself. Its call is covered by its own route, stated
/// over the array it received at entry; a forwarder that publishes a guard
/// over another value (`items[index] == 1`) does not cover the call.
#[test]
fn a_forwarded_crash_guard_covers_the_call_it_forwards() {
    let program = |guard: &str| {
        format!(
            "machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 crashes Trap items[index] == 0 \
             {{ transition items[index] == 0 {{ true -> bad(), false -> (items[index]) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             machine forward(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 crashes Trap {guard} {{ maybe_zero(items, index) }} \
             public machine read_seven(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 requires items[index] == 7 {{ forward(items, index) }}"
        )
    };
    accepted(&program("items[index] == 0"));
    let twin = rejected(&program("items[index] == 1"));
    assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");
}

/// A crash guard over a borrowed scalar (`$value == 0` over `@u64`) and one
/// over a slice's extent (`items.length == 0`) read the caller's storage at
/// the call, as the element guard does: each call is accepted when the
/// caller's `requires` rules the route out and refused without it.
#[test]
fn borrowed_scalar_and_extent_crash_guards_read_the_callers_storage() {
    let scalar = |requires: &str| {
        format!(
            "machine maybe_zero(value: @u64) -> u64 crashes Trap $value == 0 \
             {{ transition $value == 0 {{ true -> bad(), false -> ($value) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             public machine read_seven(value: @u64) -> u64 {requires} {{ maybe_zero(value) }}"
        )
    };
    accepted(&scalar("requires $value == 7"));
    let twin = rejected(&scalar(""));
    assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");

    let extent = |requires: &str| {
        format!(
            "machine maybe_empty(items: @[u8]) -> u64 crashes Trap items.length == 0 \
             {{ transition items.length == 0 {{ true -> bad(), false -> (items.length) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             public machine count(items: @[u8]) -> u64 {requires} {{ maybe_empty(items) }}"
        )
    };
    accepted(&extent("requires items.length > 0"));
    let twin = rejected(&extent(""));
    assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");
}

/// A call that lends a record's field (`@pair.items`) restates the callee's
/// crash guard `items[index] == 0` over that field path, `pair.items[0]`, and
/// the checker reads the element there at the call. The pair differs only in
/// the element the guard reads: `[7, 7]` is accepted and `[0, 7]` is refused
/// for its uncovered Trap route.
///
/// The restated route names only that field's storage (AGENTS.md rule 3):
/// writing the sibling `pair.tag` before the call leaves it accepted, while
/// writing `pair.items[0] = 0` is refused.
#[test]
fn a_crash_guard_over_a_projected_actual_reads_that_field_path() {
    let program = |setup: &str| {
        format!(
            "public data Pair {{ items: [u64; 2]; tag: u64; }} \
             machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 crashes Trap items[index] == 0 \
             {{ transition items[index] == 0 {{ true -> bad(), false -> (items[index]) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             public machine run() -> u64 {{ {setup} maybe_zero(@pair.items, 0) }}"
        )
    };
    let uncovered = |source: &str| {
        let twin = rejected(source);
        assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");
    };
    accepted(&program("let pair: Pair = Pair { items: [7, 7], tag: 0 };"));
    uncovered(&program("let pair: Pair = Pair { items: [0, 7], tag: 0 };"));
    accepted(&program(
        "let mutable pair: Pair = Pair { items: [7, 7], tag: 3 }; pair.tag = 0;",
    ));
    uncovered(&program(
        "let mutable pair: Pair = Pair { items: [7, 7], tag: 3 }; pair.items[0] = 0;",
    ));
}

/// A forwarder that lends its own formal's field covers the call's route
/// with a guard over that same field path. `pair.items[0] == 0` covers
/// `maybe_zero(@pair.items, 0)`; the twin's guard reads the sibling
/// `pair.tag` and leaves the call's route uncovered.
#[test]
fn a_forwarder_covers_a_projected_route_with_a_guard_on_that_field() {
    let program = |guard: &str| {
        format!(
            "public data Pair {{ items: [u64; 2]; tag: u64; }} \
             machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 crashes Trap items[index] == 0 \
             {{ transition items[index] == 0 {{ true -> bad(), false -> (items[index]) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             machine forward(pair: @Pair) -> u64 crashes Trap {guard} \
             {{ maybe_zero(@pair.items, 0) }} \
             public machine run() -> u64 \
             {{ let pair: Pair = Pair {{ items: [7, 7], tag: 3 }}; forward(@pair) }}"
        )
    };
    accepted(&program("pair.items[0] == 0"));
    let twin = rejected(&program("pair.tag == 0"));
    assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");
}

/// A call may select the guarded cell with a value equal to the caller's
/// selector but not the same value: a copied record field
/// (`selection.index`), a callee result whose guarantee is `result == index`,
/// or a `match` that yields `index` on both arms. The caller's
/// `requires items[index] == 7` still covers the route `items[selected] ==
/// 0`, because both read the shared input's cell at entry and the selectors
/// are equal. Each twin drops only that requirement and is refused; passing
/// an unrelated selector is refused too, so the cover comes from the
/// equality and not from the shape of the call.
#[test]
fn a_crash_guard_reads_the_cell_an_equal_selector_chooses() {
    let callee = "machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
        requires index < 2 crashes Trap items[index] == 0 \
        { transition items[index] == 0 { true -> bad(), false -> (items[index]) } \
          machine bad() -> u64 { crash Trap; } }";
    let uncovered = |source: &str| {
        let twin = rejected(source);
        assert!(unproved(&twin) && twin.contains("CrashCoverage"), "{twin}");
    };
    let field = |requires: &str| {
        format!(
            "{callee} public data Selection [copy] {{ index: u64; }} \
             public machine read_seven(items: @[u64; 2], selection: Selection) -> u64 \
             requires selection.index < 2 {requires} {{ maybe_zero(items, selection.index) }}"
        )
    };
    accepted(&field("requires items[selection.index] == 7"));
    uncovered(&field(""));

    let call_result = |requires: &str| {
        format!(
            "{callee} machine preserve(index: u64) -> u64 ensures result == index {{ index }} \
             public machine read_seven(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 {requires} \
             {{ let selected: u64 = preserve(index); maybe_zero(items, selected) }}"
        )
    };
    accepted(&call_result("requires items[index] == 7"));
    uncovered(&call_result(""));

    let join = |requires: &str| {
        format!(
            "{callee} public machine read_seven(items: @[u64; 2], index: u64, flag: bool) -> u64 \
             requires index < 2 {requires} \
             {{ let selected: u64 = match flag {{ true -> index, false -> index }}; \
                maybe_zero(items, selected) }}"
        )
    };
    accepted(&join("requires items[index] == 7"));
    uncovered(&join(""));

    uncovered(&format!(
        "{callee} public machine read_seven(items: @[u64; 2], index: u64, other: u64) -> u64 \
         requires index < 2 requires other < 2 requires items[index] == 7 \
         {{ maybe_zero(items, other) }}"
    ));
}

/// The same holds for a `requires` over the cell: `nonzero(items,
/// selection.index)` owes `items[index] != 0`, which the caller's
/// `requires items[selection.index] == 7` establishes, and nothing else does.
#[test]
fn a_requirement_reads_the_cell_an_equal_selector_chooses() {
    let program = |requires: &str| {
        format!(
            "machine nonzero(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 requires items[index] != 0 {{ items[index] }} \
             public data Selection [copy] {{ index: u64; }} \
             public machine read_seven(items: @[u64; 2], selection: Selection) -> u64 \
             requires selection.index < 2 {requires} {{ nonzero(items, selection.index) }}"
        )
    };
    accepted(&program("requires items[selection.index] == 7"));
    let twin = rejected(&program(""));
    assert!(
        unproved(&twin) && twin.contains("CallRequirement"),
        "{twin}"
    );
}

/// An actual that lends an element chosen at run time (`@rows[row]`) keeps
/// its selector: the callee's guard `items[index] == 0` names `rows[row][0]`
/// at the call. `requires rows[row][0] == 7` covers it; the twins drop that
/// requirement or state it of the sibling cell `rows[row][1]`, and are
/// refused for the uncovered route (AGENTS.md rule 3).
#[test]
fn a_crash_guard_over_a_runtime_selected_actual_reads_that_element() {
    let program = |requires: &str| {
        format!(
            "machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
             requires index < 2 crashes Trap items[index] == 0 \
             {{ transition items[index] == 0 {{ true -> bad(), false -> (items[index]) }} \
                machine bad() -> u64 {{ crash Trap; }} }} \
             public machine pick(rows: @[[u64; 2]; 2], row: u64) -> u64 requires row < 2 \
             {requires} {{ maybe_zero(@rows[row], 0) }}"
        )
    };
    accepted(&program("requires rows[row][0] == 7"));
    for twin in ["", "requires rows[row][1] == 7"] {
        let refused = rejected(&program(twin));
        assert!(
            unproved(&refused) && refused.contains("CrashCoverage"),
            "{twin}: {refused}"
        );
    }
}

/// A forwarder over `@mutable` storage that writes the element before the
/// call cannot rely on its entry guard, which described the element as it
/// arrived. The call is refused. The same forwarder without the write is
/// refused too today, because the checker cannot yet read the element's
/// value at the call through a `@mutable` formal (`TASKS.md`,
/// PROOF-KERNEL-CORE); so this test pins only the case that must stay
/// refused.
#[test]
fn a_mutable_forwarder_that_writes_the_guarded_element_is_refused() {
    let program = "machine maybe_zero(items: @[u64; 2], index: u64) -> u64 \
         requires index < 2 crashes Trap items[index] == 0 \
         { transition items[index] == 0 { true -> bad(), false -> (items[index]) } \
           machine bad() -> u64 { crash Trap; } } \
         machine forward(items: @mutable [u64; 2], index: u64) -> u64 \
         requires index < 2 crashes Trap items[index] == 0 \
         { items[index] = 0; maybe_zero(items, index) }";
    let refused = rejected(program);
    assert!(
        unproved(&refused) && refused.contains("CrashCoverage"),
        "{refused}"
    );
}

/// A suspending call's result may be bound first or tested directly by a
/// `transition`; both forms are accepted with the same nested suspension.
#[test]
fn a_suspended_result_may_be_bound_or_tested_directly() {
    for subject in [
        "let selected: bool = suspend ready(x); transition selected",
        "transition suspend ready(x)",
    ] {
        accepted(&format!(
            "machine wait(x: i32) -> i32 suspends {{ x }} \
             machine ready(x: i32) -> bool suspends {{ let v: i32 = suspend wait(x); v == 7 }} \
             machine forward(x: i32) -> i32 suspends {{ transition {{ _ -> suspend wait(x), }} }} \
             machine choose(x: i32) -> u8 suspends {{ {subject} {{ true -> 17u8, _ -> 23u8, }} }}"
        ));
    }
}
