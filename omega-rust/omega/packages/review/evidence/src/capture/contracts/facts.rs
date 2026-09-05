use super::evidence::{
    checked_contract_fact, checked_outcome_specific_guarantee, validate_checked_contract_evidence,
    validate_checked_contract_evidence_components,
};
use crate::capture::contracts::expressions::projection::project_contract_expression;
use crate::capture::contracts::propositions::application::project_contract_proposition;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::{
    PackageReviewCallableContract, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewResultCaseIdentity,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) struct ContractProjectionContext<'a> {
    pub(crate) subject_kind: &'static str,
    pub(crate) subject_name: &'a str,
    pub(crate) owner: psi_checked_trees::ContractProofFactOwner,
    pub(crate) point: psi_facts::ProgramPoint,
    pub(crate) parameters: &'a [psi_typed_trees::signature::StateParameter],
    pub(crate) domain_symbol: Option<SymbolHandle>,
    pub(crate) data_symbol: Option<SymbolHandle>,
    pub(crate) lifetime_binders: &'a [psi_typed_trees::name::Identifier],
    /// Source names map to the normalized containing telescope. Later mappings
    /// shadow earlier ones; checked expression and evidence handles stay intact.
    pub(crate) lifetime_substitutions: &'a [(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
    pub(crate) selection_exposure:
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
}

impl ContractProjectionContext<'_> {
    pub(crate) fn requires_public_nominals(&self) -> bool {
        self.selection_exposure
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
    }
}

pub(crate) fn project_callable_contracts(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    entry: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    let parameters = compilation.state_parameters(entry);
    let context = ContractProjectionContext {
        subject_kind: "callable",
        subject_name: machine.name.as_str(),
        owner: psi_checked_trees::ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        point: psi_facts::ProgramPoint::Machine {
            machine_symbol: machine.symbol,
        },
        parameters,
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders: &machine.lifetime_parameters,
        lifetime_substitutions: &[],
        selection_exposure: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    project_contracts(
        compilation,
        compilation.machine_contracts(machine),
        &context,
        binders,
    )
}

pub(crate) fn exact_contract_entailment_stand_down_contract(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    contract_index: usize,
    fact_index: usize,
) -> Result<psi_typed_trees::signature::SignatureContract, Vec<Diagnostic>> {
    let contract = compilation
        .machine_contracts(machine)
        .get(contract_index)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "contract-entailment stand-down names a missing callable contract",
            )]
        })?;
    let fact_offset = u32::try_from(fact_index).map_err(|_| {
        vec![Diagnostic::error(
            "contract-entailment stand-down fact index exceeds canonical u32",
        )]
    })?;
    if fact_offset >= contract.facts.count() {
        return Err(vec![Diagnostic::error(
            "contract-entailment stand-down names a missing contract fact",
        )]);
    }
    let fact = psi_arena::Handle::from_parts(
        contract
            .facts
            .start()
            .arena_index()
            .checked_add(fact_offset)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "contract-entailment stand-down fact coordinate overflowed",
                )]
            })?,
        contract.facts.start().generation(),
    );
    let mut exact = contract.clone();
    exact.facts = psi_arena::HandleSpan::from_parts(fact, 1);
    Ok(exact)
}

pub(crate) fn project_callable_contract_entailment_stand_down(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    entry: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    contract_index: usize,
    fact_index: usize,
) -> Result<PackageReviewCallableContract, Vec<Diagnostic>> {
    let exact = exact_contract_entailment_stand_down_contract(
        compilation,
        machine,
        contract_index,
        fact_index,
    )?;
    let parameters = compilation.state_parameters(entry);
    let context = ContractProjectionContext {
        subject_kind: "callable",
        subject_name: machine.name.as_str(),
        owner: psi_checked_trees::ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        point: psi_facts::ProgramPoint::Machine {
            machine_symbol: machine.symbol,
        },
        parameters,
        domain_symbol: None,
        data_symbol: None,
        lifetime_binders: &machine.lifetime_parameters,
        lifetime_substitutions: &[],
        selection_exposure: if machine.is_public {
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        },
    };
    let projected =
        project_contracts(compilation, std::slice::from_ref(&exact), &context, binders)?;
    let [contract] = projected.as_slice() else {
        return Err(vec![Diagnostic::error(
            "contract-entailment stand-down did not project exactly one canonical contract fact",
        )]);
    };
    Ok(contract.clone())
}

