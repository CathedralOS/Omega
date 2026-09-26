use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::capture::contracts::expressions::projection::project_contract_expression;
use crate::package_evidence::capture::contracts::facts::ContractProjectionContext;
use crate::package_evidence::capture::contracts::propositions::application::project_contract_proposition;
use crate::package_evidence::capture::semantics::declarations::{
    nominal_identity, reviewed_package_owns,
};
use crate::package_evidence::record::{PackageReviewContractFact, PackageReviewNominalIdentity};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

pub(crate) fn project_domain_predicate_facts(
    compilation: &PackageReviewInput<'_>,
    definition: &symbol_resolved_trees_to_typed_trees::typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public domain",
        subject_name: &identity.path,
        owner: typed_trees_to_checked_trees::checked_trees::ContractProofFactOwner::Unknown,
        point: typed_trees_to_checked_trees::fact_plan::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: Some(definition.symbol),
        data_symbol: None,
        lifetime_binders: &[],
        lifetime_substitutions: &[],
        selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    let reviewed_package = compilation.custody.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "domain predicate review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for offset in 0..definition.facts.count() {
        let fact_handle = arena::Handle::from_parts(
            definition
                .facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("domain predicate fact handle index overflow"),
            definition.facts.start().generation(),
        );
        require_exact_checked_domain_fact(compilation, definition.symbol, fact_handle, identity)?;
        projected.push(project_definition_contract_fact(
            compilation,
            &context,
            binders,
            fact_handle,
            reviewed_package,
        )?);
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

pub(crate) fn project_definition_contract_fact(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    fact_handle: arena::Handle<
        symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact,
    >,
    reviewed_package: PackageKeyIdentity,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact;

    match compilation.proof_facts.get(fact_handle) {
        ProofFact::Expression(expression) => Ok(PackageReviewContractFact::Expression(
            project_contract_expression(
                compilation,
                context,
                binders,
                *expression,
                Some(fact_handle),
                0,
            )?,
        )),
        ProofFact::Membership(membership) => {
            crate::package_evidence::capture::contracts::memberships::require_declaration_membership(
                &compilation.typed,
                membership,
            )?;
            let domain = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "{} `{}` predicate refers to an unresolved domain",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            let domain_identity = nominal_identity(compilation, domain.symbol)?;
            if reviewed_package_owns(&domain_identity, reviewed_package)? && !domain.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "{} `{}` predicate exposes non-public domain `{}`",
                    context.subject_kind, context.subject_name, domain.name
                ))]);
            }
            Ok(PackageReviewContractFact::Membership {
                value: project_contract_expression(
                    compilation,
                    context,
                    binders,
                    membership.value,
                    Some(fact_handle),
                    0,
                )?,
                domain: domain_identity,
            })
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
        ),
    }
}

pub(crate) fn require_exact_checked_domain_fact(
    compilation: &PackageReviewInput<'_>,
    domain_symbol: SymbolHandle,
    fact_handle: arena::Handle<
        symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact,
    >,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let point = typed_trees_to_checked_trees::fact_plan::ProgramPoint::Definition {
        symbol: domain_symbol,
    };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin
                    == typed_trees_to_checked_trees::fact_plan::FactOrigin::DomainDefinition {
                        domain_symbol,
                    }
                && fact.evidence
                    == typed_trees_to_checked_trees::fact_plan::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = compilation
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .filter(|(_, record)| record.domain_symbol == domain_symbol && record.fact == fact_handle)
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record.semantic_fact == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

pub(crate) fn semantic_fact_matches_definition_fact(
    compilation: &PackageReviewInput<'_>,
    semantic_fact: &typed_trees_to_checked_trees::fact_plan::Fact,
    fact_handle: arena::Handle<
        symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact,
    >,
) -> bool {
    use symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact;
    use typed_trees_to_checked_trees::fact_plan::FactPayload;

    match (
        compilation.proof_facts.get(fact_handle),
        semantic_fact.payload,
    ) {
        (ProofFact::Expression(expected), FactPayload::BooleanExpression(actual)) => {
            *expected == actual
        }
        (
            ProofFact::Membership(expected),
            FactPayload::DomainMembership {
                value,
                domain,
                domain_symbol,
                semantic_domain,
            },
        ) => {
            expected.value == value
                && expected.domain == domain
                && expected.domain_symbol == domain_symbol
                && expected.semantic_domain == semantic_domain
        }
        (
            ProofFact::Proposition(expected),
            FactPayload::PropositionApplication { fact, proposition },
        ) => fact == fact_handle && proposition == expected.proposition,
        _ => false,
    }
}
