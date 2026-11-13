/*
What a rejection states: the condition that was not proved, in the words the
current checker uses for the same obligation, so a programmer who knows one
checker's messages can act on the other's. For example a call requirement
reads "cannot prove requires contract for call halve from run: count > 1",
where the lowered form alone could only say "requires clause 1 of the called
machine".

The obligation kind and the lowered form give the identities: the callee and
clause of a `CallRequirement`, the operation of an `OperationSafety` and its
carrier, the machine of a guarantee. Elaboration's names table
(`structure_elaboration::MachineNames`) gives what to call them: machine and
state names, each clause's authored text, the binder an operation's value
lands in, and a call's argument text, so a callee clause is stated over the
caller's actuals as the current checker states it. Without that table (bodies
lowered without typed trees) a statement names the identities only.

A statement says what was not proved, never that it is false
(`wiki/spec/build/compiler_request.md`, "Outcomes": "Failure to find a proof
is not proof of falsehood"). So it never says "provably zero" or
"structurally false", which the current checker says when it has decided the
operand; the route has not decided it.
*/

use semantic_vocabulary::{
    CrashCause, IntegerCarrier, IntegerSign, InvocationSite, MachineId, OperationId, ScalarType,
};
use structure_elaboration::{Binder, Elaboration, MachineNames, SourceOrigin};
use terminal_check_tables::MachineTables;
use terminal_check_tables::edges::Callee;
use terminal_check_tables::obligations::{CrashSite, ObligationKind};
use terminal_psi::{OperationKind, OperationResult};

/// The statement of one unproved obligation of `machine`, which arises at
/// `site`.
pub(crate) fn unproved(
    elaboration: &Elaboration,
    machine: MachineId,
    tables: &MachineTables,
    kind: ObligationKind,
    site: SourceOrigin,
) -> String {
    let names = elaboration.names.get(machine);
    match kind {
        ObligationKind::CallRequirement { invocation, clause } => {
            let callee = tables
                .invocations
                .get(invocation)
                .and_then(|row| match row.callee {
                    Callee::Machine(callee) => Some(callee),
                    Callee::Selected { .. } => None,
                });
            let callee_names = callee.and_then(|callee| elaboration.names.get(callee));
            let text = callee_names.and_then(|callee_names| {
                let clause = callee_names.requires.get(usize::try_from(clause).ok()?)?;
                let arguments = match site {
                    SourceOrigin::Authored(span) => names.and_then(|names| names.arguments(span)),
                    SourceOrigin::Unpositioned(_) => None,
                };
                let over_actuals = clause.render(&|position| {
                    arguments
                        .and_then(|arguments| arguments.get(position))
                        .or_else(|| callee_names.formals.get(position))
                        .cloned()
                        .unwrap_or_else(|| String::from("?"))
                });
                Some((over_actuals, callee_names.authored(clause)))
            });
            match (names, callee_names, text) {
                // The current checker follows the clause over the call's
                // actuals with what the callee asked for in its own words,
                // and names the caller's state (Psi 05,
                // `explain_missing_boolean_fact`).
                (Some(names), Some(callee_names), Some((text, fact))) => format!(
                    "cannot prove requires contract for call {} from {}: {text} (needs fact \
                     `{fact}` here; establish it before the call via a prior contract \
                     guarantee, domain membership, or guard) in state `{}`",
                    callee_names.machine, names.machine, names.state
                ),
                _ => format!(
                    "cannot prove requires clause {} of the called machine",
                    u64::from(clause) + 1
                ),
            }
        }
        ObligationKind::NormalGuarantee { clause, .. }
        | ObligationKind::PublishedGuarantee { clause } => {
            let text = names.and_then(|names| {
                let text = names.ensures.get(usize::try_from(clause).ok()?)?;
                Some((names, names.authored(text)))
            });
            match text {
                Some((names, text)) => match names.exit_statement(site) {
                    Some(statement) => format!(
                        "cannot prove ensures contract for exit from {} at statement {statement}: \
                         {text}",
                        names.machine
                    ),
                    None => format!(
                        "cannot prove ensures contract for exit from {}: {text}",
                        names.machine
                    ),
                },
                None => format!("cannot prove ensures clause {}", u64::from(clause) + 1),
            }
        }
        ObligationKind::OperationSafety { operation } => {
            operation_safety(elaboration, machine, names, operation, site)
        }
        ObligationKind::DataDomain { clause, .. } => format!(
            "cannot prove requires clause {} of a data type in {}",
            u64::from(clause) + 1,
            owner(names, site)
        ),
        ObligationKind::ArrivalInvariant { edge } => {
            head_arrival(elaboration, machine, tables, edge, site).unwrap_or_else(|| {
                format!("cannot prove block invariant in {}", owner(names, site))
            })
        }
        ObligationKind::CrashCoverage { site: crash, cause } => {
            crash_coverage(elaboration, tables, names, crash, cause, site)
        }
        ObligationKind::WellFoundedness { .. } | ObligationKind::RankComparison { .. } => {
            format!("cannot prove termination in {}", owner(names, site))
        }
        ObligationKind::Claim { .. }
        | ObligationKind::Loan { .. }
        | ObligationKind::RestorationWindow { .. }
        | ObligationKind::DomainDebt { .. }
        | ObligationKind::EstablishmentDebt { .. } => {
            format!(
                "cannot prove conserved obligation in {}",
                owner(names, site)
            )
        }
    }
}

