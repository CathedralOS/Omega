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

// Logical contracts retain supplied requirements and proof goals, not runtime
// observations. Reconstruct their exact subjects without granting or requiring
// read access; executable observations and premise availability check separately.
pub(super) fn validate_contract_scope(
    module: &TerminalModule,
    machine: &TerminalMachine,
    proposition: &Proposition,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::StructuralCaseMembership { .. } => Ok(()),
        Proposition::IeeeFloatComparison {
            format,
            left,
            right,
            ..
        } => {
            for field in [left, right] {
                if !matches!(
                    structural_leaf_type(module, machine, field.root(), field.path()),
                    Some(StructuralFieldType::IeeeFloat(actual)) if actual == format
                ) {
                    return Err(ModuleError::InvalidIeeeFloatFieldTerm {
                        machine: machine.id,
                        root: field.root(),
                        path: field.path().to_vec(),
                        format: *format,
                    });
                }
            }
            Ok(())
        }
        Proposition::ByteSequenceEqual { left, right } => {
            for field in [left, right] {
                if !matches!(
                    structural_leaf_type(module, machine, field.root(), field.path()),
                    Some(StructuralFieldType::ByteSequence(_))
                ) {
                    return Err(ModuleError::InvalidByteSequenceFieldTerm {
                        machine: machine.id,
                        root: field.root(),
                        path: field.path().to_vec(),
                    });
                }
            }
            Ok(())
        }
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            validate_integer_math_term_scope(left, allowed, contract, clause)?;
            validate_integer_math_term_scope(right, allowed, contract, clause)
        }
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_term_scope(module, machine, left, allowed, contract, clause)?;
            validate_term_scope(module, machine, right, allowed, contract, clause)
        }
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_contract_scope(module, machine, proposition, allowed, contract, clause)?;
            }
            Ok(())
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_contract_scope(module, machine, premise, allowed, contract, clause)?;
            validate_contract_scope(module, machine, conclusion, allowed, contract, clause)
        }
        Proposition::ContentConservation(_) => Ok(()),
    }
}

fn validate_integer_math_term_scope(
    term: &semantic_vocabulary::IntegerMathTerm,
    allowed: &BTreeSet<ValueId>,
    contract: ContractId,
    clause: ContractClauseKind,
) -> Result<(), ModuleError> {
    match term {
        semantic_vocabulary::IntegerMathTerm::MathValue { value, .. }
            if !allowed.contains(value) =>
        {
            Err(ModuleError::ContractValueOutsideScope {
                contract,
                clause,
                value: *value,
            })
        }
        semantic_vocabulary::IntegerMathTerm::MathValue { .. }
        | semantic_vocabulary::IntegerMathTerm::IntegerLiteral(_) => Ok(()),
        semantic_vocabulary::IntegerMathTerm::Add(left, right)
        | semantic_vocabulary::IntegerMathTerm::Subtract(left, right)
        | semantic_vocabulary::IntegerMathTerm::Multiply(left, right) => {
            validate_integer_math_term_scope(left, allowed, contract, clause)?;
            validate_integer_math_term_scope(right, allowed, contract, clause)
        }
        semantic_vocabulary::IntegerMathTerm::ShiftLeft { value, count } => {
            validate_integer_math_term_scope(value, allowed, contract, clause)?;
            validate_integer_math_term_scope(count, allowed, contract, clause)
        }
    }
}

fn validate_term_scope(
    module: &TerminalModule,
    machine: &TerminalMachine,
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
            validate_term_scope(module, machine, left, allowed, contract, clause)?;
            validate_term_scope(module, machine, right, allowed, contract, clause)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            validate_term_scope(module, machine, value, allowed, contract, clause)?;
            validate_term_scope(module, machine, count, allowed, contract, clause)?;
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => {
            validate_term_scope(module, machine, operand, allowed, contract, clause)?;
        }
        ScalarTerm::BooleanField { root, path } => {
            if !matches!(
                structural_leaf_type(module, machine, *root, path),
                Some(StructuralFieldType::Scalar(ScalarType::Boolean))
            ) {
                return Err(ModuleError::InvalidBooleanFieldTerm {
                    machine: machine.id,
                    root: *root,
                    path: path.clone(),
                });
            }
        }
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            if !matches!(
                structural_leaf_type(module, machine, *root, path),
                Some(StructuralFieldType::Scalar(ScalarType::Integer(actual))) if actual == scalar_type
            ) && !matches!(
                structural_leaf_type(module, machine, *root, path),
                Some(StructuralFieldType::BoundedInteger(bounded)) if bounded.integer_type() == *scalar_type
            ) {
                return Err(ModuleError::InvalidIntegerFieldTerm {
                    machine: machine.id,
                    root: *root,
                    path: path.clone(),
                    scalar_type: *scalar_type,
                });
            }
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}
