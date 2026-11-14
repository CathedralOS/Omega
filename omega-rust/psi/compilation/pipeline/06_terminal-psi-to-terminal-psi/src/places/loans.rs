/*
The loans of one machine: which `Loan` rows the lowered form owes, and
whether each loan may be formed where it is.

A loan is a call's structural argument with a borrowed access: the call lends
the place the argument names, for the call's duration, and the place is the
caller's again when the call returns (`wiki/spec/terminal-psi/loans.md`:
"Loan resource | Exact occurrence, owner lineage, captured place, access,
parent lifetime, and restoration obligation."). The occurrence is the call,
the captured place is the argument's place, the access is the argument's,
and the lineage is the root's: a borrowed formal's root reborrows the loan
the invocation passed in (`Lender::Incoming`), any other root lends owned
storage (`Lender::Owner`). A loan formed for a call that returns ends when
it does, so it is live on no edge and no edge disposes of it. A loan formed
for a call with no normal return is carried into that terminal transfer
(`wiki/spec/language/state_contracts.md`: "A caller-local loan carried into
a terminal machine call closes at the last use of that loan and of every
loan derived from it, even when that use is inside the callee's terminal
chain"): the transfer's edge consumes it, and nothing of this machine runs
after to use the lent place again.

The rows are recomputed from the lowered form, as every other row is
(`minted`): one `Loan` per borrowed argument of each call, created at the
point before the call, whose captured place is the invocation row's actual
at that position and is the place the argument's storage path names. A row
the lowered form does not owe, or owes elsewhere or of another kind, is a
defect of the route (`ForeignIdentity`).

Whether a loan may be formed is a resource judgment, not a proposition
(loans.md: "Loan descent, access attenuation, temporal containment and
restoration are resource judgments. Logical facts cannot create, amplify,
transfer, extend, return, consume or duplicate authority."), so it has no
certificate, as the initialization judgment has none. Two refusals, each
`RefusalReason::Conflict`, a compile error of the program:

- Two loans of one call whose places may share storage, when either is
  exclusive. The places' overlap is the one relation of `places::overlap`:
  two different fields of one record are disjoint, anything else beneath one
  root may share storage. ownership.md, "Borrows and aliases": "exclusive
  loans must not overlap incompatibly"; loans.md: "A live exclusive loan does
  not prove projections disjoint."
- An exclusive reborrow of a place beneath a borrowed formal declared
  shared. loans.md, "Reborrow lineage and access": a `Read` parent rejects a
  mutable child.
*/

use language_core::ReferenceAccess;
use semantic_vocabulary::StructuralPlaceKind;
use semantic_vocabulary::{InvocationSite, ObligationId};
use terminal_check_tables::edges::{Actual, Fate};
use terminal_check_tables::obligations::{Lender, ObligationKind};
use terminal_check_tables::{
    Checked, Construct, InvocationId, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{StructuralAccess, TerminalModule};

use super::{Storage, index, lineage, overlap};

/// Why two loans of one call may not both be formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanConflict {
    /// Both loans are exclusive.
    TwiceExclusive,
    /// One is exclusive and the other shared.
    ExclusiveAndShared,
    /// An exclusive loan of storage this machine holds only shared.
    ExclusiveOfShared,
}

/// One loan of one call that may not be formed: the loan's row, the call's
/// invocation, the argument position of the loan, and for two loans of one
/// call the position of the earlier one it conflicts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictingLoan {
    pub loan: ObligationId,
    pub invocation: InvocationId,
    pub position: usize,
    pub with: Option<usize>,
    pub conflict: LoanConflict,
}

/// One loan the lowered form forms, as recomputed from it.
struct Formed {
    obligation: ObligationId,
    invocation: InvocationId,
    position: usize,
    access: ReferenceAccess,
    lineage: super::Lineage,
    /// For a reborrow, the access of the formal it reborrows.
    parent: Option<ReferenceAccess>,
}

fn reference_access(access: StructuralAccess) -> Option<ReferenceAccess> {
    match access {
        StructuralAccess::SharedBorrow => Some(ReferenceAccess::Shared),
        StructuralAccess::MutableBorrow => Some(ReferenceAccess::Mutable),
        StructuralAccess::WriteOnlyBorrow => Some(ReferenceAccess::WriteOnly),
        StructuralAccess::Owned => None,
    }
}

