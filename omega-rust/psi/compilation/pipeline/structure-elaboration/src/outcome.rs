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

Published service reach follows the same page: "published reach = authored
ceiling, when a clause is present; published reach = derived body reach,
otherwise", where "derived body reach = normalize(contributions from calls)"
and "Reach derivation computes conservative fixed points over recursive
checked-call components." A call of a machine contributes what that machine
publishes; a boundary call contributes "its requirement's explicit reach
ceiling", never the called trait's own identity. Whether a derived reach fits
a written ceiling is the checker's judgment, made on the published rows; a
row is only published here.

Published synchronous entries (`invokes`) are the same derivation on a second
axis (`Axis`): "the callee's published `invokes` joins the caller's, as its
reach does". A written clause is the published ceiling, an omitted one on a
checked body publishes what its calls contribute, and a boundary call
contributes its requirement's row, where omission is "No synchronous entry".
A call of a requirement is "not itself an entry", so nothing in the subset
this route reads originates an entry: a row is non-empty only where a clause
was written.
*/

use std::collections::BTreeSet;

use semantic_vocabulary::{CrashCause, IdTable, MachineId, ServiceId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard};

use crate::body::{
    Axis, Body, Ceiling, CrashCeiling, Expression, ExpressionKind, Signatures, Statement, Target,
};
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
    /// The service reach a caller may rely on, strictly increasing: the
    /// written ceiling, or what the body's calls contribute.
    pub service_reach: Vec<ServiceId>,
    /// The synchronous entries a caller is charged, strictly increasing:
    /// the written `invokes` ceiling, or what the body's calls contribute.
    pub synchronous_entries: Vec<ServiceId>,
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
    signatures: Signatures<'_>,
    requirements: &[MachineId],
) -> Checked<IdTable<MachineId, Outcome>> {
    let mut walk = Walk {
        signatures,
        requirements,
        reached: bodies.map(|_, _| Reached::default()),
        causes: BTreeSet::new(),
        contributed: Default::default(),
    };
    loop {
        let mut changed = false;
        for (machine, body) in bodies.iter() {
            walk.causes.clear();
            walk.contributed = Default::default();
            let completes_normally = if requirements.contains(&machine) {
                true
            } else {
                walk.statements(machine, &body.statements)?
            };
            let found = Reached {
                completes_normally,
                causes: std::mem::take(&mut walk.causes),
                contributed: std::mem::take(&mut walk.contributed).map(|mut row| {
                    row.services.sort_unstable();
                    row.services.dedup();
                    row
                }),
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
            service_reach: walk.published_row(machine, Axis::Reach, reached, true)?,
            synchronous_entries: walk.published_row(machine, Axis::Entries, reached, true)?,
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
    /// What the body's calls contribute on each ceiling axis, by `Axis`.
    contributed: [Contributed; 2],
}

/// What a body's calls contribute on one ceiling axis.
#[derive(Clone, Default, PartialEq)]
struct Contributed {
    /// Every service a call contributes; strictly increasing once the
    /// body's walk ends.
    services: Vec<ServiceId>,
    /// Some call in the body is of a requirement that omits the clause and
    /// defers the row to its selected member, or of a machine that
    /// publishes such a call.
    open: bool,
}

struct Walk<'b> {
    signatures: Signatures<'b>,
    /// Machines that are a requirement's contract and have no body.
    requirements: &'b [MachineId],
    reached: IdTable<MachineId, Reached>,
    /// Every cause a path of the body being walked may crash with.
    causes: BTreeSet<CrashCause>,
    /// What the calls of the body being walked contribute on each axis, in
    /// the order met; sorted when the body's walk ends.
    contributed: [Contributed; 2],
}

impl Walk<'_> {
    /// The routes `machine` publishes, given the causes its body reaches.
    fn published(
        &self,
        machine: MachineId,
        causes: &BTreeSet<CrashCause>,
    ) -> Checked<Vec<CrashRouteBucket>> {
        let signature = self.signatures.get(machine.into(), machine)?;
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

    /// The row `machine` publishes on `axis`, given what its body's calls
    /// contribute: one rule for reach and for synchronous entries.
    ///
    /// A requirement that omits `reaches` is open: effects.md, "defer to the
    /// selected implementation's published reach". The generic form keeps
    /// no row dependency, so it publishes nothing for one, as it does for a
    /// deferred crash route. A written ceiling over such a call cannot be
    /// compared with a contribution nobody knows yet ("The template must
    /// remain valid for every admitted selection"), so that machine is
    /// refused when its row is published (`judged`).
    fn published_row(
        &self,
        machine: MachineId,
        axis: Axis,
        reached: &Reached,
        judged: bool,
    ) -> Checked<Vec<ServiceId>> {
        let signature = self.signatures.get(machine.into(), machine)?;
        let contributed = &reached.contributed[axis as usize];
        Ok(match signature.ceiling(axis) {
            Ceiling::Authored(_) if judged && contributed.open => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Machine(match axis {
                        Axis::Reach => "reach ceiling over a requirement that defers its reach",
                        Axis::Entries => {
                            "invokes ceiling over a requirement that defers its entries"
                        }
                    }),
                ));
            }
            Ceiling::Authored(ceiling) => ceiling.clone(),
            Ceiling::Inferred => contributed.services.clone(),
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
                Statement::Store {
                    destination, value, ..
                } => {
                    if !self.expression(machine, destination)? {
                        return Ok(normal);
                    }
                    if !self.expression(machine, value)? {
                        return Ok(normal);
                    }
                }
                Statement::Evaluate(expression) => {
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
            ExpressionKind::SavedSubject(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::ByteLiteral(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Slot(_) => Ok(true),
            ExpressionKind::Not(operand)
            | ExpressionKind::BitwiseNot(operand)
            | ExpressionKind::Cast(operand)
            | ExpressionKind::Requalify(operand)
            | ExpressionKind::Establish { operand, .. } => self.expression(machine, operand),
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
                ..
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
            ExpressionKind::Array { elements } => {
                for element in elements {
                    if !self.expression(machine, element)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ExpressionKind::Index { base, index } => {
                Ok(self.expression(machine, base)? && self.expression(machine, index)?)
            }
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
            ExpressionKind::Call {
                callee, arguments, ..
            } => {
                for argument in arguments {
                    if !self.expression(machine, argument)? {
                        return Ok(false);
                    }
                }
                let semantic_vocabulary::TargetMachineCallable::Machine(callee) = callee else {
                    let signature = self.signatures.get(*callee, machine)?;
                    let routes = signature.declared_crash_routes().ok_or_else(|| {
                        refuse(
                            machine,
                            Construct::Machine,
                            Unserved::Call("boundary call has no published crash contract"),
                        )
                    })?;
                    self.causes.extend(routes.iter().map(|route| route.cause));
                    // "A boundary call contributes its requirement's explicit
                    // reach ceiling, with boundary-parent closure", which the
                    // reader applied when it read the clause. The call "is
                    // not itself an entry", and "the callee's published
                    // `invokes` joins the caller's, as its reach does".
                    for axis in Axis::ALL {
                        match signature.ceiling(axis) {
                            Ceiling::Authored(ceiling) => {
                                self.contributed[axis as usize]
                                    .services
                                    .extend(ceiling.iter().copied());
                            }
                            Ceiling::Inferred => {
                                return Err(refuse(
                                    machine,
                                    Construct::Machine,
                                    Unserved::Call(match axis {
                                        Axis::Reach => {
                                            "boundary call has no declared reach ceiling"
                                        }
                                        Axis::Entries => {
                                            "boundary call has no declared entry ceiling"
                                        }
                                    }),
                                ));
                            }
                        }
                    }
                    // process_exit.md: the canonical operation "produces no
                    // normal result or continuation", so evaluation never
                    // finishes past it, exactly as past a call of a machine
                    // no path of which returns.
                    let semantic_vocabulary::TargetMachineCallable::Boundary(boundary) = *callee
                    else {
                        return Ok(true);
                    };
                    return Ok(self.signatures.completion(boundary, machine)?
                        == semantic_vocabulary::BoundaryCompletion::NormalReturn);
                };
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
                let signature = self.signatures.get((*callee).into(), machine)?;
                let mut joins = Vec::with_capacity(Axis::ALL.len());
                for axis in Axis::ALL {
                    let row = self.published_row(*callee, axis, reached, false)?;
                    // An open requirement is itself the open call; a machine
                    // with a written ceiling closes whatever its body calls.
                    let open = match signature.ceiling(axis) {
                        Ceiling::Authored(_) => false,
                        Ceiling::Inferred => {
                            reached.contributed[axis as usize].open
                                || self.requirements.contains(callee)
                        }
                    };
                    joins.push((axis, row, open));
                }
                let published = self.published(*callee, &causes)?;
                self.causes
                    .extend(published.iter().map(|bucket| bucket.cause));
                for (axis, row, open) in joins {
                    let contributed = &mut self.contributed[axis as usize];
                    contributed.services.extend(row);
                    contributed.open |= open;
                }
                Ok(completes)
            }
        }
    }
}
