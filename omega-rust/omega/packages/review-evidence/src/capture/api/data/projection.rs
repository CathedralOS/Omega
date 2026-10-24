use super::evidence::require_rederived_data_definition_facts;
use crate::capture::PackageReviewInput;
use crate::capture::api::domains::facts::project_definition_contract_fact;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::{
    is_product_scope_instance, nominal_identity, reviewed_package_owns,
};
use crate::capture::semantics::signatures::parameters::project_type_parameters;
use crate::capture::semantics::types::{
    project_data_field, project_data_properties, review_signature_type_identity_with_binders,
};
use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    PackageReviewContractFact, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataShape, PackageReviewNominalIdentity,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::data::DataDefinition;

pub(crate) fn project_data_invariant_facts(
    compilation: &PackageReviewInput<'_>,
    definition: &DataDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public data",
        subject_name: &identity.path,
        owner: checked_trees::ContractProofFactOwner::Unknown,
        point: facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: None,
        data_symbol: Some(definition.symbol),
        lifetime_binders: &definition.lifetime_parameters,
        lifetime_substitutions: &[],
        selection_exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface,
    };
    let reviewed_package = compilation
        .custody
        .sources()
        .package_identity()
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "data invariant review requires package-aware checked compilation",
            )]
        })?;
    let mut projected = Vec::new();
    for fact_handle in definition.where_facts.handles() {
        require_exact_checked_data_fact(compilation, definition.symbol, fact_handle, identity)?;
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
pub(crate) fn require_exact_checked_data_fact(
    compilation: &PackageReviewInput<'_>,
    data_symbol: SymbolHandle,
    fact_handle: arena::Handle<typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    crate::capture::api::domains::facts::require_exact_checked_fact(
        compilation,
        data_symbol,
        fact_handle,
        identity,
        "data",
        "invariant",
        |data_symbol| facts::FactOrigin::DataDefinition { data_symbol },
        compilation
            .facts
            .semantic
            .data_definition_facts
            .iter()
            .map(|(_, record)| record),
        |record: &facts::DataDefinitionFactRecord| {
            (record.data_symbol, record.fact, record.semantic_fact)
        },
    )
}

pub(crate) fn project_public_data<'a>(
    compilation: &'a PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<'a, PackageReviewDataShape, DataDefinition>>, Vec<Diagnostic>> {
    require_rederived_data_definition_facts(compilation)?;
    let quotient_formations = validation::validate_quotient_formations(compilation.checking_input)?;
    let mut rows = Vec::new();
    for definition in compilation.data_definitions().iter().filter(|row| {
        row.is_public && is_product_scope_instance(&compilation.typed.symbols, row.symbol)
    }) {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.data_type_parameters(definition);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "data",
            &identity.path,
            &definition.lifetime_parameters,
        )?;
        let kind = if definition.quotient.is_some() {
            let matching_formations = quotient_formations
                .iter()
                .filter(|formation| formation.data_symbol == definition.symbol)
                .collect::<Vec<_>>();
            let [formation] = matching_formations.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` has {} independently rederived formation rows; expected one",
                    identity.path,
                    matching_formations.len()
                ))]);
            };
            let matching_relations = compilation
                .propositions()
                .iter()
                .filter(|relation| relation.symbol == formation.relation_symbol)
                .collect::<Vec<_>>();
            let [relation] = matching_relations.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` has {} exact relation declarations; expected one",
                    identity.path,
                    matching_relations.len()
                ))]);
            };
            if !relation.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` exposes non-public relation `{}`",
                    identity.path, relation.name
                ))]);
            }
            PackageReviewDataKind::Quotient {
                carrier: review_signature_type_identity_with_binders(
                    compilation,
                    formation.carrier,
                    &binders,
                    &definition.lifetime_parameters,
                )?,
                relation: nominal_identity(compilation, formation.relation_symbol)?,
            }
        } else {
            PackageReviewDataKind::Ordinary
        };
        let invariants =
            project_data_invariant_facts(compilation, definition, &identity, &binders)?;

        let members = compilation
            .data_members(definition)
            .iter()
            .map(
                |member| -> Result<PackageReviewDataMember, Vec<Diagnostic>> {
                    Ok(match member {
                        typed_trees::data::DataMember::Field(field) => {
                            PackageReviewDataMember::Field(project_data_field(
                                compilation,
                                field,
                                &binders,
                                &definition.lifetime_parameters,
                            )?)
                        }
                        typed_trees::data::DataMember::Variant(variant) => {
                            let mut retired_payload_identities =
                                variant.retired_payload_identities.clone();
                            retired_payload_identities.sort_unstable();
                            retired_payload_identities.dedup();
                            PackageReviewDataMember::Variant {
                                identity: variant.identity,
                                name: variant.name.as_str().to_owned(),
                                payload: compilation
                                    .data_payload_fields(variant)
                                    .iter()
                                    .map(|field| {
                                        project_data_field(
                                            compilation,
                                            field,
                                            &binders,
                                            &definition.lifetime_parameters,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                retired_payload_identities,
                            }
                        }
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        let mut retired_identities = definition.retired_identities.clone();
        retired_identities.sort_unstable();
        retired_identities.dedup();
        rows.push(ProjectedReviewRow {
            row: PackageReviewDataShape {
                identity,
                kind,
                supply: definition.supply_mode,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                properties: project_data_properties(definition.properties),
                zero_gated: definition.zero_gated,
                invariants,
                retired_identities,
                members,
            },
            declaration: definition,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
