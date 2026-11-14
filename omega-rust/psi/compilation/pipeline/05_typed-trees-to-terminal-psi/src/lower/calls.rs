/*
The call family of lowering: invocation rows and their `CallRequirement`
obligations, calls as operations, no-normal calls as terminators, and the
crash continuations and `CrashCoverage` obligations of both.

A record argument is passed by value: its expression is evaluated to a root
nothing else names (`places::record`), the call takes that root whole and
owned, and the root's home is retired on the edge into the call's
continuation. A loan for a borrowed formal passes the lent place itself,
with the loan's access, and mints one `Loan` obligation; an exclusive loan
initially invalidates the lent place as a whole. After all bodies are lowered,
their published write frames retain facts about provably untouched fields.
Scalar and record arguments are evaluated in one left-to-right
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

use super::{Evaluation, Lowering, Reached, Value};
use crate::body::{Expression, ExpressionKind, ValueType};
use crate::canonical;
use crate::outcome::Outcome;
use crate::{SourceOrigin, Unserved, refuse};

/// A call need not replace every field it borrows. Once every body has its
/// canonical operations, publish the inferred frames and use those same
/// effects in the existing write log. This changes which old contents are
/// invalidated; it never supplies a new field value.
pub(crate) fn publish_write_frames(
    diagnostic: MachineId,
    module: &mut terminal_psi::TerminalModule,
    tables: &mut terminal_check_tables::CheckTables,
) -> Checked<()> {
    use semantic_vocabulary::CanonicalStructuralPathSegment;
    use terminal_check_tables::bindings::{self, Content, Segment, WriteRow};

    let problem = |machine, concerns, detail: String| Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine,
        concerns,
        detail,
    };
    let frames = terminal_semantics::borrowed_storage::inferred_write_frames(module)
        .map_err(|detail| problem(diagnostic, Construct::Machine, detail))?;
    for machine in &mut module.machines {
        machine.contract.published_writes = frames
            .get(machine.id)
            .ok_or_else(|| {
                problem(
                    machine.id,
                    Construct::Machine,
                    "write frame has no owner".to_owned(),
                )
            })?
            .clone();
    }
    for machine in &module.machines {
        let id = machine.id;
        let tables = tables.machines.get_mut(id).ok_or_else(|| {
            problem(
                id,
                Construct::Machine,
                "write frame has no checking tables".to_owned(),
            )
        })?;
        // Operation identities can be emitted in a different order from
        // allocation. Index only actual operations, rather than allocating
        // scratch up to the largest possible identity.
        let mut writes: Vec<_> = machine
            .operations()
            .map(|operation| (operation.id, Vec::new()))
            .collect();
        writes.sort_unstable_by_key(|(operation, _)| *operation);
        for (_, row) in tables.writes.iter() {
            let ProgramPoint::AfterOperation(operation) = row.at else {
                return Err(problem(
                    id,
                    Construct::Place(row.place),
                    "a lowered storage write is not at its operation".to_owned(),
                ));
            };
            let position = writes
                .binary_search_by_key(&operation, |(operation, _)| *operation)
                .map_err(|_| {
                    problem(
                        id,
                        Construct::Operation(operation),
                        "a write names a missing operation".to_owned(),
                    )
                })?;
            writes[position].1.push(*row);
        }
        let mut revised = Vec::with_capacity(tables.writes.len());
        for operation in machine.operations() {
            let position = writes
                .binary_search_by_key(&operation.id, |(operation, _)| *operation)
                .map_err(|_| {
                    problem(
                        id,
                        Construct::Operation(operation.id),
                        "write-frame operation disappeared".to_owned(),
                    )
                })?;
            let old = &writes[position].1;
            let Some(regions) = terminal_semantics::borrowed_storage::call_write_regions(
                module, machine, operation,
            )
            .map_err(|detail| problem(id, Construct::Operation(operation.id), detail))?
            else {
                revised.extend_from_slice(old);
                continue;
            };
            if old.iter().any(|row| row.content != Content::Overwritten) {
                return Err(problem(
                    id,
                    Construct::Operation(operation.id),
                    "a call write has unexpected scalar or move contents".to_owned(),
                ));
            }
            let at = ProgramPoint::AfterOperation(operation.id);
            if let Some(result) = operation.result.structural() {
                let place = tables.roots.get(result.place).copied().ok_or_else(|| {
                    problem(
                        id,
                        Construct::Operation(operation.id),
                        "a call result has no exact root".to_owned(),
                    )
                })?;
                revised.push(WriteRow {
                    place,
                    at,
                    content: Content::Overwritten,
                });
            }
            for region in regions {
                let root = region.root.ok_or_else(|| {
                    problem(
                        id,
                        Construct::Operation(operation.id),
                        "a known write frame has no storage root".to_owned(),
                    )
                })?;
                let structural_type = machine.structural_place_type(root).ok_or_else(|| {
                    problem(
                        id,
                        Construct::Operation(operation.id),
                        "a written root has no structural type".to_owned(),
                    )
                })?;
                let path = terminal_semantics::canonical_static_projection(
                    module.structural_types.iter(),
                    structural_type,
                    &region.projection.path,
                )
                .map(|(path, _)| path)
                .or_else(|| {
                    terminal_semantics::canonical_primitive_read_path(
                        module.structural_types.iter(),
                        structural_type,
                        &region.projection.path,
                    )
                })
                .ok_or_else(|| {
                    problem(
                        id,
                        Construct::Operation(operation.id),
                        "a written region has no exact inline storage path".to_owned(),
                    )
                })?;
                let mut place = tables.roots.get(root).copied().ok_or_else(|| {
                    problem(
                        id,
                        Construct::Operation(operation.id),
                        "a written region has no exact root".to_owned(),
                    )
                })?;
                for segment in path {
                    let segment = match segment {
                        CanonicalStructuralPathSegment::Field(field) => Segment::Field(field),
                        CanonicalStructuralPathSegment::FixedIndex(index) => {
                            Segment::FixedIndex(index)
                        }
                        CanonicalStructuralPathSegment::Case(case) => Segment::Case(case),
                        CanonicalStructuralPathSegment::Referent => {
                            return Err(problem(
                                id,
                                Construct::Operation(operation.id),
                                "a write frame crosses an unserved referent".to_owned(),
                            ));
                        }
                    };
                    place = bindings::intern_projection(id, &mut tables.places, place, segment)?;
                }
                revised.push(WriteRow {
                    place,
                    at,
                    content: Content::Overwritten,
                });
            }
        }
        for (_, row) in tables.writes.iter() {
            tables
                .places
                .get_mut(row.place)
                .ok_or_else(|| {
                    problem(
                        id,
                        Construct::Place(row.place),
                        "a storage write has no exact place".to_owned(),
                    )
                })?
                .writes
                .clear();
        }
        tables.writes = semantic_vocabulary::IdTable::default();
        for row in revised {
            bindings::record_write(id, &mut tables.places, &mut tables.writes, row)?;
        }
    }
    Ok(())
}

/// Staged storage ends after the call, whether its value was transferred or
/// its captured descriptor was lent. The argument lanes keep those distinct.
enum ArgumentHome {
    Transferred(PlaceId),
    BorrowedDescriptor(PlaceId),
}

impl ArgumentHome {
    fn place(&self) -> PlaceId {
        match self {
            Self::Transferred(place) | Self::BorrowedDescriptor(place) => *place,
        }
    }
}

/// A call's evaluated arguments.
struct Arguments {
    /// In authored formal order, as the invocation row holds them.
    actuals: Vec<Actual>,
    /// The scalar lane: the values a callee clause's formals stand for.
    scalars: Vec<ValueId>,
    /// The structural lane in formal order: each record argument's root,
    /// whole and owned, and each loan's place with the loan's access.
    structural: Vec<(PlaceId, Vec<super::places::Step>, StructuralAccess)>,
    /// The homes created while staging owned values and shared descriptors.
    /// One normal continuation retires both, after all call loans end.
    homes: Vec<ArgumentHome>,
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
    ) -> Evaluation<Arguments> {
        let signature = self.signatures.get(callee, self.id)?;
        let mut evaluated = Arguments {
            actuals: Vec::with_capacity(arguments.len()),
            scalars: Vec::new(),
            structural: Vec::new(),
            homes: Vec::new(),
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
                let lent = continuing!(self.lend(place, *access, home.as_deref()));
                evaluated.actuals.push(Actual::Place(lent.place));
                evaluated.structural.push(lent.argument);
                evaluated.loans.push((lent.place, *access, lent.lender));
                evaluated
                    .homes
                    .extend(lent.temporary.map(ArgumentHome::BorrowedDescriptor));
                continue;
            }
            match argument.result {
                Some(ValueType::Record(_, _, _)) => {
                    let root = continuing!(self.record(argument));
                    evaluated
                        .actuals
                        .push(Actual::Place(self.exact_root(root)?));
                    evaluated
                        .structural
                        .push((root, Vec::new(), StructuralAccess::Owned));
                    evaluated.homes.push(ArgumentHome::Transferred(root));
                }
                Some(ValueType::Scalar(..) | ValueType::Rigid(_)) | None => {
                    let value = continuing!(self.scalar(argument));
                    evaluated.actuals.push(Actual::Value(value));
                    evaluated.scalars.push(value);
                }
            }
        }
        Ok(Reached::Value(evaluated))
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

    /// A call with no normal completion, wherever it is written: the
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
    fn invoke_no_normal(
        &mut self,
        callee: semantic_vocabulary::TargetMachineCallable,
        arguments: &[Expression],
        source: SourceOrigin,
    ) -> Evaluation<()> {
        let signature = self.signatures.get(callee, self.id)?;
        let arguments = continuing!(self.arguments(callee, arguments));
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
            semantic_vocabulary::TargetMachineCallable::Machine(callee) => self
                .terminate(Terminator::InvokeNoNormal {
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
                .map(Reached::Value),
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
                .map(Reached::Value)
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
    ) -> Evaluation<Value> {
        // A call with no normal completion is a terminator
        // (`invoke_no_normal`) and has no value: the path it lies on ends at
        // it, and so does the evaluation of whatever holds it
        // (the explicit Ended evaluation outcome).
        if !self.completes_normally(callable)? {
            if designated.is_some() {
                return Err(self.internal("designated operands on a call that never returns"));
            }
            continuing!(self.invoke_no_normal(callable, arguments, source));
            return Ok(Reached::Ended(source));
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
        let arguments = continuing!(self.arguments(callable, arguments));
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
        if !arguments.homes.is_empty() {
            let homes: Vec<_> = arguments.homes.iter().map(ArgumentHome::place).collect();
            self.retire_homes(&homes, source)?;
        }
        Ok(Reached::Value(value))
    }
}
