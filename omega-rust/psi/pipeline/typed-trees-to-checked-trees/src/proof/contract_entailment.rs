use checked_trees::{CheckedContractEntailmentAssumptionDischarge, MachineContractPlans};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::PrimitiveType;
use validation::{ContractEntailmentStandDown, ContractEntailmentStandDownReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedContractEntailmentAssumptionDischargeRecheckError {
    StandDownMissing,
    MachineMissing,
    MachineIsNotCheckedBody,
    ContractCoordinateMissing,
    FactCoordinateMissing,
    ContractIsNotPlainEnsures,
    FactIsNotPlainExpression,
    EntryStateMissing,
    ContractPlanMissing,
    ZeroContractCommitment,
    CoordinateOverflow,
    UnsupportedSource,
    CertificateMismatch,
    InvalidCertificate(&'static str),
    InvalidContext(semantic_vocabulary::PropositionError),
    KernelRejected(proof_admission::ProofError),
}

struct ParameterBinding {
    symbol: symbols::SymbolHandle,
    value: ValueId,
    scalar_type: ScalarType,
}

struct ReconstructedDischarge {
    certificate: CheckedContractEntailmentAssumptionDischarge,
    context: PropositionContext,
}

pub(crate) fn build_contract_entailment_assumption_discharges(
    program: &TypedTrees,
    contracts: &MachineContractPlans,
) -> Result<Vec<CheckedContractEntailmentAssumptionDischarge>, Vec<diagnostics::Diagnostic>> {
    let stand_downs = validation::collect_contract_entailment_stand_downs(program);
    let mut certificates = Vec::new();
    for stand_down in stand_downs.iter().filter(|stand_down| {
        stand_down.reason == ContractEntailmentStandDownReason::UnrecognizedInductiveBody
    }) {
        let reconstructed = reconstruct_discharge(program, contracts, *stand_down).map_err(|error| {
            vec![diagnostics::Diagnostic::error(format!(
                "failed to construct checked contract-entailment assumption discharge: {error:?}"
            ))]
        })?;
        let Some(reconstructed) = reconstructed else {
            continue;
        };
        accept_reconstructed(&reconstructed).map_err(|error| {
            vec![diagnostics::Diagnostic::error(format!(
                "compiler-constructed contract-entailment assumption discharge failed proof-kernel checking: {error:?}"
            ))]
        })?;
        certificates.push(reconstructed.certificate);
    }
    Ok(certificates)
}

/// Independently rebuild and check one checked-IR assumption discharge from
/// the retained typed program and its strong machine-contract plan.
pub fn recheck_contract_entailment_assumption_discharge(
    program: &TypedTrees,
    contracts: &MachineContractPlans,
    certificate: &CheckedContractEntailmentAssumptionDischarge,
) -> Result<(), CheckedContractEntailmentAssumptionDischargeRecheckError> {
    let stand_down = validation::collect_contract_entailment_stand_downs(program)
        .into_iter()
        .find(|stand_down| {
            stand_down.reason == ContractEntailmentStandDownReason::UnrecognizedInductiveBody
                && stand_down.machine_symbol == certificate.machine_symbol()
                && u32::try_from(stand_down.contract_index).ok()
                    == Some(certificate.contract_position())
                && u32::try_from(stand_down.fact_index).ok() == Some(certificate.fact_position())
        })
        .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::StandDownMissing)?;
    let reconstructed = reconstruct_discharge(program, contracts, stand_down)?
        .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::UnsupportedSource)?;
    if &reconstructed.certificate != certificate {
        return Err(CheckedContractEntailmentAssumptionDischargeRecheckError::CertificateMismatch);
    }
    accept_reconstructed(&reconstructed)
}

