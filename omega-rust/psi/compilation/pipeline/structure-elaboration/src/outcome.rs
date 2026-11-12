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
checked bodies may omit any of the three to request inference"); an authored
`crashes none` publishes none and refuses a body with a crash route ("promises
the empty value and rejects any body or selection that violates it"). Inference
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
use crate::{Unserved, refuse};

/// What a machine publishes about how it may leave.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Some path of the body reaches a normal return. When none does, a call
    /// of the machine has no normal successor (`Terminator::InvokeNoNormal`).
    pub completes_normally: bool,
    /// The published crash routes in canonical form, over the formals.
    pub crash_routes: Vec<CrashRouteBucket>,
}

/// The outcomes of every machine, indexed by `MachineId`. A callee's outcome
/// is computed before its caller's, on first use; a machine reached again
/// while its own outcome is being computed lies on a call cycle and is
/// refused as recursion, so bodies passed to `lower_bodies` directly get the
/// refusal the reader gives.
pub(crate) fn outcomes(
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    requirements: &[MachineId],
) -> Checked<IdTable<MachineId, Outcome>> {
    let mut walk = Walk {
        bodies,
        signatures,
        requirements,
        computed: bodies.map(|_, _| State::Unvisited),
        causes: BTreeSet::new(),
    };
    let mut table = IdTable::default();
    for machine in bodies.keys() {
        table.push(walk.outcome(machine, machine)?);
    }
    Ok(table)
}

#[derive(Clone)]
enum State {
    Unvisited,
    /// On the current chain of callers: reaching it again is a cycle.
    Visiting,
    Done(Outcome),
}

struct Walk<'b> {
    bodies: &'b IdTable<MachineId, Body>,
    signatures: &'b IdTable<MachineId, Signature>,
    /// Machines that are a requirement's contract and have no body.
    requirements: &'b [MachineId],
    computed: IdTable<MachineId, State>,
    /// Every cause a path of the body being walked may crash with.
    causes: BTreeSet<CrashCause>,
}

