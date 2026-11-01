//! Data definitions: members, type parameters, and generic-instance custody.
//!
//! `lower_type_parameters` is shared by every parameterized declaration.
//! Generic instances retain the derived const-argument origins of their
//! application while their members lower. Whole-data predicates retain their
//! ordinary declaration scope through generic substitution.

use crate::lowering::domain::lower_proof_facts;
use crate::lowering::expression::lower_static_machine_argument;
use crate::lowering::name::lower_name;
use crate::lowering::name::lower_names;
use crate::lowering::type_reference::{
    lower_child_type_references, lower_generic_application_origin, lower_type_reference_handle,
};
use crate::preparation::machine_equations::DeclaredTypeOrigin;
use crate::resolution::lowerer::Lowerer;
use crate::resolution::lowerer::PendingSignatureLocation;
use crate::tables::syntax_items;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::data::{
    DataDefinition, DataDefinitionStorage, DataField, DataMember, DataProperties, DataVariant,
    QuotientDefinition, QuotientEquivalenceSelection, TypeParameter, TypeParameterKind,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let prior_origins = lowerer.derived_const_argument_origins.len();
    let prior_expressions = lowerer.derived_const_argument_expressions.len();
    let prior_operators = lowerer.derived_const_argument_builtin_operators.len();
    if let Some(application) = data_definition.generic_instance {
        retain_derived_const_argument_origins(lowerer, syntax_trees, application);
    }
    let result =
        lower_data_definition_with_argument_origins(lowerer, syntax_trees, data_definition);
    lowerer
        .derived_const_argument_origins
        .truncate(prior_origins);
    lowerer
        .derived_const_argument_builtin_operators
        .truncate(prior_operators);
    lowerer
        .derived_const_argument_expressions
        .truncate(prior_expressions);
    result
}