impl Storage<'_> {
    /// The access a root's storage is held with here, when it is a borrowed
    /// formal: the declared access of its structural parameter.
    fn incoming(&self, root: semantic_vocabulary::PlaceId) -> Option<StructuralAccess> {
        self.lowered
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == root)
            .filter(|parameter| parameter.access != StructuralAccess::Owned)
            .map(|parameter| parameter.access)
    }

    /// Recomputes every loan from the lowered form and refuses `tables`
    /// unless its `Loan` rows are exactly those, each once. Returns them in
    /// block and operation order, each call's loans in argument order.
    fn formed(&self, module: &TerminalModule, tables: &MachineTables) -> Checked<Vec<Formed>> {
        let mut claimed = vec![false; tables.obligations.len()];
        let mut formed = Vec::new();
        for block in &self.lowered.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for operation in &block.operations {
                self.form_call(
                    module,
                    tables,
                    InvocationSite::Operation(operation.id),
                    before,
                    Construct::Operation(operation.id),
                    &mut claimed,
                    &mut formed,
                )?;
                before = ProgramPoint::AfterOperation(operation.id);
            }
            if let Some(site) = crate::call_contracts::terminator_site(&block.terminator) {
                self.form_call(
                    module,
                    tables,
                    site,
                    before,
                    Construct::Block(block.id),
                    &mut claimed,
                    &mut formed,
                )?;
            }
        }
        for (obligation, row) in tables.obligations.iter() {
            if matches!(row.kind, ObligationKind::Loan { .. }) && !claimed[index(obligation.get())]
            {
                return Err(self.foreign(
                    Construct::Obligation(obligation),
                    "a loan row that no call forms",
                ));
            }
        }
        // Terminal transfers account for exactly the loans they carry.
        // A normal call's temporary loan has no edge disposition.
        let mut disposed = vec![false; tables.obligations.len()];
        for (edge, row) in tables.edges.iter() {
            for disposition in &row.dispositions {
                let obligation =
                    tables
                        .obligations
                        .get(disposition.obligation)
                        .ok_or_else(|| {
                            self.foreign(
                                Construct::Edge(edge),
                                "edge disposition names no obligation",
                            )
                        })?;
                if !matches!(obligation.kind, ObligationKind::Loan { .. }) {
                    continue;
                }
                let loan = formed
                    .iter()
                    .find(|loan| loan.obligation == disposition.obligation)
                    .ok_or_else(|| {
                        self.foreign(
                            Construct::Edge(edge),
                            "edge disposition names no formed loan",
                        )
                    })?;
                let invocation = tables.invocations.get(loan.invocation).ok_or_else(|| {
                    self.foreign(Construct::Edge(edge), "disposed loan has no invocation")
                })?;
                let expected = match invocation.site {
                    InvocationSite::InvokeNoNormal(site) => (site, Fate::Consumed),
                    InvocationSite::ExternalCompletion(site) => (site, Fate::Consumed),
                    _ => {
                        return Err(self.foreign(
                            Construct::Edge(edge),
                            "normal call loan has an edge disposition",
                        ));
                    }
                };
                if expected != (edge, disposition.fate)
                    || std::mem::replace(&mut disposed[index(disposition.obligation.get())], true)
                {
                    return Err(self.foreign(
                        Construct::Edge(edge),
                        "loan disposition repeats or differs from its terminal transfer",
                    ));
                }
            }
        }
        for loan in &formed {
            let invocation = tables.invocations.get(loan.invocation).ok_or_else(|| {
                self.foreign(
                    Construct::Obligation(loan.obligation),
                    "formed loan has no invocation",
                )
            })?;
            if matches!(
                invocation.site,
                InvocationSite::InvokeNoNormal(_) | InvocationSite::ExternalCompletion(_)
            ) && !disposed[index(loan.obligation.get())]
            {
                return Err(self.foreign(
                    Construct::Obligation(loan.obligation),
                    "terminal call loan has no exact edge disposition",
                ));
            }
        }
        Ok(formed)
    }

    /// Normal and terminal calls form the same loans before invocation. A
    /// terminal call has no restoration successor; formation still checks
    /// its exact captured storage, access and parent authority.
    #[allow(clippy::too_many_arguments)]
    fn form_call(
        &self,
        module: &TerminalModule,
        tables: &MachineTables,
        site: InvocationSite,
        before: ProgramPoint,
        concerns: Construct,
        claimed: &mut [bool],
        formed: &mut Vec<Formed>,
    ) -> Checked<()> {
        let Some(inputs) = crate::call_contracts::inputs_at(self.lowered, site)? else {
            return Ok(());
        };
        let arguments = inputs.structural_arguments;
        if !arguments
            .iter()
            .any(|argument| argument.access != StructuralAccess::Owned)
        {
            return Ok(());
        }
        let mut rows = tables
            .invocations
            .iter()
            .filter(|(_, row)| row.site == site);
        let (Some((invocation, row)), None) = (rows.next(), rows.next()) else {
            return Err(self.foreign(concerns, "a call does not have exactly one invocation row"));
        };
        let call =
            crate::call_contracts::invocation(module, self.lowered, tables, invocation, row)?;
        let parameters = call.contract.structural_parameters;
        let places = row
            .actuals
            .iter()
            .enumerate()
            .filter_map(|(position, actual)| match actual {
                Actual::Place(place) => Some((position, *place)),
                Actual::Value(_) => None,
            })
            .collect::<Vec<_>>();
        if places.len() != arguments.len() || parameters.len() != arguments.len() {
            return Err(self.foreign(
                concerns,
                "an invocation row's places are not the call's structural arguments",
            ));
        }
        for ((argument, (position, place)), parameter) in
            arguments.iter().zip(places).zip(parameters)
        {
            let Some(access) = reference_access(argument.access) else {
                continue;
            };
            if access == ReferenceAccess::WriteOnly {
                return Err(self.unsupported(concerns, "a write-only loan"));
            }
            if parameter.access != argument.access {
                return Err(self.foreign(
                    concerns,
                    "a loan's access is not its formal's declared access",
                ));
            }
            let lent = self.argument(argument, concerns)?;
            if lineage(self.machine, &tables.places, place)? != lent {
                return Err(self.foreign(
                    concerns,
                    "an invocation row's place is not the place the call lends",
                ));
            }
            let parent = self.incoming(argument.place);
            let lender = if parent.is_some() {
                Lender::Incoming
            } else {
                match self.root_kind(argument.place, concerns)? {
                    StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::OperationResult { .. } => Lender::Owner,
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    } if access == ReferenceAccess::Shared
                        && self
                            .declaration(structural_type, concerns)?
                            .shape
                            .is_shared_readable_byte_view() =>
                    {
                        Lender::Owner
                    }
                    _ => {
                        return Err(self.unsupported(
                            concerns,
                            "a loan of storage that is neither a formal nor a result",
                        ));
                    }
                }
            };
            let kind = ObligationKind::Loan {
                captured: place,
                access,
                lender,
            };
            let (obligation, row) = tables
                .obligations
                .iter()
                .find(|(obligation, row)| row.kind == kind && !claimed[index(obligation.get())])
                .ok_or_else(|| self.foreign(concerns, "a loan the call forms has no row"))?;
            if row.created_at != before {
                return Err(self.foreign(
                    Construct::Obligation(obligation),
                    "a loan is not created at the point before its call",
                ));
            }
            claimed[index(obligation.get())] = true;
            formed.push(Formed {
                obligation,
                invocation,
                position,
                access,
                lineage: lent,
                parent: parent.and_then(reference_access),
            });
        }
        Ok(())
    }

    /// The verdict on every `Loan` row, and each conflict found. A loan
    /// that may be formed is `Ok`; one that may not is a `Conflict`
    /// refusal, listed once in the conflicts with what it conflicts with.
    pub(crate) fn loans(
        &self,
        module: &TerminalModule,
        tables: &MachineTables,
    ) -> Checked<(Vec<(ObligationId, Checked<()>)>, Vec<ConflictingLoan>)> {
        let formed = self.formed(module, tables)?;
        let mut verdicts = Vec::with_capacity(formed.len());
        let mut conflicts = Vec::new();
        for (at, loan) in formed.iter().enumerate() {
            let exclusive = loan.access.is_exclusive();
            let mut found = None;
            if exclusive && loan.parent == Some(ReferenceAccess::Shared) {
                found = Some((None, LoanConflict::ExclusiveOfShared));
            }
            if found.is_none() {
                // Only an earlier loan of the same call: the later of two is
                // the one reported.
                for earlier in formed[..at]
                    .iter()
                    .filter(|earlier| earlier.invocation == loan.invocation)
                {
                    let earlier_exclusive = earlier.access.is_exclusive();
                    if (exclusive || earlier_exclusive) && overlap(&earlier.lineage, &loan.lineage)
                    {
                        let conflict = if exclusive && earlier_exclusive {
                            LoanConflict::TwiceExclusive
                        } else {
                            LoanConflict::ExclusiveAndShared
                        };
                        found = Some((Some(earlier.position), conflict));
                        break;
                    }
                }
            }
            let verdict = match found {
                None => Ok(()),
                Some((with, conflict)) => {
                    conflicts.push(ConflictingLoan {
                        loan: loan.obligation,
                        invocation: loan.invocation,
                        position: loan.position,
                        with,
                        conflict,
                    });
                    Err(Refusal {
                        reason: RefusalReason::Conflict,
                        machine: self.machine,
                        concerns: Construct::Obligation(loan.obligation),
                        detail: String::from(match conflict {
                            LoanConflict::TwiceExclusive => {
                                "an exclusive loan of storage an earlier argument of the same call \
                                 lends exclusively"
                            }
                            LoanConflict::ExclusiveAndShared => {
                                "a loan of storage an earlier argument of the same call lends, \
                                 one of the two exclusively"
                            }
                            LoanConflict::ExclusiveOfShared => {
                                "an exclusive reborrow of storage this machine holds shared"
                            }
                        }),
                    })
                }
            };
            verdicts.push((loan.obligation, verdict));
        }
        Ok((verdicts, conflicts))
    }
}
