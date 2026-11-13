/*
How each machine may leave: whether some path completes normally, and the
crash routes it publishes. Both are read from structure, as a fixed point
over the call graph, once every body is read. Nothing here proves a route possible or impossible:
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

/// The outcomes of every machine, indexed by `MachineId`.
///
/// Machines may call each other in a cycle, so the table is the least fixed
/// point of one rule: every machine starts as never completing and reaching
/// no crash, each body is walked against the table as it stands, and the
/// walk is repeated until no outcome changes. Both terms only grow (a
/// machine found to complete stays so, a cause found stays), so the walk
/// ends. A machine whose only paths call it again never completes normally,
/// which is what `wiki/spec/terminal-psi/control_flow.md` says of it: "An
/// infinite path has no normal return".
///
/// A machine in `requirements` is a requirement's contract and has no body:
/// its member may return, and it may crash only as its authored ceiling says.
pub(crate) fn outcomes(
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    requirements: &[MachineId],
) -> Checked<IdTable<MachineId, Outcome>> {
    let mut walk = Walk {
        signatures,
        requirements,
        reached: bodies.map(|_, _| Reached::default()),
        causes: BTreeSet::new(),
    };
    loop {
        let mut changed = false;
        for (machine, body) in bodies.iter() {
            walk.causes.clear();
            let completes_normally = if requirements.contains(&machine) {
                true
            } else {
                walk.statements(machine, &body.statements)?
            };
            let found = Reached {
                completes_normally,
                causes: std::mem::take(&mut walk.causes),
            };
            let slot = walk.reached.get_mut(machine).ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Call("callee outside the module"),
                )
            })?;
            if *slot != found {
                *slot = found;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut table = IdTable::default();
    for (machine, reached) in walk.reached.iter() {
        table.push(Outcome {
            completes_normally: reached.completes_normally,
            crash_routes: walk.published(machine, &reached.causes)?,
        });
    }
    Ok(table)
}

/// What structure reaches in one body, against the table as it stands.
#[derive(Clone, Default, PartialEq)]
struct Reached {
    completes_normally: bool,
    /// Every cause a path of the body may crash with, from a `crash`
    /// statement or from a callee that may.
    causes: BTreeSet<CrashCause>,
}

struct Walk<'b> {
    signatures: &'b IdTable<MachineId, Signature>,
    /// Machines that are a requirement's contract and have no body.
    requirements: &'b [MachineId],
    reached: IdTable<MachineId, Reached>,
    /// Every cause a path of the body being walked may crash with.
    causes: BTreeSet<CrashCause>,
}

impl Walk<'_> {
    /// The routes `machine` publishes, given the causes its body reaches.
    fn published(
        &self,
        machine: MachineId,
        causes: &BTreeSet<CrashCause>,
    ) -> Checked<Vec<CrashRouteBucket>> {
        let signature =
            terminal_check_tables::row(self.signatures, machine, machine, Construct::Machine)?;
        Ok(match &signature.crashes {
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
        })
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
                // An erased binding evaluates nothing, so it reaches no call.
                Statement::Declare { .. } | Statement::Witness { .. } => {}
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
            | ExpressionKind::Cast(operand)
            | ExpressionKind::Requalify(operand) => self.expression(machine, operand),
            ExpressionKind::Field { base, .. }
            | ExpressionKind::Payload { base, .. }
            | ExpressionKind::IsCase { subject: base, .. }
            | ExpressionKind::Borrow { place: base, .. } => self.expression(machine, base),
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
                // A caller sees the causes the callee publishes, as the
                // table stands: its ceiling when it authors one, what its
                // body reaches when the routes are inferred, none under
                // `crashes none` or a deferring requirement.
                let reached = self.reached.get(*callee).ok_or_else(|| {
                    refuse(
                        machine,
                        Construct::Machine,
                        Unserved::Call("callee outside the module"),
                    )
                })?;
                let completes = reached.completes_normally;
                let causes = reached.causes.clone();
                let published = self.published(*callee, &causes)?;
                self.causes
                    .extend(published.iter().map(|bucket| bucket.cause));
                Ok(completes)
            }
        }
    }
}