/// A substituted argument does not become a new authored occurrence in the
/// template's public fields. Exclude only exact occurrences carried by this
/// instance's arguments; independent template references still retain exposure.
fn retain_derived_const_argument_origins(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    application: syntax::types::TypeReferenceHandle,
) {
    use syntax::types::TypeReferenceNode;
    let mut pending = vec![application];
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        let table = &syntax_trees.type_references;
        if let Some(normalization) = table.const_argument_normalization(handle) {
            if normalization.authored_expression.is_valid() {
                lowerer
                    .derived_const_argument_expressions
                    .push(normalization.authored_expression);
            }
            lowerer
                .derived_const_argument_origins
                .extend_from_slice(table.const_argument_origins(normalization.selections));
            lowerer
                .derived_const_argument_builtin_operators
                .extend_from_slice(
                    table.const_argument_builtin_operators(normalization.builtin_operators),
                );
        }
        let authored = table.generic_application_origin(handle);
        if authored.is_valid() {
            pending.push(authored);
        }
        match table.type_reference(handle) {
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend_from_slice(table.type_reference_handles(*arguments));
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                for constraint in table.constraints(*constraints) {
                    if let syntax::types::TypeConstraintNode::Domain(domain) = constraint {
                        pending.extend_from_slice(table.type_reference_handles(domain.arguments));
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            _ => {}
        }
    }
}

fn lower_data_definition_with_argument_origins(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    // A `data` header admits a `Value` binder (`data Wrap<Count:u32>`) so the
    // runtime subject rides the same spine as on machine signatures. The
    // binder is kept as ordinary data: it lowers to an implicit leading
    // descriptor field of its carrier type, so a construction spells the
    // argument positionally by name (`Wrap { Count: 4, .. }`), `where` facts
    // over the binder discharge at the construction gate like any other
    // field, and the index participates in the write-net obligations a
    // declared field does. The binder's own `Value` parameter identity is
    // retained on the lowered template so later legs can still distinguish
    // it from an authored field; a template-level argument spelling
    // (`Wrap<4>` in static position) remains a separate leg.
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters)?;
    let mut members = HandleSpan::empty();
    for parameter in syntax_items(&syntax_trees).type_parameters(data_definition.type_parameters) {
        let syntax::item::TypeParameterKind::Value { type_reference } = &parameter.kind else {
            continue;
        };
        let field = DataMember::Field(DataField {
            identity: None,
            symbol: SymbolHandle::invalid(),
            name: lower_name(syntax_trees, &parameter.name),
            relevance: language_core::BindingRelevance::default(),
            type_reference: lower_type_reference_handle(lowerer, syntax_trees, *type_reference)?,
        });
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut members, field);
    }
    for member in syntax_trees.items.data_members(data_definition.members) {
        if matches!(member, syntax::item::DataMember::Retired(_)) {
            continue;
        }
        let authored_type = match member {
            syntax::item::DataMember::Field(field) => Some(field.type_reference),
            _ => None,
        };
        let member = lower_data_member(lowerer, syntax_trees, member)?;
        let handle = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut members, member);
        if let Some(authored_type) = authored_type
            && let Some(sources) = &mut lowerer.equation_sources
        {
            sources
                .declared_types
                .push((DeclaredTypeOrigin::Field(handle), authored_type));
        }
    }
    let retired_identities = syntax_items(&syntax_trees)
        .data_members(data_definition.members)
        .iter()
        .filter_map(|member| match member {
            syntax::item::DataMember::Retired(identity) => Some(*identity),
            _ => None,
        })
        .collect();
    let quotient = data_definition
        .quotient
        .as_ref()
        .map(|quotient| {
            Ok::<_, Diagnostic>(QuotientDefinition {
                carrier: lower_type_reference_handle(lowerer, syntax_trees, quotient.carrier)?,
                relation: lower_names(
                    syntax_trees,
                    syntax_items(&syntax_trees).identifier_path_members(quotient.relation),
                ),
                relation_symbol: SymbolHandle::invalid(),
                equivalence: quotient
                    .equivalence
                    .as_ref()
                    .map(|selection| {
                        Ok::<_, Diagnostic>(QuotientEquivalenceSelection {
                            relation: syntax_items(&syntax_trees)
                                .identifier_path_members(selection.relation)
                                .iter()
                                .map(|name| lower_name(syntax_trees, name))
                                .collect(),
                            relation_symbol: SymbolHandle::invalid(),
                            trait_name: lower_name(syntax_trees, &selection.trait_name),
                            trait_symbol: SymbolHandle::invalid(),
                            trait_arguments: lower_child_type_references(
                                lowerer,
                                syntax_trees,
                                selection.trait_arguments,
                            )?,
                            conformance_name: lower_name(syntax_trees, &selection.conformance_name),
                            conformance_symbol: SymbolHandle::invalid(),
                        })
                    })
                    .transpose()?,
            })
        })
        .transpose()?;
    // Retain every default-domain obligation. This early integer folder can
    // establish zero eligibility only inside its supported fragment; an
    // unknown result requires establishment, just like a known false result.
    // In particular, unresolved float predicates need their typed formats,
    // literal landing and selected operations before zero can be admitted.
    // Symbol assignment rechecks these contributions without dropping facts;
    // later typing and proof validation still own their semantic validity.
    let type_equations = crate::preparation::generic_data::template_type_equation_offsets(
        syntax_trees,
        data_definition,
        lowerer.constant_selection.as_ref(),
    )?;
    let where_facts = crate::lowering::domain::lower_proof_facts_excluding(
        lowerer,
        syntax_trees,
        data_definition.where_facts,
        &type_equations,
    )?;
    // Names are not bound yet: treating every identifier as a zero-valued
    // field would also zero static arguments. Resolved fact admission refines
    // this conservative gate once subjects and domains have exact identities.
    let zero_gated = !where_facts.is_empty();

    Ok(DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: lower_name(syntax_trees, &data_definition.name),
        is_public: data_definition.is_public,
        storage: DataDefinitionStorage {
            predicate_subject_symbol: SymbolHandle::invalid(),
            predicate_subject_type: symbol_resolved_trees::types::TypeReference::Unit,
            supply_mode: data_definition.supply_mode,
            lifetime_parameters: lower_names(syntax_trees, &data_definition.lifetime_parameters),
            type_parameters,
            conformance_bounds: crate::lowering::machine::lower_generic_conformance_bounds(
                lowerer,
                syntax_trees,
                &data_definition.conformance_bounds,
            )?,
            generic_instance: data_definition
                .generic_instance
                .map(|origin| lower_generic_application_origin(lowerer, syntax_trees, origin))
                .transpose()?,
            properties: DataProperties {
                carry: data_definition.properties.carry,
                multiplicity: data_definition.properties.multiplicity,
            },
            cleanup_selection: data_definition
                .cleanup_selection
                .as_ref()
                .map(|selection| lower_static_machine_argument(lowerer, syntax_trees, selection))
                .transpose()?,
            quotient,
            where_facts,
            zero_gated,
            retired_identities,
            members,
        },
    })
}

