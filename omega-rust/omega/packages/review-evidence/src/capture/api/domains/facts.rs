use crate::capture::PackageReviewInput;
use crate::capture::contracts::expressions::projection::project_contract_expression;
use crate::capture::contracts::facts::{ContractProjectionContext, reviewed_package};
use crate::capture::contracts::propositions::application::project_contract_proposition;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::{PackageReviewContractFact, PackageReviewNominalIdentity};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

pub(crate) fn project_domain_predicate_facts(
    compilation: &PackageReviewInput<'_>,
    definition: &typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public domain",
        subject_name: &identity.path,
        owner: checked_trees::ContractProofFactOwner::Unknown,
        point: facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: Some(definition.symbol),
        data_symbol: None,
        lifetime_binders: &[],
        lifetime_substitutions: &[],
        selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    let reviewed_package = reviewed_package(compilation, "domain predicate")?;
    let mut projected = Vec::new();
    for fact_handle in definition.facts.handles() {
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
    fact_handle: arena::Handle<typed_trees::domain::ProofFact>,
    reviewed_package: PackageKeyIdentity,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use typed_trees::domain::ProofFact;

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
            crate::capture::contracts::memberships::require_declaration_membership(
                compilation.checking_input,
                membership,
            )?;
            let domain = compilation
                .domain_definition(membership.domain_symbol)
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
    fact_handle: arena::Handle<typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    require_exact_checked_fact(
        compilation,
        domain_symbol,
        fact_handle,
        identity,
        "domain",
        "predicate",
        |domain_symbol| facts::FactOrigin::DomainDefinition { domain_symbol },
        compilation
            .facts
            .semantic
            .domain_definition_facts
            .iter()
            .map(|(_, record)| record),
        |record: &facts::DomainDefinitionFactRecord| {
            (record.domain_symbol, record.fact, record.semantic_fact)
        },
    )
}

/// The exact-checked definition-fact requirement both public fact owners
/// share: exactly one semantic definition row for the symbol's fact handle and
/// exactly one retained ownership record bound to that row. `kind` and `noun`
/// complete the diagnostics (`public {kind} \`x\` {noun} fact`), `origin`
/// builds the definition origin the rows must carry, and `record_fields`
/// reads each retained record's symbol, typed fact, and semantic handle.
pub(crate) fn require_exact_checked_fact<'a, R: 'a>(
    compilation: &PackageReviewInput<'_>,
    symbol: SymbolHandle,
    fact_handle: arena::Handle<typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
    kind: &'static str,
    noun: &'static str,
    origin: impl Fn(SymbolHandle) -> facts::FactOrigin,
    retained_facts: impl Iterator<Item = &'a R>,
    record_fields: impl Fn(
        &R,
    ) -> (
        SymbolHandle,
        arena::Handle<typed_trees::domain::ProofFact>,
        facts::FactHandle,
    ),
) -> Result<(), Vec<Diagnostic>> {
    let point = facts::ProgramPoint::Definition { symbol };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == origin(symbol)
                && fact.evidence == facts::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public {kind} `{}` {noun} fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = retained_facts
        .filter(|record| {
            let (record_symbol, record_fact, _) = record_fields(record);
            record_symbol == symbol && record_fact == fact_handle
        })
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record_fields(record).2 == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public {kind} `{}` {noun} fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

pub(crate) fn semantic_fact_matches_definition_fact(
    compilation: &PackageReviewInput<'_>,
    semantic_fact: &facts::Fact,
    fact_handle: arena::Handle<typed_trees::domain::ProofFact>,
) -> bool {
    use facts::FactPayload;
    use typed_trees::domain::ProofFact;

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
