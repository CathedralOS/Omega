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
        "typing call that gives one lifetime two regions"
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
        "typing assigned value type"
    );
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
