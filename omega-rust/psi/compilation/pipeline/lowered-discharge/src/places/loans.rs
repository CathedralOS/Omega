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
storage (`Lender::Owner`). Every loan here is formed for one call and ends
when it returns, so none is live on an edge and no edge disposes of one.

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
use terminal_check_tables::edges::Actual;
use terminal_check_tables::obligations::{Lender, ObligationKind};
use terminal_check_tables::{
    Checked, Construct, InvocationId, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{StructuralAccess, TerminalMachine};

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
    fn formed(&self, machines: &[TerminalMachine], tables: &MachineTables) -> Checked<Vec<Formed>> {
        let mut claimed = vec![false; tables.obligations.len()];
        let mut formed = Vec::new();
        for block in &self.lowered.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for operation in &block.operations {
                let concerns = Construct::Operation(operation.id);
                let arguments = operation.kind.structural_arguments();
                let lends = arguments
                    .iter()
                    .any(|argument| argument.access != StructuralAccess::Owned);
                if lends {
                    let Some(inputs) = operation.kind.module_invocation() else {
                        return Err(self.unsupported(concerns, "a loan to something not a call"));
                    };
                    let callee = crate::terminal_machine(machines, inputs.callee, self.machine)?;
                    let site = InvocationSite::Operation(operation.id);
                    let (invocation, row) = tables
                        .invocations
                        .iter()
                        .find(|(_, row)| row.site == site)
                        .ok_or_else(|| self.foreign(concerns, "a call has no invocation row"))?;
                    // The structural lane's places, in argument order.
                    let places: Vec<_> = row
                        .actuals
                        .iter()
                        .enumerate()
                        .filter_map(|(position, actual)| match actual {
                            Actual::Place(place) => Some((position, *place)),
                            Actual::Value(_) => None,
                        })
                        .collect();
                    if places.len() != arguments.len()
                        || callee.structural_parameters.len() != arguments.len()
                    {
                        return Err(self.foreign(
                            concerns,
                            "an invocation row's places are not the call's structural arguments",
                        ));
                    }
                    for ((argument, (position, place)), parameter) in arguments
                        .iter()
                        .zip(places)
                        .zip(&callee.structural_parameters)
                    {
                        let Some(access) = reference_access(argument.access) else {
                            continue;
                        };
                        if access == ReferenceAccess::WriteOnly {
                            return Err(self.unsupported(concerns, "a write-only loan"));
                        }
                        // Terminal validation passes a borrowed argument only
                        // to a formal declared with the same access.
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
                        let (obligation, _) = tables
                            .obligations
                            .iter()
                            .find(|(obligation, row)| {
                                row.kind == kind && !claimed[index(obligation.get())]
                            })
                            .ok_or_else(|| {
                                self.foreign(concerns, "a loan the call forms has no row")
                            })?;
                        let row = terminal_check_tables::row(
                            &tables.obligations,
                            obligation,
                            self.machine,
                            Construct::Obligation(obligation),
                        )?;
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
                }
                before = ProgramPoint::AfterOperation(operation.id);
            }
            // A terminator lends nothing in this route: a no-normal call
            // that lends is refused where it is lowered.
            if block.terminator.module_invocation().is_some_and(|inputs| {
                inputs
                    .structural_arguments
                    .iter()
                    .any(|argument| argument.access != StructuralAccess::Owned)
            }) {
                return Err(self.unsupported(
                    Construct::Block(block.id),
                    "a no-normal call that lends storage",
                ));
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
        Ok(formed)
    }

    /// The verdict on every `Loan` row, and each conflict found. A loan
    /// that may be formed is `Ok`; one that may not is a `Conflict`
    /// refusal, listed once in the conflicts with what it conflicts with.
    pub(crate) fn loans(
        &self,
        machines: &[TerminalMachine],
        tables: &MachineTables,
    ) -> Checked<(Vec<(ObligationId, Checked<()>)>, Vec<ConflictingLoan>)> {
        let formed = self.formed(machines, tables)?;
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