/// R2 rung 2 slice 1: fold one default-domain fact AT THE ZERO VALUE --
/// every field name reads 0, literals read themselves, `+ - *` fold,
/// comparisons and `&&`/`||` yield 1/0. `None` = outside the fragment.
pub(crate) fn lower_type_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_parameters: HandleSpan<syntax::item::TypeParameter>,
) -> Result<HandleSpan<TypeParameter>, Diagnostic> {
    let mut lowered = Vec::new();
    for parameter in syntax_trees.items.type_parameters(type_parameters) {
        let (kind, pending_service_reaches) = match &parameter.kind {
            syntax::item::TypeParameterKind::Type => (TypeParameterKind::Type, None),
            syntax::item::TypeParameterKind::Implementation => {
                (TypeParameterKind::Implementation, None)
            }
            syntax::item::TypeParameterKind::Const { type_reference } => (
                TypeParameterKind::Const {
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        *type_reference,
                    )?,
                },
                None,
            ),
            syntax::item::TypeParameterKind::Value { type_reference } => (
                TypeParameterKind::Value {
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        *type_reference,
                    )?,
                },
                None,
            ),
            syntax::item::TypeParameterKind::Machine { contract } => {
                let contract = contract.as_ref().ok_or_else(|| {
                    Diagnostic::error(format!(
                        "machine parameter `{}` reached symbol resolution without its mandatory `requires machine` contract",
                        parameter.name.as_str(syntax_trees)
                    ))
                })?;
                match contract {
                    syntax::item::MachineParameterContract::RequirementIdentity => (
                        TypeParameterKind::Machine {
                            contract: symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity,
                        },
                        None,
                    ),
                    syntax::item::MachineParameterContract::Structural(contract) => {
                        let lowered_contract = crate::lowering::state::lower_state_signature_parts(lowerer, syntax_trees, crate::lowering::state::StateSignatureParts { name: &contract.name,
spelling: contract.spelling,
lifetime_parameters: &contract.lifetime_parameters,
type_parameters: contract.type_parameters,
parameters: contract.parameters,
native_callback_parameters: &contract.native_callback_parameters,
return_type_handle: contract.return_type,
is_default: contract.is_default,
service_reach_is_installation_bound: false,
service_reach_keyword_source_spans: &contract.service_reach_keyword_source_spans,
service_reaches: contract.service_reaches,
invokes: contract.invokes,
suspends_keyword_source_spans: &contract.suspends_keyword_source_spans,
blocks_keyword_source_spans: &contract.blocks_keyword_source_spans,
suspends: contract.suspends,
blocks: contract.blocks,
contracts: contract.contracts,
terminates_guarantee: contract.terminates_guarantee,
where_facts: contract.where_facts })?;
                        (
                            TypeParameterKind::Machine {
                                contract: symbol_resolved_trees::data::MachineParameterContract::Structural(
                                    lowered_contract.signature,
                                ),
                            },
                            Some((
                                lowered_contract.service_reach_keyword_source_spans,
                                lowered_contract.service_reaches,
                            )),
                        )
                    }
                    syntax::item::MachineParameterContract::Nominal {
                        requirement,
                        trait_arguments,
                    } => (
                        TypeParameterKind::Machine {
                            contract: symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                                requirement: lower_names(
                                    syntax_trees,
                                    syntax_trees.items.identifier_path_members(*requirement),
                                ),
                                trait_arguments: crate::lowering::type_reference::lower_child_type_references(
                                    lowerer,
                                    syntax_trees,
                                    *trait_arguments,
                                )?,
                            },
                        },
                        None,
                    ),
                }
            }
            syntax::item::TypeParameterKind::Proposition { contract } => {
                let contract = contract.as_ref().ok_or_else(|| {
                    Diagnostic::error(format!(
                        "proposition parameter `{}` reached symbol resolution without its mandatory authored signature",
                        parameter.name.as_str(syntax_trees)
                    ))
                })?;
                (
                    TypeParameterKind::Proposition {
                        contract: symbol_resolved_trees::data::PropositionParameterSignature {
                            name: lower_name(syntax_trees, &contract.name),
                            parameters: crate::lowering::state::lower_state_parameters(
                                lowerer,
                                syntax_trees,
                                contract.parameters,
                            )?,
                        },
                    },
                    None,
                )
            }
        };
        lowered.push((
            TypeParameter {
                symbol: SymbolHandle::invalid(),
                name: lower_name(syntax_trees, &parameter.name),
                kind,
                bounds: DataProperties {
                    carry: parameter.bounds.carry,
                    multiplicity: parameter.bounds.multiplicity,
                },
            },
            pending_service_reaches,
        ));
    }

    let (parameters, pending_service_reaches): (Vec<_>, Vec<_>) = lowered.into_iter().unzip();
    let span = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .data_type_parameters
        .insert_many(parameters);
    if !span.is_empty() {
        for (index, authored) in pending_service_reaches.into_iter().enumerate() {
            let Some((keyword_source_spans, authored)) = authored else {
                continue;
            };
            let handle =
                span.handle_at(u32::try_from(index).expect("type-parameter span fits u32"));
            let owner = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .data_type_parameters
                .get(handle)
                .name
                .clone();
            lowerer.pending_signature_service_reaches.push(
                crate::resolution::lowerer::PendingSignatureServiceReach {
                    location: PendingSignatureLocation::MachineParameter(handle),
                    owner: crate::resolution::lowerer::PendingSignatureOwner::Requirement(owner),
                    reaches:
                        crate::selection::service_reaches::PendingAuthoredServiceReach::Authored {
                            keyword_source_spans,
                            authored,
                        },
                },
            );
        }
    }

    Ok(span)
}

