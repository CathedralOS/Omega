/*
How each machine may leave: whether some path completes normally, and the
crash routes it publishes. Both are read from structure, callees first, once
every body is read. Nothing here proves a route possible or impossible:
disproving a route is the checker's work, so a route that structure reaches
is kept.

Published routes follow `wiki/spec/language/effects.md`: an authored
`crashes` clause is the published ceiling; a public body without one
publishes none ("On a public checked body omitted `crashes` means no crash
route"); a private body without one publishes what its body may do ("Private
checked bodies may omit any of the three to request inference"). Inference
here is structural: each cause the body may crash with, from a `crash`
statement or from a callee's published route, is published with the
alternative `Truth`. That is the widest route of that cause, so it never
hides a crash a caller must cover; a narrower guard needs proof and belongs to
the checker.
*/

use std::collections::BTreeSet;

use semantic_vocabulary::{CrashCause, IdTable, MachineId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard};

use crate::body::{Body, CrashCeiling, Expression, ExpressionKind, Signature, Statement, Target};
use crate::canonical;

/// What a machine publishes about how it may leave.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Some path of the body reaches a normal return. When none does, a call
    /// of the machine has no normal successor (`Terminator::InvokeNoNormal`).
    pub completes_normally: bool,
    /// The published crash routes in canonical form, over the formals.
    pub crash_routes: Vec<CrashRouteBucket>,
}

/// The outcomes of every machine, indexed by `MachineId`, worked out with
/// each callee before its callers.
pub(crate) fn outcomes(
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
) -> Checked<IdTable<MachineId, Outcome>> {
    let order = callees_first(bodies)?;
    let mut computed: Vec<Option<Outcome>> = vec![None; bodies.len()];
    for machine in &order {
        let body = terminal_check_tables::row(bodies, *machine, *machine, Construct::Machine)?;
        let signature =
            terminal_check_tables::row(signatures, *machine, *machine, Construct::Machine)?;
        let mut walk = Walk {
            machine: *machine,
            computed: &computed,
            causes: BTreeSet::new(),
        };
        let completes_normally = walk.statements(&body.statements)?;
        let crash_routes = match &signature.crashes {
            CrashCeiling::Authored(routes) => routes.clone(),
            CrashCeiling::Inferred => canonical::routes(
                walk.causes
                    .iter()
                    .map(|cause| (*cause, CrashRouteGuard::Truth)),
            ),
        };
        let slot = position(*machine)
            .and_then(|position| computed.get_mut(position))
            .ok_or_else(|| missing(*machine))?;
        *slot = Some(Outcome {
            completes_normally,
            crash_routes,
        });
    }
    let mut table = IdTable::default();
    for (position, outcome) in computed.into_iter().enumerate() {
        let machine = MachineId::new(position as u64 + 1).expect("positions start at 1");
        table.push(outcome.ok_or_else(|| missing(machine))?);
    }
    Ok(table)
}

/// Every machine with each callee before its callers: a depth-first
/// post-order over the calls the bodies make. The reader already refuses
/// recursion; bodies given to `lower_bodies` directly have not been read, so a
/// cycle among them is refused here rather than looped on.
fn callees_first(bodies: &IdTable<MachineId, Body>) -> Checked<Vec<MachineId>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Unvisited,
        Open,
        Settled,
    }
    let count = bodies.len();
    let mut marks = vec![Mark::Unvisited; count];
    let mut order = Vec::with_capacity(count);
    for root in 1..=count as u64 {
        let root = MachineId::new(root).expect("positions start at 1");
        // Each frame is a machine and the callees it has left to visit.
        let mut stack: Vec<(MachineId, Vec<MachineId>)> = Vec::new();
        let enter = |machine: MachineId,
                     marks: &mut Vec<Mark>,
                     stack: &mut Vec<(MachineId, Vec<MachineId>)>|
         -> Checked<()> {
            let at = position(machine)
                .filter(|at| *at < count)
                .ok_or_else(|| missing(machine))?;
            match marks[at] {
                Mark::Settled => Ok(()),
                Mark::Open => Err(crate::refuse(
                    machine,
                    Construct::Machine,
                    crate::Unserved::Call("recursion"),
                )),
                Mark::Unvisited => {
                    marks[at] = Mark::Open;
                    let body =
                        terminal_check_tables::row(bodies, machine, machine, Construct::Machine)?;
                    let mut callees = Vec::new();
                    for statement in &body.statements {
                        statement_calls(statement, &mut callees);
                    }
                    callees.reverse();
                    stack.push((machine, callees));
                    Ok(())
                }
            }
        };
        enter(root, &mut marks, &mut stack)?;
        while let Some((machine, callees)) = stack.last_mut() {
            match callees.pop() {
                Some(callee) => enter(callee, &mut marks, &mut stack)?,
                None => {
                    let machine = *machine;
                    stack.pop();
                    if let Some(at) = position(machine) {
                        marks[at] = Mark::Settled;
                    }
                    order.push(machine);
                }
            }
        }
    }
    Ok(order)
}

