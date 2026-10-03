//! Allocate each callable's formals and derive its contracts in that namespace.
//!
//! Emission and call substitution borrow the same record. Authored parameter
//! positions are retained separately from dense Terminal structural positions.

use super::admission::AdmittedBody;
use super::claims::{LoweredUnitClaims, lower_unit_entry_claims};
use super::ordinary_calls;
use super::parameters::{lower_unit_parameters, lower_unit_scalar_parameter_types};
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::terminal_identities::StructuralDomainBinding;
use crate::terminal_identities::allocate_dense;
use crate::terminal_identities::value_id;
use checked_trees::CheckedTrees;

use semantic_vocabulary::Proposition;

use semantic_vocabulary::StructuralTypeId;
use symbols::SymbolHandle;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::ValueDeclaration;

#[derive(Clone)]
pub(super) struct MachineSignature {
    pub source: SymbolHandle,
    pub parameters: Vec<StructuralParameterDeclaration>,
    pub scalar_parameters: Vec<ValueDeclaration>,
    /// Proof-only erased scalar formals in authored order.
    pub erased_scalar_parameters: Vec<ValueDeclaration>,
    /// Erased proof-only formals in authored order. The checked roster is
    /// retained (not just emitted declarations) because `Formal` actuals
    /// resolve positions by parameter symbol.
    pub erased_proof_parameters: Vec<checked_trees::CheckedErasedProofParameterPlan>,
    pub predicate_parameters: Vec<StructuralParameterDeclaration>,
    pub claims: LoweredUnitClaims,
    /// A hook's actual borrowed parameter or its reserved proof-only subject.
    pub cleanup_receiver: Option<semantic_vocabulary::PlaceId>,
    /// Every published `requires` row in canonical contract order, combining
    /// authored clauses and runtime requirements. Calls allocate their exact
    /// obligation roster from this slice before any body is emitted.
    pub requires: Vec<Proposition>,
    pub runtime_requirements: Vec<Proposition>,
}

impl MachineSignature {
    pub(super) fn call_target(&self) -> ordinary_calls::Target<'_> {
        ordinary_calls::Target {
            parameters: &self.parameters,
            scalar_parameters: &self.scalar_parameters,
            erased_scalar_parameters: &self.erased_scalar_parameters,
            predicate_parameters: &self.predicate_parameters,
            requires: &self.requires,
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
    domain_ids: &[StructuralDomainBinding],
    structural_types: &[StructuralTypeDeclaration],
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
        let parameters = lower_unit_parameters(
            plan.structural_parameters,
            type_ids,
            structural_types,
            domain_ids,
            next_place,
        )?;
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
        let erased_scalar_parameters =
            lower_unit_scalar_parameter_types(plan.erased_scalar_parameters)?
                .into_iter()
                .map(|scalar_type| {
                    Ok(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
        let erased_proof_parameters = plan.erased_proof_parameters.to_vec();
        // Claims stay machine-local: unrelated callees cannot shift these IDs.
        let claims =
            lower_unit_entry_claims(plan.machine, plan.state, plan.entry_claims, &parameters)?;
        signatures.push(MachineSignature {
            source: plan.machine,
            parameters,
            scalar_parameters,
            erased_scalar_parameters,
            erased_proof_parameters,
            predicate_parameters: Vec::new(),
            claims,
            cleanup_receiver: None,
            requires: Vec::new(),
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
        let cleanup_context = crate::unit::unit_cleanup::contexts::prepare_signature(
            checked,
            signature.source,
            &mut signature.predicate_parameters,
            structural_types,
            next_place,
        )?;
        signature.cleanup_receiver = cleanup_context
            .as_ref()
            .and_then(|context| context.receiver);
        let requirements = super::entry_requirements::lower(
            checked,
            signature.source,
            &signature.scalar_parameters,
            &signature.erased_scalar_parameters,
            &signature.predicate_parameters,
            structural_types,
        )?;
        signature.requires = requirements.requires;
        signature.runtime_requirements = requirements.runtime_requirements;
        if let Some(context) = cleanup_context {
            context.validate_contract(&signature.requires)?;
        }
    }
    Ok(signatures)
}
