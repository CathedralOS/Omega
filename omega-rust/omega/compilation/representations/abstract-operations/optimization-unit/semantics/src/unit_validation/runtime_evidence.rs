//! Replays immutable declaration subjects against current optimizer operands.
//! Removed functions need no runtime inputs; an orphan call inside a retained
//! function refuses the rewrite. No old Terminal operand supplies a snapshot.
use crate::OptimizationUnitValidationError;
use abstract_operations::{AbstractOperation as O, AbstractStructuralCallResult as R};
use optimization_unit::PsiOptimizationUnit;
use terminal_psi::{DirectCallRole, EvidenceContractLaneKind};
use terminal_semantics::evidence_calls::{
    RuntimeEvidenceBody, RuntimeEvidenceInputs, RuntimeEvidenceOperation, RuntimeEvidenceResult,
    validate_runtime_evidence_inputs_in,
};

pub(super) fn validate(unit: &PsiOptimizationUnit) -> Result<(), OptimizationUnitValidationError> {
    validate_current(unit)
        .map_err(|reason| OptimizationUnitValidationError::InvalidRuntimeEvidence { reason })
}
fn validate_current(unit: &PsiOptimizationUnit) -> Result<(), &'static str> {
    let Some(catalog) = &unit.runtime_evidence else {
        if unit
            .functions
            .iter()
            .any(|function| !function.evidence_contract_lanes.is_empty())
        {
            return Err("current evidence lanes have no verified declaration catalog");
        }
        return Ok(());
    };
    if catalog.psi() != unit.psi {
        return Err("runtime evidence declaration catalog belongs to another Terminal input");
    }
    let calls = catalog
        .calls()
        .iter()
        .filter(|call| unit.function(call.caller).is_some())
        .cloned()
        .collect::<Vec<_>>();
    let lanes = unit
        .functions
        .iter()
        .flat_map(|function| function.evidence_contract_lanes.iter().cloned())
        .collect::<Vec<_>>();
    for function in &unit.functions {
        let expected = catalog
            .lanes()
            .iter()
            .filter(|lane| lane.machine == function.machine)
            .collect::<Vec<_>>();
        if !function.evidence_contract_lanes.iter().eq(expected) {
            return Err("optimizer changed the verified evidence declaration lane");
        }
    }
    let consumes_snapshots = calls
        .iter()
        .any(|call| call.runtime_call.is_some() && !call.evidence_arguments.is_empty());
    let checks_calls = consumes_snapshots
        || lanes
            .iter()
            .any(|lane| lane.kind == EvidenceContractLaneKind::Requires);
    // Declaration admission is unconditional; graph replay is demanded only
    // by an actual snapshot consumer. Missing call inputs refuse before it.
    let graph = if consumes_snapshots {
        crate::descriptor_results::optimized_graph(unit)
            .ok_or("runtime evidence has no reconstructible current descriptor graph")?
    } else {
        Vec::new()
    };
    let mut bodies = Vec::with_capacity(unit.functions.len());
    for function in &unit.functions {
        let mut declarations = catalog
            .declarations()
            .iter()
            .filter(|(machine, _)| *machine == function.machine);
        let (_, declaration) = declarations
            .next()
            .ok_or("runtime evidence body lost its checked declaration")?;
        if declarations.next().is_some() {
            return Err("runtime evidence body has ambiguous checked declarations");
        }
        let mut operations = Vec::new();
        for block in function.blocks.iter().filter(|_| checks_calls) {
            for (index, node) in block.nodes.iter().enumerate() {
                let (
                    site,
                    callee,
                    role,
                    arguments,
                    structural_arguments,
                    result,
                    selected_evidence,
                ) = match &node.operation {
                    O::Call {
                        psi_operation,
                        callee,
                        arguments,
                        result,
                        scalar_type,
                        ..
                    } => (
                        (*psi_operation).into(),
                        *callee,
                        DirectCallRole::Scalar,
                        arguments.clone(),
                        Vec::new(),
                        RuntimeEvidenceResult::Scalar {
                            id: *result,
                            scalar_type: *scalar_type,
                        },
                        Vec::new(),
                    ),
                    O::CallStructural {
                        psi_operation,
                        callee,
                        arguments,
                        structural_arguments,
                        result,
                        ..
                    } => {
                        let Some(callee) = callee.machine() else {
                            continue;
                        };
                        let (role, result, selected) = match result {
                            R::Unit => (
                                DirectCallRole::Unit,
                                RuntimeEvidenceResult::Unit,
                                Vec::new(),
                            ),
                            R::Scalar(value) => (
                                DirectCallRole::Scalar,
                                RuntimeEvidenceResult::Scalar {
                                    id: value.value,
                                    scalar_type: value.scalar_type,
                                },
                                Vec::new(),
                            ),
                            R::Structural {
                                result,
                                selected_evidence,
                            } => (
                                DirectCallRole::Structural,
                                RuntimeEvidenceResult::Structural(result.clone()),
                                selected_evidence.clone(),
                            ),
                        };
                        (
                            (*psi_operation).into(),
                            callee,
                            role,
                            arguments.clone(),
                            structural_arguments.clone(),
                            result,
                            selected,
                        )
                    }
                    O::InvokeNoNormal {
                        psi_edge,
                        callee,
                        arguments,
                        structural_arguments,
                        ..
                    } => (
                        semantic_vocabulary::InvocationSite::InvokeNoNormal(*psi_edge),
                        *callee,
                        DirectCallRole::NoNormal,
                        arguments.clone(),
                        structural_arguments.clone(),
                        RuntimeEvidenceResult::NoNormal,
                        Vec::new(),
                    ),
                    _ => continue,
                };
                operations.push(RuntimeEvidenceOperation {
                    site,
                    position: (
                        block.id,
                        u32::try_from(index)
                            .map_err(|_| "current evidence operation position exceeds u32")?,
                    ),
                    callee,
                    role,
                    arguments,
                    structural_arguments,
                    result,
                    selected_evidence,
                });
            }
        }
        let observes = calls.iter().any(|call| {
            call.caller == function.machine
                && call.runtime_call.is_some()
                && !call.evidence_arguments.is_empty()
        });
        let mut values = Vec::new();
        if observes {
            values.extend(function.parameters.iter().map(|value| value.value));
            for call in &operations {
                values.extend_from_slice(&call.arguments);
                if let Some((value, _)) = call.result.scalar() {
                    values.push(value);
                }
            }
            values.sort_unstable();
            values.dedup();
        }
        let scalar_expressions = values
            .into_iter()
            .filter_map(|value| {
                crate::descriptor_results::scalar_expression(function, value)
                    .map(|expression| (value, expression))
            })
            .collect();
        let payload_contracts = if observes {
            let current = terminal_semantics::descriptor_results::descriptor_machine(
                &graph,
                function.machine,
            )
            .ok_or("current evidence body has no descriptor machine")?;
            current
                .place_types
                .keys()
                .filter_map(|root| {
                    super::operation_contracts::structural_source_contract(function, *root)
                        .map(|source| (*root, source.payload_contract()))
                })
                .collect()
        } else {
            Vec::new()
        };
        bodies.push(RuntimeEvidenceBody {
            id: function.machine,
            entry: function.entry,
            declaration: declaration.clone(),
            parameters: function
                .parameters
                .iter()
                .map(|parameter| parameter.value)
                .collect(),
            structural_parameters: function.structural_parameters.clone(),
            outcome_specific_ensures: function
                .verified_contract
                .as_ref()
                .ok_or("runtime evidence body has no verified contract")?
                .outcome_specific_ensures
                .clone(),
            calls: operations,
            scalar_expressions,
            payload_contracts,
        });
    }
    validate_runtime_evidence_inputs_in(&RuntimeEvidenceInputs {
        identities: &unit.identities,
        structural_types: &unit.structural_types,
        structural_domains: &unit.structural_domains,
        proposition_applications: catalog.applications(),
        evidence_terms: catalog.terms(),
        evidence_contract_lanes: &lanes,
        proof_output_calls: &calls,
        machines: &bodies,
        graph: &graph,
    })
}
