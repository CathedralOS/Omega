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
    CallableContractId, InvocationSite, MachineId, ObligationId, OperationId, PlaceId, ScalarTerm,
    ValueId,
};
use terminal_check_tables::edges::{Actual, Callee, InvocationRow};
use terminal_check_tables::obligations::{CrashSite, ExecutionSite, Lender, ObligationKind};
use terminal_check_tables::{
    Checked, Construct, ExactPlaceId, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{OperationKind, StructuralAccess, StructuralArgument, Terminator};

use super::{Evaluation, Lowering, Reached, Value};
use crate::body::{CallTarget, CrashCeiling, Expression, ExpressionKind, Signature, ValueType};
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
        source_span: None,
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
                // The same region the ownership analysis gives a write of this
                // storage, a stored view's descriptor included.
                let path = terminal_semantics::canonical_inline_region(
                    module.structural_types.iter(),
                    structural_type,
                    &region.projection.path,
                )
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

/// One loan a call forms. `formation` is the access the program wrote and
/// the loan's occurrence records; `lent` is the formal's access, the one the
/// callee receives (`ReferenceAccess::forms`).
pub(super) struct CallLoan {
    place: ExactPlaceId,
    formation: ReferenceAccess,
    lent: ReferenceAccess,
    lender: Lender,
}

/// A call's evaluated arguments.
pub(super) struct Arguments {
    /// In authored formal order, as the invocation row holds them.
    pub(super) actuals: Vec<Actual>,
    /// The scalar lane: the values a callee clause's formals stand for.
    pub(super) scalars: Vec<ValueId>,
    /// The structural lane in formal order: each record argument's root,
    /// whole and owned, and each loan's place with the loan's access.
    pub(super) structural: Vec<(PlaceId, Vec<super::places::Step>, StructuralAccess)>,
    /// Homes staged for owned values or captured borrowed descriptors. A
    /// normal continuation retires them only after its call loans end.
    homes: Vec<ArgumentHome>,
    /// One per loan, in formal order, retaining formation and callee access.
    pub(super) loans: Vec<CallLoan>,
    /// The erased lane: the term each `[erased]` formal stands for, in
    /// position order. Nothing was evaluated for it, and it is not among
    /// `actuals`: an invocation row's actual is a value or a place, and the
    /// checker reads these terms from the call itself.
    pub(super) erased: Vec<ScalarTerm>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum EvaluatedCallTarget {
    Machine(MachineId),
    Selected {
        callable: ValueId,
        contract: CallableContractId,
    },
    Boundary {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
}

impl Lowering<'_> {
    fn target_signature(
        &self,
        target: &CallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
    ) -> Checked<Signature> {
        if let Some(required) = required_contract {
            let target = match target {
                CallTarget::Machine(machine) => (*machine).into(),
                CallTarget::Boundary { boundary } => {
                    semantic_vocabulary::TargetMachineCallable::Boundary(*boundary)
                }
                CallTarget::Selected { .. } => {
                    return Err(
                        self.internal("selected callable also names a declared public contract")
                    );
                }
            };
            return self
                .signatures
                .invocation(target, Some(required), self.id)
                .cloned();
        }
        match target {
            CallTarget::Machine(machine) => terminal_check_tables::row(
                self.signatures.machines,
                *machine,
                self.id,
                Construct::Machine,
            )
            .cloned(),
            CallTarget::Selected { contract, .. } => self
                .callables
                .contracts
                .get(*contract)
                .map(|contract| contract.signature.clone())
                .ok_or_else(|| self.internal("selected call has no retained public contract")),
            CallTarget::Boundary { boundary, .. } => self.boundary_signature(*boundary),
        }
    }

    fn evaluated_target(&mut self, target: &CallTarget) -> Evaluation<EvaluatedCallTarget> {
        match target {
            CallTarget::Machine(machine) => {
                Ok(Reached::Value(EvaluatedCallTarget::Machine(*machine)))
            }
            CallTarget::Boundary { boundary } => {
                Ok(Reached::Value(EvaluatedCallTarget::Boundary {
                    boundary: *boundary,
                }))
            }
            CallTarget::Selected { callable, contract } => {
                if !matches!(callable.result,
                    Some(ValueType::Scalar(semantic_vocabulary::ScalarType::Callable(found), _))
                        if found == *contract
                ) {
                    return Err(self.internal(
                        "selected call's evaluated operand changes its public contract",
                    ));
                }
                let callable = continuing!(self.scalar(callable));
                Ok(Reached::Value(EvaluatedCallTarget::Selected {
                    callable,
                    contract: *contract,
                }))
            }
        }
    }

    fn target_outcome(
        &self,
        target: &CallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
    ) -> Checked<Outcome> {
        let mut outcome = match target {
            CallTarget::Machine(machine) => self.outcome(*machine).cloned(),
            CallTarget::Boundary { boundary, .. } => self.boundary_outcome(*boundary),
            CallTarget::Selected { contract, .. } => {
                let row = self.callables.contracts.get(*contract).ok_or_else(|| {
                    self.internal("selected call has no retained public contract")
                })?;
                let signature = self.target_signature(target, None)?;
                let crash_routes = match signature.crashes {
                    CrashCeiling::Authored(routes) => routes,
                    CrashCeiling::DeclaredNone => Vec::new(),
                    CrashCeiling::Inferred => {
                        return Err(self.internal("selected call has an open crash ceiling"));
                    }
                };
                // An unknown callable may return. A selected body cannot narrow
                // this public continuation or contribute private guarantees.
                Ok(Outcome {
                    operational: [row.envelope.suspends, row.envelope.blocks],
                    completes_normally: true,
                    crash_routes,
                    service_reach: match signature.reaches {
                        crate::Ceiling::Authored(row) => row,
                        crate::Ceiling::Inferred => {
                            return Err(
                                self.internal("selected callable has no published service ceiling")
                            );
                        }
                    },
                    fixed_service_reach: None,
                    synchronous_entries: match signature.invokes {
                        crate::Ceiling::Authored(row) => row,
                        crate::Ceiling::Inferred => {
                            return Err(
                                self.internal("selected callable has no published entry ceiling")
                            );
                        }
                    },
                })
            }
        }?;
        if required_contract.is_some() {
            let signature = self.target_signature(target, required_contract)?;
            // A routed public requirement keeps its own continuation and
            // written bounds; only omitted clauses use the completed source.
            outcome.completes_normally = true;
            outcome.operational = signature.operational.published(outcome.operational);
            if let Some(routes) = signature.declared_crash_routes() {
                outcome.crash_routes = routes.to_vec();
            }
            if let crate::Ceiling::Authored(row) = signature.reaches {
                outcome.service_reach = row;
            }
            if let crate::Ceiling::Authored(row) = signature.invokes {
                outcome.synchronous_entries = row;
            }
        }
        Ok(outcome)
    }

    /// Evaluates a call's arguments left to right.
    fn arguments(
        &mut self,
        signature: &Signature,
        arguments: &[Expression],
    ) -> Evaluation<Arguments> {
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
            if let ExpressionKind::Borrow { access, place, .. } = &argument.kind {
                if signature.borrowed(position).map(|formal| formal.access) != Some(*access) {
                    return Err(self.internal("a loan for a formal that is not borrowed alike"));
                }
                let lent = continuing!(self.lend(place, *access));
                evaluated.actuals.push(Actual::Place(lent.place));
                evaluated.structural.push(lent.argument);
                evaluated.loans.push(CallLoan {
                    place: lent.place,
                    formation: *access,
                    lent: *access,
                    lender: lent.lender,
                });
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
        // Contracts observe the exact actual subtree, even when no body
        // operation reads that leaf. Naming a leaf supplies no premise; a
        // guarantee will use its independently checked after-call identity.
        let bindings = evaluated
            .actuals
            .iter()
            .filter_map(|actual| match actual {
                Actual::Place(place) => Some(*place),
                Actual::Value(_) => None,
            })
            .enumerate()
            .map(|(lane, actual)| {
                let formal = PlaceId::new(lane as u64 + 1)
                    .ok_or_else(|| self.internal("structural formal identity overflow"))?;
                Ok((formal, actual))
            })
            .collect::<Checked<Vec<_>>>()?;
        let result = signature.structural_result_place();
        for clause in signature.requires.iter().chain(&signature.ensures) {
            self.name_clause_contents(clause, &bindings, result)?;
        }
        Ok(Reached::Value(evaluated))
    }

    /// A terminal call within this component becomes a block transfer.
    /// Capture both argument lanes with the ordinary call evaluation order;
    /// the edge checks their access and retires staging homes after capture.
    pub(super) fn tail_arguments(
        &mut self,
        callee: MachineId,
        arguments: &[Expression],
        source: SourceOrigin,
    ) -> Evaluation<(
        Vec<ValueId>,
        Vec<StructuralArgument>,
        Vec<terminal_psi::TerminalAffineCleanupAction>,
    )> {
        let signature = self.signatures.get(callee.into(), self.id)?.clone();
        let captured = continuing!(self.arguments(&signature, arguments));
        if !captured.erased.is_empty() {
            return Err(self.internal("a loop transfer carries erased arguments"));
        }
        let structural = self.structural_arguments(
            captured.structural,
            ExecutionSite::Block(self.current()?),
            source,
        )?;
        let cleanup = captured
            .homes
            .iter()
            .map(|home| terminal_psi::TerminalAffineCleanupAction::RetireHome(home.place()))
            .collect();
        Ok(Reached::Value((captured.scalars, structural, cleanup)))
    }

    pub(super) fn structural_arguments(
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
    /// and ends when it returns. A call that never returns carries its
    /// loans on its terminal edge instead. Formation uses the access written
    /// by the caller, even when the callee receives weaker access.
    pub(super) fn loans(
        &mut self,
        loans: &[CallLoan],
        before: ProgramPoint,
        source: SourceOrigin,
    ) -> Vec<ObligationId> {
        loans
            .iter()
            .map(|loan| {
                self.obligation(
                    ObligationKind::Loan {
                        captured: loan.place,
                        access: loan.formation,
                        lender: loan.lender,
                    },
                    before,
                    source,
                )
            })
            .collect()
    }

    pub(super) fn terminal_loan_dispositions(
        loans: &[ObligationId],
        fate: terminal_check_tables::edges::Fate,
    ) -> Vec<terminal_check_tables::edges::Disposition> {
        loans
            .iter()
            .map(|loan| terminal_check_tables::edges::Disposition {
                obligation: *loan,
                fate,
            })
            .collect()
    }

    /// What an exclusive loan's callee may have stored: anything beneath the
    /// lent place, from the call's normal continuation on. Nothing else the
    /// caller holds changes, so siblings keep what they held.
    fn returned_loans(&mut self, loans: &[CallLoan], operation: OperationId) -> Checked<()> {
        for loan in loans {
            if loan.lent.is_exclusive() {
                self.unknown_contents(loan.place, operation)?;
            }
        }
        Ok(())
    }

    fn outcome(&self, callee: MachineId) -> Checked<&'_ Outcome> {
        terminal_check_tables::row(self.outcomes, callee, self.id, Construct::Machine)
    }

    /// Whether the caller may retain a normal continuation. Checked body
    /// outcomes stay within their source visibility; an abstract requirement
    /// cannot inherit its implementation's private non-return fact.
    fn completes_normally(
        &mut self,
        callable: &CallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
        arguments: &Arguments,
    ) -> Checked<bool> {
        let signature = self.target_signature(callable, required_contract)?;
        if crate::inhabitation::excludes_result(&self.records.inhabitation, signature.result) {
            return Ok(false);
        }
        if self.published_postcondition_is_false(&signature, arguments)? {
            return Ok(false);
        }
        // Requirement routing does not publish the chosen body's outcome.
        // Keep the continuation governed by that declared contract.
        if required_contract.is_some() {
            return Ok(true);
        }
        Ok(match callable {
            CallTarget::Selected { .. } => true,
            CallTarget::Machine(callee) => self.outcome(*callee)?.completes_normally,
            CallTarget::Boundary { boundary } => {
                self.signatures.completion(*boundary, self.id)?
                    == semantic_vocabulary::BoundaryCompletion::NormalReturn
            }
        })
    }

    /// Bind genuine scalar formals once, with their exact declared carriers.
    /// A callee result ID cannot acquire a caller constant with the same number.
    fn published_postcondition_is_false(
        &mut self,
        signature: &crate::body::Signature,
        arguments: &Arguments,
    ) -> Checked<bool> {
        if signature.ensures.is_empty() {
            return Ok(false);
        }
        let ids = signature.formal_values();
        let mut relevant = arguments.actuals.iter();
        let mut erased = arguments.erased.iter();
        let mut bindings = Vec::new();
        for (position, ty) in signature.parameters.iter().enumerate() {
            let known = if signature.is_erased(position) {
                let term = erased
                    .next()
                    .ok_or_else(|| self.internal("missing erased call actual"))?;
                terminal_semantics::inhabitation::known_scalar_value_with(term, |id| {
                    self.known_scalar(id)
                })
            } else {
                let actual = relevant
                    .next()
                    .ok_or_else(|| self.internal("missing call actual"))?;
                match actual {
                    Actual::Value(value) => self.known_scalar(*value),
                    Actual::Place(_) => None,
                }
            };
            if let ValueType::Scalar(expected, _) = ty
                && signature.borrowed(position).is_none()
                && let Some(id) = ids[position]
            {
                bindings.push((id, known.filter(|(actual, _)| actual == expected)));
            }
        }
        if relevant.next().is_some() || erased.next().is_some() {
            return Err(self.internal("extra call actuals in postcondition substitution"));
        }
        bindings.sort_unstable_by_key(|(id, _)| *id);
        Ok(terminal_semantics::inhabitation::predicates_are_false_with(
            &signature.ensures,
            |formal| {
                bindings
                    .binary_search_by_key(&formal, |(id, _)| *id)
                    .ok()
                    .and_then(|index| bindings[index].1)
            },
        ))
    }

    fn known_scalar(
        &mut self,
        value: ValueId,
    ) -> Option<(
        semantic_vocabulary::ScalarType,
        semantic_vocabulary::IntegerValue,
    )> {
        let values = &self.tables.values;
        let operation_locations = &self.operation_locations;
        let blocks = &self.blocks;
        let types = DraftScalarValues { values };
        let constants = &mut self.scalar_constants;
        while constants.registered_count() < values.len() {
            let id = ValueId::new(
                u64::try_from(constants.registered_count())
                    .ok()?
                    .checked_add(1)?,
            )?;
            values.get(id)?;
            if !constants.register(id) {
                return None;
            }
        }
        constants.value(value, |value| {
            let row = values.get(value)?;
            if row.rigid.is_some() {
                return None;
            }
            // A draft join can still gain arrivals. Only already-emitted
            // operation definitions are stable enough to memoize here.
            let terminal_check_tables::values::ValueOrigin::Operation(operation) = row.origin
            else {
                return None;
            };
            let (block, ordinal) = operation_locations.get(operation)?.as_ref()?;
            let operation = blocks.get(*block)?.operations.get(*ordinal)?;
            terminal_semantics::scalar_operation_origin(operation, value, &types)
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
        target: EvaluatedCallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
        signature: &Signature,
        arguments: Arguments,
        outcome: &Outcome,
        acknowledgement: terminal_psi::CallAcknowledgement,
        source: SourceOrigin,
        static_reach_binding: Option<u32>,
    ) -> Checked<()> {
        if let EvaluatedCallTarget::Boundary { boundary } = target {
            return self.invoke_boundary_no_normal(boundary, arguments, acknowledgement, source);
        }
        let EvaluatedCallTarget::Machine(machine) = target else {
            return Err(
                self.internal("runtime callable has no no-normal public completion contract")
            );
        };
        // This invocation retains the caller activation and the lent storage.
        // It has no normal continuation at which a loan could be restored or
        // its contents observed, but formation still owes the ordinary access
        // and overlap judgments at the exact call site.
        let crash_continuations = canonical::substituted_routes(
            &outcome.crash_routes,
            &arguments.scalars,
            &self.tables.values,
            self.id,
        )?;
        let site = self.current()?;
        let before = self.here()?;
        let loans = self.loans(&arguments.loans, before, source);
        let edge = self.no_successor_edge(
            Self::terminal_loan_dispositions(&loans, terminal_check_tables::edges::Fate::Consumed),
            source,
        )?;
        let requirements = self.invocation(
            InvocationSite::InvokeNoNormal(edge),
            target,
            required_contract,
            signature.requires.len(),
            &arguments.actuals,
            before,
            source,
            acknowledgement,
        )?;
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
        self.terminate(Terminator::InvokeNoNormal {
            edge,
            static_reach_binding,
            required_contract: self
                .signatures
                .canonical_contract(required_contract, self.id)?,
            callee: machine,
            structural_arguments,
            arguments: arguments.scalars,
            erased_arguments: arguments.erased,
            erased_proof_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: requirements,
            crash_continuations,
        })
    }

    /// The invocation row of one call and its `CallRequirement` obligations,
    /// one per callee `requires` clause, created at the point before it.
    pub(super) fn invocation(
        &mut self,
        site: InvocationSite,
        target: EvaluatedCallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
        clauses: usize,
        actuals: &[Actual],
        before: ProgramPoint,
        source: SourceOrigin,
        acknowledgement: terminal_psi::CallAcknowledgement,
    ) -> Checked<Vec<ObligationId>> {
        self.call_formations.push(terminal_psi::CallFormation {
            site: match site {
                InvocationSite::Operation(operation) => {
                    terminal_psi::CallSite::Operation(operation)
                }
                InvocationSite::InvokeNoNormal(edge) | InvocationSite::ExternalCompletion(edge) => {
                    terminal_psi::CallSite::Edge(edge)
                }
                InvocationSite::Cleanup { .. } => {
                    return Err(self.internal(
                        "an owned cleanup invocation cannot use an ordinary source-call formation",
                    ));
                }
            },
            acknowledgement,
            tail_callee: None,
        });
        if let SourceOrigin::AuthoredNode {
            source_member,
            owner,
            node,
            ..
        } = source
        {
            self.source_calls
                .push(typed_trees::SourceCallCorrespondence {
                    machine: self.id,
                    source_member,
                    site,
                    owner,
                    source: node,
                });
        }
        let invocation = self.tables.invocations.push(InvocationRow {
            required_contract: self
                .signatures
                .canonical_contract(required_contract, self.id)?,
            site,
            callee: match target {
                EvaluatedCallTarget::Machine(machine) => {
                    Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(machine))
                }
                EvaluatedCallTarget::Selected { callable, .. } => Callee::Selected { callable },
                EvaluatedCallTarget::Boundary { boundary, .. } => Callee::Declared(
                    semantic_vocabulary::TargetMachineCallable::Boundary(boundary),
                ),
            },
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
                source_span: None,
                detail: String::from("invocation is past the end of this machine"),
            })?
            .requirements
            .clone_from(&requirements);
        Ok(requirements)
    }

    pub(super) fn call(
        &mut self,
        callee: &CallTarget,
        required_contract: Option<semantic_vocabulary::ContractId>,
        arguments: &[Expression],
        designated: Option<crate::body::DesignatedOperands>,
        result: Option<ValueType>,
        (source, static_reach_binding): (SourceOrigin, Option<u32>),
        acknowledgement: terminal_psi::CallAcknowledgement,
    ) -> Evaluation<Value> {
        if designated.is_some() && !matches!(callee, CallTarget::Boundary { .. }) {
            return Err(self.internal("designated operands on a call that is no boundary call"));
        }
        let signature = self.target_signature(callee, required_contract)?;
        let outcome = self.target_outcome(callee, required_contract)?;
        match (result, signature.result) {
            (Some(ValueType::Rigid(actual)), Some(ValueType::Rigid(declared)))
                if actual == declared => {}
            (Some(ValueType::Rigid(_)), _) | (_, Some(ValueType::Rigid(_))) => {
                return Err(self.internal("a call changed its declared rigid result type"));
            }
            _ => {}
        }
        if matches!(callee, CallTarget::Boundary { .. })
            && self.signature.termination != crate::Termination::Silent
            && signature.termination == crate::Termination::Silent
        {
            return Err(refuse(
                self.id,
                Construct::Machine,
                Unserved::Call("boundary call without a termination guarantee"),
            ));
        }
        // Capture the code value before any argument can write its backing.
        // Both returning and terminal calls consume this same evaluated plan.
        let target = continuing!(self.evaluated_target(callee));
        let mut arguments = continuing!(self.arguments(&signature, arguments));
        if !self.completes_normally(callee, required_contract, &arguments)? {
            if designated.is_some() {
                return Err(self.internal("designated operands on a call that never returns"));
            }
            self.invoke_no_normal(
                target,
                required_contract,
                &signature,
                arguments,
                &outcome,
                acknowledgement,
                source,
                static_reach_binding,
            )?;
            return Ok(Reached::Ended(source));
        }
        let crash_continuations = canonical::substituted_routes(
            &outcome.crash_routes,
            &arguments.scalars,
            &self.tables.values,
            self.id,
        )?;
        let before = self.here()?;
        let operation = self.operation_identity(source);
        if let Some(designated) = designated {
            self.designated.push((operation, designated));
        }
        let requirements = self.invocation(
            InvocationSite::Operation(operation),
            target,
            required_contract,
            signature.requires.len(),
            &arguments.actuals,
            before,
            source,
            acknowledgement,
        )?;
        let _ = self.loans(&arguments.loans, before, source);
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
            std::mem::take(&mut arguments.structural),
            ExecutionSite::Operation(operation),
            source,
        )?;
        let erased_arguments = std::mem::take(&mut arguments.erased);
        let scalars = std::mem::take(&mut arguments.scalars);
        let kind = match target {
            EvaluatedCallTarget::Selected { callable, contract } => OperationKind::CallMachine {
                callee: callable,
                contract,
                arguments: scalars,
                erased_arguments,
                erased_proof_arguments: Vec::new(),
                structural_arguments,
                claim_transfers: Vec::new(),
                requirement_obligations: requirements,
                crash_continuations,
            },
            EvaluatedCallTarget::Boundary { boundary } => {
                if !erased_arguments.is_empty() {
                    return Err(self.internal("boundary call cannot discard erased actuals"));
                }
                OperationKind::BoundaryCall {
                    boundary,
                    arguments: scalars,
                    structural_arguments,
                    completion_receipts: Vec::new(),
                    requirement_obligations: requirements,
                }
            }
            EvaluatedCallTarget::Machine(callee) => match result {
                Some(ValueType::Scalar(..) | ValueType::Rigid(_))
                    if structural_arguments.is_empty() =>
                {
                    OperationKind::Call {
                        callee,
                        arguments: scalars,
                        erased_arguments,
                        erased_proof_arguments: Vec::new(),
                        requirement_obligations: requirements,
                        crash_continuations,
                    }
                }
                Some(ValueType::Scalar(..) | ValueType::Rigid(_)) => {
                    OperationKind::CallStructuralScalar {
                        callee,
                        arguments: scalars,
                        erased_arguments,
                        erased_proof_arguments: Vec::new(),
                        structural_arguments,
                        claim_transfers: Vec::new(),
                        requirement_obligations: requirements,
                        crash_continuations,
                    }
                }
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
                Some(ValueType::Record(..)) => OperationKind::CallStructuralWithScalarArguments {
                    callee,
                    arguments: scalars,
                    erased_arguments,
                    erased_proof_arguments: Vec::new(),
                    structural_arguments,
                    claim_transfers: Vec::new(),
                    requirement_obligations: requirements,
                    crash_continuations,
                    selected_evidence: Vec::new(),
                },
            },
        };
        let value = match result {
            Some(ValueType::Scalar(scalar_type, _)) => {
                self.emit(operation, Some(scalar_type), kind)?
            }
            Some(value_type @ ValueType::Record(..)) => {
                let (root, exact) = self.produce(operation, value_type, kind)?;
                self.unknown_contents(exact, operation)?;
                self.name_contents(exact, value_type)?;
                Value::Record(root)
            }
            None => self.emit(operation, None, kind)?,
            Some(ValueType::Rigid(rigid)) => {
                let value = self.rigid_value(
                    rigid,
                    terminal_check_tables::values::ValueOrigin::Operation(operation),
                );
                self.emit_operation(
                    operation,
                    terminal_psi::OperationResult::Scalar(super::declaration(
                        value,
                        terminal_check_tables::values::RigidType::DECLARED_AS,
                    )),
                    kind,
                )?;
                Value::Scalar(value)
            }
        };
        let canonical_contract = self
            .signatures
            .canonical_contract(required_contract, self.id)?;
        let Some(emitted) = self.draft()?.operations.last_mut() else {
            return Err(self.internal("a call emitted no operation"));
        };
        if emitted.id != operation {
            return Err(self.internal("a selected contract lost its exact call operation"));
        }
        emitted.required_contract = canonical_contract;
        emitted.static_reach_binding = static_reach_binding;
        self.returned_loans(&arguments.loans, operation)?;
        if !arguments.homes.is_empty() {
            let homes: Vec<_> = arguments.homes.iter().map(ArgumentHome::place).collect();
            self.retire_homes(&homes, source)?;
        }
        Ok(Reached::Value(value))
    }
}

/// Immutable scalar declarations borrowed separately from the mutable cache.
struct DraftScalarValues<'a> {
    values: &'a semantic_vocabulary::IdTable<ValueId, terminal_check_tables::values::ValueRow>,
}

impl terminal_semantics::ValueTypes for DraftScalarValues<'_> {
    fn value_type(&self, value: ValueId) -> Option<semantic_vocabulary::ScalarType> {
        let row = self.values.get(value)?;
        row.rigid.is_none().then_some(row.scalar_type)
    }
}