/// An arrival into a loop head is a call of the head's member, and the
/// head's invariant is that member's `requires`
/// (`wiki/spec/language/state_contracts.md`: "Machine `requires` applies at
/// every invocation, including terminal and recursive calls."). It is stated
/// as the current checker states that call's requirement: the clauses over
/// the call's actuals, then as the member wrote them, and the calling
/// machine's state. The calling machine is the one whose names record a
/// call at the arrival's site. `None` for a join, whose predicate the
/// checker inferred, and for the entry's jump, which no call made.
fn head_arrival(
    elaboration: &Elaboration,
    machine: MachineId,
    tables: &MachineTables,
    edge: semantic_vocabulary::EdgeId,
    site: SourceOrigin,
) -> Option<String> {
    let terminal_check_tables::edges::Destination::Block(head) =
        tables.edges.get(edge)?.destination
    else {
        return None;
    };
    let member = elaboration
        .sources
        .get(machine)?
        .heads
        .iter()
        .find(|(block, _)| *block == head)?
        .1;
    let callee = elaboration.names.get(member)?;
    let SourceOrigin::Authored(span) = site else {
        return None;
    };
    let (caller, arguments) = elaboration
        .names
        .iter()
        .find_map(|(_, names)| Some((names, names.arguments(span)?)))?;
    if callee.requires.is_empty() {
        return None;
    }
    let over_actuals: Vec<String> = callee
        .requires
        .iter()
        .map(|clause| {
            clause.render(&|position| {
                arguments
                    .get(position)
                    .or_else(|| callee.formals.get(position))
                    .cloned()
                    .unwrap_or_else(|| String::from("?"))
            })
        })
        .collect();
    let authored: Vec<String> = callee
        .requires
        .iter()
        .map(|clause| callee.authored(clause))
        .collect();
    Some(format!(
        "cannot prove requires contract for call {} from {}: {} (needs fact `{}` here; \
         establish it before the call via a prior contract guarantee, domain membership, or \
         guard) in state `{}`",
        callee.machine,
        caller.machine,
        over_actuals.join(" && "),
        authored.join(" && "),
        caller.state
    ))
}

/// The current checker's statement of an uncovered crash (Psi 05,
/// `check_published_ceiling_coverage`): a call's surviving route names the
/// caller and callee, a `crash` statement names its machine, and both say
/// what to publish. The current checker also gives a statement and call
/// number, which count slots of its own statement layout; the lowered form
/// has no such slots, and the statement is positioned at the site instead.
fn crash_coverage(
    elaboration: &Elaboration,
    tables: &MachineTables,
    names: Option<&MachineNames>,
    crash: CrashSite,
    cause: CrashCause,
    site: SourceOrigin,
) -> String {
    let Some(names) = names else {
        return format!("cannot prove crash coverage in {}", owner(names, site));
    };
    // A call site is an operation, or a block that ends in a call with no
    // normal return; either way the invocation row names the callee.
    let callee = tables
        .invocations
        .iter()
        .find(|(_, row)| match (crash, row.site) {
            (CrashSite::Operation(operation), InvocationSite::Operation(called)) => {
                operation == called
            }
            (CrashSite::Block(block), InvocationSite::InvokeNoNormal(edge)) => tables
                .edges
                .get(edge)
                .is_some_and(|row| row.source == block),
            _ => false,
        })
        .and_then(|(_, row)| match row.callee {
            Callee::Machine(callee) => elaboration.names.get(callee),
            Callee::Selected { .. } => None,
        });
    match callee {
        Some(callee) => format!(
            "call from `{}` to `{}` has an uncovered {cause:?} crash route; publish a \
             same-cause route whose guard covers this invocation",
            names.machine, callee.machine
        ),
        None => format!(
            "machine `{}` has an uncovered {cause:?} crash; publish a same-cause `crashes` \
             route whose guard covers this site",
            names.machine
        ),
    }
}

