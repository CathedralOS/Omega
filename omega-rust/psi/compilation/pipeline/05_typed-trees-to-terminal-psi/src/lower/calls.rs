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
    /// One per loan, in formal order: the place lent, its access and where
    /// the authority comes from.
    pub(super) loans: Vec<(ExactPlaceId, ReferenceAccess, Lender)>,
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

#[derive(Debug, Clone, Copy)]
pub(super) struct EvaluatedCall {
    pub target: EvaluatedCallTarget,
    pub acknowledgement: language_core::CallOperationalAcknowledgement,
}

impl Lowering<'_> {
    fn target_signature(&self, target: &CallTarget) -> Checked<Signature> {
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

    fn target_outcome(&self, target: &CallTarget) -> Checked<Outcome> {
        match target {
            CallTarget::Machine(machine) => self.outcome(*machine).cloned(),
            CallTarget::Boundary { boundary, .. } => self.boundary_outcome(*boundary),
            CallTarget::Selected { .. } => {
                let signature = self.target_signature(target)?;
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
        }
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
    /// loans on its terminal edge instead.
    pub(super) fn loans(
        &mut self,
        loans: &[(ExactPlaceId, ReferenceAccess, Lender)],
        before: ProgramPoint,
        source: SourceOrigin,
    ) -> Vec<ObligationId> {
        loans
            .iter()
            .map(|(captured, access, lender)| {
                self.obligation(
                    ObligationKind::Loan {
                        captured: *captured,
                        access: *access,
                        lender: *lender,
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
    fn invoke_no_normal(
        &mut self,
        target: EvaluatedCallTarget,
        signature: &Signature,
        arguments: Arguments,
        outcome: &Outcome,
        acknowledgement: language_core::CallOperationalAcknowledgement,
        source: SourceOrigin,
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
        let crash_continuations =
            canonical::substituted_routes(&outcome.crash_routes, &arguments.scalars, self.id)?;
        let site = self.current()?;
        let before = self.here()?;
        let loans = self.loans(&arguments.loans, before, source);
        let edge = self.no_successor_edge(
            Self::terminal_loan_dispositions(&loans, terminal_check_tables::edges::Fate::Consumed),
            source,
        )?;
        let requirements = self.invocation(
            InvocationSite::InvokeNoNormal(edge),
            EvaluatedCall {
                target,
                acknowledgement,
            },
            signature.requires.len(),
            &arguments.actuals,
            before,
            source,
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
            static_reach_binding: None,
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
        call: EvaluatedCall,
        clauses: usize,
        actuals: &[Actual],
        before: ProgramPoint,
        source: SourceOrigin,
    ) -> Checked<Vec<ObligationId>> {
        if let Some((owner, node)) = source.node() {
            self.source_calls
                .push(typed_trees::SourceCallCorrespondence {
                    machine: self.id,
                    site,
                    owner,
                    source: node,
                });
        }
        let invocation = self.tables.invocations.push(InvocationRow {
            operational_acknowledgement: call.acknowledgement,
            site,
            callee: match call.target {
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
                detail: String::from("invocation is past the end of this machine"),
            })?
            .requirements
            .clone_from(&requirements);
        Ok(requirements)
    }

    pub(super) fn call(
        &mut self,
        callee: &CallTarget,
        arguments: &[Expression],
        designated: Option<crate::body::DesignatedOperands>,
        acknowledgement: language_core::CallOperationalAcknowledgement,
        result: Option<ValueType>,
        source: SourceOrigin,
    ) -> Evaluation<Value> {
        if designated.is_some() && !matches!(callee, CallTarget::Boundary { .. }) {
            return Err(self.internal("designated operands on a call that is no boundary call"));
        }
        let signature = self.target_signature(callee)?;
        let outcome = self.target_outcome(callee)?;
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
        if !outcome.completes_normally {
            if designated.is_some() {
                return Err(self.internal("designated operands on a call that never returns"));
            }
            self.invoke_no_normal(
                target,
                &signature,
                arguments,
                &outcome,
                acknowledgement,
                source,
            )?;
            return Ok(Reached::Ended(source));
        }
        let crash_continuations =
            canonical::substituted_routes(&outcome.crash_routes, &arguments.scalars, self.id)?;
        let before = self.here()?;
        let operation = self.operation_identity(source);
        if let Some(designated) = designated {
            self.designated.push((operation, designated));
        }
        let requirements = self.invocation(
            InvocationSite::Operation(operation),
            EvaluatedCall {
                target,
                acknowledgement,
            },
            signature.requires.len(),
            &arguments.actuals,
            before,
            source,
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
                Some(ValueType::Rigid(_)) => {
                    return Err(self.internal("a call returns a value of a rigid type"));
                }
            },
        };
        let value = match result {
            Some(ValueType::Scalar(scalar_type, _)) => {
                self.emit(operation, Some(scalar_type), kind)?
            }
            Some(value_type @ ValueType::Record(record, _, _)) => {
                let (root, exact) = self.produce(operation, record, kind)?;
                self.unknown_contents(exact, operation)?;
                self.initial_leaves(exact, value_type)?;
                Value::Record(root)
            }
            None => self.emit(operation, None, kind)?,
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