impl Walk<'_> {
    /// `machine`'s outcome, computing it on first use. `caller` is the
    /// machine whose body named it, for the refusal.
    fn outcome(&mut self, machine: MachineId, caller: MachineId) -> Checked<Outcome> {
        let state = self.computed.get(machine).ok_or_else(|| {
            refuse(
                caller,
                Construct::Machine,
                Unserved::Call("callee outside the module"),
            )
        })?;
        match state {
            State::Done(outcome) => return Ok(outcome.clone()),
            State::Visiting => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Call("recursion"),
                ));
            }
            State::Unvisited => {}
        }
        let (bodies, signatures) = (self.bodies, self.signatures);
        let body = terminal_check_tables::row(bodies, machine, machine, Construct::Machine)?;
        let signature =
            terminal_check_tables::row(signatures, machine, machine, Construct::Machine)?;
        self.set(machine, State::Visiting)?;
        // A callee's walk runs inside this one, so the caller's causes are
        // set aside while it does.
        let callers_causes = std::mem::take(&mut self.causes);
        // A requirement has no body to walk. Its member may return, and it
        // may crash only as its authored ceiling says.
        let completes_normally = if self.requirements.contains(&machine) {
            Ok(true)
        } else {
            self.statements(machine, &body.statements)
        };
        let causes = std::mem::replace(&mut self.causes, callers_causes);
        let completes_normally = completes_normally?;
        let crash_routes = match &signature.crashes {
            CrashCeiling::Authored(routes) => routes.clone(),
            // A requirement that defers its crash routes to the selected
            // member publishes none in the generic form (`lower_generic`).
            CrashCeiling::Inferred if self.requirements.contains(&machine) => Vec::new(),
            CrashCeiling::Inferred => {
                canonical::routes(causes.iter().map(|cause| (*cause, CrashRouteGuard::Truth)))
            }
            // The author promised no crash route, so nothing is inferred and
            // none is published. Each crash site the body reaches then owes
            // coverage by no route, which holds only where the site is
            // unreachable: lowered-discharge proves that from what is known
            // there (`requires d != 0` against a `d == 0` guard) or rejects.
            CrashCeiling::DeclaredNone => Vec::new(),
        };
        let outcome = Outcome {
            completes_normally,
            crash_routes,
        };
        self.set(machine, State::Done(outcome.clone()))?;
        Ok(outcome)
    }

    fn set(&mut self, machine: MachineId, state: State) -> Checked<()> {
        let slot = self.computed.get_mut(machine).ok_or_else(|| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Call("callee outside the module"),
            )
        })?;
        *slot = state;
        Ok(())
    }

    /// Whether the statements can fall through to a normal completion or
    /// complete normally in an arm. Statements after a no-successor exit are
    /// not reached; lowering refuses them.
    fn statements(&mut self, machine: MachineId, statements: &[Statement]) -> Checked<bool> {
        let mut normal = false;
        for statement in statements {
            match statement {
                Statement::Bind { value, .. } => {
                    if !self.expression(machine, value)? {
                        return Ok(normal);
                    }
                }
                Statement::Evaluate(expression)
                | Statement::Store {
                    value: expression, ..
                } => {
                    if !self.expression(machine, expression)? {
                        return Ok(normal);
                    }
                }
                Statement::Declare { .. } => {}
                // Exactly one arm runs and each completes the machine, so
                // the dispatch completes normally when some arm does and
                // nothing follows it.
                Statement::Dispatch {
                    subject,
                    arms,
                    default,
                    ..
                } => {
                    if !self.expression(machine, subject)? {
                        return Ok(normal);
                    }
                    for (_, target, _) in arms {
                        normal |= self.target(machine, target)?;
                    }
                    if let Some((target, _)) = default {
                        normal |= self.target(machine, target)?;
                    }
                    return Ok(normal);
                }
                Statement::Arm { guard, target, .. } => {
                    if let Some(guard) = guard
                        && !self.expression(machine, guard)?
                    {
                        return Ok(normal);
                    }
                    normal |= self.target(machine, target)?;
                    if guard.is_none() {
                        return Ok(normal);
                    }
                }
            }
        }
        // The last statement fell through: a Unit machine completes here.
        Ok(true)
    }

    fn target(&mut self, machine: MachineId, target: &Target) -> Checked<bool> {
        match target {
            Target::Value(expression) => self.expression(machine, expression),
            Target::Unit => Ok(true),
            Target::Crash(cause) => {
                self.causes.insert(*cause);
                Ok(false)
            }
        }
    }

    /// Records the crash causes of every call in `expression`; false when a
    /// call in it has no normal completion, so evaluation never finishes.
    fn expression(&mut self, machine: MachineId, expression: &Expression) -> Checked<bool> {
        match &expression.kind {
            ExpressionKind::Integer(_) | ExpressionKind::Boolean(_) | ExpressionKind::Slot(_) => {
                Ok(true)
            }
            ExpressionKind::Not(operand)
            | ExpressionKind::BitwiseNot(operand)
            | ExpressionKind::Cast(operand) => self.expression(machine, operand),
            ExpressionKind::Field { base, .. }
            | ExpressionKind::Payload { base, .. }
            | ExpressionKind::IsCase { subject: base, .. } => self.expression(machine, base),
            // One arm runs: the match completes when its subject does and
            // some arm does, and every arm's causes are reachable.
            ExpressionKind::Match {
                subject,
                arms,
                default,
            } => {
                if !self.expression(machine, subject)? {
                    return Ok(false);
                }
                let mut completes = false;
                for (_, arm) in arms {
                    completes |= self.expression(machine, arm)?;
                }
                if let Some(arm) = default {
                    completes |= self.expression(machine, arm)?;
                }
                Ok(completes)
            }
            // Operands in authored order; the establishment itself cannot
            // crash or fail to complete.
            ExpressionKind::Record { fields } | ExpressionKind::Case { fields, .. } => {
                for (_, operand) in fields {
                    if !self.expression(machine, operand)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ExpressionKind::Strict { left, right, .. } => {
                Ok(self.expression(machine, left)? && self.expression(machine, right)?)
            }
            // The right operand may be skipped, so the expression completes
            // whenever its left operand does; the right operand's causes are
            // still reachable.
            ExpressionKind::ShortCircuit { left, right, .. } => {
                if !self.expression(machine, left)? {
                    return Ok(false);
                }
                self.expression(machine, right)?;
                Ok(true)
            }
            ExpressionKind::Call { callee, arguments } => {
                for argument in arguments {
                    if !self.expression(machine, argument)? {
                        return Ok(false);
                    }
                }
                let outcome = self.outcome(*callee, machine)?;
                self.causes
                    .extend(outcome.crash_routes.iter().map(|bucket| bucket.cause));
                Ok(outcome.completes_normally)
            }
        }
    }
}