pub(crate) fn project_trait_requirement_contracts(
    compilation: &CheckedCompilation,
    requirement: &psi_typed_trees::signature::StateSignature,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    project_contracts(
        compilation,
        compilation.state_signature_contracts(requirement),
        context,
        binders,
    )
}

pub(crate) fn project_contracts(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    use psi_typed_trees::{domain::ProofFact, signature::SignatureContractKind};

    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "contract review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for contract in contracts {
        let (kind, guarded_symbols, result_case) = match contract.kind {
            SignatureContractKind::Requires => (PackageReviewContractKind::Requires, None, None),
            SignatureContractKind::Ensures => (PackageReviewContractKind::Ensures, None, None),
            SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } => (
                PackageReviewContractKind::Ensures,
                Some((result_data, result_case)),
                Some(PackageReviewResultCaseIdentity {
                    result_data: nominal_identity(compilation, result_data)?,
                    result_case: nominal_identity(compilation, result_case)?,
                }),
            ),
            SignatureContractKind::Crashes { .. } => continue,
        };
        if contract.facts.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an empty public {:?} contract",
                context.subject_kind, context.subject_name, kind
            ))]);
        }
        for offset in 0..contract.facts.count() {
            let fact_handle = psi_arena::Handle::from_parts(
                contract
                    .facts
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("proof fact handle index overflow"),
                contract.facts.start().generation(),
            );
            let fact = match compilation.proof_facts.get(fact_handle) {
                ProofFact::Expression(expression) => {
                    PackageReviewContractFact::Expression(project_contract_expression(
                        compilation,
                        context,
                        binders,
                        *expression,
                        Some(fact_handle),
                        0,
                    )?)
                }
                ProofFact::Membership(membership) => {
                    let domain = compilation
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(format!(
                                "reviewed {} `{}` contract refers to an unresolved domain",
                                context.subject_kind, context.subject_name
                            ))]
                        })?;
                    let domain_identity = nominal_identity(compilation, domain.symbol)?;
                    if context.requires_public_nominals()
                        && reviewed_package_owns(&domain_identity, reviewed_package)?
                        && !domain.is_public
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "reviewed {} `{}` exposes non-public domain `{}` in its contract",
                            context.subject_kind, context.subject_name, domain.name
                        ))]);
                    }
                    PackageReviewContractFact::Membership {
                        value: project_contract_expression(
                            compilation,
                            context,
                            binders,
                            membership.value,
                            Some(fact_handle),
                            0,
                        )?,
                        domain: domain_identity,
                    }
                }
                ProofFact::Proposition(application) => project_contract_proposition(
                    compilation,
                    context,
                    binders,
                    application,
                    Some(fact_handle),
                    &[],
                    &[],
                    &mut Vec::new(),
                    0,
                )?,
            };
            let evidence_lane_position = if let Some((result_data, result_case)) = guarded_symbols {
                let checked = checked_outcome_specific_guarantee(
                    compilation,
                    context,
                    fact_handle,
                    result_data,
                    result_case,
                    contract.binding.as_ref(),
                )?;
                validate_checked_contract_evidence_components(
                    compilation,
                    context,
                    contract.binding.as_ref(),
                    psi_checked_trees::ContractProofFactOwner::Machine {
                        machine_symbol: checked.machine_symbol,
                    },
                    psi_checked_trees::ContractProofFactKind::Ensures,
                    checked.evidence_term,
                    &fact,
                )?
            } else {
                let checked = checked_contract_fact(compilation, context, fact_handle, kind)?;
                validate_checked_contract_evidence(
                    compilation,
                    context,
                    contract.binding.as_ref(),
                    checked,
                    &fact,
                )?
            };
            projected.push(PackageReviewCallableContract {
                kind,
                result_case: result_case.clone(),
                binding: match kind {
                    PackageReviewContractKind::Ensures => contract
                        .binding
                        .as_ref()
                        .map(|binding| binding.as_str().to_owned()),
                    PackageReviewContractKind::Requires => None,
                },
                evidence_lane_position,
                fact,
            });
        }
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}