fn statement_calls(statement: &Statement, calls: &mut Vec<MachineId>) {
    match statement {
        Statement::Bind { value, .. } | Statement::Evaluate(value) => {
            expression_calls(value, calls);
        }
        Statement::Arm { guard, target, .. } => {
            if let Some(guard) = guard {
                expression_calls(guard, calls);
            }
            match target {
                Target::Value(value) => expression_calls(value, calls),
                Target::Unit | Target::Crash(_) => {}
            }
        }
    }
}

fn expression_calls(expression: &Expression, calls: &mut Vec<MachineId>) {
    match &expression.kind {
        ExpressionKind::Integer(_) | ExpressionKind::Boolean(_) | ExpressionKind::Slot(_) => {}
        ExpressionKind::Not(operand) | ExpressionKind::BitwiseNot(operand) => {
            expression_calls(operand, calls);
        }
        ExpressionKind::Strict { left, right, .. }
        | ExpressionKind::ShortCircuit { left, right, .. } => {
            expression_calls(left, calls);
            expression_calls(right, calls);
        }
        ExpressionKind::Call { callee, arguments } => {
            for argument in arguments {
                expression_calls(argument, calls);
            }
            calls.push(*callee);
        }
    }
}

fn position(machine: MachineId) -> Option<usize> {
    usize::try_from(machine.get()).ok()?.checked_sub(1)
}

fn missing(machine: MachineId) -> terminal_check_tables::Refusal {
    crate::refuse(
        machine,
        Construct::Machine,
        crate::Unserved::Completion("outcome of a callee read before the callee"),
    )
}

struct Walk<'c> {
    machine: MachineId,
    computed: &'c [Option<Outcome>],
    /// Every cause a path of the body may crash with.
    causes: BTreeSet<CrashCause>,
}

impl Walk<'_> {
    fn callee(&self, callee: MachineId) -> Checked<&Outcome> {
        position(callee)
            .and_then(|position| self.computed.get(position))
            .and_then(Option::as_ref)
            .ok_or_else(|| missing(self.machine))
    }

    /// Whether the statements can fall through to a normal completion or
    /// complete normally in an arm. Statements after a no-successor exit are
    /// not reached; lowering refuses them.
    fn statements(&mut self, statements: &[Statement]) -> Checked<bool> {
        let mut normal = false;
        for statement in statements {
            match statement {
                Statement::Bind { value, .. } => {
                    if !self.expression(value)? {
                        return Ok(normal);
                    }
                }
                Statement::Evaluate(expression) => {
                    if !self.expression(expression)? {
                        return Ok(normal);
                    }
                }
                Statement::Arm { guard, target, .. } => {
                    if let Some(guard) = guard
                        && !self.expression(guard)?
                    {
                        return Ok(normal);
                    }
                    normal |= self.target(target)?;
                    if guard.is_none() {
                        return Ok(normal);
                    }
                }
            }
        }
        // The last statement fell through: a Unit machine completes here.
        Ok(true)
    }

    fn target(&mut self, target: &Target) -> Checked<bool> {
        match target {
            Target::Value(expression) => self.expression(expression),
            Target::Unit => Ok(true),
            Target::Crash(cause) => {
                self.causes.insert(*cause);
                Ok(false)
            }
        }
    }

    /// Records the crash causes of every call in `expression`; false when a
    /// call in it has no normal completion, so evaluation never finishes.
    fn expression(&mut self, expression: &Expression) -> Checked<bool> {
        match &expression.kind {
            ExpressionKind::Integer(_) | ExpressionKind::Boolean(_) | ExpressionKind::Slot(_) => {
                Ok(true)
            }
            ExpressionKind::Not(operand) | ExpressionKind::BitwiseNot(operand) => {
                self.expression(operand)
            }
            ExpressionKind::Strict { left, right, .. }
            | ExpressionKind::ShortCircuit { left, right, .. } => {
                Ok(self.expression(left)? && self.expression(right)?)
            }
            ExpressionKind::Call { callee, arguments } => {
                for argument in arguments {
                    if !self.expression(argument)? {
                        return Ok(false);
                    }
                }
                let outcome = self.callee(*callee)?;
                let completes = outcome.completes_normally;
                let causes: Vec<CrashCause> = outcome
                    .crash_routes
                    .iter()
                    .map(|bucket| bucket.cause)
                    .collect();
                self.causes.extend(causes);
                Ok(completes)
            }
        }
    }
}
