use super::*;

pub(super) fn validate_contract_clause_kind(
    proposition: &Proposition,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_contract_clause_kind(proposition, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_clause_kind(premise, contract, clause)?;
            validate_contract_clause_kind(conclusion, contract, clause)
        }
        Proposition::ContentConservation(_) if clause == ContractClauseKind::Requires => {
            Err(ModuleError::ContentConservationRequiresEnsures { contract })
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_contract_scope(
    proposition: &Proposition,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralCaseMembership { .. } => Ok(()),
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            validate_integer_math_term_scope(left, allowed, contract, clause)?;
            validate_integer_math_term_scope(right, allowed, contract, clause)
        }
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_contract_scope(proposition, allowed, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_scope(premise, allowed, contract, clause)?;
            validate_contract_scope(conclusion, allowed, contract, clause)
        }
        Proposition::ContentConservation(_) => Ok(()),
    }
}

fn validate_integer_math_term_scope(
    term: &psi_core::IntegerMathTerm,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        psi_core::IntegerMathTerm::MathValue { value, .. } if !allowed.contains(value) => {
            Err(ModuleError::ContractValueOutsideScope {
                contract,
                clause,
                value: *value,
            })
        }
        psi_core::IntegerMathTerm::MathValue { .. }
        | psi_core::IntegerMathTerm::IntegerLiteral(_) => Ok(()),
        psi_core::IntegerMathTerm::Add(left, right)
        | psi_core::IntegerMathTerm::Subtract(left, right)
        | psi_core::IntegerMathTerm::Multiply(left, right) => {
            validate_integer_math_term_scope(left, allowed, contract, clause)?;
            validate_integer_math_term_scope(right, allowed, contract, clause)
        }
        psi_core::IntegerMathTerm::ShiftLeft { value, count } => {
            validate_integer_math_term_scope(value, allowed, contract, clause)?;
            validate_integer_math_term_scope(count, allowed, contract, clause)
        }
    }
}

fn validate_term_scope(
    term: &ScalarTerm,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        ScalarTerm::Value { id, .. } => {
            if !allowed.contains(id) {
                return Err(ModuleError::ContractValueOutsideScope {
                    contract,
                    clause,
                    value: *id,
                });
            }
        }
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
        | ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            validate_term_scope(left, allowed, contract, clause)?;
            validate_term_scope(right, allowed, contract, clause)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            validate_term_scope(value, allowed, contract, clause)?;
            validate_term_scope(count, allowed, contract, clause)?;
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => {
            validate_term_scope(operand, allowed, contract, clause)?;
        }
        ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}
