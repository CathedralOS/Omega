//! Machines generic over a lifetime, `machine keep<'a>(..)`. A lifetime
//! parameter selects no type, layout or monomorphized copy, so the machine
//! is read and lowered once; what it says is a relation between the regions
//! of its inputs and its result, which each call instantiates with the
//! caller's regions. lifetimes.md: "Lifetime arguments are erased regions,
//! separate from runtime generic arity, layout and monomorphization. Erasure
//! does not remove semantic lifetime relations or loan obligations."
//!
//! The accepted programs are the shapes of the toolchain's own Build
//! helpers (`dependency::from_source<'storage>` and
//! `dependency::with_alias<'storage>`); each twin breaks the relation one
//! way. Read from source, as a Build program.

use super::build_purpose::judge_build;
use super::*;

fn accepted(build: &str) {
    match judge_build(build).unwrap() {
        RouteVerdict::Accepted(accepted) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("{other:?}"),
    }
}

fn refusal(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        other => panic!("{other:?}"),
    }
}

const HOLDER: &str = "data Holder<'storage> { ready: bool; name: &'storage [u8]; }\n";

/// A machine stores a view of its own lifetime into a holder applied at it,
/// and another builds a holder at the lifetime of the one it is given. The
/// caller's holder is at the region its local is decided as, the literal
/// lands in that region, and each call instantiates `'storage` with it.
#[test]
fn a_call_instantiates_a_machine_lifetime_with_the_caller_region() {
    accepted(&format!(
        "{HOLDER}
machine named<'storage>(holder: &mut Holder<'storage>, name: &'storage [u8]) {{
    holder.name = name;
    holder.ready = true;
}}
machine copied<'storage>(holder: &Holder<'storage>) -> Holder<'storage> {{
    Holder {{ ready: holder.ready, name: holder.name }}
}}
machine build_entry(source: &Holder) {{
    let mut made: Holder = copied(source);
    named(&mut made, \"made\");
}}
"
    ));
}

/// The two holders `adopt` takes must be at one region. Two formals whose
/// lifetimes the caller elided are two regions, so the call is refused
/// rather than letting one holder keep a view that may outlive its backing.
#[test]
fn a_call_that_gives_one_lifetime_two_regions_is_refused() {
    assert_eq!(
        refusal(&format!(
            "{HOLDER}
machine adopt<'storage>(holder: &mut Holder<'storage>, other: &Holder<'storage>) {{
    holder.name = other.name;
}}
machine build_entry(holder: &mut Holder, other: &Holder) {{
    adopt(holder, other);
}}
"
        )),
        "typing call that gives one lifetime two regions while reading the body of `build_entry`"
    );
}