fn reconstruct_discharge(
    program: &TypedTrees,
    contracts: &MachineContractPlans,
    stand_down: ContractEntailmentStandDown,
) -> Result<Option<ReconstructedDischarge>, CheckedContractEntailmentAssumptionDischargeRecheckError>
{
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == stand_down.machine_symbol)
        .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::MachineMissing)?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return Err(
            CheckedContractEntailmentAssumptionDischargeRecheckError::MachineIsNotCheckedBody,
        );
    }
    let contract = program
        .machine_contracts(machine)
        .get(stand_down.contract_index)
        .ok_or(
            CheckedContractEntailmentAssumptionDischargeRecheckError::ContractCoordinateMissing,
        )?;
    if contract.kind != SignatureContractKind::Ensures {
        return Err(
            CheckedContractEntailmentAssumptionDischargeRecheckError::ContractIsNotPlainEnsures,
        );
    }
    let fact = program
        .proof_facts
        .span_or_empty(contract.facts)
        .get(stand_down.fact_index)
        .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::FactCoordinateMissing)?;
    let typed_trees::domain::ProofFact::Expression(goal_expression) = fact else {
        return Err(
            CheckedContractEntailmentAssumptionDischargeRecheckError::FactIsNotPlainExpression,
        );
    };

    let entry = program
        .machine_states(machine)
        .first()
        .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::EntryStateMissing)?;
    let parameters = parameter_bindings(program, entry)?;
    let context = PropositionContext::from_value_types(
        parameters
            .iter()
            .map(|parameter| (parameter.value, parameter.scalar_type)),
    )
    .map_err(CheckedContractEntailmentAssumptionDischargeRecheckError::InvalidContext)?;
    let Some(goal) = lower_proposition(program, *goal_expression, &parameters) else {
        return Ok(None);
    };

    let mut assumptions = Vec::new();
    for requires in program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(requires.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(assumption) = lower_proposition(program, *expression, &parameters) {
                assumptions.push(assumption);
            }
        }
    }
    let Some(selected_assumption_position) = selected_assumption_position(&assumptions, &goal)
    else {
        return Ok(None);
    };
    let selected_assumption_position =
        u32::try_from(selected_assumption_position).map_err(|_| {
            CheckedContractEntailmentAssumptionDischargeRecheckError::CoordinateOverflow
        })?;
    let contract_position = u32::try_from(stand_down.contract_index).map_err(|_| {
        CheckedContractEntailmentAssumptionDischargeRecheckError::CoordinateOverflow
    })?;
    let fact_position = u32::try_from(stand_down.fact_index).map_err(|_| {
        CheckedContractEntailmentAssumptionDischargeRecheckError::CoordinateOverflow
    })?;
    let machine_contract_commitment = contracts
        .for_machine(machine.symbol)
        .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::ContractPlanMissing)?
        .commitment;
    if machine_contract_commitment.is_zero() {
        return Err(
            CheckedContractEntailmentAssumptionDischargeRecheckError::ZeroContractCommitment,
        );
    }
    let certificate = CheckedContractEntailmentAssumptionDischarge::new(
        machine.symbol,
        contract_position,
        fact_position,
        machine_contract_commitment,
        assumptions,
        goal,
        selected_assumption_position,
    )
    .map_err(CheckedContractEntailmentAssumptionDischargeRecheckError::InvalidCertificate)?;
    Ok(Some(ReconstructedDischarge {
        certificate,
        context,
    }))
}

fn selected_assumption_position(assumptions: &[Proposition], goal: &Proposition) -> Option<usize> {
    assumptions.iter().position(|assumption| assumption == goal)
}

fn accept_reconstructed(
    reconstructed: &ReconstructedDischarge,
) -> Result<(), CheckedContractEntailmentAssumptionDischargeRecheckError> {
    let proof = proof_admission::ProofNode {
        conclusion: reconstructed.certificate.goal().clone(),
        rule: proof_admission::ProofRule::Assumption {
            index: usize::try_from(reconstructed.certificate.selected_assumption_position())
                .map_err(|_| {
                    CheckedContractEntailmentAssumptionDischargeRecheckError::CoordinateOverflow
                })?,
        },
    };
    proof_admission::accept_certificate(
        &reconstructed.context,
        reconstructed.certificate.goal(),
        reconstructed.certificate.assumptions(),
        &[],
        &proof,
    )
    .map(|_| ())
    .map_err(CheckedContractEntailmentAssumptionDischargeRecheckError::KernelRejected)
}

fn parameter_bindings(
    program: &TypedTrees,
    entry: &typed_trees::state::State,
) -> Result<Vec<ParameterBinding>, CheckedContractEntailmentAssumptionDischargeRecheckError> {
    let mut bindings = Vec::new();
    for (position, parameter) in program.state_parameters(entry).iter().enumerate() {
        if parameter.is_mutable {
            continue;
        }
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            continue;
        };
        let Some(scalar_type) = scalar_type(primitive) else {
            continue;
        };
        let raw = u64::try_from(position)
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::CoordinateOverflow)?;
        let value = ValueId::new(raw)
            .ok_or(CheckedContractEntailmentAssumptionDischargeRecheckError::CoordinateOverflow)?;
        bindings.push(ParameterBinding {
            symbol: parameter.symbol,
            value,
            scalar_type,
        });
    }
    Ok(bindings)
}

