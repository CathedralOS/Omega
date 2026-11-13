/*
The closed list of obligations. An obligation is an identity, not a fact.
Two families share one table and one identity space:

- Conserved obligations (claims, loans, windows, debts) live across edges.
  Every edge accounts for each incoming one exactly once
  (`edges::Disposition`), and none is ever dropped silently.
- Proof questions are reconstructed at one site and discharged there, once,
  by evidence; they do not travel along edges.

This enum is the one extension point for obligations. A construct-family
lane adds a variant here, with the specification sentence that creates it,
and a row in the crate README. It never adds a table or a key form.
*/

use language_core::{Multiplicity, ReferenceAccess};
use semantic_vocabulary::{
    BlockId, ClaimId, CrashCause, CycleComponentId, DomainSemanticId, EdgeId, ObligationId,
    OperationId,
};

use crate::{ExactPlaceId, InvocationId, ProgramPoint};

/// One obligation, indexed by the lowered form's `ObligationId`. Elaboration
/// mints an `ObligationId` for every obligation it reconstructs, of either
/// family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObligationRow {
    pub kind: ObligationKind,
    pub created_at: ProgramPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationKind {
    // Conserved obligations.
    /// An owned claim. `wiki/spec/terminal-psi/ownership.md`: "Structural
    /// ownership is an exact live set of roots, projected claims, and
    /// remaining subtrees". `established_at` is where it was established, not
    /// its current home: "A claim declaration's path describes its
    /// establishment, not its permanent location." `frontier` is the
    /// lowered form's claim identity for the same claim.
    Claim {
        established_at: ExactPlaceId,
        multiplicity: Multiplicity,
        frontier: ClaimId,
    },
    /// A loan. `wiki/spec/terminal-psi/loans.md`: "Loan resource | Exact
    /// occurrence, owner lineage, captured place, access, parent lifetime,
    /// and restoration obligation." The occurrence is `created_at`.
    Loan {
        captured: ExactPlaceId,
        access: ReferenceAccess,
        lender: Lender,
    },
    /// Restoration debt on a hole in borrowed storage.
    /// `wiki/spec/language/ownership.md`: "The move transfers the removed
    /// value's exact custody to its destination and records a restoration
    /// obligation on the original place and loan."
    RestorationWindow {
        hole: ExactPlaceId,
        loan: ObligationId,
    },
    /// A domain that a write broke and that must be proved again.
    /// `wiki/spec/language/dependent_values.md`: "Otherwise the write opens a
    /// window on the place, whose actual new contents remain available to
    /// ordinary flow reasoning. The domain must be proved again at the next
    /// consumption point". Distinct from `RestorationWindow`: "Ownership
    /// restoration and domain restoration are separate obligations;
    /// satisfying one cannot discharge the other."
    DomainDebt {
        place: ExactPlaceId,
        domain: DomainSemanticId,
    },
    /// Storage that must be established as a whole valid value before typed
    /// use. dependent_values.md: "Exclusive storage recasts may open initial
    /// target-establishment debt before a valid target value exists."
    EstablishmentDebt { place: ExactPlaceId },

    // Proof questions.
    /// One callee `requires` clause at one invocation, by position.
    /// `wiki/spec/terminal-psi/fact_validity.md`: "Call coverage reconstructs
    /// one exact obligation for every callee `requires` clause."
    CallRequirement {
        invocation: InvocationId,
        clause: u32,
    },
    /// The safety of a partial operation, proved before it executes.
    /// `wiki/spec/terminal-psi/control_flow.md`: "Each partial operation still
    /// proves its own safety from facts available before it executes".
    OperationSafety { site: ExecutionSite },
    /// One clause of a data type's `requires`, owed by one operation that
    /// forms or changes a value of that type: a record or case construction,
    /// a zero establishment, or a store into a field of a place of that type
    /// or of a type that contains it. `wiki/spec/language/data_and_literals.md`:
    /// "Construction and every restoring write prove the owning
    /// default-domain clauses." The goal is the clause with `self` replaced
    /// by the values the place holds once the operation completes, proved
    /// from what is known before it.
    /// `wiki/spec/terminal-psi/structural_access.md`: "The obligation binds
    /// the initializer or replacement SSA value, never the old field
    /// contents."
    ///
    /// One operation can owe clauses of several types: a store to `l.from.x`
    /// owes `Point`'s for `l.from` and `Line`'s for `l`. `clause` is the
    /// position in the operation's whole list: the types that own the
    /// written path from the innermost outward, or for a zero establishment
    /// the data-typed places of the zero value in declaration order, parent
    /// first; within one type its clauses in authored order.
    DataDomain { operation: OperationId, clause: u32 },
    /// A block invariant on one incoming edge. control_flow.md: "A scalar
    /// block invariant is a semantic row identifying a machine, destination
    /// block, scalar predicate, and one obligation for every actual incoming
    /// edge."
    ArrivalInvariant { edge: EdgeId },
    /// One published guarantee, by position, on one normal exit edge.
    /// control_flow.md: "All normal returns, including returns after a cycle,
    /// must establish the published guarantees." The real obligation is per
    /// return edge; `published` names the clause's `PublishedGuarantee`, the
    /// identity the module's `ContractClause` carries.
    NormalGuarantee {
        exit: EdgeId,
        clause: u32,
        published: ObligationId,
    },
    /// One published `ensures` clause, by position: the per-clause identity
    /// Terminal Psi's `ContractClause` holds. It holds exactly when every
    /// normal return edge's `NormalGuarantee` for this clause holds, which is
    /// vacuously so for a machine with no normal return (control_flow.md:
    /// "All normal returns ... must establish the published guarantees").
    /// It is discharged from those rows, never proved on its own.
    PublishedGuarantee { clause: u32 },
    /// A crash site's guard and same-cause route coverage, one per site and
    /// cause. `wiki/spec/terminal-psi/calls_and_outcomes.md`: "The verifier
    /// proves every site guard from independent entry/CFG facts and checks
    /// same-cause coverage".
    CrashCoverage { site: CrashSite, cause: CrashCause },
    /// Well-foundedness of one recursive component's rank relation.
    /// control_flow.md: "One grouped control judgment covers each
    /// reconstructed component, with a shared well-foundedness question and
    /// comparison obligations for every internal edge."
    WellFoundedness { component: CycleComponentId },
    /// The rank comparison on one internal edge of that component.
    RankComparison {
        component: CycleComponentId,
        edge: EdgeId,
    },
}

