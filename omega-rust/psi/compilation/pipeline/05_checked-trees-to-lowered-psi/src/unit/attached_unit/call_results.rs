//! Receive a structural result in the caller's namespace, independently of
//! statement versus expression evaluation. Claims come from checked custody;
//! qualifications come from the public callable, never a provider's body.
//! Preparation retains semantic domains until the closure allocates their
//! Terminal identities. Emission joins those identities and the actual claims
//! once, after operands have completed.
use crate::emission::claim_bindings::{self, ClaimBindings, PreparedResultClaim};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use checked_trees::{CheckedTrees, CheckedUnitCallCoordinate, CheckedUnitCallPlan};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralOperationResult, StructuralTypeDeclaration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Prepared {
    value: StructuralOperationResult,
    claims: Vec<PreparedResultClaim>,
    domains: Vec<language_semantics::SemanticDomainId>,
    projected: Vec<checked_trees::CheckedStructuralPathQualification>,
    /// Zero for an ordinary body call. Boundary requirements and dynamic
    /// slots establish only their public result domains through CallEnsures.
    issuer: symbols::SymbolHandle,
}

/// Whole-carrier semantic domains a module-route call's receiving result carries:
/// the checked custody roster beside the callee's authored `ensures result
/// in Domain` clauses. The structural domain catalog selects from this same
/// roster, so a receiving result can never name a domain the closure did
/// not lower. A boundary or dynamic call receives its public result roster
/// instead, which the catalog selects from the published requirement.
pub(crate) fn module_result_qualification_domains(
    checked: &CheckedTrees,
    call: &CheckedUnitCallPlan,
) -> Vec<language_semantics::SemanticDomainId> {
    let mut domains = call
        .structural_custody()
        .map(|custody| custody.result_qualifications.clone())
        .unwrap_or_default();
    if let checked_trees::CheckedUnitCallRoute::Module { .. } = call.route() {
        domains.append(&mut super::parameters::authored_result_whole_domains(
            checked,
            call.target.machine,
        ));
    }
    domains.sort_by_key(|domain| domain.0);
    domains.dedup();
    domains
}

pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    call: &CheckedUnitCallPlan,
    place: PlaceId,
    structural_type: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
) -> Result<Prepared, LoweringError> {
    let (Some(binding), Some(custody)) = (call.structural_result(), call.structural_custody())
    else {
        return unsupported("structural call result has no checked receiving custody");
    };
    let plans = &checked.facts.flow.terminal_unit_effects;
    let public_result = match call.route() {
        checked_trees::CheckedUnitCallRoute::Module { .. } => None,
        checked_trees::CheckedUnitCallRoute::Boundary => Some(
            &super::call_closure::unique_unit_boundary(
                super::bodies::UnitPlans::published(&checked.facts.flow),
                call.target.machine,
            )?
            .result,
        ),
        checked_trees::CheckedUnitCallRoute::Dynamic { .. } => Some(
            &plans
                .dynamic_slots
                .iter()
                .find(|slot| slot.slot == call.target.machine)
                .ok_or(LoweringError::Unsupported(
                    "structural result names an unpublished dynamic slot",
                ))?
                .result,
        ),
    };
    let (domains, projected, issuer) = if let Some(public_result) = public_result {
        let checked_trees::CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications,
        } = public_result
        else {
            return unsupported("structural call target declares a non-structural result");
        };
        if *type_identity != binding.type_identity || *multiplicity != binding.multiplicity {
            return unsupported("structural call result disagrees with its declaration");
        }
        // A selected implementation returns its actual claims, but cannot
        // add private qualifications to the caller's public slot contract.
        (
            qualifications.clone(),
            projected_qualifications.clone(),
            call.target.machine,
        )
    } else {
        let (_, state) = crate::declarations::state_declaration(checked, call.target.state)?;
        (
            module_result_qualification_domains(checked, call),
            super::parameters::result_projected_qualifications(
                checked,
                call.target.machine,
                state.return_type,
            )?,
            symbols::SymbolHandle::default(),
        )
    };
    Ok(Prepared {
        value: StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity: crate::terminal_identities::structural_multiplicity(binding.multiplicity),
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        claims: claim_bindings::prepare_call_result_claims(
            identities,
            &custody.result_claims,
            structural_type,
            types,
        )?,
        domains,
        projected,
        issuer,
    })
}

impl Prepared {
    pub(crate) fn emit(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
        domain_ids: &[StructuralDomainBinding],
        types: &[StructuralTypeDeclaration],
        claims: &mut ClaimBindings<'_>,
    ) -> Result<StructuralOperationResult, LoweringError> {
        let mut value = self.value.clone();
        value.qualifications = self
            .domains
            .iter()
            .map(|domain| lookup_domain_id_for_carrier(domain_ids, *domain, value.structural_type))
            .collect::<Result<Vec<_>, _>>()?;
        // Terminal domain identities order by catalog position, which need
        // not match the checked roster's semantic-domain order.
        value.qualifications.sort();
        value.projected_qualifications = super::parameters::lower_projected_qualifications(
            identities,
            &self.projected,
            domain_ids,
            value.structural_type,
            types,
        )?;
        value.claims =
            claim_bindings::emit_call_result_claims(&self.claims, state, coordinate, claims)?;
        value.claims.sort();
        if self.issuer.is_valid() {
            value.qualification_establishments =
                super::catalog::call_result_qualification_establishments(
                    identities,
                    checked,
                    state,
                    coordinate,
                    self.issuer,
                    &self.domains,
                    domain_ids,
                )?;
        }
        Ok(value)
    }
}
