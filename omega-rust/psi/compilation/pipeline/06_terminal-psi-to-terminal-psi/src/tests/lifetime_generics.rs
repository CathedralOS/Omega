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

#[test]
fn selected_generic_helpers_keep_their_ordinary_lifetime_obligations() {
    let source = r#"
machine make<'storage>(bytes: &'storage [u8]) -> u64 { bytes.len }
machine relay<'storage, Kind, machine Make>(bytes: &'storage [u8], other: &'storage [u8], count: &mut u64) -> u64
requires machine Make(bytes: &'storage [u8]) -> u64 reaches none crashes none;
{ let length: u64 = Make(bytes); count = length; length }
machine entry(left: &[u8], right: &[u8], count: &mut u64) {
    let selected: u64 = relay<u32, make>(left, left, count);
}
"#;
    let judged = |text: &str| {
        let typed = super::boundary_calls::typed(text);
        let template_symbol = typed
            .machines()
            .iter()
            .find(|machine| {
                machine.name.as_str() == "relay"
                    && typed
                        .machine_instantiation(machine.symbol)
                        .unwrap()
                        .is_none()
            })
            .expect("the authored helper has its own source declaration")
            .symbol;
        let (prepared, _) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed,
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("ordinary preparation must retain the selected helper");
        let template = prepared
            .machines()
            .iter()
            .find(|machine| machine.symbol == template_symbol)
            .unwrap();
        assert!(!template.lifetime_parameters.is_empty());
        let callback = prepared
            .machine_type_parameters(template)
            .iter()
            .find_map(|parameter| prepared.retained_static_machine_contract(parameter.symbol))
            .expect("the callback keeps its actual declared contract");
        let typed_trees::data::MachineParameterContractView::Structural(callback) = callback else {
            panic!("the callback is an actual structural requirement");
        };
        assert!(callback.declares_no_crash_route);
        assert!(
            prepared
                .authored_service_reach_rows_for(callback.symbol)
                .next()
                .is_some(),
            "the pure callback has an authored reach bound"
        );
        let path = prepared
            .machines()
            .iter()
            .filter(|machine| machine.symbol != template.symbol)
            .find_map(|machine| {
                prepared
                    .machine_instantiation_path(machine.symbol, template.symbol)
                    .filter(|path| !path.is_empty())
            })
            .expect("the call must keep its exact capture and specialization path");
        assert!(
            path.iter().any(|receipt| !prepared
                .expression_table
                .static_machine_application(receipt.binding_application())
                .arguments
                .is_empty()),
            "the selected helper keeps its original static tuple"
        );
        super::boundary_calls::compilation_results_of(&prepared)
    };
    let accepted = judged(source);
    for (position, verdict) in accepted.iter().enumerate() {
        assert!(
            matches!(verdict, Ok(RouteVerdict::Accepted(_))),
            "form {position}: {verdict:?}"
        );
    }
    let mut checked_envelopes = 0;
    for verdict in &accepted {
        let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
            continue;
        };
        let Some((requirement, _)) = accepted
            .elaboration
            .requirement_ceilings
            .iter()
            .find(|(_, ceiling)| ceiling.envelope.is_some())
        else {
            continue;
        };
        let mut forged = accepted.elaboration.clone();
        let envelope = forged
            .requirement_ceilings
            .get_mut(requirement)
            .unwrap()
            .envelope
            .as_mut()
            .unwrap();
        envelope.blocks = !envelope.blocks;
        let refused = crate::judge(Ok(forged));
        assert!(
            matches!(refused, Err(Refusal { reason: RefusalReason::ForeignIdentity, ref detail, .. })
            if detail.starts_with("abstract envelope differs from its complete source declaration")),
            "the abstract bound must keep its actual header: {refused:?}"
        );
        checked_envelopes += 1;
    }
    assert!(
        checked_envelopes > 0,
        "the actual callback requirement must retain its operational bound"
    );
    let entering = source.replace(
        "-> u64 reaches none crashes none;",
        "-> u64 reaches none crashes none invokes bytes;",
    );
    let erased = entering
        .replace(
            "(bytes: &'storage [u8])",
            "(prior [erased]: u32, bytes: &'storage [u8])",
        )
        .replace("Make(bytes)", "Make(0u32, bytes)");
    for (text, erased_formal) in [(&entering, false), (&erased, true)] {
        let mut checked_parameter = false;
        let entering = judged(text);
        for verdict in &entering {
            let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
                panic!("the actual parameter-entry bound must remain valid: {verdict:?}");
            };
            let Some((requirement, _)) =
                accepted
                    .elaboration
                    .requirement_ceilings
                    .iter()
                    .find(|(_, row)| {
                        row.envelope.as_ref().is_some_and(|envelope| {
                            envelope.invocations.iter().any(|entry| {
                                matches!(
                                    entry,
                                    terminal_psi::TerminalCallableInvocation::Parameter(_)
                                )
                            })
                        })
                    })
            else {
                continue;
            };
            let source = accepted
                .elaboration
                .static_schemas
                .iter()
                .flat_map(|schema| &schema.requirements)
                .find(|source| {
                    source
                        .instance
                        .as_ref()
                        .is_some_and(|instance| instance.machine == requirement)
                })
                .expect("the runtime row has its exact source declaration");
            assert_eq!(
                source.signature.erased.as_slice(),
                if erased_formal { &[0][..] } else { &[][..] }
            );
            assert_eq!(
                source.envelope.invocations,
                vec![terminal_psi::TerminalCallableInvocation::Parameter(0)]
            );
            let mut forged = accepted.elaboration.clone();
            forged
                .requirement_ceilings
                .get_mut(requirement)
                .unwrap()
                .envelope
                .as_mut()
                .unwrap()
                .invocations
                .retain(|entry| {
                    !matches!(
                        entry,
                        terminal_psi::TerminalCallableInvocation::Parameter(_)
                    )
                });
            let refused = crate::ceilings::judge(&forged).unwrap_err();
            assert_eq!(
                refused.reason,
                RefusalReason::ForeignIdentity,
                "{refused:?}"
            );
            assert_eq!(
                refused.detail,
                "abstract envelope differs from its complete source declaration"
            );
            checked_parameter = true;
        }
        assert!(
            checked_parameter,
            "the source schema must retain its exact parameter-entry permission"
        );
    }
    let refused = judged(&source.replace("(left, left, count)", "(left, right, count)"));
    assert!(
        refused.iter().any(|verdict| match verdict {
            Err(crate::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal
                .detail
                .contains("call that gives one lifetime two regions"),
            _ => false,
        }),
        "{refused:?}"
    );
}