fn scalar_type(primitive: PrimitiveType) -> Option<ScalarType> {
    let (sign, bits) = match primitive {
        PrimitiveType::Bool => return Some(ScalarType::Boolean),
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Addr => return IntegerType::address(64).ok().map(ScalarType::Integer),
        PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
}

fn lower_proposition(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[ParameterBinding],
) -> Option<Proposition> {
    let proposition = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(true) => Proposition::Truth,
        ExpressionNode::Boolean(false) => Proposition::Falsehood,
        ExpressionNode::Name(_) => Proposition::Equal(
            lower_scalar_term(program, expression, parameters, ScalarType::Boolean)?,
            ScalarTerm::boolean(true),
        ),
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::And | BinaryOperator::Or => {
                let mut propositions = vec![
                    lower_proposition(program, binary.left, parameters)?,
                    lower_proposition(program, binary.right, parameters)?,
                ];
                propositions.sort();
                if binary.operator == BinaryOperator::And {
                    Proposition::Conjunction(propositions)
                } else {
                    Proposition::Disjunction(propositions)
                }
            }
            BinaryOperator::Equal
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                let scalar_type = infer_scalar_type(program, binary.left, parameters)
                    .or_else(|| infer_scalar_type(program, binary.right, parameters))?;
                let mut left = lower_scalar_term(program, binary.left, parameters, scalar_type)?;
                let mut right = lower_scalar_term(program, binary.right, parameters, scalar_type)?;
                match binary.operator {
                    BinaryOperator::Equal => {
                        if right < left {
                            std::mem::swap(&mut left, &mut right);
                        }
                        Proposition::Equal(left, right)
                    }
                    BinaryOperator::Less => Proposition::LessThan(left, right),
                    BinaryOperator::LessOrEqual => Proposition::LessOrEqual(left, right),
                    BinaryOperator::Greater => Proposition::LessThan(right, left),
                    BinaryOperator::GreaterOrEqual => Proposition::LessOrEqual(right, left),
                    _ => unreachable!(),
                }
            }
            _ => return None,
        },
        _ => return None,
    };
    proposition.validate().ok()?;
    Some(proposition)
}

fn infer_scalar_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[ParameterBinding],
) -> Option<ScalarType> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => Some(ScalarType::Boolean),
        ExpressionNode::Name(path) if path.members.count() == 1 => parameters
            .iter()
            .find(|parameter| parameter.symbol == path.symbol)
            .map(|parameter| parameter.scalar_type),
        ExpressionNode::Integer(literal) => {
            use numerics::literals::LandedIntegerType;
            let primitive = match literal.landing()?.landed_type {
                LandedIntegerType::I8 => PrimitiveType::I8,
                LandedIntegerType::I16 => PrimitiveType::I16,
                LandedIntegerType::I32 => PrimitiveType::I32,
                LandedIntegerType::I64 => PrimitiveType::I64,
                LandedIntegerType::U8 => PrimitiveType::U8,
                LandedIntegerType::U16 => PrimitiveType::U16,
                LandedIntegerType::U32 => PrimitiveType::U32,
                LandedIntegerType::U64 => PrimitiveType::U64,
                LandedIntegerType::Addr => PrimitiveType::Addr,
            };
            scalar_type(primitive)
        }
        _ => None,
    }
}

fn lower_scalar_term(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[ParameterBinding],
    expected: ScalarType,
) -> Option<ScalarTerm> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) if expected == ScalarType::Boolean => {
            Some(ScalarTerm::boolean(*value))
        }
        ExpressionNode::Name(path) if path.members.count() == 1 => {
            let parameter = parameters.iter().find(|parameter| {
                parameter.symbol == path.symbol && parameter.scalar_type == expected
            })?;
            Some(ScalarTerm::value(parameter.value, parameter.scalar_type))
        }
        ExpressionNode::Integer(literal) => {
            let ScalarType::Integer(integer_type) = expected else {
                return None;
            };
            let value = match integer_type.sign() {
                IntegerSign::Signed => IntegerValue::Signed(i128::from(literal.value_i64()?)),
                IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(literal.value_u64()?)),
            };
            ScalarTerm::integer(integer_type, value).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn false_or_non_assumption_goal_has_no_discharge() {
        assert_eq!(
            selected_assumption_position(&[Proposition::Truth], &Proposition::Falsehood),
            None
        );
        assert_eq!(
            selected_assumption_position(
                &[Proposition::Falsehood, Proposition::Truth],
                &Proposition::Truth,
            ),
            Some(1)
        );
    }
}
