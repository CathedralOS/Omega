/*
Call components: which machines lie on a call cycle, and whether the cycle is
one this crate lowers. A component is a strongly connected set of machines in
the call graph of the bodies read: one machine that calls itself, or several
that reach each other.

`wiki/spec/language/termination.md`, "Calls and productive loops": "Runtime
recursive call cycles are tail-position only and lower to constant-stack
iteration." and "A productive terminal-transfer cycle without a termination
promise may run forever and owes no rank". So a component is lowered as a
loop (`lower`: one head block per member, a jump for each call between
members) when every call between its members stands in terminal position,
where the call's result completes the caller. Source nesting plays no part:
`wiki/spec/terminal-psi/control_flow.md`, "Source nesting does not bound the
component or justify any edge."

Everything else about a cycle that the loops lane has not yet served is
refused here by name, as `Unsupported`, before anything is lowered
(`wiki/drafts/designs/loops_recursion_termination_lane.md`, "Slices").
*/

use semantic_vocabulary::{IdTable, MachineId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::CrashRouteGuard;

use crate::body::{
    Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target, Termination,
    ValueType,
};
use crate::{Unserved, refuse};

/// The members of each machine's call component in `MachineId` order; empty
/// for a machine on no cycle.
pub(crate) type Components = IdTable<MachineId, Vec<MachineId>>;

/// One call in a body: its callee, and whether its result completes the
/// caller.
struct CallSite {
    callee: MachineId,
    terminal: bool,
}

fn index(machine: MachineId) -> usize {
    usize::try_from(machine.get() - 1).unwrap_or(usize::MAX)
}

pub(crate) fn components(
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
) -> Checked<Components> {
    let count = bodies.len();
    let mut calls: Vec<Vec<CallSite>> = Vec::with_capacity(count);
    for (machine, body) in bodies.iter() {
        let mut sites = Vec::new();
        statements(&body.statements, &mut sites);
        if sites.iter().any(|site| index(site.callee) >= count) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Call("callee outside the module"),
            ));
        }
        calls.push(sites);
    }
    // `reaches[a][b]`: a chain of one or more calls leads from `a` to `b`.
    // ponytail: dense n x n reachability by one walk per machine; Tarjan's
    // components if closures grow to thousands of machines.
    let mut reaches = vec![vec![false; count]; count];
    for (start, row) in reaches.iter_mut().enumerate() {
        let mut pending: Vec<usize> = calls[start].iter().map(|site| index(site.callee)).collect();
        while let Some(next) = pending.pop() {
            if !std::mem::replace(&mut row[next], true) {
                pending.extend(calls[next].iter().map(|site| index(site.callee)));
            }
        }
    }
    let on_cycle = |machine: usize| reaches[machine][machine];
    let mut table = Components::default();
    for (machine, body) in bodies.iter() {
        let this = index(machine);
        let signature =
            terminal_check_tables::row(signatures, machine, machine, Construct::Machine)?;
        let members: Vec<MachineId> = if on_cycle(this) {
            bodies
                .keys()
                .filter(|other| reaches[this][index(*other)] && reaches[index(*other)][this])
                .collect()
        } else {
            Vec::new()
        };
        let unserved = |what| Err(refuse(machine, Construct::Machine, what));
        // A witness is the component's ranking, and a promise must be
        // checked against what the machine derives. Neither is read yet, so
        // neither is accepted where it would have to be: a witness anywhere,
        // a promise wherever a cycle is reached.
        match signature.termination {
            Termination::Silent => {}
            Termination::Witnessed => return unserved(Unserved::Contract("ranking witness")),
            Termination::Promised => {
                if on_cycle(this) || (0..count).any(|other| reaches[this][other] && on_cycle(other))
                {
                    return unserved(Unserved::Contract("termination promise over a call cycle"));
                }
            }
        }
        if !members.is_empty() {
            if calls[this]
                .iter()
                .any(|site| !site.terminal && members.contains(&site.callee))
            {
                return unserved(Unserved::Call("recursion outside a terminal position"));
            }
            if uses_storage(body, signature) {
                return unserved(Unserved::Machine("storage on a call cycle"));
            }
            // A head's parameters are its member's runtime scalar formals.
            // An erased binding has no runtime lane to carry around the
            // loop, and a value of a type parameter is not one the heads
            // declare yet; each needs its own slice.
            if !signature.erased.is_empty()
                || body
                    .statements
                    .iter()
                    .any(|statement| matches!(statement, Statement::Witness { .. }))
            {
                return unserved(Unserved::Machine("erased binding on a call cycle"));
            }
            let rigid = |value_type: &ValueType| matches!(value_type, ValueType::Rigid(_));
            if signature.parameters.iter().any(rigid)
                || signature.result.as_ref().is_some_and(rigid)
                || body.slots.iter().any(rigid)
            {
                return unserved(Unserved::Machine(
                    "value of a type parameter on a call cycle",
                ));
            }
            if !signature.ensures.is_empty() {
                return unserved(Unserved::Contract("ensures on a call cycle"));
            }
            if let CrashCeiling::Authored(routes) = &signature.crashes
                && routes.iter().any(|bucket| {
                    bucket
                        .alternatives
                        .iter()
                        .any(|guard| !matches!(guard, CrashRouteGuard::Truth))
                })
            {
                return unserved(Unserved::Contract("guarded crash route on a call cycle"));
            }
        }
        table.push(members);
    }
    Ok(table)
}

