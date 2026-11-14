/*
The call family of lowering: invocation rows and their `CallRequirement`
obligations, calls as operations, no-normal calls as terminators, and the
crash continuations and `CrashCoverage` obligations of both.

A record argument is passed by value: its expression is evaluated to a root
nothing else names (`places::record`), the call takes that root whole and
owned, and the root's home is retired on the edge into the call's
continuation. A loan for a borrowed formal passes the lent place itself,
with the loan's access, and mints one `Loan` obligation; an exclusive loan
leaves the lent place with contents this machine cannot name once the call
returns. Scalar and record arguments are evaluated in one left-to-right
order (calls_and_outcomes.md: "Mixed scalar and structural operands share
that order; they do not form two separately evaluated lists") and then
travel in Terminal Psi's two lanes.
*/

use language_core::ReferenceAccess;
use semantic_vocabulary::{
    InvocationSite, MachineId, ObligationId, OperationId, PlaceId, ScalarTerm, ValueId,
};
use terminal_check_tables::edges::{Actual, Callee, Disposition, Fate, InvocationRow};
use terminal_check_tables::obligations::{CrashSite, ExecutionSite, Lender, ObligationKind};
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{OperationKind, StructuralAccess, StructuralArgument, Terminator};

use super::{Lowering, Value};
use crate::body::{Expression, ExpressionKind, ValueType};
use crate::canonical;
use crate::outcome::Outcome;
use crate::{SourceOrigin, Unserved, refuse};

/// A call's evaluated arguments.
struct Arguments {
    /// In authored formal order, as the invocation row holds them.
    actuals: Vec<Actual>,
    /// The scalar lane: the values a callee clause's formals stand for.
    scalars: Vec<ValueId>,
    /// The structural lane in formal order: each record argument's root,
    /// whole and owned, and each loan's place with the loan's access.
    structural: Vec<(PlaceId, Vec<super::places::Step>, StructuralAccess)>,
    /// The roots the call consumes, the record arguments' own: their homes
    /// are retired after it.
    consumed: Vec<PlaceId>,
    /// One per loan, in formal order: the place lent, its access and where
    /// the authority comes from.
    loans: Vec<(ExactPlaceId, ReferenceAccess, Lender)>,
    /// The erased lane: the term each `[erased]` formal stands for, in
    /// position order. Nothing was evaluated for it, and it is not among
    /// `actuals`: an invocation row's actual is a value or a place, and the
    /// checker reads these terms from the call itself.
    erased: Vec<ScalarTerm>,
}

