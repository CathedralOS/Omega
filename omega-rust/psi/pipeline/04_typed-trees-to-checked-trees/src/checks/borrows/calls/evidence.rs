//! One invocation's recorded non-interference comparisons. Admission and replay
//! reconstruct subjects at the original call boundary and run the same bound
//! judgment; retained rows are outputs to compare, never hints to that judgment.

use arena::Handle;
use checked_trees::{
    BorrowArgumentAccessFact, BorrowCallCompatibilityOperand, BorrowCallCompatibilitySubject,
    BorrowCallFact, BorrowCompatibilityConclusion, BorrowCompatibilityDerivation,
    BorrowCompatibilityFormation, BorrowFacts, BorrowLoanFact, CapturedPlace,
    CheckedBorrowCallCompatibilityCertificate, FlowStateFact,
};

pub(super) fn argument_operand(
    borrow: &BorrowFacts,
    ordinal: usize,
    access: &BorrowArgumentAccessFact,
) -> BorrowCallCompatibilityOperand {
    BorrowCallCompatibilityOperand {
        subject: BorrowCallCompatibilitySubject::Argument(ordinal),
        place: CapturedPlace {
            root_symbol: access.root_symbol,
            segments: borrow.access_segments(access).to_vec(),
        },
        access: access.kind.clone(),
    }
}

pub(super) fn loan_operand(
    borrow: &BorrowFacts,
    handle: Handle<BorrowLoanFact>,
    loan: &BorrowLoanFact,
) -> BorrowCallCompatibilityOperand {
    BorrowCallCompatibilityOperand {
        subject: BorrowCallCompatibilitySubject::ActiveLoan(handle),
        place: CapturedPlace {
            root_symbol: loan.root_symbol,
            segments: borrow.loan_segments(loan).to_vec(),
        },
        access: loan.kind.clone(),
    }
}

use super::super::overlap::{
    StatedOrderingPremise, captured_place_compatibility_with_selector_snapshot,
};

pub(super) struct CallCompatibility<'call> {
    pub state: &'call FlowStateFact,
    pub handle: Handle<BorrowCallFact>,
    pub call: &'call BorrowCallFact,
    pub certificates: &'call mut Vec<CheckedBorrowCallCompatibilityCertificate>,
}

impl CallCompatibility<'_> {
    pub(super) fn non_interfering(
        &mut self,
        program: &typed_trees::TypedTrees,
        left: BorrowCallCompatibilityOperand,
        right: BorrowCallCompatibilityOperand,
        premises: &[StatedOrderingPremise],
    ) -> bool {
        let evidence = captured_place_compatibility_with_selector_snapshot(
            program,
            &left.place,
            &left.access,
            &right.place,
            &right.access,
            premises,
        );
        if !evidence.compatibility.non_interfering {
            return false;
        }
        let derivation = if evidence.premises.is_empty() {
            BorrowCompatibilityDerivation::Structural
        } else {
            BorrowCompatibilityDerivation::Premised
        };
        self.certificates
            .push(CheckedBorrowCallCompatibilityCertificate {
                formation: BorrowCompatibilityFormation {
                    machine_symbol: self.state.machine_symbol,
                    state_symbol: self.state.state_symbol,
                    statement_index: self.call.statement_index,
                },
                call: self.handle,
                call_ordinal: self.call.call_ordinal,
                target_symbol: self.call.target_symbol,
                receiver_symbol: self.call.receiver_symbol,
                left,
                right,
                selector_snapshot: evidence.selector_snapshot,
                premises: evidence.premises,
                conclusion: BorrowCompatibilityConclusion {
                    disjoint: evidence.compatibility.disjoint,
                    containment: evidence.compatibility.containment,
                    non_interfering: evidence.compatibility.non_interfering,
                },
                derivation,
            });
        true
    }
}