/// The instruction or terminator whose execution consumes an obligation.
/// A terminator is identified by its block; an operation has its own identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionSite {
    Block(BlockId),
    Operation(OperationId),
}

/// Crash coverage uses the same instruction and terminator coordinates.
pub type CrashSite = ExecutionSite;

/// Where a loan's authority comes from. loans.md: "A direct reborrow names
/// its immediate parent resource, exact formation event, and projection. A
/// multihop chain cannot skip intermediate parents."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lender {
    /// Lent directly from owned storage at the captured place's root.
    Owner,
    /// Reborrowed from this immediate parent loan.
    Loan(ObligationId),
    /// Reborrowed from the loan the invocation passed in at the captured
    /// place's root, a borrowed formal. That parent resource is the
    /// caller's, so this machine has no row for it and names it by the
    /// root; its access is the formal's declared access. loans.md: "A
    /// direct reborrow names its immediate parent resource, exact formation
    /// event, and projection."
    Incoming,
}

impl ObligationKind {
    /// True for the family every edge must account for exactly once.
    pub const fn is_conserved(&self) -> bool {
        matches!(
            self,
            Self::Claim { .. }
                | Self::Loan { .. }
                | Self::RestorationWindow { .. }
                | Self::DomainDebt { .. }
                | Self::EstablishmentDebt { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::PsiSemanticId;

    #[test]
    fn resources_are_conserved_and_questions_are_not() {
        let place = ExactPlaceId::new(1).unwrap();
        let edge = EdgeId::new(1).unwrap();
        let loan = ObligationKind::Loan {
            captured: place,
            access: ReferenceAccess::Mutable,
            lender: Lender::Owner,
        };
        let window = ObligationKind::RestorationWindow {
            hole: place,
            loan: ObligationId::new(1).unwrap(),
        };
        assert!(loan.is_conserved() && window.is_conserved());
        assert!(!ObligationKind::ArrivalInvariant { edge }.is_conserved());
        assert!(
            !ObligationKind::DataDomain {
                operation: OperationId::new(1).unwrap(),
                clause: 0,
            }
            .is_conserved()
        );
        assert!(
            !ObligationKind::NormalGuarantee {
                exit: edge,
                clause: 0,
                published: ObligationId::new(1).unwrap(),
            }
            .is_conserved()
        );
    }
}