impl Lowering<'_> {
    /// Evaluates a call's arguments left to right.
    fn arguments(
        &mut self,
        callee: semantic_vocabulary::TargetMachineCallable,
        arguments: &[Expression],
    ) -> Checked<Arguments> {
        let signature = self.signatures.get(callee, self.id)?;
        let mut evaluated = Arguments {
            actuals: Vec::with_capacity(arguments.len()),
            scalars: Vec::new(),
            structural: Vec::new(),
            consumed: Vec::new(),
            loans: Vec::new(),
            erased: Vec::new(),
        };
        for (position, argument) in arguments.iter().enumerate() {
            // contracts.md, "Explicit erased bindings": "Construction
            // supplies the erased term". The argument at an erased position
            // is that term; it is named, not run.
            if signature.is_erased(position) {
                let witness = self.witness(argument)?;
                evaluated.erased.push(witness.term());
                continue;
            }
            if let ExpressionKind::Borrow {
                access,
                place,
                home,
            } = &argument.kind
            {
                if signature.borrowed(position).map(|formal| formal.access) != Some(*access) {
                    return Err(self.internal("a loan for a formal that is not borrowed alike"));
                }
                let (lent, exact, lender) = self.lend(place, *access, home.as_deref())?;
                evaluated.actuals.push(Actual::Place(exact));
                evaluated.structural.push(lent);
                evaluated.loans.push((exact, *access, lender));
                continue;
            }
            match argument.result {
                Some(ValueType::Record(_, _, _)) => {
                    let root = self.record(argument)?;
                    evaluated
                        .actuals
                        .push(Actual::Place(self.exact_root(root)?));
                    evaluated
                        .structural
                        .push((root, Vec::new(), StructuralAccess::Owned));
                    evaluated.consumed.push(root);
                }
                Some(ValueType::Scalar(..) | ValueType::Rigid(_)) | None => {
                    let value = self.scalar(argument)?;
                    evaluated.actuals.push(Actual::Value(value));
                    evaluated.scalars.push(value);
                }
            }
        }
        Ok(evaluated)
    }

    fn structural_arguments(
        &mut self,
        captured: Vec<(PlaceId, Vec<super::places::Step>, StructuralAccess)>,
        site: ExecutionSite,
        source: SourceOrigin,
    ) -> Checked<Vec<StructuralArgument>> {
        captured
            .into_iter()
            .map(|(place, steps, access)| {
                Ok(StructuralArgument {
                    place,
                    path: self.storage_path(&steps, site, source)?,
                    access,
                })
            })
            .collect()
    }

    /// Mints one `Loan` per loan of a call, at the point before it, in
    /// formal order. loans.md: "Loan resource | Exact occurrence, owner
    /// lineage, captured place, access, parent lifetime, and restoration
    /// obligation." The occurrence is the call: each loan is formed for it
    /// and ends when it returns, so none is live on any edge.
    fn loans(
        &mut self,
        loans: &[(ExactPlaceId, ReferenceAccess, Lender)],
        before: ProgramPoint,
        source: SourceOrigin,
    ) {
        for (captured, access, lender) in loans {
            self.obligation(
                ObligationKind::Loan {
                    captured: *captured,
                    access: *access,
                    lender: *lender,
                },
                before,
                source,
            );
        }
    }

    /// What an exclusive loan's callee may have stored: anything beneath the
    /// lent place, from the call's normal continuation on. Nothing else the
    /// caller holds changes, so siblings keep what they held.
    fn returned_loans(
        &mut self,
        loans: &[(ExactPlaceId, ReferenceAccess, Lender)],
        operation: OperationId,
    ) -> Checked<()> {
        for (captured, access, _) in loans {
            if access.is_exclusive() {
                self.unknown_contents(*captured, operation)?;
            }
        }
        Ok(())
    }

    fn outcome(&self, callee: MachineId) -> Checked<&'_ Outcome> {
        terminal_check_tables::row(self.outcomes, callee, self.id, Construct::Machine)
    }

    /// Whether a call of `callable` can return to its caller: a machine
    /// when some path of its body does, a boundary requirement unless its
    /// declaration completes externally.
    fn completes_normally(
        &self,
        callable: semantic_vocabulary::TargetMachineCallable,
    ) -> Checked<bool> {
        Ok(match callable {
            semantic_vocabulary::TargetMachineCallable::Machine(callee) => {
                self.outcome(callee)?.completes_normally
            }
            semantic_vocabulary::TargetMachineCallable::Boundary(boundary) => {
                self.signatures.completion(boundary, self.id)?
                    == semantic_vocabulary::BoundaryCompletion::NormalReturn
            }
        })
    }

    /// The callee and arguments of a call in terminal position whose callee
    /// has no normal completion; such a call is the block's terminator.
    pub(super) fn no_normal<'e>(
        &self,
        expression: &'e Expression,
    ) -> Checked<Option<(semantic_vocabulary::TargetMachineCallable, &'e [Expression])>> {
        match &expression.kind {
            ExpressionKind::Call {
                callee, arguments, ..
            } if !self.completes_normally(*callee)? => Ok(Some((*callee, arguments))),
            ExpressionKind::Call { .. }
            | ExpressionKind::SavedSubject(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::ByteLiteral(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Slot(_)
            | ExpressionKind::Array { .. }
            | ExpressionKind::Index { .. }
            | ExpressionKind::Record { .. }
            | ExpressionKind::Case { .. }
            | ExpressionKind::Payload { .. }
            | ExpressionKind::IsCase { .. }
            | ExpressionKind::Match { .. }
            | ExpressionKind::Field { .. }
            | ExpressionKind::Strict { .. }
            | ExpressionKind::Not(_)
            | ExpressionKind::BitwiseNot(_)
            | ExpressionKind::Cast(_)
            | ExpressionKind::Requalify(_)
            | ExpressionKind::Establish { .. }
            | ExpressionKind::ShortCircuit { .. }
            | ExpressionKind::Borrow { .. } => Ok(None),
        }
    }

    /// A call with no normal completion, in terminal position: the
    /// arguments, then the invocation as the block's terminator, with the
    /// callee's routes substituted as its crash continuations and one
    /// `CrashCoverage` obligation per surviving cause at this site. One
    /// path serves both declaration classes, as `call` does for calls that
    /// return: a machine no path of which returns is `InvokeNoNormal`, and a
    /// boundary requirement that completes externally is
    /// `ExternalCompletion` (calls_and_outcomes.md, "External process exit":
    /// "a terminal transfer retaining the exact requirement and ordered
    /// arguments, with no normal successor, return postconditions, or
    /// implicit cleanup").
    ///
    /// Nothing of this machine runs after the transfer, so nothing is
    /// retired, restored or cleaned up on its edge (terminal-psi
    /// ownership.md: process exit "creates no return value, consumption
    /// receipt, cleanup successor, or post-exit frontier").
    pub(super) fn invoke_no_normal(
        &mut self,
        callee: semantic_vocabulary::TargetMachineCallable,
        arguments: &[Expression],
        source: SourceOrigin,
    ) -> Checked<()> {
        let signature = self.signatures.get(callee, self.id)?;
        let arguments = self.arguments(callee, arguments)?;
        let routes = match callee {
            semantic_vocabulary::TargetMachineCallable::Machine(callee) => {
                &self.outcome(callee)?.crash_routes[..]
            }
            semantic_vocabulary::TargetMachineCallable::Boundary(_) => signature
                .declared_crash_routes()
                .ok_or_else(|| self.internal("a boundary call has no published crash ceiling"))?,
        };
        let crash_continuations =
            canonical::substituted_routes(routes, &arguments.scalars, self.id)?;
        let site = self.current()?;
        let before = self.here()?;
        // state_contracts.md, "Terminal calls in transition arms": "A
        // caller-local loan carried into a terminal machine call closes at
        // the last use of that loan and of every loan derived from it, even
        // when that use is inside the callee's terminal chain". The callee
        // never returns, so each loan is formed here like any call's and is
        // handed to the transfer whole: this machine's edge consumes it and
        // no place of this machine is read again.
        let lent = self.tables.obligations.len();
        self.loans(&arguments.loans, before, source);
        let mut carried = Vec::with_capacity(arguments.loans.len());
        for raw in lent + 1..=self.tables.obligations.len() {
            let obligation = ObligationId::new(raw as u64)
                .ok_or_else(|| self.internal("a loan of a no-normal call has no identity"))?;
            carried.push(Disposition {
                obligation,
                fate: Fate::Consumed,
            });
        }
        let edge = self.no_successor_edge(carried, source)?;
        let invocation_site = match callee {
            semantic_vocabulary::TargetMachineCallable::Machine(_) => {
                InvocationSite::InvokeNoNormal(edge)
            }
            semantic_vocabulary::TargetMachineCallable::Boundary(_) => {
                InvocationSite::ExternalCompletion(edge)
            }
        };
        let requirements =
            self.invocation(invocation_site, callee, &arguments.actuals, before, source)?;
        for bucket in &crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Block(site),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
        let structural_arguments =
            self.structural_arguments(arguments.structural, ExecutionSite::Block(site), source)?;
        match callee {
            semantic_vocabulary::TargetMachineCallable::Machine(callee) => {
                self.terminate(Terminator::InvokeNoNormal {
                    edge,
                    static_reach_binding: None,
                    callee,
                    structural_arguments,
                    arguments: arguments.scalars,
                    erased_arguments: arguments.erased,
                    erased_proof_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: requirements,
                    crash_continuations,
                })
            }
            semantic_vocabulary::TargetMachineCallable::Boundary(boundary) => {
                let semantic_vocabulary::BoundaryCompletion::External(kind) =
                    self.signatures.completion(boundary, self.id)?
                else {
                    return Err(self.internal("a returning boundary call as a terminator"));
                };
                // process_exit.md: the canonical operation "permits no
                // source crash". Its declaration is the one place that could
                // say otherwise, and the terminator has no continuation
                // roster to hold a route. A caller that promises termination
                // owes nothing more for it: "Under its requirements and
                // admitted provider progress premises, the canonical
                // operation eventually ends the bound domain", and "An exit
                // leaf needs no further cycle decrease".
                if !crash_continuations.is_empty() {
                    return Err(refuse(
                        self.id,
                        Construct::Block(site),
                        Unserved::Call("external completion that declares a crash route"),
                    ));
                }
                if !arguments.erased.is_empty() {
                    return Err(self.internal("boundary call carries erased runtime operands"));
                }
                self.terminate(Terminator::ExternalCompletion {
                    edge,
                    boundary,
                    kind,
                    arguments: arguments.scalars,
                    structural_arguments,
                    requirement_obligations: requirements,
                    // calls_and_outcomes.md: "The frontier lower bound
                    // reports obligations definitely live at the site." The
                    // route mints no claim, so none is definitely live.
                    frontier_lower_bound: Vec::new(),
                })
            }
        }
    }

    /// The invocation row of one call and its `CallRequirement` obligations,
    /// one per callee `requires` clause, created at the point before it.
    fn invocation(
        &mut self,
        site: InvocationSite,
        callee: semantic_vocabulary::TargetMachineCallable,
        actuals: &[Actual],
        before: ProgramPoint,
        source: SourceOrigin,
    ) -> Checked<Vec<ObligationId>> {
        let clauses = self.signatures.get(callee, self.id)?.requires.len();
        let invocation = self.tables.invocations.push(InvocationRow {
            site,
            callee: Callee::Declared(callee),
            actuals: actuals.to_vec(),
            requirements: Vec::new(),
        });
        let mut requirements = Vec::with_capacity(clauses);
        for clause in 0..clauses {
            let clause = self.position(clause)?;
            requirements.push(self.obligation(
                ObligationKind::CallRequirement { invocation, clause },
                before,
                source,
            ));
        }
        let id = self.id;
        self.tables
            .invocations
            .get_mut(invocation)
            .ok_or(Refusal {
                reason: RefusalReason::ForeignIdentity,
                machine: id,
                concerns: Construct::Invocation(invocation),
                detail: String::from("invocation is past the end of this machine"),
            })?
            .requirements
            .clone_from(&requirements);
        Ok(requirements)
    }

    pub(super) fn call(
        &mut self,
        callable: semantic_vocabulary::TargetMachineCallable,
        arguments: &[Expression],
        designated: Option<crate::body::DesignatedOperands>,
        result: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Value> {
        // A call with no normal completion is a terminator (`invoke_no_normal`)
        // and has no value, so an expression cannot hold one.
        if !self.completes_normally(callable)? {
            return Err(refuse(
                self.id,
                self.current.map_or(Construct::Machine, Construct::Block),
                Unserved::Call("no-normal call inside an expression"),
            ));
        }
        if matches!(
            callable,
            semantic_vocabulary::TargetMachineCallable::Boundary(_)
        ) && self.signature.termination != crate::Termination::Silent
            && self.signatures.get(callable, self.id)?.termination == crate::Termination::Silent
        {
            return Err(refuse(
                self.id,
                Construct::Machine,
                Unserved::Call(
                    "a terminating caller needs the boundary's published termination guarantee",
                ),
            ));
        }
        if designated.is_some()
            && !matches!(
                callable,
                semantic_vocabulary::TargetMachineCallable::Boundary(_)
            )
        {
            return Err(self.internal("designated operands on a call that is no boundary call"));
        }
        let arguments = self.arguments(callable, arguments)?;
        let routes = match callable {
            semantic_vocabulary::TargetMachineCallable::Machine(callee) => {
                &self.outcome(callee)?.crash_routes[..]
            }
            semantic_vocabulary::TargetMachineCallable::Boundary(_) => self
                .signatures
                .get(callable, self.id)?
                .declared_crash_routes()
                .ok_or_else(|| self.internal("a boundary call has no published crash ceiling"))?,
        };
        // Both declarations owe the caller's same-cause coverage. A boundary
        // retains these routes on its declaration instead of a second encoded
        // continuation roster; the checker restates them at the actual call.
        let crash_continuations =
            canonical::substituted_routes(routes, &arguments.scalars, self.id)?;
        let before = self.here()?;
        let operation = self.operation_identity(source);
        let requirements = self.invocation(
            InvocationSite::Operation(operation),
            callable,
            &arguments.actuals,
            before,
            source,
        )?;
        self.loans(&arguments.loans, before, source);
        for bucket in &crash_continuations {
            self.obligation(
                ObligationKind::CrashCoverage {
                    site: CrashSite::Operation(operation),
                    cause: bucket.cause,
                },
                before,
                source,
            );
        }
        let structural_arguments = self.structural_arguments(
            arguments.structural,
            ExecutionSite::Operation(operation),
            source,
        )?;
        let erased_arguments = arguments.erased;
        let scalars = arguments.scalars;
        let kind = match callable {
            semantic_vocabulary::TargetMachineCallable::Boundary(boundary) => {
                if !erased_arguments.is_empty() {
                    return Err(self.internal("boundary call carries erased runtime operands"));
                }
                if let Some(designated) = designated {
                    self.designated.push((operation, designated));
                }
                OperationKind::BoundaryCall {
                    boundary,
                    arguments: scalars,
                    structural_arguments,
                    completion_receipts: Vec::new(),
                    requirement_obligations: requirements,
                }
            }
            semantic_vocabulary::TargetMachineCallable::Machine(callee) => match result {
                Some(ValueType::Scalar(..)) if structural_arguments.is_empty() => {
                    OperationKind::Call {
                        callee,
                        arguments: scalars,
                        erased_arguments,
                        erased_proof_arguments: Vec::new(),
                        requirement_obligations: requirements,
                        crash_continuations,
                    }
                }
                Some(ValueType::Scalar(..)) => OperationKind::CallStructuralScalar {
                    callee,
                    arguments: scalars,
                    erased_arguments,
                    erased_proof_arguments: Vec::new(),
                    structural_arguments,
                    claim_transfers: Vec::new(),
                    requirement_obligations: requirements,
                    crash_continuations,
                },
                None => OperationKind::CallUnit {
                    callee,
                    arguments: scalars,
                    erased_arguments,
                    erased_proof_arguments: Vec::new(),
                    structural_arguments,
                    claim_transfers: Vec::new(),
                    requirement_obligations: requirements,
                    crash_continuations,
                },
                Some(ValueType::Record(_, _, _)) => {
                    OperationKind::CallStructuralWithScalarArguments {
                        callee,
                        arguments: scalars,
                        erased_arguments,
                        erased_proof_arguments: Vec::new(),
                        structural_arguments,
                        claim_transfers: Vec::new(),
                        requirement_obligations: requirements,
                        crash_continuations,
                        selected_evidence: Vec::new(),
                    }
                }
                Some(ValueType::Rigid(_)) => {
                    return Err(self.internal("a call returns a value of a rigid type"));
                }
            },
        };
        // The actual result keeps one constructor across both declaration
        // classes; storage, unknown writes and loan restoration are shared.
        let value = match result {
            Some(ValueType::Scalar(scalar, _)) => self.emit(operation, Some(scalar), kind)?,
            None => self.emit(operation, None, kind)?,
            Some(value_type @ ValueType::Record(record, _, _)) => {
                let (root, exact) = self.produce(operation, record, kind)?;
                self.unknown_contents(exact, operation)?;
                self.initial_leaves(exact, value_type)?;
                Value::Record(root)
            }
            Some(ValueType::Rigid(_)) => {
                return Err(self.internal("a call returns a value of a rigid type"));
            }
        };
        self.returned_loans(&arguments.loans, operation)?;
        if !arguments.consumed.is_empty() {
            self.retire_homes(&arguments.consumed, source)?;
        }
        Ok(value)
    }
}