fn lower_data_member(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    member: &syntax::item::DataMember,
) -> Result<DataMember, Diagnostic> {
    match member {
        syntax::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            identity: field.identity,
            symbol: SymbolHandle::invalid(),
            name: lower_name(syntax_trees, &field.name),
            relevance: field.relevance,
            type_reference: lower_type_reference_handle(
                lowerer,
                syntax_trees,
                field.type_reference,
            )?,
        })),
        syntax::item::DataMember::Variant(variant) => {
            let mut payload = HandleSpan::empty();
            for field in syntax_trees.items.data_payload_fields(variant.payload) {
                let lowered = DataField {
                    identity: field.identity,
                    symbol: SymbolHandle::invalid(),
                    name: lower_name(syntax_trees, &field.name),
                    relevance: field.relevance,
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        field.type_reference,
                    )?,
                };
                let handle = lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .data_payload_fields
                    .append_to_span(&mut payload, lowered);
                if let Some(sources) = &mut lowerer.equation_sources {
                    sources
                        .declared_types
                        .push((DeclaredTypeOrigin::Payload(handle), field.type_reference));
                }
            }
            let where_facts = lower_proof_facts(lowerer, syntax_trees, variant.where_facts)?;
            Ok(DataMember::Variant(DataVariant {
                identity: variant.identity,
                symbol: SymbolHandle::invalid(),
                name: lower_name(syntax_trees, &variant.name),
                payload,
                where_facts,
                retired_payload_identities: variant.retired_payload_identities.clone(),
            }))
        }
        syntax::item::DataMember::Retired(_) => unreachable!("retired identities are metadata"),
    }
}
