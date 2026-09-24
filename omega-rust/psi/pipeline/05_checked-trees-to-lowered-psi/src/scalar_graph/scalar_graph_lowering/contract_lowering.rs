//! Lowering closed scalar contracts and content evidence.

use crate::emission::scalar_types::integer_value;
use crate::scalar_graph::scalar_graph_lowering::known_evaluation::KnownDirectScalar;
use crate::scalar_graph::{
    CheckedTrees, ClosedScalarContractValue, ClosedScalarValueContractPlan, IeeeFloatFormat,
    LoweredContentIdentityReshuffles, LoweredContentPartitionCompositions, LoweringError,
    PrimitiveType, ScalarType, lower_content_identity_reshuffles,
    lower_content_partition_compositions, unsupported,
};

pub(crate) fn lower_content_evidence(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
) -> Result<
    (
        LoweredContentIdentityReshuffles,
        LoweredContentPartitionCompositions,
    ),
    LoweringError,
> {
    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let mut identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
    let partition_facts = checked
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let partition_compositions =
        lower_content_partition_compositions(&partition_facts, &mut identity_reshuffles)?;
    Ok((identity_reshuffles, partition_compositions))
}

pub(crate) fn closed_scalar_contract_plan(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<&ClosedScalarValueContractPlan, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|plan| &plan.closed_scalar_values)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked contract plan",
        ))
}

pub(crate) fn validate_empty_scalar_contract_source(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    use checked_trees::signature::SignatureContractKind;

    let source_machine = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .ok_or(LoweringError::Unsupported(
            "empty scalar contract has no authored machine",
        ))?;
    let contracts = checked
        .signature_contracts
        .span(source_machine.contracts)
        .ok_or(LoweringError::Unsupported(
            "empty scalar contract has an invalid authored clause span",
        ))?;
    // Match the closed scalar contract owner's roster: named witnesses have
    // separate evidence custody, but outcome-specific clauses never disappear.
    // Crash routes are retained independently below graph preparation.
    if contracts.iter().any(|contract| match contract.kind {
        SignatureContractKind::EnsuresForResultCase { .. } => true,
        SignatureContractKind::Requires | SignatureContractKind::Ensures => {
            contract.binding.is_none()
        }
        SignatureContractKind::Crashes { .. } => false,
    }) {
        return unsupported("empty scalar contract would erase an authored normal clause");
    }
    let entry =
        checked
            .machine_states(source_machine)
            .first()
            .ok_or(LoweringError::Unsupported(
                "empty scalar contract has no authored entry state",
            ))?;
    // Bracket ranges contribute requires rows even without signature clauses;
    // other qualifications retain their existing, separate contract owners.
    if checked.state_parameters(entry).iter().any(|parameter| {
        checked_trees::wire::type_reference_carries_range(&checked.typed, parameter.type_reference)
    }) {
        return unsupported("empty scalar contract would erase an authored parameter range");
    }
    Ok(())
}

/// Recognize the one D40 contract shape whose entire value is proof-only.
/// Runtime scalar-contract lowering must not manufacture an IEEE comparison
/// or a `FloatMeaning` runtime value for this clause, so every checked source
/// coordinate is replayed before the clause is erased from `MachineContract`.
pub(crate) fn exact_direct_result_float_meaning_reflexivity_contract(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    result_type: ScalarType,
    allow_crash_contracts: bool,
) -> bool {
    let Some(contract_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|plan| &plan.closed_scalar_values)
    else {
        return false;
    };
    if !contract_plan.requires().is_empty()
        || contract_plan.ensures() != [None]
        || contract_plan.has_outcome_specific_clauses()
        || (!allow_crash_contracts && contract_plan.has_crash_clauses())
    {
        return false;
    }
    let Some(source_machine) = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
    else {
        return false;
    };
    let mut ensures_contracts =
        checked
            .machine_contracts(source_machine)
            .iter()
            .filter(|contract| {
                contract.binding.is_none()
                    && contract.kind == checked_trees::signature::SignatureContractKind::Ensures
            });
    let Some(ensures) = ensures_contracts.next() else {
        return false;
    };
    if ensures_contracts.next().is_some() {
        return false;
    }
    let [checked_trees::domain::ProofFact::Expression(source_expression)] =
        checked.proof_facts.span_or_empty(ensures.facts)
    else {
        return false;
    };
    let Some(projection) = checked
        .facts
        .proof
        .direct_result_float_meaning_reflexivity(machine, *source_expression)
    else {
        return false;
    };
    if projection.validate().is_err() {
        return false;
    }
    let checked_trees::CheckedFloatProjectionSource::DirectMachineResult(result) =
        projection.source
    else {
        return false;
    };
    if result.owner_machine != machine {
        return false;
    }
    matches!(
        (result_type, result.fallback.primitive, projection.operation),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            PrimitiveType::F32,
            numerics::float_projection::FloatProjectionOperation::Meaning32,
        ) | (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            PrimitiveType::F64,
            numerics::float_projection::FloatProjectionOperation::Meaning64,
        )
    )
}

pub(crate) fn validate_closed_scalar_contract(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    result_type: ScalarType,
    expected_value: Option<KnownDirectScalar>,
    allow_crash_contracts: bool,
) -> Result<KnownDirectScalar, LoweringError> {
    let contract = closed_scalar_contract_plan(checked, machine)?;
    let ([Some(requires)], [Some(ensures)]) = (contract.requires(), contract.ensures()) else {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
    if contract.has_outcome_specific_clauses()
        || (!allow_crash_contracts && contract.has_crash_clauses())
    {
        return unsupported("machine must have exactly one requires and one ensures clause");
    }
    let (requires, ensures) = match (result_type, requires, ensures) {
        (
            ScalarType::Boolean,
            ClosedScalarContractValue::Boolean(requires),
            ClosedScalarContractValue::Boolean(ensures),
        ) => (
            KnownDirectScalar::Boolean(*requires),
            KnownDirectScalar::Boolean(*ensures),
        ),
        (
            ScalarType::Integer(_),
            ClosedScalarContractValue::Integer(requires),
            ClosedScalarContractValue::Integer(ensures),
        ) => (
            KnownDirectScalar::Integer(integer_value(requires, result_type)?),
            KnownDirectScalar::Integer(integer_value(ensures, result_type)?),
        ),
        _ => return unsupported("contract scalar type must match the machine result type"),
    };
    if requires != ensures {
        return unsupported("requires and ensures must carry the same closed equality");
    }
    if expected_value.is_some_and(|expected| expected != requires) {
        return match result_type {
            ScalarType::Boolean => {
                unsupported("Boolean contract literal must match the compile-known result")
            }
            ScalarType::Integer(_) => {
                unsupported("contract literals must equal the executed literal")
            }
            ScalarType::IeeeFloat(_) => {
                unsupported("closed scalar contract evaluation does not carry IEEE float literals")
            }
        };
    }
    Ok(requires)
}
