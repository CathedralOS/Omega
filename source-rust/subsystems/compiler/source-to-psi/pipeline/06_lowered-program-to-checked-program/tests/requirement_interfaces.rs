//! A trait requirement's declared interface must decide what a call through
//! it owes: each conformance calls the machine its own row supplies, a
//! requirement's guarded crash route is owed at the call with the caller's
//! actual, its service reach and its synchronous entries bound the caller,
//! and a member inherited from a parent trait calls the row that supplies
//! it. Each test pairs an accepted program with a twin that changes one
//! input, so a checker or lowering that read another row, formal, service,
//! entry argument or parent gives the twin the wrong verdict.
//!
//! The spec rules: a conformance row selects the implementation of its
//! requirement (`wiki/spec/language/conformances.md`); a caller covers each
//! crash route of what it calls (`wiki/spec/language/effects.md`, "Guarded
//! crashes"); reach and `invokes` ceilings bound every call
//! (`effects.md`, "Direct synchronous invocation").

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, Verdict};
use terminal_psi::OperationKind;

fn verdict(source: &str) -> Verdict {
    Program::try_of(&[("product/main.omg", source)])
        .unwrap_or_else(|refusal| panic!("{source}\n{refusal:?}"))
        .judged(|_| {})
}

#[track_caller]
fn accepted(source: &str) {
    verdict(source).accepted()
}

/// The one statement the checker gives for a call outside its caller's
/// declared reach or entries.
#[track_caller]
fn exceeded(source: &str) -> String {
    let rejection = verdict(source).rejected();
    let [statement] = rejection.exceeded.as_slice() else {
        panic!("{source}\n{rejection:?}")
    };
    statement.clone()
}

/// `Red` supplies `paint` with `red` and `Blue` with `blue`; `Red::paint(x)`
/// calls `red` and `Blue::paint(x)` calls `blue`. Exchanging the two rows
/// exchanges the callees.
#[test]
fn each_conformance_calls_the_machine_its_own_row_supplies() {
    for (red_paints, blue_paints) in [("red", "blue"), ("blue", "red")] {
        let fill = |paint: &str| if paint == "red" { "blue" } else { "red" };
        let program = Program::of(&format!(
            "trait Render {{ machine paint(x: u8) -> bool; machine fill(x: u8) -> bool; }}
machine red(x: u8) -> bool {{ true }} machine blue(x: u8) -> bool {{ false }}
Red satisfies Render {{ Render::paint = {red_paints}; Render::fill = {}; }}
Blue satisfies Render {{ Render::paint = {blue_paints}; Render::fill = {}; }}
machine one(x: u8) -> bool {{ Red::paint(x) }}
machine two(x: u8) -> bool {{ Blue::paint(x) }}",
            fill(red_paints),
            fill(blue_paints),
        ));
        for (caller, selected) in [("one", red_paints), ("two", blue_paints)] {
            let call = program::the_operation(program.machine(caller), |kind| {
                matches!(kind, OperationKind::Call { .. })
            });
            let OperationKind::Call { callee, .. } = call.kind else {
                unreachable!()
            };
            assert_eq!(program.machine_named(callee), selected, "{caller}");
        }
        program.judged(|_| {}).accepted();
    }
}

const SERVICE: &str = "trait Service { machine read(x: u8, y: u8) crashes Trap y == 0; }
Impl satisfies Service { Service::read = go; }
machine go(x: u8, y: u8) crashes Trap y == 0 { }";

/// `Service::read` may trap when its `y` is 0. Calling it through `Impl`
/// with `(a, b)` owes that route over `b`: a caller that declares the trap
/// is accepted, a caller that requires `b != 0` and declares no crash is
/// accepted, and one that requires `a != 0` instead is rejected for the trap.
#[test]
fn a_call_through_a_requirement_owes_its_guarded_route_over_the_actual() {
    accepted(&format!(
        "{SERVICE} machine caller(a: u8, b: u8) crashes Trap {{ Impl::read(a, b) }}"
    ));
    accepted(&format!(
        "{SERVICE} machine caller(a: u8, b: u8) crashes none requires b != 0 {{ Impl::read(a, b) }}"
    ));
    let twin = verdict(&format!(
        "{SERVICE} machine caller(a: u8, b: u8) crashes none requires a != 0 {{ Impl::read(a, b) }}"
    ))
    .rejected();
    assert_eq!(
        twin.unproved,
        ["CrashCoverage { site: Operation(OperationId(1)), cause: Trap }"]
    );
}

