use super::evidence::require_rederived_data_definition_facts;
use crate::evidence::{
    PackageReviewContractFact, PackageReviewDataKind, PackageReviewDataMember,
    PackageReviewDataShape, PackageReviewNominalIdentity, PackageReviewSourceLocationRole,
};
use crate::projection::api::domains::facts::{
    project_definition_contract_fact, semantic_fact_matches_definition_fact,
};
use crate::projection::contracts::callables::ContractProjectionContext;
use crate::projection::contracts::parameters::collect_type_parameter_source_locations;
use crate::projection::contracts::source_locations::project_required_proof_fact_source_locations;
use crate::projection::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::projection::semantics::signatures::parameters::project_type_parameters;
use crate::projection::semantics::types::{
    project_data_field, project_data_properties, review_signature_type_identity_with_binders,
};
use crate::projection::source::ProjectedReviewRow;
use crate::projection::source::locations::project_nested_declaration_source_location;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_data_invariant_facts(
    compilation: &CheckedCompilation,
    definition: &psi_typed_trees::data::DataDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public data",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::Unknown,
        point: psi_facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: None,
        data_symbol: Some(definition.symbol),
        lifetime_binders: &definition.lifetime_parameters,
    };
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "data invariant review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for offset in 0..definition.where_facts.count() {
        let fact_handle = psi_arena::Handle::from_parts(
            definition
                .where_facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("data invariant fact handle index overflow"),
            definition.where_facts.start().generation(),
        );
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
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let point = psi_facts::ProgramPoint::Definition {
        symbol: data_symbol,
    };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == psi_facts::FactOrigin::DataDefinition { data_symbol }
                && fact.evidence == psi_facts::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public data `{}` invariant fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = compilation
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .filter(|(_, record)| record.data_symbol == data_symbol && record.fact == fact_handle)
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record.semantic_fact == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public data `{}` invariant fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

pub(crate) fn project_public_data(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDataShape>>, Vec<Diagnostic>> {
    require_rederived_data_definition_facts(compilation)?;
    let quotient_formations = psi_validation::validate_quotient_formations(compilation)?;
    let mut rows = Vec::new();
    for definition in compilation
        .data_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
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
                        psi_typed_trees::data::DataMember::Field(field) => {
                            PackageReviewDataMember::Field(project_data_field(
                                compilation,
                                field,
                                &binders,
                                &definition.lifetime_parameters,
                            )?)
                        }
                        psi_typed_trees::data::DataMember::Variant(variant) => {
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
            declaration: definition.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                for member in compilation.data_members(definition) {
                    match member {
                        psi_typed_trees::data::DataMember::Field(field) => {
                            locations.push(project_nested_declaration_source_location(
                                compilation,
                                field.symbol,
                                PackageReviewSourceLocationRole::DataMember,
                                "public data field",
                            )?);
                        }
                        psi_typed_trees::data::DataMember::Variant(variant) => {
                            locations.push(project_nested_declaration_source_location(
                                compilation,
                                variant.symbol,
                                PackageReviewSourceLocationRole::DataMember,
                                "public data case",
                            )?);
                            for field in compilation.data_payload_fields(variant) {
                                locations.push(project_nested_declaration_source_location(
                                    compilation,
                                    field.symbol,
                                    PackageReviewSourceLocationRole::DataMember,
                                    "public data case payload field",
                                )?);
                            }
                        }
                    }
                }
                locations.extend(project_required_proof_fact_source_locations(
                    compilation,
                    definition.where_facts,
                    "public data invariant",
                )?);
                locations
            },
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}