fn statements(statements: &[Statement], sites: &mut Vec<CallSite>) {
    for statement in statements {
        match statement {
            Statement::Bind { value, .. } | Statement::Evaluate(value) => expression(value, sites),
            Statement::Store {
                destination, value, ..
            } => {
                expression(destination, sites);
                expression(value, sites);
            }
            // An erased term is a literal or a binding: it calls nothing.
            Statement::Declare { .. } | Statement::Witness { .. } => {}
            Statement::Dispatch {
                subject,
                arms,
                default,
                ..
            } => {
                expression(subject, sites);
                for (_, arm, _) in arms {
                    target(arm, sites);
                }
                if let Some((arm, _)) = default {
                    target(arm, sites);
                }
            }
            Statement::Arm {
                guard, target: arm, ..
            } => {
                if let Some(guard) = guard {
                    expression(guard, sites);
                }
                target(arm, sites);
            }
        }
    }
}

/// An arm's target is the one terminal position: a call there forwards its
/// result as the machine's (`wiki/spec/language/machines.md`: "A machine
/// call in a terminal `transition` arm forwards its normal result as the
/// current invocation's result, with no continuation in that invocation.").
fn target(target: &Target, sites: &mut Vec<CallSite>) {
    match target {
        Target::Unit | Target::Crash(_) => {}
        Target::Value(value) => match &value.kind {
            ExpressionKind::Call { callee, arguments } => {
                for argument in arguments {
                    expression(argument, sites);
                }
                sites.push(CallSite {
                    callee: *callee,
                    terminal: true,
                });
            }
            _ => expression(value, sites),
        },
    }
}

fn expression(value: &Expression, sites: &mut Vec<CallSite>) {
    match &value.kind {
        ExpressionKind::SavedSubject(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Slot(_) => {}
        ExpressionKind::Not(operand)
        | ExpressionKind::BitwiseNot(operand)
        | ExpressionKind::Cast(operand)
        | ExpressionKind::Requalify(operand)
        | ExpressionKind::Field { base: operand, .. }
        | ExpressionKind::Payload { base: operand, .. }
        | ExpressionKind::IsCase {
            subject: operand, ..
        }
        | ExpressionKind::Borrow { place: operand, .. } => expression(operand, sites),
        ExpressionKind::Array { elements } => {
            for element in elements {
                expression(element, sites);
            }
        }
        ExpressionKind::Index { base, index } => {
            expression(base, sites);
            expression(index, sites);
        }
        ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
            for (_, operand) in fields {
                expression(operand, sites);
            }
        }
        ExpressionKind::Match {
            subject,
            arms,
            default,
            ..
        } => {
            expression(subject, sites);
            for (_, arm) in arms {
                expression(arm, sites);
            }
            if let Some(arm) = default {
                expression(arm, sites);
            }
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            expression(left, sites);
            expression(right, sites);
        }
        ExpressionKind::Call { callee, arguments } => {
            for argument in arguments {
                expression(argument, sites);
            }
            sites.push(CallSite {
                callee: *callee,
                terminal: false,
            });
        }
    }
}

/// Whether the machine holds a data value anywhere: a record or sum formal,
/// result, local or expression, or a local declared without an initializer.
/// Storage across a jump back needs structural block parameters and a write
/// log that follows them, which a later slice of the loops lane serves.
fn uses_storage(body: &Body, signature: &Signature) -> bool {
    fn stored(value: &Expression) -> bool {
        if matches!(value.result, Some(ValueType::Record(_))) {
            return true;
        }
        match &value.kind {
            ExpressionKind::SavedSubject(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Slot(_) => false,
            ExpressionKind::Array { .. }
            | ExpressionKind::Index { .. }
            | ExpressionKind::Record { .. }
            | ExpressionKind::Field { .. }
            | ExpressionKind::Case { .. }
            | ExpressionKind::Payload { .. }
            | ExpressionKind::IsCase { .. }
            | ExpressionKind::Match { .. }
            | ExpressionKind::Borrow { .. } => true,
            ExpressionKind::Not(operand)
            | ExpressionKind::BitwiseNot(operand)
            | ExpressionKind::Cast(operand)
            | ExpressionKind::Requalify(operand) => stored(operand),
            ExpressionKind::Strict { left, right, .. }
            | ExpressionKind::ShortCircuit { left, right, .. } => stored(left) || stored(right),
            ExpressionKind::Call { arguments, .. } => arguments.iter().any(stored),
        }
    }
    fn arm(target: &Target) -> bool {
        match target {
            Target::Value(value) => stored(value),
            Target::Unit | Target::Crash(_) => false,
        }
    }
    let data = |value_type: &ValueType| matches!(value_type, ValueType::Record(_));
    signature.parameters.iter().any(data)
        || !signature.borrowed.is_empty()
        || signature.result.as_ref().is_some_and(data)
        || body.slots.iter().any(data)
        || body.statements.iter().any(|statement| match statement {
            Statement::Bind { value, .. } | Statement::Evaluate(value) => stored(value),
            // Refused on a cycle by its own name, as an erased binding.
            Statement::Witness { .. } => false,
            Statement::Declare { .. } | Statement::Store { .. } | Statement::Dispatch { .. } => {
                true
            }
            Statement::Arm { guard, target, .. } => {
                guard.as_ref().is_some_and(stored) || arm(target)
            }
        })
}