/// `MotorControl::stop` reaches its own service: a caller that reaches
/// `MotorControl` may call it and one that reaches nothing may not. When
/// `stop` reaches `Other` instead, reaching `MotorControl` does not cover it.
#[test]
fn a_requirement_reaches_the_service_it_declares() {
    let own = "boundary trait MotorControl { machine stop() reaches MotorControl crashes none; }";
    accepted(&format!(
        "{own} machine go() reaches MotorControl crashes none {{ MotorControl::stop(); }}"
    ));
    let none = exceeded(&format!(
        "{own} machine go() reaches none crashes none {{ MotorControl::stop(); }}"
    ));
    assert!(
        none.starts_with("machine `go` reaches `none` but its call of"),
        "{none}"
    );
    let other = exceeded(
        "boundary trait Other {} \
         boundary trait MotorControl { machine stop() reaches Other crashes none; } \
         machine go() reaches MotorControl crashes none { MotorControl::stop(); }",
    );
    assert!(other.contains("main.omg::Other"), "{other}");
}

const DRIVER: &str = "data Fn = machine() reaches none crashes none;
boundary trait Driver {
    machine first(a: Fn, b: Fn) reaches none crashes none invokes a;
    machine second(a: Fn, b: Fn) reaches none crashes none invokes b;
}";

/// `Driver::first` invokes its first formal and `Driver::second` its
/// second, though both have the same type. A caller passing `(f, g)` to
/// `first` may declare `invokes f` but not `invokes g`; through `second`
/// it may declare `invokes g`.
#[test]
fn an_entry_ceiling_names_the_formal_it_invokes() {
    let caller = |entry: &str, member: &str| {
        format!(
            "{DRIVER} machine c(f: Fn, g: Fn) reaches Driver crashes none invokes {entry} \
             {{ Driver::{member}(f, g); }}"
        )
    };
    accepted(&caller("f", "first"));
    accepted(&caller("g", "second"));
    let twin = exceeded(&caller("g", "first"));
    assert!(
        twin.starts_with("machine `c` invokes `parameter 2` but its call of"),
        "{twin}"
    );
}

const HANDLER: &str = "boundary trait Handler<T> { machine run() reaches none crashes none; }
boundary trait Driver {
    machine read() reaches none crashes none invokes Handler<u8>;
    machine idle() reaches none crashes none;
}";

/// `Driver::read` enters `Handler<u8>`, the application it names, and
/// `Driver::idle` enters nothing. A caller of `read` that declares
/// `invokes Handler<u8>` is accepted and one that declares `Handler<u16>`
/// is rejected; a caller of `idle` may declare either.
#[test]
fn an_entry_ceiling_keeps_its_applied_target() {
    let caller = |entry: &str, member: &str| {
        format!(
            "{HANDLER} machine caller() reaches Driver crashes none invokes Handler<{entry}> \
             {{ Driver::{member}(); }}"
        )
    };
    accepted(&caller("u8", "read"));
    accepted(&caller("u16", "idle"));
    let twin = exceeded(&caller("u16", "read"));
    assert!(twin.contains("compiler-type\\(atom\\(u16\\)\\)"), "{twin}");
}

const PARENTED: &str = "boundary trait Parent<const N: u64> {}
boundary trait Handler<const N: u64>: Parent<0u64> {}
boundary trait Driver {
    machine dispatch() reaches none invokes Handler<17u64> invokes Handler<29u64> crashes none;
}";

/// `dispatch` enters `Handler<17>` and `Handler<29>`, two entries that keep
/// their arguments. A caller must declare both: `Handler<17>` alone is
/// rejected for the other, and their parent `Parent<0>` alone covers
/// neither.
#[test]
fn each_entry_argument_is_its_own_entry_and_brings_no_parent() {
    let caller = |entries: &str| {
        format!(
            "{PARENTED} machine caller() reaches Driver crashes none {entries} \
             {{ Driver::dispatch(); }}"
        )
    };
    accepted(&caller("invokes Handler<17u64> invokes Handler<29u64>"));
    let missing = exceeded(&caller("invokes Handler<17u64>"));
    assert!(missing.contains("arguments(integer(17))"), "{missing}");
    let parent = exceeded(&caller("invokes Parent<0u64>"));
    assert!(parent.contains("main.omg::Parent"), "{parent}");
}