/// A record result at a lifetime no input names: the caller chooses it
/// (lifetimes.md, "Caller-chosen result lifetimes"), and here uses the
/// result where no place decides the choice. The body builds its view from
/// a quoted literal, which is valid at every choice.
#[test]
fn a_record_result_at_a_lifetime_no_input_supplies_is_chosen_by_the_caller() {
    let verdict = judge_build(&format!(
        "{HOLDER}
machine fresh<'storage>(ready: bool) -> Holder<'storage> {{
    Holder {{ ready: ready, name: \"\" }}
}}
machine build_entry() {{
    let ready: bool = fresh(true).ready;
}}
"
    ));
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "the literal is valid at every choice: {verdict:?}"
    );
}

/// Inside the machine its two lifetimes are two regions: a view of one is
/// not a value of a holder at the other.
#[test]
fn a_view_of_one_machine_lifetime_is_not_stored_at_another() {
    assert_eq!(
        refusal(&format!(
            "{HOLDER}
machine named<'storage, 'other>(holder: &mut Holder<'storage>, name: &'other [u8]) {{
    holder.name = name;
}}
machine build_entry() {{ }}
"
        )),
        "typing assigned value type while reading the body of `named`"
    );
}

/// A record value's type carries the regions it is applied at, so a view
/// stored in one record moves only into a record of the same region. Two
/// holders at `'long` and `'short`, or at two elided input lifetimes, are
/// two types; at one lifetime the exchange is accepted. Judged at the
/// declaration, with no call.
#[test]
fn a_view_moves_between_records_only_at_one_region() {
    for signature in [
        "<'long, 'short>(long: &mut Holder<'long>, short: &Holder<'short>)",
        "(long: &mut Holder, short: &Holder)",
    ] {
        assert_eq!(
            refusal(&format!(
                "{HOLDER}machine build_entry() {{ }}\n\
                 machine exchange{signature} {{ long.name = short.name; }}\n"
            )),
            "typing assigned value type while reading the body of `exchange`",
            "{signature}"
        );
    }
    accepted(&format!(
        "{HOLDER}machine build_entry() {{ }}\n\
         machine exchange<'one>(long: &mut Holder<'one>, short: &Holder<'one>) {{ long.name = short.name; }}\n"
    ));
}

/// A literal passed where a lifetime-generic call stands for an argument of
/// another call: the inner call takes its region from what the outer call
/// expects, and the literal is a value of that region, not of the lifetime
/// spelled in the inner callee's signature. This is `Build::dependency(..,
/// dependency::from_source(Source::Path { .. }))`.
#[test]
fn a_literal_inside_a_nested_generic_call_takes_the_expected_region() {
    accepted(&format!(
        "{HOLDER}
machine wrapped<'storage>(holder: Holder<'storage>) -> Holder<'storage> {{
    holder
}}
machine ready(holder: Holder) -> bool {{
    holder.ready
}}
machine build_entry() {{
    let made: bool = ready(wrapped(Holder {{ ready: true, name: \"made\" }}));
}}
"
    ));
}

/// An implementation refines a requirement whose formal elides a lifetime.
/// Each declaration's elided lifetime is a parameter of its own, so the two
/// signatures are the same only once the implementation's is instantiated
/// with the requirement's regions, which is what the refinement witness's
/// call does.
#[test]
fn an_implementation_refines_a_requirement_with_an_elided_lifetime() {
    let verdicts = super::build_purpose::judge_build_all(&format!(
        "{HOLDER}
trait Readies {{
    machine ready(holder: Holder) -> bool;
}}
Plain satisfies Readies {{
    machine ready(holder: Holder) -> bool {{
        holder.ready
    }}
}}
machine build_entry() {{ }}
"
    ));
    assert!(verdicts.len() > 1, "the witness is judged: {verdicts:?}");
    for verdict in verdicts {
        assert!(
            matches!(verdict, Ok(RouteVerdict::Accepted(_))),
            "{verdict:?}"
        );
    }
}

/// A boundary requirement with its own lifetime, as std's
/// `WindowsFilesystemRemovalIo::open<'scope>`. A trait's schema cannot state
/// the requirement's own regions, so the requirement is a template: its
/// contract is checked once in its own form, and the program checks each call
/// at its actual arguments and regions (generics.md, "Publication and
/// assumptions"). The closure comes first, then that form.
fn lifetime_requirement(contract: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(&format!(
            "data Holder {{ value: u32; }}
boundary trait Source {{
    machine take<'scope>(holder: &'scope Holder, used: u32) -> u32
    reaches none
    {contract}
    crashes none;
}}
machine entry() -> u32 {{ let holder: Holder = Holder {{ value: 3 }}; Source::take(&holder, 1) }}"
        )),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let entry = super::boundary_calls::entry(&typed);
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            super::build_purpose::roster(&[entry], &[None], source::DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

#[test]
fn a_call_owes_a_lifetime_requirement_at_its_actual_arguments() {
    // The holder the call lends has value 3.
    let verdicts = lifetime_requirement("requires used > 0 && holder.value == 3");
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    let Ok(RouteVerdict::Accepted(closure)) = &verdicts[0] else {
        panic!("the call establishes the requirement: {verdicts:?}")
    };
    closure.publish_invocation().unwrap();
    assert!(
        matches!(verdicts[1], Ok(RouteVerdict::Accepted(_))),
        "{verdicts:?}"
    );

    let verdicts = lifetime_requirement("requires used > 0 && holder.value == 4");
    assert!(
        matches!(verdicts[0], Ok(RouteVerdict::Rejected { .. })),
        "{verdicts:?}"
    );
}

#[test]
fn a_lifetime_requirement_is_checked_once_for_every_argument() {
    // `used - 1` underflows at 0. The declaration's contract is formed for
    // every `used`, both where the program reads the boundary and in the
    // requirement's own form; the call passing 1 does not make it total.
    let verdicts = lifetime_requirement("requires used - 1 < 10");
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Rejected { .. }))),
        "{verdicts:?}"
    );

    let verdicts = lifetime_requirement("requires used > 0 && used - 1 < 10");
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
}