/// The current checker's statement of a partial operation's safety
/// (`validation`'s arithmetic domains): overflow of an add, subtract or
/// multiply, the count of a shift, the definedness of a division or
/// remainder. Each names the operation's carrier.
fn operation_safety(
    elaboration: &Elaboration,
    machine: MachineId,
    names: Option<&MachineNames>,
    operation: OperationId,
    site: SourceOrigin,
) -> String {
    let owner = owner(names, site);
    let found = usize::try_from(machine.get() - 1)
        .ok()
        .and_then(|position| elaboration.module.machines.get(position))
        .and_then(|lowered| {
            lowered
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|candidate| candidate.id == operation)
        });
    let Some(found) = found else {
        return format!("cannot prove operation safety in {owner}");
    };
    let carrier = match &found.result {
        OperationResult::Scalar(declaration) => primitive(declaration.scalar_type),
        OperationResult::Unit | OperationResult::Structural(_) => String::from("?"),
    };
    let signed = matches!(
        &found.result,
        OperationResult::Scalar(declaration)
            if matches!(declaration.scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed)
    );
    let partial = |operation: &str| {
        let conditions = if signed {
            "the divisor must be proven nonzero and the signed `MIN / -1` primitive pair must \
             be excluded"
        } else {
            "the divisor must be proven nonzero"
        };
        format!("partial exact {operation} in {owner}: {conditions}")
    };
    match found.kind {
        OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. } => format!(
            "exact arithmetic in {owner} may overflow `{carrier}`: the operands are not provably \
             in range (decision 17 -- exact arithmetic is a proof obligation)"
        ),
        // A left shift's goal is a conjunction: the count is below the width
        // and the shifted value fits. The route does not say which conjunct
        // it failed, so the statement names both, each in the current
        // checker's words.
        OperationKind::ExactIntegerShiftLeft { .. } => format!(
            "exact arithmetic in {owner} may overflow `{carrier}`: the shift count is not \
             provably below the operand width or the shifted value is not provably in range"
        ),
        OperationKind::ExactIntegerShiftRight { .. } => {
            format!(
                "shift count in {owner} is not provably below the operand width for `{carrier}`"
            )
        }
        // In the current checker's words, so a pinned fragment of its
        // diagnostic matches; the operand's carrier is not named because
        // the operation row holds only the operand's value identity.
        OperationKind::IntegerExactCast { .. } => format!(
            "Exact integer cast in {owner} to `{carrier}` is not provably representable; \
             constrain the source with a range or dominating guard, or use a named Wrapping, \
             Saturating, or Trapping conversion"
        ),
        OperationKind::ExactIntegerDivide { .. } => partial("division"),
        OperationKind::ExactIntegerRemainder { .. } => partial("remainder"),
        _ => format!("cannot prove operation safety in {owner}"),
    }
}

/// Where a value is computed, as the current checker names it:
/// "machine `run` state `entry` local `quotient`", or "... return value".
pub(crate) fn owner(names: Option<&MachineNames>, site: SourceOrigin) -> String {
    let Some(names) = names else {
        return String::from("this machine");
    };
    let mut owner = format!("machine `{}` state `{}`", names.machine, names.state);
    let binder = match site {
        SourceOrigin::Authored(span) => names.binder(span),
        SourceOrigin::Unpositioned(_) => None,
    };
    match binder {
        Some(Binder::Local(name)) => {
            owner.push_str(" local `");
            owner.push_str(name);
            owner.push('`');
        }
        Some(Binder::ReturnValue) => owner.push_str(" return value"),
        None => {}
    }
    owner
}

/// A scalar type as Omega spells it.
fn primitive(scalar_type: ScalarType) -> String {
    match scalar_type {
        ScalarType::Boolean => String::from("bool"),
        ScalarType::IeeeFloat(_) => String::from("float"),
        ScalarType::Callable(contract) => format!("machine contract {}", contract.get()),
        ScalarType::Integer(integer) => {
            let sign = match integer.sign() {
                IntegerSign::Signed => 'i',
                IntegerSign::Unsigned => 'u',
            };
            match integer.carrier() {
                IntegerCarrier::Address => format!("{sign}size"),
                IntegerCarrier::Fixed => format!("{sign}{}", integer.bits()),
            }
        }
    }
}