/// `C` declares no member of its own and inherits `id` from its parent `Q`.
/// `Selected` supplies `Q::id` with `same`, and `Selected::id(x)` calls
/// `same`; supplying it with `twice` calls `twice`.
#[test]
fn a_member_inherited_from_a_parent_trait_calls_the_row_that_supplies_it() {
    for supplied in ["same", "twice"] {
        let program = Program::of(&format!(
            "trait Q {{ machine id(x: u8) -> u8; }} trait C: Q {{}}
machine same(x: u8) -> u8 {{ x }} machine twice(x: u8) -> u8 {{ x }}
Selected satisfies C {{ Q::id = {supplied}; }}
machine call(x: u8) -> u8 {{ Selected::id(x) }}"
        ));
        let call = program::the_operation(program.machine("call"), |kind| {
            matches!(kind, OperationKind::Call { .. })
        });
        let OperationKind::Call { callee, .. } = call.kind else {
            unreachable!()
        };
        assert_eq!(program.machine_named(callee), supplied);
        program.judged(|_| {}).accepted();
    }
}

/// `Red` selects the plain machine `red` for `Render::paint`, over a
/// `Render` with no parameters and over `Render<u8>`.
fn applied_render(applied: bool, red_requires: &str) -> String {
    let (declared, applied, carrier) = if applied {
        ("Render<T>", "Render<u8>", "T")
    } else {
        ("Render", "Render", "u8")
    };
    format!(
        "trait {declared} {{ machine paint(x: {carrier}, n: u8) -> {carrier}; }}
Red satisfies {applied} {{ Render::paint = red; }}
machine red(x: u8, n: u8) -> u8 {red_requires} {{ x }}
machine call(x: u8) -> u8 {{ Red::paint(x, 0u8) }}"
    )
}

/// `Red satisfies Render<u8>` selects the plain machine `red` for
/// `Render::paint`, and `Red::paint(x, 0)` calls `red`. `red` must refine
/// `paint` as `Render<u8>` applies it, exactly as it refines a `Render`
/// with no parameters: one that asks for nothing more is accepted, and the
/// twin whose `red` also requires `n != 0`, which the requirement does not
/// promise, is refused for that precondition.
#[test]
fn a_member_of_an_applied_trait_refines_the_application_its_conformance_names() {
    for applied in [false, true] {
        let program = Program::of(&applied_render(applied, ""));
        let call = program::the_operation(program.machine("call"), |kind| {
            matches!(kind, OperationKind::Call { .. })
        });
        let OperationKind::Call { callee, .. } = call.kind else {
            unreachable!()
        };
        assert_eq!(program.machine_named(callee), "red");
        program.judged(|_| {}).accepted();

        let twin = format!("{:?}", verdict(&applied_render(applied, "requires n != 0")));
        assert!(
            twin.contains("cannot prove the input precondition of `red`"),
            "{applied}: {twin}"
        );
    }
}

/// `Local` supplies `keep` and `pick<const N: u8>` with its own bodies.
/// `keep` is the generic `keep<T>` or, for comparison, a plain member over
/// `u8`; `requires` is what `Local::keep` asks beyond the requirement.
fn generic_members(generic: bool, requires: &str) -> String {
    let (keep, carrier, call) = if generic {
        ("keep<T>", "T", "keep::<u8>")
    } else {
        ("keep", "u8", "keep")
    };
    format!(
        "trait Keep {{ machine {keep}(x: {carrier}, n: u8) -> {carrier}; machine pick<const N: u8>(x: u8) -> u8; }}
Local satisfies Keep {{
    machine {keep}(x: {carrier}, n: u8) -> {carrier} {requires} {{ x }}
    machine pick<const N: u8>(x: u8) -> u8 {{ x +% N }}
}}
machine call(x: u8) -> u8 {{ Local::pick::<4>(Local::{call}(x, 0u8)) }}"
    )
}

/// `Local::keep::<u8>(x, 0)` and `Local::pick::<4>(…)` each enter the body
/// prepared for that call's own arguments, not the generic declaration, and
/// the program is accepted. The twin whose `keep` also requires `n != 0`,
/// which `0` breaks, gets the same verdict as the same twin over a plain
/// `keep`: the instance is judged as an ordinary member is.
#[test]
fn a_named_call_of_a_generic_member_enters_the_instance_for_its_arguments() {
    let program = Program::of(&generic_members(true, ""));
    let entered = program
        .machine("call")
        .operations()
        .filter_map(|operation| match operation.kind {
            OperationKind::Call { callee, .. } => Some(program.machine_named(callee)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(entered.len(), 2, "{entered:?}");
    assert!(
        entered.iter().all(|callee| callee.contains("$specialized")),
        "{entered:?}"
    );
    program.judged(|_| {}).accepted();

    let generic = verdict(&generic_members(true, "requires n != 0")).rejected();
    let plain = verdict(&generic_members(false, "requires n != 0")).rejected();
    assert_eq!(format!("{generic:?}"), format!("{plain:?}"));
    assert!(
        generic
            .unproved
            .iter()
            .any(|kind| kind.starts_with("CallRequirement")),
        "{generic:?}"
    );
}
