//! Allocate each callable's formals and derive its contracts in that namespace.
//!
//! Emission and call substitution borrow the same record. Authored parameter
//! positions are retained separately from dense Terminal structural positions.

use super::RuntimeRequirementOwner;
use super::admission::AdmittedBody;
use super::claims::{LoweredUnitClaims, lower_unit_entry_claims};
use super::ordinary_calls;
use super::parameters::{lower_unit_parameters, lower_unit_scalar_parameter_types};
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::terminal_identities::allocate_dense;
use crate::terminal_identities::value_id;
use crate::unit::runtime_requirements::lower_structural_runtime_requirement;
use checked_trees::CheckedTrees;
use language_semantics::SemanticDomainId;
use semantic_vocabulary::Proposition;
use semantic_vocabulary::StructuralDomainId;
use semantic_vocabulary::StructuralTypeId;
use symbols::SymbolHandle;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::ValueDeclaration;

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
    /// Every `requires` row the emitted contract carries, in order: closed
    /// authored clauses merged into one proposition, then runtime
    /// requirements. Call sites size their obligation roster from this exact
    /// slice.
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
                        &signature.erased_scalar_parameters,
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
        // Publish each closed authored `requires` clause as its own row
        // beside the runtime requirements: a Unit callee's roster has one row
        // per authored clause, so a caller owes one obligation per clause. A
        // clause the closed lane never retained keeps the historical
        // runtime-only contract rather than a partial roster drifting from
        // the checked plan. Crash-gated arithmetic's runtime requirements can
        // restate an authored clause; the canonical dedup below keeps one
        // row per fact, where one merged conjunction beside them would have
        // published every clause twice.
        let authored_requires = contract.closed_scalar_values.authored_requires();
        if authored_requires.iter().all(Option::is_some) {
            signature.requires = authored_requires
                .iter()
                .map(|clause| {
                    crate::scalar_graph::scalar_contracts::clauses(
                        std::slice::from_ref(clause),
                        &signature.scalar_parameters,
                        &signature.erased_scalar_parameters,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .filter(|clause| *clause != Proposition::Truth)
                .collect();
        }
        signature
            .requires
            .extend(signature.runtime_requirements.iter().cloned());
        // A requires roster is itself a conjunction, so a row that is a
        // top-level conjunction (crash arithmetic's runtime package restates
        // the authored clauses that way) is split into its conjuncts. Each
        // fact is then one row, and the dedup below cannot keep both a
        // clause and a conjunction containing it.
        let mut pending = std::mem::take(&mut signature.requires);
        pending.reverse();
        while let Some(requirement) = pending.pop() {
            match requirement {
                Proposition::Conjunction(conjuncts) => {
                    pending.extend(conjuncts.into_iter().rev());
                }
                other => signature.requires.push(other),
            }
        }
        // The codec publishes requires rows in canonical byte order, so merge
        // then normalize rather than exposing clause/runtime ordering
        // accidents. `Proposition`'s derived `Ord` is Rust declaration order,
        // not the canonical key; the shared helper orders by encoded bytes.
        crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut signature.requires)?;
    }
    Ok(signatures)
}
