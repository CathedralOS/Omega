//! Conservative entry origins for unversioned structural field terms used in
//! private crash proofs. Owned and mutable roots may change through calls;
//! only an exact shared-borrow parameter is stable without mutation custody.

use semantic_vocabulary::{ContentTerm, PlaceId, Proposition, ScalarTerm};
use terminal_psi::{StructuralAccess, TerminalMachine};

pub(super) fn retains_entry_meaning(proposition: &Proposition, machine: &TerminalMachine) -> bool {
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => scalar(left, machine) && scalar(right, machine),
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => children
            .iter()
            .all(|child| retains_entry_meaning(child, machine)),
        Proposition::Implication {
            premise,
            conclusion,
        } => retains_entry_meaning(premise, machine) && retains_entry_meaning(conclusion, machine),
        Proposition::IeeeFloatComparison { left, right, .. } => {
            shared_parameter(left.root(), machine) && shared_parameter(right.root(), machine)
        }
        Proposition::ByteSequenceEqual { left, right } => {
            shared_parameter(left.root(), machine) && shared_parameter(right.root(), machine)
        }
        Proposition::StructuralCaseMembership { subject, .. } => {
            shared_parameter(subject.root(), machine)
        }
        Proposition::ContentConservation(conservation) => {
            content(conservation.left(), machine) && content(conservation.right(), machine)
        }
        // Mathematical values and atoms retain their separate semantic IDs;
        // they contain no unversioned structural observations.
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::IntegerMathEqual(..)
        | Proposition::IntegerMathLessThan(..)
        | Proposition::IntegerMathLessOrEqual(..) => true,
    }
}

fn shared_parameter(root: PlaceId, machine: &TerminalMachine) -> bool {
    machine.structural_parameters.iter().any(|parameter| {
        parameter.place == root && parameter.access == StructuralAccess::SharedBorrow
    })
}

fn content(term: &ContentTerm, machine: &TerminalMachine) -> bool {
    match term {
        ContentTerm::Projection { subject, .. } => shared_parameter(subject.root, machine),
        ContentTerm::Separate(children) => children.iter().all(|child| content(child, machine)),
    }
}

fn scalar(term: &ScalarTerm, machine: &TerminalMachine) -> bool {
    match term {
        ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
            // Structural validation has already rejoined the parameter place,
            // field path and declared leaf type. Do not recover another root.
            shared_parameter(*root, machine)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar(operand, machine),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { left, right, .. } => {
            scalar(left, machine) && scalar(right, machine)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar(value, machine) && scalar(count, machine)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => true,
    }
}
