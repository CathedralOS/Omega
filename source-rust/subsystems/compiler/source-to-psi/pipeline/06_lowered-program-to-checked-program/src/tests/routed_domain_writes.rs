//! A value that carries a domain with establishment routes keeps that domain
//! only while it is unchanged, or while the machine changing it is one the
//! domain's declaration names.
//!
//! domains.md, "Establishment": "Assignment, move, and permitted copy
//! transport existing qualification between places of the same declared type;
//! rebuilding equal carrier fields does not. Mutation invalidates
//! subject-bound facts unless preserved or re-established." A machine outside
//! the routes cannot re-establish the domain, so it cannot change part of
//! such a value and still hold or return it under the domain.
//!
//! Each rejected program differs from its accepted twin only in which storage
//! one statement changes or which machine contains that statement.

use super::*;

const TICKET: &str = "data Ticket { amount: u32; }\n\
     domain Ticket::Stamped established by stamp;\n";
const STAMP: &str = "machine stamp(ticket: Ticket) -> Ticket in Stamped { ticket as +Stamped }\n";

const KEPT: &str = "is not one of the domain's establishment routes";

fn verdict(text: &str) -> Result<Vec<String>, String> {
    match super::entryless::judged(text, false) {
        RouteVerdict::Rejected { violations, .. } => Ok(violations
            .iter()
            .map(|violation| violation.statement.clone())
            .collect()),
        RouteVerdict::Accepted(_) => Err("accepted".into()),
        RouteVerdict::AcceptedPrefix(_) => Err(String::from("unfinished")),
        RouteVerdict::Incomplete(_) => Err("incomplete".into()),
        RouteVerdict::FallBack(refusal) => Err(format!("not supported: {}", refusal.detail)),
    }
}

fn accepted(text: &str) {
    assert_eq!(verdict(text).unwrap_err(), "accepted", "{text}");
}

fn rejected(text: &str, domain: &str) {
    let violations = verdict(text).unwrap_or_else(|other| panic!("{other}: {text}"));
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains(KEPT) && violation.contains(domain)),
        "{violations:?}"
    );
}

/// The owned value is moved into a local and handed back unchanged; the twin
/// writes one field first and still returns it as stamped.
#[test]
fn a_field_write_does_not_keep_a_routed_domain_on_an_owned_value() {
    let program = |change: &str| {
        format!(
            "{TICKET}{STAMP}\
             machine pass(ticket: Ticket in Stamped) -> Ticket in Stamped {{\n    \
             let mutable held: Ticket in Stamped = ticket;\n    {change}\n    held\n}}\n"
        )
    };
    accepted(&program(""));
    rejected(&program("held.amount = 1000u32;"), "Ticket::Stamped");
}

/// The same write through storage the machine was lent. The accepted twin
/// lends the stamped value on without changing it.
#[test]
fn a_field_write_through_a_loan_does_not_keep_a_routed_domain() {
    let program = |body: &str| {
        format!(
            "{TICKET}{STAMP}\
             machine look(ticket: @mutable Ticket in Stamped) {{ }}\n\
             machine visit(ticket: @mutable Ticket in Stamped) {{ {body} }}\n"
        )
    };
    accepted(&program("look(ticket);"));
    rejected(&program("ticket.amount = 1000u32;"), "Ticket::Stamped");
}

/// The machine the domain names may change the value: it could have added
/// the domain to the changed value by its own cast.
#[test]
fn an_establishment_route_may_change_the_value_it_stamps() {
    accepted(&format!(
        "{TICKET}\
         machine stamp(ticket: Ticket) -> Ticket in Stamped {{\n    \
         let mutable held: Ticket in Stamped = ticket as +Stamped;\n    \
         held.amount = 5u32;\n    held\n}}\n"
    ));
}

/// A stamped ticket kept in a field of an unqualified record. Its sibling
/// field and the whole ticket may be stored; a field of the ticket may not.
#[test]
fn only_storage_beneath_the_qualified_value_is_protected() {
    let program = |parameters: &str, change: &str| {
        format!(
            "{TICKET}{STAMP}\
             data Stub {{ ticket: Ticket in Stamped; count: u32; }}\n\
             machine change(stub: @mutable Stub{parameters}) {{ {change} }}\n"
        )
    };
    accepted(&program("", "stub.count = 2u32;"));
    accepted(&program(", next: Ticket in Stamped", "stub.ticket = next;"));
    rejected(&program("", "stub.ticket.amount = 9u32;"), "Ticket::Stamped");
}

/// The callee sees only the inner record, so the machine that lends it out
/// of a sealed holder answers for the change. The twin lends the same field
/// of a holder that carries no domain.
#[test]
fn lending_part_of_a_qualified_value_for_change_is_a_change() {
    let program = |qualification: &str| {
        format!(
            "data Inner {{ value: u32; }}\n\
             data Holder {{ inner: Inner; tag: u32; }}\n\
             domain Holder::Sealed established by seal;\n\
             machine seal(holder: Holder) -> Holder in Sealed {{ holder as +Sealed }}\n\
             machine bump(inner: @mutable Inner) {{ inner.value = 1u32; }}\n\
             machine poke(holder: @mutable Holder{qualification}) {{ bump(@mutable holder.inner); }}\n"
        )
    };
    accepted(&program(""));
    rejected(&program(" in Sealed"), "Holder::Sealed");
}

/// A local reference to the inner field hides the holder's domain from the
/// store that goes through it; the store still changes the sealed holder.
#[test]
fn a_store_through_a_reference_reaches_the_qualified_lender() {
    let program = |qualification: &str| {
        format!(
            "data Inner {{ value: u32; }}\n\
             data Holder {{ inner: Inner; tag: u32; }}\n\
             domain Holder::Sealed established by seal;\n\
             machine seal(holder: Holder) -> Holder in Sealed {{ holder as +Sealed }}\n\
             machine poke(holder: Holder{qualification}) -> Holder{qualification} {{\n    \
             let mutable held: Holder{qualification} = holder;\n    \
             let inner: @mutable Inner = @mutable held.inner;\n    \
             inner.value = 3u32;\n    held\n}}\n"
        )
    };
    accepted(&program(""));
    rejected(&program(" in Sealed"), "Holder::Sealed");
}
