//! Allocate each callable's formals and derive its contracts in that namespace.
//!
//! Emission and call substitution borrow the same record. Authored parameter
//! positions are retained separately from dense Terminal structural positions.

use super::RuntimeRequirementOwner;
use super::admission::AdmittedBody;
use super::claims::{LoweredUnitClaims, lower_unit_entry_claims};
use super::ordinary_calls;
use super::parameters::{lower_unit_parameters, lower_unit_scalar_parameter_types};
use crate::psi_lowering::{
    CheckedTrees, LoweringError, Proposition, SemanticDomainId, StructuralDomainId,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeId, ValueDeclaration,
    allocate_dense, lower_structural_runtime_requirement, unsupported, value_id,
};
use symbols::SymbolHandle;

pub(super) struct MachineSignature {
    pub source: SymbolHandle,
    pub parameters: Vec<StructuralParameterDeclaration>,
    pub scalar_parameters: Vec<ValueDeclaration>,
    pub predicate_parameters: Vec<StructuralParameterDeclaration>,
    pub claims: LoweredUnitClaims,
    pub runtime_requirements: Vec<Proposition>,
}

impl MachineSignature {
    pub(super) fn call_target(&self) -> ordinary_calls::Target<'_> {
        ordinary_calls::Target {
            parameters: &self.parameters,
            scalar_parameters: &self.scalar_parameters,
            predicate_parameters: &self.predicate_parameters,
            runtime_requirements: &self.runtime_requirements,
        }
    }
}

pub(super) fn find(
    signatures: &[MachineSignature],
    source: SymbolHandle,
) -> Result<&MachineSignature, LoweringError> {
    signatures
        .iter()
        .find(|signature| signature.source == source)
        .ok_or(LoweringError::Unsupported(
            "call target has no allocated machine signature",
        ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    checked: &CheckedTrees,
    bodies: &[AdmittedBody<'_>],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
    structural_types: &[StructuralTypeDeclaration],
    requirements_owner: RuntimeRequirementOwner,
    next_place: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<MachineSignature>, LoweringError> {
    let mut signatures = Vec::with_capacity(bodies.len());

    // Allocate every formal before deriving any contract. This preserves the
    // closure's allocation and validation order and avoids placeholder values.
    for admitted in bodies {
        let body = admitted.source();
        let plan = body.entry()?;
        if body.qualifications().iter().any(|domain| {
            !plan
                .structural_parameters
                .iter()
                .any(|parameter| parameter.qualifications.contains(domain))
        }) {
            return unsupported(
                "Unit body qualification is not represented by an exact structural parameter precondition",
            );
        }
        let parameters =
            lower_unit_parameters(plan.structural_parameters, type_ids, domain_ids, next_place)?;
        let scalar_parameters = lower_unit_scalar_parameter_types(plan.scalar_parameters)?
            .into_iter()
            .map(|scalar_type| {
                Ok(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(allocate_dense(next_value)?),
                    scalar_type,
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        // Claims stay machine-local: unrelated callees cannot shift these IDs.
        let claims =
            lower_unit_entry_claims(plan.machine, plan.state, plan.entry_claims, &parameters)?;
        signatures.push(MachineSignature {
            source: plan.machine,
            parameters,
            scalar_parameters,
            predicate_parameters: Vec::new(),
            claims,
            runtime_requirements: Vec::new(),
        });
    }

    for (signature, admitted) in signatures.iter_mut().zip(bodies) {
        let plan = admitted.source().entry()?;
        if plan.structural_parameters.len() != signature.parameters.len() {
            return unsupported("Unit predicate signature does not match its structural roster");
        }
        signature.predicate_parameters = plan
            .structural_parameters
            .iter()
            .zip(&signature.parameters)
            .map(|(source, terminal)| StructuralParameterDeclaration {
                position: source.position,
                ..terminal.clone()
            })
            .collect();
    }

    for signature in &mut signatures {
        let Some(contract) = checked.facts.contract_plans.for_machine(signature.source) else {
            continue;
        };
        // Requirements justify body operations and calls independently of
        // whether the published crash ceiling mentions gated arithmetic.
        if (requirements_owner == RuntimeRequirementOwner::UnitClosure
            && contract.crash.structural_runtime_requirements().is_some())
            || contract.crash.uses_structural_proof_gated_arithmetic()
        {
            let checked_requirements = contract.crash.structural_runtime_requirements().ok_or(
                LoweringError::Unsupported(
                    "proof-gated structural arithmetic lacks a complete checked requirement package",
                ),
            )?;
            let requirements = checked_requirements
                .iter()
                .map(|requirement| {
                    lower_structural_runtime_requirement(
                        requirement,
                        &signature.scalar_parameters,
                        &signature.predicate_parameters,
                        structural_types,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut keyed = requirements
                .into_iter()
                .map(|requirement| {
                    terminal_codec::canonical_proposition_order_key(&requirement)
                        .map(|key| (key, requirement))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "structural runtime requirement is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            signature.runtime_requirements = keyed
                .into_iter()
                .map(|(_, requirement)| requirement)
                .collect();
        }
    }
    Ok(signatures)
}
