/*
    Per-node translation of syntax into the symbol-resolved carrier.

    `item` dispatches each root item to the module owning its carrier area;
    `statement` and `expression` translate bodies; `type_reference` and `name`
    serve every other module. Each translator borrows the `Lowerer`, appends
    to its trees, and records the selections a later phase settles. Nothing
    here assigns a symbol. Calls retain their ordinary declaration route.
*/

pub(crate) mod data {
    /*
        Data definitions: members, type parameters, and generic-instance custody.

        `lower_type_parameters` is shared by every parameterized declaration.
        Generic instances retain the derived const-argument origins of their
        application while their members lower. Whole-data predicates retain their
        ordinary declaration scope through generic substitution.
    */

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

    /*
        A substituted argument does not become a new authored occurrence in the
        template's public fields. Exclude only exact occurrences carried by this
        instance's arguments; independent template references still retain exposure.
    */
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
                            pending
                                .extend_from_slice(table.type_reference_handles(domain.arguments));
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
        let type_parameters =
            lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters)?;
        let mut members = HandleSpan::empty();
        for parameter in syntax_items(syntax_trees).type_parameters(data_definition.type_parameters)
        {
            let syntax::item::TypeParameterKind::Value { type_reference } = &parameter.kind else {
                continue;
            };
            let field = DataMember::Field(DataField {
                identity: None,
                symbol: SymbolHandle::invalid(),
                name: lower_name(syntax_trees, &parameter.name),
                relevance: language_core::BindingRelevance::default(),
                type_reference: lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    *type_reference,
                )?,
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
        let retired_identities = syntax_items(syntax_trees)
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
                        syntax_items(syntax_trees).identifier_path_members(quotient.relation),
                    ),
                    relation_symbol: SymbolHandle::invalid(),
                    equivalence: quotient
                        .equivalence
                        .as_ref()
                        .map(|selection| {
                            Ok::<_, Diagnostic>(QuotientEquivalenceSelection {
                                relation: syntax_items(syntax_trees)
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
                                conformance_name: lower_name(
                                    syntax_trees,
                                    &selection.conformance_name,
                                ),
                                conformance_symbol: SymbolHandle::invalid(),
                            })
                        })
                        .transpose()?,
                })
            })
            .transpose()?;
        let type_equations = crate::preparation::generic_data::template_type_equation_offsets(
            syntax_trees,
            data_definition,
            lowerer.constant_selection.as_ref(),
        )?;
        let requires_facts = crate::lowering::domain::lower_proof_facts_excluding(
            lowerer,
            syntax_trees,
            data_definition.requires_facts,
            &type_equations,
        )?;
        let zero_gated = !requires_facts.is_empty();

        Ok(DataDefinition {
            symbol: SymbolHandle::invalid(),
            name: lower_name(syntax_trees, &data_definition.name),
            is_public: data_definition.is_public,
            storage: DataDefinitionStorage {
                predicate_subject_symbol: SymbolHandle::invalid(),
                predicate_subject_type: symbol_resolved_trees::types::TypeReference::Unit,
                supply_mode: data_definition.supply_mode,
                lifetime_parameters: lower_names(
                    syntax_trees,
                    &data_definition.lifetime_parameters,
                ),
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
                    .map(|selection| {
                        lower_static_machine_argument(lowerer, syntax_trees, selection)
                    })
                    .transpose()?,
                quotient,
                requires_facts,
                zero_gated,
                retired_identities,
                members,
            },
        })
    }

    /*
        R2 rung 2 slice 1: fold one default-domain fact AT THE ZERO VALUE --
        every field name reads 0, literals read themselves, `+ - *` fold,
        comparisons and `&&`/`||` yield 1/0. `None` = outside the fragment.
    */
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
                    syntax::item::MachineParameterContract::Inferred => (
                        TypeParameterKind::Machine {
                            contract: symbol_resolved_trees::data::MachineParameterContract::Inferred,
                        },
                        None,
                    ),
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
conformance_bounds: &contract.conformance_bounds,
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
declares_no_crash_route: contract.declares_no_crash_route,
})?;
                        (
                            TypeParameterKind::Machine {
                                contract: symbol_resolved_trees::data::MachineParameterContract::Structural(
                                    lowered_contract.signature,
                                ),
                            },
                            Some((
                                lowered_contract.service_reach_keyword_source_spans,
                                lowered_contract.service_reaches,
                                lowered_contract.callable_scope,
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
        for ordinal in 0..span.count() {
            let handle = span.handle_at(ordinal);
            if crate::selection::callable_inference::pending(
                lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .data_type_parameters
                    .get(handle),
            ) {
                lowerer.pending_callable_types.push(handle);
            }
        }
        if !span.is_empty() {
            for (index, authored) in pending_service_reaches.into_iter().enumerate() {
                let Some((keyword_source_spans, authored, callable_scope)) = authored else {
                    continue;
                };
                let handle =
                    span.handle_at(u32::try_from(index).expect("type-parameter span fits u32"));
                lowerer.runtime_callable_scopes.bind(
                    callable_scope,
                    crate::runtime_callable_scopes::CallableScopeOwner::Signature(
                        PendingSignatureLocation::MachineParameter(handle),
                    ),
                );
                let owner = lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .data_type_parameters
                    .get(handle)
                    .name
                    .clone();
                lowerer
                    .pending_signature_service_reaches
                    .push(crate::resolution::lowerer::PendingSignatureServiceReach {
                    location: PendingSignatureLocation::MachineParameter(handle),
                    owner: crate::resolution::lowerer::PendingSignatureOwner::Requirement(owner),
                    reaches:
                        crate::selection::service_reaches::PendingAuthoredServiceReach::Authored {
                            keyword_source_spans,
                            authored,
                        },
                });
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
                let requires_facts =
                    lower_proof_facts(lowerer, syntax_trees, variant.requires_facts)?;
                Ok(DataMember::Variant(DataVariant {
                    identity: variant.identity,
                    symbol: SymbolHandle::invalid(),
                    name: lower_name(syntax_trees, &variant.name),
                    payload,
                    requires_facts,
                    retired_payload_identities: variant.retired_payload_identities.clone(),
                }))
            }
            syntax::item::DataMember::Retired(_) => unreachable!("retired identities are metadata"),
        }
    }
}
pub(crate) mod domain {
    /*
        Domain definitions: index arguments, aliases, member operators, and the
        proof facts a domain body declares.
    */

    use crate::lowering::data::lower_type_parameters;
    use crate::lowering::expression::lower_expression_into_table;
    use crate::lowering::expression::lower_static_machine_argument;
    use crate::lowering::name::lower_name;
    use crate::lowering::operator::lower_operator_definition;
    use crate::lowering::type_reference::lower_child_type_references;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_items;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::domain::{
        DomainAliasConstituent, DomainAliasDefinition, DomainDefinition, ProofFact,
        ProofMembershipFact,
    };
    use symbols::SymbolHandle;
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_domain_definition(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        domain: &syntax::item::DomainDefinition,
    ) -> Result<DomainDefinition, Diagnostic> {
        let type_parameters = lower_type_parameters(lowerer, syntax_trees, domain.type_parameters)?;
        let index_arguments =
            lower_child_type_references(lowerer, syntax_trees, domain.index_arguments)?;
        let alias = domain
            .alias
            .as_ref()
            .map(|alias| lower_domain_alias(lowerer, syntax_trees, alias));
        let authored_routes = domain
            .authored_routes
            .iter()
            .map(|route| {
                route
                    .iter()
                    .map(|value| lower_name(syntax_trees, value))
                    .collect()
            })
            .collect();
        let facts = lower_proof_facts(lowerer, syntax_trees, domain.facts)?;
        let operators = lowerer.with_authored_expression_exposure(
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
        |lowerer| lower_domain_operators(lowerer, syntax_trees, domain.operators),
    )?;

        let mut semantic_identity =
            declared_module_path(syntax_trees, domain.name.source_span().source_id)
                .map(|module| format!("{module}::{}", domain.name.as_str(syntax_trees)))
                .unwrap_or_else(|| domain.name.as_str(syntax_trees).to_owned());
        let owner = lowerer
            .sources
            .as_deref()
            .map(|sources| {
                sources.file_at(domain.name.source_span()).ok_or_else(|| {
                    Diagnostic::error("domain declaration is missing its checked source owner")
                        .with_source_span(domain.name.source_span())
                })
            })
            .transpose()?;
        if let Some(owner) = owner {
            let scope = match owner.dependency_scope {
                source::DependencyScope::Product => "product",
                source::DependencyScope::Build => "build",
            };
            if let Some(package) = owner.package_identity {
                let hexadecimal = b"0123456789abcdef";
                let digest = package
                    .digest()
                    .into_iter()
                    .flat_map(|byte| {
                        [
                            char::from(hexadecimal[usize::from(byte >> 4)]),
                            char::from(hexadecimal[usize::from(byte & 15)]),
                        ]
                    })
                    .collect::<String>();
                semantic_identity = format!("package:{digest}:{scope}::{semantic_identity}");
            } else if owner.origin == source::SourceOrigin::Toolchain {
                semantic_identity = format!("toolchain:{scope}::{semantic_identity}");
            } else if owner.origin == source::SourceOrigin::ToolchainContract {
                semantic_identity = format!("toolchain-contract:{scope}::{semantic_identity}");
            } else {
                semantic_identity = format!("unmanaged:{scope}::{semantic_identity}");
            }
        }
        let semantic_id = lowerer
            .symbol_resolved_trees
            .semantic_domains
            .intern(&semantic_identity);
        let semantic_roles = language_semantics::DomainSemanticRoles {
            denotation_dimension: (!operators.is_empty()).then_some(semantic_id),
            arithmetic_policy: None,
        };

        Ok(DomainDefinition {
            symbol: SymbolHandle::invalid(),
            name: lower_name(syntax_trees, &domain.name),
            subject_name: symbol_resolved_trees::name::DiagnosticName::generated_static("self"),
            subject_symbol: SymbolHandle::invalid(),
            type_parameters,
            target_type: lower_type_reference_handle(lowerer, syntax_trees, domain.target_type)?,
            index_arguments,
            is_public: domain.is_public,
            alias,
            cleanup_selection: domain
                .cleanup_selection
                .as_ref()
                .map(|selection| lower_static_machine_argument(lowerer, syntax_trees, selection))
                .transpose()?,
            authored_routes,
            classification: domain.classification,
            predicate_presence: domain.predicate_presence,
            facts,
            operators,
            semantic_clause_token_count: domain.semantic_clause_token_count,
            semantic_id,
            semantic_roles,
            establishment_routes: Vec::new(),
        })
    }

    /*
        The logical module path declared by the source that owns `source`, if any.
        Scans root items directly so declaration order cannot hide a module header
        that follows the domain in source order.
    */
    fn declared_module_path(
        syntax_trees: &SyntaxTrees,
        source: source::SourceId,
    ) -> Option<String> {
        syntax_trees.root_items().find_map(|item| {
            let syntax::item::Item::Module(module) = item else {
                return None;
            };
            let members = syntax_trees.items.identifier_path_members(module.path);
            (members.first()?.source_span().source_id == source)
                .then(|| members.joined_path_in(syntax_trees))
        })
    }

    fn lower_domain_alias(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        alias: &syntax::item::DomainAliasDefinition,
    ) -> DomainAliasDefinition {
        let constituents = alias
            .constituents
            .iter()
            .map(|constituent| {
                let mut domain = arena::HandleSpan::empty();
                for member in syntax_trees.items.identifier_path_members(*constituent) {
                    lowerer
                        .symbol_resolved_trees
                        .tables
                        .declarations
                        .domain_path_members
                        .append_to_span(&mut domain, lower_name(syntax_trees, member));
                }
                DomainAliasConstituent {
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                }
            })
            .collect();
        DomainAliasDefinition { constituents }
    }

    fn lower_domain_operators(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        operators: arena::HandleSpan<syntax::item::OperatorDefinition>,
    ) -> Result<arena::HandleSpan<symbol_resolved_trees::operator::OperatorDefinition>, Diagnostic>
    {
        let mut span = arena::HandleSpan::empty();

        for operator in syntax_trees.items.operators(operators) {
            let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .operator_definitions
                .append_to_span(&mut span, operator);
        }

        Ok(span)
    }

    pub(crate) fn lower_proof_facts(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        facts: arena::HandleSpan<syntax::item::ProofFact>,
    ) -> Result<arena::HandleSpan<ProofFact>, Diagnostic> {
        lower_proof_facts_excluding(lowerer, syntax_trees, facts, &[])
    }

    /*
        Lower runtime facts while leaving template instantiation obligations with
        their synthesis owner. Offsets refer to the original span, preserving source
        custody for every retained fact rather than copying/reindexing its syntax.
    */
    pub(crate) fn lower_proof_facts_excluding(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        facts: arena::HandleSpan<syntax::item::ProofFact>,
        instantiation_obligation_offsets: &[usize],
    ) -> Result<arena::HandleSpan<ProofFact>, Diagnostic> {
        let mut lowered = arena::HandleSpan::empty();

        for (offset, fact) in syntax_trees.items.proof_facts(facts).iter().enumerate() {
            if instantiation_obligation_offsets.contains(&offset) {
                continue;
            }
            let source_fact = arena::Handle::from_parts(
                facts
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                    .expect("proof fact source handle overflow"),
                facts.start().generation(),
            );
            let source_span = syntax_trees.items.proof_fact_source_span(source_fact);
            let fact = match fact {
                syntax::item::ProofFact::Expression(expression) => {
                    let expression =
                        lower_expression_into_table(lowerer, syntax_trees, *expression)?;
                    ProofFact::Expression(expression)
                }
                syntax::item::ProofFact::Membership(membership) => {
                    let value =
                        lower_expression_into_table(lowerer, syntax_trees, membership.value)?;
                    let mut domain = arena::HandleSpan::empty();
                    for member in
                        syntax_items(syntax_trees).identifier_path_members(membership.domain)
                    {
                        lowerer
                            .symbol_resolved_trees
                            .tables
                            .declarations
                            .domain_path_members
                            .append_to_span(&mut domain, lower_name(syntax_trees, member));
                    }
                    let domain_arguments =
                        crate::lowering::type_reference::lower_indexed_application_arguments(
                            lowerer,
                            syntax_trees,
                            membership.domain_arguments,
                        )?;
                    ProofFact::Membership(ProofMembershipFact {
                        value,
                        domain,
                        domain_symbol: SymbolHandle::invalid(),
                        domain_arguments,
                        authored_domain_selection: None,
                    })
                }
            };

            let is_membership = matches!(fact, ProofFact::Membership(_));
            let fact = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .proof_facts
                .append_to_span(&mut lowered, fact);
            if let Some(source_span) = source_span {
                lowerer
                    .symbol_resolved_trees
                    .set_proof_fact_source_span(fact, source_span);
            }
            if is_membership && let Some(exposure) = lowerer.current_authored_expression_exposure {
                lowerer.pending_authored_proof_memberships.push(
                    crate::resolution::lowerer::PendingAuthoredProofMembership { fact, exposure },
                );
            }
        }

        Ok(lowered)
    }
}
pub(crate) mod expression {
    /*
        Expressions into the shared expression table.

        Authored state-body expressions are retained for declaration-selection
        admission; compiler-generated inserts are not. Static call receivers are
        recorded as pending module calls for `symbols` to normalize.
    */

    use crate::lowering::name::lower_name;
    use crate::lowering::statement::hoist_temps::reference_temp;
    use crate::lowering::type_reference::lower_child_type_references;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_expressions;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_core::atomic::AtomicOrderingPlan;
    use symbol_resolved_trees::expression::{
        BinaryOperator, EvidenceProjection, ExpressionHandle, ExpressionNode, ExpressionTable,
        StaticMachineArgument, StaticSymbolApplication, TableAtomicExpression,
        TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
        TableIndexedExpression, TableMemberExpression, TableMembershipExpression, TableNamePath,
        TableRangeExpression, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
        UnaryOperator,
    };
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::SymbolHandle;
    use syntax::types::TypeReferenceHandle;
    use syntax::types::TypeReferenceNode;
    use syntax_trees as syntax;
    use syntax_trees::SyntaxTrees;

    pub(crate) mod match_dispatch {

        /*
            Resolve one capture definition and selected-arm aliases without introducing
            synthetic locals. Alias definitions are scoped by generational handles;
            equal spellings or equivalent-looking subjects cannot select another arm.
        */

        use super::{expression_table, lower_expression_into_table, lower_name};
        use crate::resolution::lowerer::{Lowerer, MatchArmCapture};
        use crate::tables::syntax_expressions;
        use arena::Handle;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern,
            TableBinaryExpression, TableDestructurePattern, TableMatchArm, TableMatchExpression,
            TableMatchField, TableMemberExpression, TableSavedSubject,
        };
        use symbol_resolved_trees::statement::OutcomeProofSelector;
        use syntax_trees::{SyntaxTrees, expression as syntax};

        pub(super) fn lower_match(
            lowerer: &mut Lowerer,
            syntax_trees: &SyntaxTrees,
            dispatch: syntax::TableMatchExpression,
        ) -> Result<ExpressionHandle, Diagnostic> {
            let subject = lower_expression_into_table(lowerer, syntax_trees, dispatch.subject)?;
            let capture = if dispatch.capture.is_valid() {
                if syntax_expressions(syntax_trees)
                    .match_subject(dispatch.capture)
                    .subject
                    != dispatch.subject
                {
                    return Err(Diagnostic::error(
                        "match subject capture does not name its defining subject",
                    ));
                }
                expression_table(lowerer).insert_match_subject(subject)
            } else {
                Handle::invalid()
            };
            let mut arms = Vec::new();
            for arm in syntax_trees.expressions.match_arms(dispatch.arms) {
                let pattern = match arm.pattern {
                    syntax::MatchPattern::Value(value) => MatchPattern::Value(
                        lower_expression_into_table(lowerer, syntax_trees, value)?,
                    ),
                    syntax::MatchPattern::Wildcard => MatchPattern::Wildcard,
                    syntax::MatchPattern::Destructure(pattern) => lower_destructure_pattern(
                        lowerer,
                        syntax_trees,
                        pattern,
                        dispatch.capture,
                        capture,
                    )?,
                };
                let has_aliases = if let (
                    syntax::MatchPattern::Destructure(source),
                    MatchPattern::Destructure(resolved),
                ) = (arm.pattern, pattern)
                {
                    lowerer.expression_match_captures.push(MatchArmCapture {
                        syntax_capture: dispatch.capture,
                        syntax_head: source.head,
                        resolved_capture: capture,
                        resolved_head: resolved.head,
                    });
                    true
                } else {
                    false
                };
                let value = lower_expression_into_table(lowerer, syntax_trees, arm.value);
                if has_aliases {
                    lowerer.expression_match_captures.pop();
                }
                arms.push(TableMatchArm {
                    pattern,
                    value: value?,
                    source_span: arm.source_span,
                });
            }
            let arms = expression_table(lowerer).insert_match_arms(arms);
            Ok(
                expression_table(lowerer).insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    capture,
                    arms,
                })),
            )
        }

        /*
            Declaration-input preparation lowers selected calls before executable
            bodies. Preserve the current transition's lexical aliases while doing so;
            this installs no statements and does not complete or admit the machine.
        */
        pub(in crate::lowering) fn with_transition_pattern_captures<T>(
            lowerer: &mut Lowerer,
            syntax_trees: &SyntaxTrees,
            statement: syntax_trees::statement::StatementHandle,
            operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
        ) -> Result<T, Diagnostic> {
            let outer = lowerer.expression_match_captures.len();
            let result = (|| {
                if let syntax_trees::statement::StatementNode::Transition(transition) =
                    syntax_trees.statements.statement(statement)
                {
                    for retained in syntax_trees
                        .statements
                        .transition_patterns(transition.patterns)
                    {
                        let subject =
                            lower_expression_into_table(lowerer, syntax_trees, retained.subject)?;
                        let head = lower_expression_into_table(
                            lowerer,
                            syntax_trees,
                            retained.pattern.head,
                        )?;
                        let capture = expression_table(lowerer).insert_transition_subject(subject);
                        lowerer.expression_match_captures.push(MatchArmCapture {
                            syntax_capture: retained.capture,
                            syntax_head: retained.pattern.head,
                            resolved_capture: capture,
                            resolved_head: head,
                        });
                    }
                }
                operation(lowerer)
            })();
            lowerer.expression_match_captures.truncate(outer);
            result
        }

        pub(super) fn lower_saved_subject(
            lowerer: &mut Lowerer,
            syntax_trees: &SyntaxTrees,
            saved: syntax::TableSavedSubject,
        ) -> Result<ExpressionHandle, Diagnostic> {
            let binding = lowerer
        .expression_match_captures
        .iter()
        .rev()
        .find(|binding| {
            binding.syntax_capture == saved.capture && binding.syntax_head == saved.pattern_head
        })
        .copied()
        .ok_or_else(|| {
            let head = match syntax_trees.expressions.expression(saved.pattern_head) {
                syntax::ExpressionNode::Name(path) => syntax_trees.expressions
                    .identifier_path_members(*path).iter()
                    .map(|name| name.as_str(syntax_trees)).collect::<Vec<_>>().join("::"),
                _ => "<non-name>".to_owned(),
            };
            Diagnostic::error(format!(
                "saved match subject read has no enclosing arm capture in `{}::{}` for `{head}`: capture {:?}, head {:?}, purpose {:?}, enclosing {:?}",
                lowerer.current_machine_name.as_deref().unwrap_or("<declaration>"),
                lowerer.current_state_name.as_deref().unwrap_or("<signature>"),
                saved.capture, saved.pattern_head,
                syntax_trees.expressions.match_subject(saved.capture).purpose,
                lowerer.expression_match_captures.iter().map(|binding| (binding.syntax_capture,binding.syntax_head)).collect::<Vec<_>>(),
            ))
        })?;
            Ok(
                expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
                    component: saved.component,
                    capture: binding.resolved_capture,
                    pattern_head: binding.resolved_head,
                })),
            )
        }

        pub(crate) fn lower_destructure_pattern(
            lowerer: &mut Lowerer,
            syntax_trees: &SyntaxTrees,
            pattern: syntax::TableDestructurePattern,
            syntax_capture: Handle<syntax::MatchSubjectCapture>,
            capture: Handle<symbol_resolved_trees::expression::MatchSubjectCapture>,
        ) -> Result<MatchPattern, Diagnostic> {
            let application_type = if pattern.application_type.is_valid() {
                let reference = crate::lowering::type_reference::lower_type_reference_handle(
                    lowerer,
                    syntax_trees,
                    pattern.application_type,
                )?;
                lowerer
                    .declaration_child_type_references()
                    .append(reference)
            } else {
                Default::default()
            };
            let head = lower_expression_into_table(lowerer, syntax_trees, pattern.head)?;
            let mut fields = Vec::new();
            for field in syntax_trees.expressions.match_fields(pattern.fields) {
                use symbol_resolved_trees::expression::MatchFieldAction;
                let action = match &field.action {
                    syntax::MatchFieldAction::Bind(binding) => {
                        MatchFieldAction::Bind(lower_name(syntax_trees, binding))
                    }
                    syntax::MatchFieldAction::Equal(value) => MatchFieldAction::Equal(
                        lower_expression_into_table(lowerer, syntax_trees, *value)?,
                    ),
                    syntax::MatchFieldAction::Waive => MatchFieldAction::Waive,
                };
                fields.push(TableMatchField {
                    component: field.component,
                    member: lower_name(syntax_trees, &field.member),
                    field_symbol: Handle::invalid(),
                    action,
                });
            }
            let selectors = syntax_trees
                .statements
                .outcome_proof_selectors(pattern.proof_selectors)
                .iter()
                .map(|selector| OutcomeProofSelector {
                    output_field: lower_name(syntax_trees, &selector.output_field),
                    binding: lower_name(syntax_trees, &selector.binding),
                });
            let proof_selectors = expression_table(lowerer).insert_match_proof_selectors(selectors);
            lowerer.expression_match_captures.push(MatchArmCapture {
                syntax_capture,
                syntax_head: pattern.head,
                resolved_capture: capture,
                resolved_head: head,
            });
            let guard = lower_expression_into_table(lowerer, syntax_trees, pattern.guard);
            lowerer.expression_match_captures.pop();
            let guard = guard?;
            let saved =
                expression_table(lowerer).insert(ExpressionNode::SavedSubject(TableSavedSubject {
                    component: Default::default(),
                    capture,
                    pattern_head: head,
                }));
            let mut condition = ExpressionHandle::invalid();
            for field in &fields {
                if let MatchFieldAction::Equal(expected) = field.action {
                    let projected = expression_table(lowerer).insert(ExpressionNode::Member(
                        TableMemberExpression {
                            receiver: saved,
                            member_symbol: field.field_symbol,
                            member: field.member.clone(),
                            case_variant: None,
                        },
                    ));
                    let equality = expression_table(lowerer).insert(ExpressionNode::Binary(
                        TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: projected,
                            operator: BinaryOperator::Equal,
                            right: expected,
                        },
                    ));
                    condition = if condition.is_valid() {
                        expression_table(lowerer).insert(ExpressionNode::Binary(
                            TableBinaryExpression {
                                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                                left: condition,
                                operator: BinaryOperator::And,
                                right: equality,
                            },
                        ))
                    } else {
                        equality
                    };
                }
            }
            if guard.is_valid() {
                condition = if condition.is_valid() {
                    expression_table(lowerer).insert(ExpressionNode::Binary(
                        TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: condition,
                            operator: BinaryOperator::And,
                            right: guard,
                        },
                    ))
                } else {
                    guard
                };
            }
            if !condition.is_valid() {
                condition = expression_table(lowerer).insert(ExpressionNode::Boolean(true));
            }
            let fields = expression_table(lowerer).insert_match_fields(fields);
            Ok(MatchPattern::Destructure(TableDestructurePattern {
                application_type,
                head,
                fields,
                has_rest: pattern.has_rest,
                guard,
                condition,
                proof_selectors,
            }))
        }
    }

    pub(crate) fn lower_expression_into_table(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expression: syntax::expression::ExpressionHandle,
    ) -> Result<ExpressionHandle, Diagnostic> {
        if !expression.is_valid() {
            return Ok(ExpressionHandle::invalid());
        }
        let source_span = syntax_trees.expressions.source_span(expression);
        let lowered = if let Some((name, _)) = lowerer.match_subject_temp(expression) {
            reference_temp(lowerer, DiagnosticName::generated(name))
        } else {
            lower_expression_node_into_table(
                lowerer,
                syntax_trees,
                syntax_trees.expressions.expression(expression),
            )?
        };
        expression_table(lowerer).set_source_span(lowered, source_span);
        if let syntax::expression::ExpressionNode::Call(call) =
            syntax_trees.expressions.expression(expression)
            && call.target_is_static
            && call.receiver.is_valid()
            && let syntax::expression::ExpressionNode::Name(receiver) =
                syntax_trees.expressions.expression(call.receiver)
        {
            let mut path = syntax_expressions(syntax_trees)
                .identifier_path_members(*receiver)
                .iter()
                .map(|value| lower_name(syntax_trees, value))
                .collect::<Vec<_>>();
            path.push(lower_name(syntax_trees, &call.target));
            lowerer.pending_static_module_calls.push((lowered, path));
        }
        if let Some(exposure) = lowerer.current_authored_expression_exposure {
            expression_table(lowerer).set_authored_expression_exposure(lowered, exposure);
            lowerer.pending_authored_expressions.push(
                crate::resolution::lowerer::PendingAuthoredExpression {
                    expression: lowered,
                    exposure,
                },
            );
        }
        if let Some(partition) = lowerer.current_compiler_selection_partition {
            expression_table(lowerer).set_compiler_selection_partition(lowered, partition);
        }
        if let Some(sources) = &mut lowerer.equation_sources {
            sources.expressions.push((expression, lowered));
        }
        Ok(lowered)
    }

    /*
        Lower an authored state-body expression and retain it for package
        declaration-selection admission. The temporary context propagates through
        every recursively lowered syntax child; direct compiler-generated inserts
        remain outside the authored set.
    */
    pub(crate) fn lower_private_expression_into_table(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expression: syntax::expression::ExpressionHandle,
    ) -> Result<ExpressionHandle, Diagnostic> {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;
        let exposure = if syntax_trees
            .roots
            .constant_extent_probes
            .iter()
            .any(|(root, public)| *root == expression && *public)
        {
            Exposure::PublicInterface
        } else {
            Exposure::PrivateImplementation
        };
        let previous = lowerer
            .current_authored_expression_exposure
            .replace(exposure);
        let result = lower_expression_into_table(lowerer, syntax_trees, expression);
        lowerer.current_authored_expression_exposure = previous;
        result
    }

    fn lower_expression_node_into_table(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expression: &syntax::expression::ExpressionNode,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let syntax::expression::ExpressionNode::Binary(binary) = expression else {
            return lower_nonbinary_expression_node_into_table(lowerer, syntax_trees, expression);
        };
        lower_binary_expression_into_table(lowerer, syntax_trees, binary)
    }

    fn lower_binary_expression_into_table(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        binary: &syntax::expression::TableBinaryExpression,
    ) -> Result<ExpressionHandle, Diagnostic> {
        let left = lower_expression_into_table(lowerer, syntax_trees, binary.left)?;
        let right = lower_expression_into_table(lowerer, syntax_trees, binary.right)?;
        Ok(
            expression_table(lowerer).insert(ExpressionNode::Binary(TableBinaryExpression {
                policy: binary.policy,
                left,
                operator: lower_binary_operator(binary.operator),
                right,
            })),
        )
    }

    fn lower_nonbinary_expression_node_into_table(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        expression: &syntax::expression::ExpressionNode,
    ) -> Result<ExpressionHandle, Diagnostic> {
        match expression {
            syntax::expression::ExpressionNode::TypeExpression(reference) => {
                let reference = lower_type_reference_handle(lowerer, syntax_trees, *reference)?;
                let reference = lowerer
                    .declaration_child_type_references()
                    .append(reference);
                Ok(expression_table(lowerer).insert(ExpressionNode::TypeExpression(reference)))
            }
            syntax::expression::ExpressionNode::Match(dispatch) => {
                match_dispatch::lower_match(lowerer, syntax_trees, *dispatch)
            }
            syntax::expression::ExpressionNode::SavedSubject(saved) => {
                match_dispatch::lower_saved_subject(lowerer, syntax_trees, *saved)
            }
            syntax::expression::ExpressionNode::ArrayLiteral(values) => {
                let span = expression_table(lowerer).reserve_expression_handles(values.count());
                for (offset, value) in syntax_expressions(syntax_trees)
                    .expression_handles(*values)
                    .iter()
                    .enumerate()
                {
                    let value = lower_expression_into_table(lowerer, syntax_trees, *value)?;
                    expression_table(lowerer).set_expression_handle_at_offset(
                        span,
                        offset
                            .try_into()
                            .expect("expression handle span count overflow"),
                        value,
                    );
                }
                Ok(expression_table(lowerer).insert(ExpressionNode::ArrayLiteral(span)))
            }
            syntax::expression::ExpressionNode::Atomic(atomic) => {
                if !atomic.result_custody.is_valid_for(atomic.ordering)
                    || (atomic.result_custody.has_structural_result() && atomic.result.is_valid())
                {
                    return Err(Diagnostic::error(
                        "atomic expression result custody does not match its operation axis",
                    ));
                }
                let value = if matches!(
                    atomic.ordering,
                    AtomicOrderingPlan::CompareExchangeOnce { .. }
                ) {
                    lower_expression_node_into_table(
                        lowerer,
                        syntax_trees,
                        syntax_trees.expressions.expression(atomic.value),
                    )?
                } else {
                    lower_expression_into_table(lowerer, syntax_trees, atomic.value)?
                };
                let result = if atomic.result.is_valid() {
                    lower_expression_into_table(lowerer, syntax_trees, atomic.result)?
                } else {
                    ExpressionHandle::invalid()
                };
                Ok(expression_table(lowerer).insert(ExpressionNode::Atomic(
                    TableAtomicExpression {
                        value,
                        result,
                        ordering: atomic.ordering,
                        result_custody: atomic.result_custody,
                    },
                )))
            }
            syntax::expression::ExpressionNode::Binary(_) => {
                unreachable!("binary expressions use the stack-bounded lowering path")
            }
            syntax::expression::ExpressionNode::Boolean(value) => {
                Ok(expression_table(lowerer).insert(ExpressionNode::Boolean(*value)))
            }
            syntax::expression::ExpressionNode::Cast(cast) => {
                let value = lower_expression_into_table(lowerer, syntax_trees, cast.value)?;
                let target_type =
                    lower_type_reference_handle(lowerer, syntax_trees, cast.target_type)?;
                let target_type = lowerer
                    .declaration_child_type_references()
                    .append(target_type);
                let mut target_label = HandleSpan::empty();
                for member in
                    syntax_expressions(syntax_trees).identifier_path_members(cast.target_label)
                {
                    expression_table(lowerer)
                        .push_name_path_member(&mut target_label, lower_name(syntax_trees, member));
                }
                let mut semantic_domain = HandleSpan::empty();
                for member in
                    syntax_expressions(syntax_trees).identifier_path_members(cast.semantic_domain)
                {
                    expression_table(lowerer).push_name_path_member(
                        &mut semantic_domain,
                        lower_name(syntax_trees, member),
                    );
                }
                let semantic_domain_arguments = lower_child_type_references(
                    lowerer,
                    syntax_trees,
                    cast.semantic_domain_arguments,
                )?;
                Ok(
                    expression_table(lowerer).insert(ExpressionNode::Cast(TableCastExpression {
                        value,
                        target_type,
                        target_label,
                        domain: cast.domain,
                        semantic_domain,
                        semantic_domain_arguments,
                        semantic_domain_symbol: SymbolHandle::invalid(),
                        form: cast.form,
                    })),
                )
            }
            syntax::expression::ExpressionNode::Call(call) => {
                let callee = call
                    .callee
                    .is_valid()
                    .then(|| lower_expression_into_table(lowerer, syntax_trees, call.callee))
                    .transpose()?
                    .unwrap_or_else(ExpressionHandle::invalid);
                let (receiver, requirement_application) = match call
                    .receiver
                    .is_valid()
                    .then(|| syntax_trees.expressions.expression(call.receiver))
                {
                    Some(syntax::expression::ExpressionNode::TypeExpression(application))
                        if call.target_is_static =>
                    {
                        lower_applied_requirement_qualifier(lowerer, syntax_trees, *application)?
                    }
                    Some(_) => (
                        lower_expression_into_table(lowerer, syntax_trees, call.receiver)?,
                        arena::Handle::invalid(),
                    ),
                    None => (ExpressionHandle::invalid(), arena::Handle::invalid()),
                };
                let arguments =
                    expression_table(lowerer).reserve_expression_handles(call.arguments.count());
                for (offset, argument) in syntax_expressions(syntax_trees)
                    .expression_handles(call.arguments)
                    .iter()
                    .enumerate()
                {
                    let argument = lower_expression_into_table(lowerer, syntax_trees, *argument)?;
                    expression_table(lowerer).set_expression_handle_at_offset(
                        arguments,
                        offset
                            .try_into()
                            .expect("expression handle span count overflow"),
                        argument,
                    );
                }
                let machine_arguments = call
                    .machine_arguments
                    .iter()
                    .map(|argument| lower_static_machine_argument(lowerer, syntax_trees, argument))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice();
                Ok(
                    expression_table(lowerer).insert(ExpressionNode::Call(TableCallExpression {
                        callee,
                        authored_call_binding: None,
                        receiver,
                        target_symbol: SymbolHandle::invalid(),
                        target: lower_name(syntax_trees, &call.target),
                        requirement_application,
                        machine_arguments,
                        arguments,
                        evidence_arguments: call
                            .evidence_arguments
                            .iter()
                            .map(|value| lower_name(syntax_trees, value))
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        operational_acknowledgement: call.operational_acknowledgement,
                    })),
                )
            }
            syntax::expression::ExpressionNode::UndeterminedNaN(format) => {
                Ok(expression_table(lowerer).insert(ExpressionNode::UndeterminedNaN(*format)))
            }
            syntax::expression::ExpressionNode::Float(value) => {
                let Some(value) = value.literal(syntax_trees) else {
                    return Err(Diagnostic::error(format!(
                        "invalid float literal `{}`",
                        value.as_str(syntax_trees)
                    )));
                };
                Ok(expression_table(lowerer).insert(ExpressionNode::Float(value)))
            }
            syntax::expression::ExpressionNode::Indexed(indexed) => {
                let collection =
                    lower_expression_into_table(lowerer, syntax_trees, indexed.collection)?;
                let index = lower_expression_into_table(lowerer, syntax_trees, indexed.index)?;
                Ok(expression_table(lowerer).insert(ExpressionNode::Indexed(
                    TableIndexedExpression { collection, index },
                )))
            }
            syntax::expression::ExpressionNode::Integer(value) => {
                Ok(expression_table(lowerer).insert(ExpressionNode::Integer(value.clone())))
            }
            syntax::expression::ExpressionNode::Membership(membership) => {
                let value = lower_expression_into_table(lowerer, syntax_trees, membership.value)?;
                let mut domain = HandleSpan::empty();
                for member in
                    syntax_expressions(syntax_trees).identifier_path_members(membership.domain)
                {
                    expression_table(lowerer)
                        .push_name_path_member(&mut domain, lower_name(syntax_trees, member));
                }
                Ok(expression_table(lowerer).insert(ExpressionNode::Membership(
                    TableMembershipExpression {
                        is_pattern: false,
                        value,
                        domain,
                        domain_symbol: SymbolHandle::invalid(),
                        case_type_symbol: SymbolHandle::invalid(),
                        case_symbol: SymbolHandle::invalid(),
                    },
                )))
            }
            syntax::expression::ExpressionNode::Member(member) => {
                let receiver = lower_expression_into_table(lowerer, syntax_trees, member.receiver)?;
                Ok(expression_table(lowerer).insert(ExpressionNode::Member(
                    TableMemberExpression {
                        receiver,
                        member_symbol: SymbolHandle::invalid(),
                        member: lower_name(syntax_trees, &member.member),
                        case_variant: member
                            .case_variant
                            .as_ref()
                            .map(|value| lower_name(syntax_trees, value)),
                    },
                )))
            }
            syntax::expression::ExpressionNode::Borrow(expression) => {
                let target = lower_expression_into_table(lowerer, syntax_trees, expression.target)?;
                Ok(expression_table(lowerer).insert(ExpressionNode::Borrow(
                    TableBorrowExpression {
                        target,
                        access: expression.access,
                    },
                )))
            }
            syntax::expression::ExpressionNode::Name(path) => {
                let mut members = HandleSpan::empty();
                for member in syntax_trees.expressions.identifier_path_members(*path) {
                    expression_table(lowerer)
                        .push_name_path_member(&mut members, lower_name(syntax_trees, member));
                }
                let member_symbols =
                    expression_table(lowerer).reserve_name_path_member_symbols(members.count());
                Ok(
                    expression_table(lowerer).insert(ExpressionNode::Name(TableNamePath::new(
                        members,
                        member_symbols,
                        SymbolHandle::invalid(),
                        SymbolHandle::invalid(),
                    ))),
                )
            }
            syntax::expression::ExpressionNode::Range(range) => {
                let start = range
                    .start
                    .is_valid()
                    .then(|| lower_expression_into_table(lowerer, syntax_trees, range.start))
                    .transpose()?
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| lower_expression_into_table(lowerer, syntax_trees, range.end))
                    .transpose()?
                    .unwrap_or_else(ExpressionHandle::invalid);
                Ok(
                    expression_table(lowerer).insert(ExpressionNode::Range(TableRangeExpression {
                        start,
                        end,
                        end_inclusive: range.end_inclusive,
                    })),
                )
            }
            syntax::expression::ExpressionNode::String(value) => {
                Ok(expression_table(lowerer).insert(ExpressionNode::String(value.clone())))
            }
            syntax::expression::ExpressionNode::Unary(unary) => {
                let operand = lower_expression_into_table(lowerer, syntax_trees, unary.operand)?;
                Ok(
                    expression_table(lowerer).insert(ExpressionNode::Unary(TableUnaryExpression {
                        operator: lower_unary_operator(unary.operator),
                        operand,
                    })),
                )
            }
            syntax::expression::ExpressionNode::ZeroValue(type_reference) => {
                let type_reference =
                    lower_type_reference_handle(lowerer, syntax_trees, *type_reference)?;
                let type_reference = lowerer
                    .declaration_child_type_references()
                    .append(type_reference);
                Ok(expression_table(lowerer).insert(ExpressionNode::ZeroValue(type_reference)))
            }
            syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
                let application_type = if struct_literal.application_type.is_valid() {
                    let reference = lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        struct_literal.application_type,
                    )?;
                    lowerer
                        .declaration_child_type_references()
                        .append(reference)
                } else {
                    Default::default()
                };

                let fields =
                    expression_table(lowerer).reserve_struct_fields(struct_literal.fields.count());
                for (offset, field) in syntax_expressions(syntax_trees)
                    .struct_fields(struct_literal.fields)
                    .iter()
                    .enumerate()
                {
                    let value = lower_expression_into_table(lowerer, syntax_trees, field.value)?;
                    expression_table(lowerer).set_struct_field_at_offset(
                        fields,
                        offset
                            .try_into()
                            .expect("struct literal field span count overflow"),
                        TableStructLiteralField {
                            name: lower_name(syntax_trees, &field.name),
                            field_symbol: SymbolHandle::invalid(),
                            value,
                        },
                    );
                }
                Ok(
                    expression_table(lowerer).insert(ExpressionNode::StructLiteral(
                        TableStructLiteral {
                            application_type,
                            type_name: lower_name(syntax_trees, &struct_literal.constructor_name),
                            type_symbol: SymbolHandle::invalid(),
                            case_name: None,
                            case_symbol: None,
                            fields,
                        },
                    )),
                )
            }
        }
    }

    /*
        Lower an applied requirement qualifier into the trait path a static call
        names and the exact application kept beside it. Only a generic trait path
        qualifies; any other type expression stays an invalid runtime receiver.
    */
    fn lower_applied_requirement_qualifier(
        lowerer: &mut Lowerer,
        syntax_trees: &syntax::SyntaxTrees,
        application: TypeReferenceHandle,
    ) -> Result<(ExpressionHandle, arena::Handle<TypeReference>), Diagnostic> {
        let TypeReferenceNode::Generic { base_name, .. } =
            syntax_trees.type_references.type_reference(application)
        else {
            return Err(Diagnostic::error(
                "an applied requirement call names a generic trait application before `::`",
            ));
        };
        let mut path = Default::default();
        for member in base_name.as_str(syntax_trees).split("::") {
            expression_table(lowerer).push_name_path_member(
                &mut path,
                DiagnosticName::from_str(member, base_name.source_span()),
            );
        }
        let receiver = expression_table(lowerer).insert(ExpressionNode::Name(
            symbol_resolved_trees::expression::TableNamePath {
                members: path,
                ..Default::default()
            },
        ));
        expression_table(lowerer).set_source_span(receiver, base_name.source_span());
        let reference = lower_type_reference_handle(lowerer, syntax_trees, application)?;
        let application = lowerer
            .declaration_child_type_references()
            .append(reference);
        Ok((receiver, application))
    }

    pub(crate) fn lower_static_machine_argument(
        lowerer: &mut Lowerer,
        syntax_trees: &syntax::SyntaxTrees,
        argument: &syntax::expression::StaticMachineArgument,
    ) -> Result<StaticMachineArgument, Diagnostic> {
        let type_reference = if argument.type_reference.is_valid() {
            let reference =
                lower_type_reference_handle(lowerer, syntax_trees, argument.type_reference)?;
            lowerer
                .declaration_child_type_references()
                .append(reference)
        } else {
            arena::Handle::invalid()
        };
        Ok(StaticMachineArgument {
            type_reference,
            path: argument
                .path
                .iter()
                .map(|value| lower_name(syntax_trees, value))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            application: argument
                .application
                .as_ref()
                .map(|application| {
                    Ok::<_, Diagnostic>(Box::new(StaticSymbolApplication {
                        member: if application.member.is_valid() {
                            let source = syntax_expressions(syntax_trees)
                                .static_member(application.member)
                                .ok_or_else(|| {
                                    Diagnostic::error(
                                        "qualified static member has no owning syntax record",
                                    )
                                })?;
                            let member =
                                lower_static_machine_argument(lowerer, syntax_trees, source)?;
                            expression_table(lowerer).insert_static_member(member)
                        } else {
                            arena::Handle::invalid()
                        },
                        lifetime_arguments: application
                            .lifetime_arguments
                            .iter()
                            .map(|value| lower_name(syntax_trees, value))
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        arguments: application
                            .arguments
                            .iter()
                            .map(|argument| {
                                lower_static_machine_argument(lowerer, syntax_trees, argument)
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .into_boxed_slice(),
                    }))
                })
                .transpose()?,
            const_literal: argument.const_literal.clone(),
            evidence_projection: argument.evidence_projection.as_ref().map(|projection| {
                EvidenceProjection {
                    term: lower_name(syntax_trees, &projection.term),
                    member: lower_name(syntax_trees, &projection.member),
                }
            }),
            symbol: SymbolHandle::invalid(),
        })
    }

    pub(super) fn expression_table(lowerer: &mut Lowerer) -> &mut ExpressionTable {
        &mut lowerer.symbol_resolved_trees.tables.bodies.expressions
    }

    fn lower_unary_operator(operator: syntax::expression::UnaryOperator) -> UnaryOperator {
        match operator {
            syntax::expression::UnaryOperator::BitwiseNot => UnaryOperator::BitwiseNot,
            syntax::expression::UnaryOperator::LogicalNot => UnaryOperator::LogicalNot,
        }
    }

    fn lower_binary_operator(operator: syntax::expression::BinaryOperator) -> BinaryOperator {
        match operator {
            syntax::expression::BinaryOperator::Add => BinaryOperator::Add,
            syntax::expression::BinaryOperator::And => BinaryOperator::And,
            syntax::expression::BinaryOperator::BitwiseAnd => BinaryOperator::BitwiseAnd,
            syntax::expression::BinaryOperator::BitwiseOr => BinaryOperator::BitwiseOr,
            syntax::expression::BinaryOperator::BitwiseXor => BinaryOperator::BitwiseXor,
            syntax::expression::BinaryOperator::Divide => BinaryOperator::Divide,
            syntax::expression::BinaryOperator::Equal => BinaryOperator::Equal,
            syntax::expression::BinaryOperator::Greater => BinaryOperator::Greater,
            syntax::expression::BinaryOperator::GreaterOrEqual => BinaryOperator::GreaterOrEqual,
            syntax::expression::BinaryOperator::Less => BinaryOperator::Less,
            syntax::expression::BinaryOperator::LessOrEqual => BinaryOperator::LessOrEqual,
            syntax::expression::BinaryOperator::Modulo => BinaryOperator::Modulo,
            syntax::expression::BinaryOperator::Multiply => BinaryOperator::Multiply,
            syntax::expression::BinaryOperator::NotEqual => BinaryOperator::NotEqual,
            syntax::expression::BinaryOperator::Or => BinaryOperator::Or,
            syntax::expression::BinaryOperator::ShiftLeft => BinaryOperator::ShiftLeft,
            syntax::expression::BinaryOperator::ShiftRight => BinaryOperator::ShiftRight,
            syntax::expression::BinaryOperator::Subtract => BinaryOperator::Subtract,
        }
    }
}
pub(crate) mod item {
    /*
        Root item dispatch.

        `lower_item` records the item's authored-expression exposure, routes each
        syntax item kind to the module owning its carrier area, and pushes the
        result into the lowerer's trees. Every product target member and its
        independent portable interface lower together; requested realizations never
        decide which source bodies are retained or checked.
    */

    use crate::lowering::data::lower_data_definition;
    use crate::lowering::data::lower_type_parameters;
    use crate::lowering::domain::lower_domain_definition;
    use crate::lowering::machine::lower_machine_into;
    use crate::lowering::measure::lower_measure_definition;
    use crate::lowering::name::lower_name;
    use crate::lowering::name::lower_names;
    use crate::lowering::operator::lower_bare_bodyless_signature;
    use crate::lowering::operator::lower_operator_definition;
    use crate::lowering::proposition::lower_proposition_definition;
    use crate::lowering::trait_definition::lower_trait_definition;
    use crate::lowering::type_reference::lower_child_type_references;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::ConstResolutionMode;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_items;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::trait_definition::ConformanceRowSource;
    use syntax::item::Item;
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_item(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        declaration: syntax::item::ItemHandle,
    ) -> Result<(), Diagnostic> {
        let inputs = crate::resolution::type_inputs::TypeInputScope::new(lowerer);
        let item = syntax_trees.root_item(declaration);
        let conformance_ordinal = lowerer.symbol_resolved_trees.conformances.len();
        let trait_ordinal = lowerer.symbol_resolved_trees.traits.len();
        let data_ordinal = lowerer.symbol_resolved_trees.data_definitions.len();
        use crate::runtime_callable_scopes::CallableScopeOwner;
        let owner = match item {
            Item::Data(_) => CallableScopeOwner::Data(data_ordinal),
            Item::TypeAlias(_) => {
                CallableScopeOwner::Alias(lowerer.symbol_resolved_trees.type_aliases.len())
            }
            Item::Domain(_) => {
                CallableScopeOwner::Domain(lowerer.symbol_resolved_trees.domain_definitions.len())
            }
            Item::Trait(_) => CallableScopeOwner::Trait(trait_ordinal),
            Item::Conformance(_) => CallableScopeOwner::Conformance(conformance_ordinal),
            Item::Operator(_) => {
                CallableScopeOwner::Operator(lowerer.symbol_resolved_trees.operators.len())
            }
            Item::Machine(machine)
                if machine.target.is_none()
                    && !machine.target_family_interface
                    && crate::lowering::operator::is_bare_bodyless_signature(machine) =>
            {
                CallableScopeOwner::Operator(lowerer.symbol_resolved_trees.operators.len())
            }
            Item::Proposition(_) => {
                CallableScopeOwner::Proposition(lowerer.symbol_resolved_trees.propositions.len())
            }
            Item::Measure(_) => {
                CallableScopeOwner::Measure(lowerer.symbol_resolved_trees.measures.len())
            }
            _ => CallableScopeOwner::Root,
        };
        lowerer.with_runtime_callable_scope(owner, |lowerer| {
            lowerer.with_authored_expression_exposure(item_expression_exposure(item), |lowerer| {
                lower_item_with_exposure(lowerer, syntax_trees, item)
            })
        })?;
        let input = match item {
            Item::Conformance(_) => Some(
                crate::resolution::operand_owners::DeclarationInput::Conformance(
                    conformance_ordinal,
                ),
            ),
            Item::Trait(_) => Some(crate::resolution::operand_owners::DeclarationInput::Trait(
                trait_ordinal,
            )),
            _ => None,
        };
        if let Some(input) = input {
            lowerer.declaration_inputs.push((declaration, input));
        }
        if matches!(item, Item::Data(_))
            && let Some(sources) = &mut lowerer.equation_sources
        {
            sources.data_declarations.push((declaration, data_ordinal));
        }
        inputs.finish(lowerer, symbols::SymbolHandle::invalid());
        Ok(())
    }

    fn lower_item_with_exposure(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        item: &Item,
    ) -> Result<(), Diagnostic> {
        match item {
            syntax::item::Item::TypeAlias(alias) => {
                let alias = crate::lowering::type_aliases::lower_type_alias_definition(
                    lowerer,
                    syntax_trees,
                    alias,
                )?;
                lowerer.symbol_resolved_trees.type_aliases.push(alias);
            }
            syntax::item::Item::Data(data_definition) => {
                let lowered = lower_data_definition(lowerer, syntax_trees, data_definition)?;
                crate::lowering::wire::derive_wire_schema(lowerer, &lowered);
                lowerer.symbol_resolved_trees.data_definitions.push(lowered);
            }
            Item::Domain(domain_definition) => {
                let domain_definition =
                    lower_domain_definition(lowerer, syntax_trees, domain_definition)?;
                lowerer
                    .symbol_resolved_trees
                    .domain_definitions
                    .push(domain_definition);
            }
            Item::Machine(machine)
                if machine.target.is_none()
                    && !machine.target_family_interface
                    && crate::lowering::operator::is_bare_bodyless_signature(machine) =>
            {
                let operator = lower_bare_bodyless_signature(lowerer, syntax_trees, machine)?;
                lowerer.symbol_resolved_trees.operators.push(operator);
            }
            Item::Machine(machine) => {
                lower_machine_into(lowerer, syntax_trees, machine)?;
            }
            Item::Trait(trait_definition) => {
                let trait_definition =
                    lower_trait_definition(lowerer, syntax_trees, trait_definition)?;
                lowerer.symbol_resolved_trees.traits.push(trait_definition);
            }
            Item::Conformance(conformance) => {
                let arguments = lower_child_type_references(
                    lowerer,
                    syntax_trees,
                    conformance.trait_arguments,
                )?;
                let type_parameters =
                    lower_type_parameters(lowerer, syntax_trees, conformance.type_parameters)?;
                let conformance_name = conformance
                    .alias
                    .as_ref()
                    .unwrap_or(&conformance.trait_name)
                    .as_str(syntax_trees);
                let mut rows = Vec::new();
                for member in syntax_trees.items.conformance_members(conformance.members) {
                    match member {
                        syntax::item::ConformanceMember::Machine(machine) => {
                            let edge_trait = syntax_items(syntax_trees)
                                .satisfies_clauses(machine.satisfies)
                                .iter()
                                .find(|clause| {
                                    clause.requirement.as_ref().is_some_and(|name| {
                                        name.as_str(syntax_trees)
                                            == machine.name.as_str(syntax_trees)
                                    })
                                })
                                .map(|clause| clause.trait_name.clone());
                            rows.push(lower_closed_machine_row(
                                lowerer,
                                syntax_trees,
                                conformance_name,
                                edge_trait.as_ref(),
                                None,
                                machine,
                                ConformanceRowSource::Inline,
                            )?);
                        }
                        syntax::item::ConformanceMember::Synthesized(machine) => {
                            rows.push(lower_closed_machine_row(
                                lowerer,
                                syntax_trees,
                                conformance_name,
                                None,
                                None,
                                machine,
                                ConformanceRowSource::Synthesized,
                            )?);
                        }
                        syntax::item::ConformanceMember::TraitDefault {
                            declaring_trait,
                            requirement_ordinal,
                            machine,
                        } => {
                            rows.push(lower_closed_machine_row(
                                lowerer,
                                syntax_trees,
                                conformance_name,
                                Some(declaring_trait),
                                Some(*requirement_ordinal),
                                machine,
                                ConformanceRowSource::TraitDefault,
                            )?);
                        }
                        /*
                            A selection row declares no machine. The slot keeps
                            the authored path and its source span; closed
                            conformance normalization resolves that path to one
                            exact existing machine from where it was written and
                            records it as the realization, so every later stage
                            calls and checks the selected declaration itself.
                        */
                        syntax::item::ConformanceMember::Selection {
                            declaring_trait,
                            requirement,
                            target,
                        } => {
                            let target_members =
                                syntax_items(syntax_trees).identifier_path_members(*target);
                            let authored_realization_source_span = target_members
                                .first()
                                .zip(target_members.last())
                                .and_then(|(first, last)| {
                                    (first.source_span().source_id == last.source_span().source_id)
                                        .then(|| {
                                            source::SourceSpan::new(
                                                first.source_span().source_id,
                                                source::Span::new(
                                                    first.source_span().span.start,
                                                    last.source_span().span.end,
                                                ),
                                            )
                                        })
                                });
                            rows.push(symbol_resolved_trees::trait_definition::ConformanceRow {
                                declaring_trait: symbols::SymbolHandle::invalid(),
                                declaring_trait_name: lower_name(syntax_trees, declaring_trait),
                                requirement: symbols::SymbolHandle::invalid(),
                                requirement_name: lower_name(syntax_trees, requirement),
                                provisional_requirement_ordinal: None,
                                realization_machine: symbols::SymbolHandle::invalid(),
                                realization_state: symbols::SymbolHandle::invalid(),
                                realization_name: DiagnosticName::generated(
                                    target_members.joined_path_in(syntax_trees),
                                ),
                                authored_realization_source_span,
                                provisional_realization_ordinal: None,
                                source: ConformanceRowSource::Reference,
                            });
                        }
                    }
                }
                lowerer.symbol_resolved_trees.conformances.push(
                    symbol_resolved_trees::trait_definition::Conformance {
                        symbol: symbols::SymbolHandle::invalid(),
                        is_public: conformance.is_public,
                        lifetime_parameters: lower_names(
                            syntax_trees,
                            &conformance.lifetime_parameters,
                        ),
                        type_parameters,
                        trait_name: lower_name(syntax_trees, &conformance.trait_name),
                        trait_symbol: symbols::SymbolHandle::invalid(),
                        trait_lifetime_arguments: lower_names(
                            syntax_trees,
                            &conformance.trait_lifetime_arguments,
                        ),
                        arguments,
                        alias: conformance
                            .alias
                            .as_ref()
                            .map(|name| lower_name(syntax_trees, name)),
                        rows,
                    },
                );
            }
            Item::Measure(measure) => {
                let measure = lower_measure_definition(lowerer, syntax_trees, measure)?;
                lowerer.symbol_resolved_trees.measures.push(measure);
            }
            Item::Operator(operator) => {
                let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
                lowerer.symbol_resolved_trees.operators.push(operator);
            }
            Item::Proposition(proposition) => {
                let proposition = lower_proposition_definition(lowerer, syntax_trees, proposition)?;
                lowerer.symbol_resolved_trees.propositions.push(proposition);
            }
            Item::Const(definition) => {
                let pending = lowerer.const_resolution_mode != ConstResolutionMode::Complete
                    && crate::constant::requires_const_initializer_evaluation(
                        syntax_trees,
                        definition,
                    )
                    && !crate::constant::pending_const_initializer_leaves(
                        syntax_trees,
                        definition,
                        lowerer.constant_selection.as_ref().ok_or_else(|| {
                            Diagnostic::error(
                                "initializer preparation lost its declaration selector",
                            )
                            .with_source_span(definition.name.source_span())
                        })?,
                    )
                    .map_err(|reason| {
                        Diagnostic::error(reason).with_source_span(definition.name.source_span())
                    })?
                    .is_empty();
                if lowerer.const_resolution_mode == ConstResolutionMode::Complete {
                    crate::constant::validate_const_definition(
                        syntax_trees,
                        definition,
                        lowerer.constant_selection.as_ref(),
                    )?;
                }
                let initializer =
                    crate::constant::retain_const_initializer(lowerer, syntax_trees, definition)?;
                let evaluated_value = if pending || definition.pending_target_observation {
                    None
                } else if definition.is_public
                    && lowerer.const_resolution_mode == ConstResolutionMode::Complete
                {
                    Some(
                        crate::constant::evaluated_declaration_value(
                            syntax_trees,
                            definition,
                            lowerer.constant_selection.as_ref(),
                        )
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                                "public const `{}` has no canonical declaration identity: {reason}",
                                crate::constant::semantic_const_name(syntax_trees, definition),
                            ))
                            .with_source_span(definition.name.source_span())
                        })?,
                    )
                } else {
                    crate::constant::evaluated_declaration_value(
                        syntax_trees,
                        definition,
                        lowerer.constant_selection.as_ref(),
                    )
                    .ok()
                };
                let declared_type =
                    lower_type_reference_handle(lowerer, syntax_trees, definition.type_reference)?;
                let ordinal = lowerer.symbol_resolved_trees.roots.const_declarations.len();
                let declaration = lowerer.symbol_resolved_trees.roots.const_declarations.push(
                    symbol_resolved_trees::constant::ConstDeclaration {
                        symbol: symbols::SymbolHandle::invalid(),
                        is_public: definition.is_public,
                        declared_type,
                        initializer,
                        authored_initializer: lowerer
                            .pending_const_initializers
                            .last()
                            .filter(|_| definition.normalization.is_some())
                            .map_or(initializer, |record| record.authored),
                        initializer_source_span: syntax_trees
                            .expressions
                            .source_span(definition.value),
                        pending_target_observation: definition.pending_target_observation,
                        evaluated_value,
                    },
                );
                if let Some(sources) = &mut lowerer.equation_sources {
                    sources.declared_types.push((
                        crate::preparation::machine_equations::DeclaredTypeOrigin::Constant(
                            ordinal,
                        ),
                        definition.type_reference,
                    ));
                }
                lowerer.pending_const_declarations.push(
                    crate::resolution::lowerer::PendingConstDeclaration {
                        declaration,
                        scope: definition.scope.clone(),
                        semantic_name: crate::constant::semantic_const_name(
                            syntax_trees,
                            definition,
                        ),
                        source_span: definition.name.source_span(),
                        is_public: definition.is_public,
                    },
                );
            }
            Item::Module(module) => {
                lowerer
                    .namespace_declarations
                    .record_module(syntax_trees, module.path);
            }
            Item::Use(import) => {
                lowerer
                    .namespace_declarations
                    .record_import(syntax_trees, import.path);
            }
            Item::Capability(_) | Item::Package(_) => {}
        }

        Ok(())
    }

    fn item_expression_exposure(item: &Item) -> AuthoredDeclarationSelectionExposure {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as Exposure;

        match item {
            syntax::item::Item::Data(definition) if definition.is_public => {
                Exposure::PublicInterface
            }
            syntax::item::Item::TypeAlias(definition) if definition.is_public => {
                Exposure::PublicInterface
            }
            syntax::item::Item::Conformance(definition) if definition.is_public => {
                Exposure::PublicInterface
            }
            syntax::item::Item::Domain(definition) if definition.is_public => {
                Exposure::PublicInterface
            }
            syntax::item::Item::Proposition(definition) if definition.is_public => {
                Exposure::PublicInterface
            }
            syntax::item::Item::Machine(machine) => {
                crate::lowering::machine::machine_interface_exposure(machine)
            }
            syntax::item::Item::Operator(operator) if operator.is_public => {
                Exposure::PublicInterface
            }
            syntax::item::Item::Trait(definition) if definition.is_public => {
                Exposure::PublicInterface
            }
            _ => Exposure::PrivateImplementation,
        }
    }

    fn lower_closed_machine_row(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        conformance_name: &str,
        declaring_trait: Option<&syntax::identifier::Identifier>,
        requirement_ordinal: Option<usize>,
        machine: &syntax::item::Machine,
        source: ConformanceRowSource,
    ) -> Result<symbol_resolved_trees::trait_definition::ConformanceRow, Diagnostic> {
        let requirement_name = machine.name.clone();
        let realization_name = declaring_trait.map_or_else(
            || {
                format!(
                    "{conformance_name}::{}",
                    requirement_name.as_str(syntax_trees)
                )
            },
            |declaring_trait| {
                format!(
                    "{conformance_name}::{}::{}",
                    declaring_trait.as_str(syntax_trees),
                    requirement_name.as_str(syntax_trees),
                )
            },
        );
        let realization_ordinal = lowerer.symbol_resolved_trees.machines.len();
        let mut realization = machine.clone();
        realization.name = syntax::identifier::Identifier::new(
            realization_name.clone(),
            machine.name.source_span(),
        );
        realization.attached_data = None;
        realization.satisfies = if source == ConformanceRowSource::Inline {
            machine.satisfies
        } else {
            arena::HandleSpan::empty()
        };
        lower_machine_into(lowerer, syntax_trees, &realization)?;
        Ok(symbol_resolved_trees::trait_definition::ConformanceRow {
            declaring_trait: symbols::SymbolHandle::invalid(),
            declaring_trait_name: declaring_trait
                .map(|name| lower_name(syntax_trees, name))
                .unwrap_or_default(),
            requirement: symbols::SymbolHandle::invalid(),
            requirement_name: lower_name(syntax_trees, &requirement_name),
            provisional_requirement_ordinal: requirement_ordinal,
            realization_machine: symbols::SymbolHandle::invalid(),
            realization_state: symbols::SymbolHandle::invalid(),
            realization_name: DiagnosticName::generated(realization_name),
            authored_realization_source_span: None,
            provisional_realization_ordinal: Some(realization_ordinal),
            source,
        })
    }
}
#[path = "lowering_machine.rs"]
pub(crate) mod machine;
pub(crate) mod mathematical {
    /*
        Mathematical `let`/`boundary let` declarations: binders, the ordinary
        telescope, the dependent result type, and the transparent or
        named-assumption body.

        These declarations live on `SyntaxTreeRoots::mathematical_definitions`,
        outside the `Item` grammar, so they lower through their own entry point
        after `lower_items`. The binder kinds stay the authored `Type`/`Const`/
        `Value` carriers — a `u: core::Level` binder reads as a universe level only
        once its carrier resolves, which is the typed-tree elaboration leg's
        classification to make, not this stage's.
    */

    use crate::lowering::data::lower_type_parameters;
    use crate::lowering::expression::expression_table;
    use crate::lowering::expression::lower_expression_into_table;
    use crate::lowering::name::lower_name;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_expressions;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::mathematical::{
        MathematicalBody, MathematicalDefinition, MathematicalParameter, MathematicalType,
        MathematicalTypeHandle,
    };
    use symbols::SymbolHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    /*
        Translate every parsed mathematical declaration into the resolved carrier,
        in authored order, after the ordinary root items. Symbols stay invalid
        here; `symbols::assign` allocates them like every other declaration kind.
    */
    pub(crate) fn lower_mathematical_definitions(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        for handle in syntax.root_mathematical_definition_handles() {
            let inputs = crate::resolution::type_inputs::TypeInputScope::new(lowerer);
            let owner = crate::runtime_callable_scopes::CallableScopeOwner::Mathematical(
                lowerer.symbol_resolved_trees.mathematical_definitions.len(),
            );
            let definition = lowerer
                .with_runtime_callable_scope(owner, |lowerer| {
                    lower_mathematical_definition(
                        lowerer,
                        syntax,
                        syntax.root_mathematical_definition(*handle),
                    )
                })
                .map_err(|diagnostic| vec![diagnostic])?;
            lowerer
                .symbol_resolved_trees
                .roots
                .mathematical_definitions
                .push(definition);
            inputs.finish(lowerer, SymbolHandle::invalid());
        }
        Ok(())
    }

    fn lower_mathematical_definition(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        definition: &syntax::item::MathematicalDefinition,
    ) -> Result<MathematicalDefinition, Diagnostic> {
        let binders = lower_type_parameters(lowerer, syntax, definition.type_parameters)?;

        let mut parameters = Vec::new();
        for handle in syntax.items.mathematical_parameters(definition.parameters) {
            let parameter = syntax.items.mathematical_parameter(*handle);
            parameters.push(MathematicalParameter {
                symbol: SymbolHandle::invalid(),
                name: lower_name(syntax, &parameter.name),
                relevance: parameter.relevance,
                ty: lower_mathematical_type(lowerer, syntax, parameter.ty)?,
            });
        }
        let parameters = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .mathematical_parameters
            .insert_many(parameters);

        let result = lower_mathematical_type(lowerer, syntax, definition.result)?;

        let body = match definition.body {
            syntax::item::MathematicalDefinitionBody::Definition(term) => {
                MathematicalBody::Definition(lower_expression_into_table(lowerer, syntax, term)?)
            }
            syntax::item::MathematicalDefinitionBody::Assumption => MathematicalBody::Assumption,
        };

        Ok(MathematicalDefinition {
            symbol: SymbolHandle::invalid(),
            name: lower_name(syntax, &definition.name),
            is_public: definition.is_public,
            binders,
            parameters,
            result,
            body,
        })
    }

    /*
        Lower the declaration-local mathematical type grammar. Ordinary references
        reuse the shared type lowering; arrows and value-argument applications are
        the proof-surface-only forms.
    */
    fn lower_mathematical_type(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        handle: syntax::item::MathematicalTypeHandle,
    ) -> Result<MathematicalTypeHandle, Diagnostic> {
        let node = match syntax.items.mathematical_type(handle) {
            syntax::item::MathematicalTypeNode::Ordinary(type_reference) => {
                MathematicalType::Ordinary(lower_type_reference_handle(
                    lowerer,
                    syntax,
                    *type_reference,
                )?)
            }
            syntax::item::MathematicalTypeNode::Arrow {
                binder,
                domain,
                codomain,
            } => MathematicalType::Arrow {
                binder: binder.as_ref().map(|value| lower_name(syntax, value)),
                domain: lower_mathematical_type(lowerer, syntax, *domain)?,
                codomain: lower_mathematical_type(lowerer, syntax, *codomain)?,
            },
            syntax::item::MathematicalTypeNode::Application { callee, arguments } => {
                let arguments = syntax_expressions(syntax)
                    .expression_handles(*arguments)
                    .iter()
                    .map(|argument| lower_expression_into_table(lowerer, syntax, *argument))
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                MathematicalType::Application {
                    callee: lower_mathematical_type(lowerer, syntax, *callee)?,
                    arguments: expression_table(lowerer).insert_expression_handles(arguments),
                }
            }
        };
        Ok(lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .mathematical_types
            .insert(node))
    }
}
pub(crate) mod measure {
    /* Measure definitions and their optional parameter. */

    use crate::lowering::expression::expression_table;
    use crate::lowering::expression::lower_expression_into_table;
    use crate::lowering::name::lower_operator_path_name;
    use crate::lowering::state::lower_state_parameter;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::measure::MeasureDefinition;
    use symbols::SymbolHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_measure_definition(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        measure: &syntax::item::MeasureDefinition,
    ) -> Result<MeasureDefinition, Diagnostic> {
        let name = lower_operator_path_name(lowerer, syntax_trees, measure.name);

        let parameter = measure
            .parameter
            .is_valid()
            .then(|| lower_state_parameter(lowerer, syntax_trees, measure.parameter))
            .transpose()?;

        let return_type = measure
            .return_type
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, measure.return_type))
            .transpose()?;

        let mut lowered_components = Vec::new();
        for expression in syntax_trees.expressions.expression_handles(measure.body) {
            let lowered = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
            lowered_components.push(lowered);
        }
        let body = expression_table(lowerer).insert_expression_handles(lowered_components);

        Ok(MeasureDefinition {
            symbol: SymbolHandle::invalid(),
            name,
            parameter,
            return_type,
            lexicographic: measure.lexicographic,
            body,
        })
    }
}
pub(crate) mod name {
    /* Identifier to diagnostic name. */

    use symbol_resolved_trees::name::DiagnosticName;
    use syntax_trees::{self as syntax, SyntaxTrees};

    use crate::resolution::lowerer::Lowerer;
    use arena::HandleSpan;
    use syntax::identifier::Identifier;

    pub(crate) fn lower_name(
        syntax_trees: &syntax_trees::SyntaxTrees,
        name: &Identifier,
    ) -> DiagnosticName {
        DiagnosticName::from_str(name.as_str(syntax_trees), name.source_span())
    }

    /* Lower a run of source identifiers into fresh diagnostic names. */
    pub(crate) fn lower_names(
        syntax_trees: &SyntaxTrees,
        names: &[Identifier],
    ) -> Vec<DiagnosticName> {
        names
            .iter()
            .map(|name| lower_name(syntax_trees, name))
            .collect()
    }

    pub(crate) fn lower_operator_path_name(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        name: HandleSpan<Identifier>,
    ) -> HandleSpan<DiagnosticName> {
        lower_name_members(
            syntax_trees,
            name,
            &mut lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .operator_path_members,
        )
    }

    /* Lower an identifier path into a fresh span of `arena`. */
    pub(crate) fn lower_name_members(
        syntax_trees: &SyntaxTrees,
        name: HandleSpan<Identifier>,
        arena: &mut arena::Arena<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        let mut span = HandleSpan::empty();
        for member in syntax_trees.items.identifier_path_members(name) {
            arena.append_to_span(&mut span, lower_name(syntax_trees, member));
        }
        span
    }
}
pub(crate) mod operator {
    /*
        Operator definitions. Domain homes are selected later by
        `selection::domain_operator_homes`.
    */

    use crate::lowering::data::lower_type_parameters;
    use crate::lowering::name::lower_names;
    use crate::lowering::name::lower_operator_path_name;
    use crate::lowering::state::{lower_signature_contracts, lower_state_parameters};
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_core::atomic::ATOMIC_FENCE_NAMESPACE;
    use numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE;
    use numerics::float_projection::FloatProjectionOperation;
    use std::path::Path;
    use syntax::types::TypeReferenceNode;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_operator_definition(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        operator: &syntax::item::OperatorDefinition,
    ) -> Result<symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
        Ok(symbol_resolved_trees::operator::OperatorDefinition {
            is_public: operator.is_public,
            is_boundary: operator.is_boundary,
            symbol: Default::default(),
            name: lower_operator_path_name(lowerer, syntax_trees, operator.name),
            lifetime_parameters: lower_names(syntax_trees, &operator.lifetime_parameters),
            type_parameters: lower_type_parameters(
                lowerer,
                syntax_trees,
                operator.type_parameters,
            )?,
            parameters: lower_state_parameters(lowerer, syntax_trees, operator.parameters)?,
            return_type: operator
                .return_type
                .is_valid()
                .then(|| lower_type_reference_handle(lowerer, syntax_trees, operator.return_type))
                .transpose()?,
            contracts: lower_signature_contracts(lowerer, syntax_trees, operator.contracts)?,
            spelling: operator.spelling,
            token_count: operator.token_count,
        })
    }

    /*
        Whether a `machine` item is a top-level token-bearing boundary signature
        (`boundary machine + Owner::name(...);`): bodyless, boundary and tokened.
    */
    pub(crate) fn is_token_bearing_boundary_signature(machine: &syntax::item::Machine) -> bool {
        machine.boundary && machine.bodyless && machine.spelling.is_some()
    }

    /*
        Whether a `machine` item is a bare bodyless signature: no body, not
        boundary, no `satisfies`/`via`, with or without a fixed
        token. The item parser admits the shape;
        only an exact compiler-catalog primitive may own it, so a token-bearing
        declaration (`machine + Owner::name(...);`) rejects here with the same
        missing-body guidance rather than lowering as an unsupplied checked body.
    */
    pub(crate) fn is_bare_bodyless_signature(machine: &syntax::item::Machine) -> bool {
        machine.bodyless && !machine.boundary && machine.satisfies.is_empty()
    }

    /*
        The compiler catalog's exact identity for a bare bodyless signature, or
        the rejection every other one earns.

        The executable-supply contract's catalog row supplies "exact compiler-owned
    bodyless machine" declarations from the closed catalog, and "merely naming
    a declaration `Float::meaning32` grants no primitive implementation". The
        key is therefore exact declaration custody, never the leaf spelling: the
        declaring source must be the sealed toolchain float-operations file, and
        the path must be one a catalog publishes -- the projection rows
        (`Float::meaning32`, `Float::meaning64`) or a semantic-definition row
        (`FloatSemantics::<name>` with its complete normalized signature, so the
        `from_integer` carriers select distinct rows). The admitted declaration
        lowers to the same resolved operator declaration the retiring `operator`
        introducer produced, so the sealed routes (hermetic toolchain symbol
        identity, exact signature shape, catalog contract identity) validate it
        unchanged; a same-spelled declaration in any other source is refused here,
        and a sealed declaration whose signature matches no row rejects instead of
        lowering as an ordinary declaration.
    */
    pub(crate) fn lower_bare_bodyless_signature(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        machine: &syntax::item::Machine,
    ) -> Result<symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
        let name = machine.name.as_str(syntax_trees);
        let (namespace, leaf) = name.rsplit_once("::").unwrap_or(("", name));
        let family = CatalogFamily::naming(namespace, leaf);
        let sealed_source =
            family.is_some_and(|family| {
                lowerer
                    .sources
                    .as_ref()
                    .and_then(|sources| sources.get(machine.name.source_span().source_id))
                    .is_some_and(|file| {
                        file.origin == source::SourceOrigin::Toolchain
                            && file.path.strip_prefix(&file.package_root).ok().is_some_and(
                                |relative| relative == Path::new(family.sealed_source()),
                            )
                    })
            });
        let Some(family) = family.filter(|_| sealed_source) else {
            let token = machine
                .spelling
                .map(|spelling| format!("{} ", spelling.symbol()))
                .unwrap_or_default();
            let guidance = if family.is_some() {
                format!(
                    "`{name}` names a compiler primitive, but only the sealed toolchain declaration \
                 supplies it; merely naming a declaration `{name}` grants no primitive \
                 implementation or proof authority"
                )
            } else {
                format!(
                    "`{name}` has no body and is neither a boundary signature nor a compiler-catalog \
                 primitive; a nonboundary direct machine must own a checked body \
                 (`machine {token}{name}(...) {{ ... }}`), a boundary contract is spelled \
                 `boundary machine {token}...;`, and an external leaf `satisfies Requirement \
                 via <Binding>;`"
                )
            };
            return Err(Diagnostic::error(guidance).with_source_span(machine.name.source_span()));
        };
        let Some(entry) = syntax_trees.items.state_handles(machine.states).first() else {
            return Err(Diagnostic::error(format!(
                "`{name}` is a compiler-catalog signature without an entry signature"
            ))
            .with_source_span(machine.name.source_span()));
        };
        let entry = syntax_trees.items.state(*entry);
        match family {
            CatalogFamily::FloatProjection => {}
            CatalogFamily::AtomicFence => {
                require_exact_atomic_fence_row(syntax_trees, machine, entry)?;
            }
            CatalogFamily::FloatSemantics => {
                require_exact_semantic_row(syntax_trees, machine, entry, namespace, leaf)?;
            }
            CatalogFamily::RankingView(declaration) => {
                require_exact_ranking_view_row(syntax_trees, machine, entry, declaration)?;
            }
        }
        let name_span = machine.name.source_span();
        let mut path = HandleSpan::empty();
        for member in name.split("::") {
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .operator_path_members
                .append_to_span(
                    &mut path,
                    crate::lowering::name::lower_name(
                        syntax_trees,
                        &syntax::identifier::Identifier::new(member, name_span),
                    ),
                );
        }
        Ok(symbol_resolved_trees::operator::OperatorDefinition {
            is_public: machine.is_public,
            is_boundary: false,
            symbol: Default::default(),
            name: path,
            lifetime_parameters: lower_names(syntax_trees, &machine.lifetime_parameters),
            type_parameters: lower_type_parameters(lowerer, syntax_trees, machine.type_parameters)?,
            parameters: lower_state_parameters(lowerer, syntax_trees, entry.parameters)?,
            return_type: entry
                .return_type
                .is_valid()
                .then(|| lower_type_reference_handle(lowerer, syntax_trees, entry.return_type))
                .transpose()?,
            contracts: lower_signature_contracts(lowerer, syntax_trees, machine.contracts)?,
            spelling: machine.spelling,
            token_count: 0,
        })
    }

    /*
        The closed catalogs a bare bodyless signature may name, each with the one
        sealed toolchain source (relative to the core package root) whose
        declaration it supplies. A family is selected by exact path; the leaf
        spelling alone selects nothing.
    */
    #[derive(Clone, Copy)]
    enum CatalogFamily {
        /* Place-free portable fence (`atomic.omg`). */
        AtomicFence,
        /* `Float::meaning32` / `Float::meaning64` (`float_operations.omg`). */
        FloatProjection,
        /*
            `FloatSemantics::<name>` rows (`float_operations.omg`), keyed on the
            complete signature.
        */
        FloatSemantics,
        /*
            A canonical ranking view with a declaration row (`Nat::Descending` in
            `nat.omg`).
        */
        RankingView(language_semantics::RankingViewDeclaration),
    }

    impl CatalogFamily {
        fn naming(namespace: &str, leaf: &str) -> Option<Self> {
            if namespace == ATOMIC_FENCE_NAMESPACE
                && leaf == language_core::atomic::ATOMIC_FENCE_NAME
            {
                return Some(Self::AtomicFence);
            }
            if FloatProjectionOperation::from_source_identity(namespace, leaf).is_some() {
                return Some(Self::FloatProjection);
            }
            if numerics::float_semantics_catalog::FloatSemanticOperation::names_a_row(
                namespace, leaf,
            ) {
                return Some(Self::FloatSemantics);
            }
            language_semantics::RankingViewId::from_catalog_declaration(namespace, leaf)
                .map(Self::RankingView)
        }

        fn sealed_source(self) -> &'static str {
            match self {
                Self::AtomicFence => language_core::atomic::ATOMIC_CORE_SOURCE,
                Self::FloatProjection | Self::FloatSemantics => FLOAT_PROJECTION_CORE_SOURCE,
                Self::RankingView(declaration) => declaration.source,
            }
        }
    }

    /*
        A sealed ranking-view signature must match its declaration row exactly:
        one plain parameter of the row's carrier and the row's result, no type or
        lifetime parameters; a drifted declaration rejects instead of lowering as
        an ordinary (unsupplied) declaration.
    */
    fn require_exact_ranking_view_row(
        syntax_trees: &SyntaxTrees,
        machine: &syntax::item::Machine,
        entry: &syntax::item::State,
        declaration: language_semantics::RankingViewDeclaration,
    ) -> Result<(), Diagnostic> {
        let name = machine.name.as_str(syntax_trees);
        let drift = |detail: String| {
            Diagnostic::error(format!(
                "`{name}` names the compiler ranking-view catalog, but {detail}; the sealed \
             declaration must be exactly `machine {}({}) -> {};`",
                declaration.path(),
                declaration.parameter,
                declaration.result
            ))
            .with_source_span(machine.name.source_span())
        };
        if !machine.type_parameters.is_empty() || !machine.lifetime_parameters.is_empty() {
            return Err(drift(
                "catalog rows declare no type or lifetime parameters".to_owned(),
            ));
        }
        let named_spelling = |handle: syntax::types::TypeReferenceHandle| match syntax_trees
            .tables
            .type_references
            .type_reference(handle)
        {
            TypeReferenceNode::Named(identifier) => Some(identifier.as_str(syntax_trees)),
            _ => None,
        };
        let [parameter] = syntax_trees.items.state_parameters(entry.parameters) else {
            return Err(drift("the row ranks exactly one subject".to_owned()));
        };
        let parameter = syntax_trees.items.state_parameter(*parameter);
        if parameter.is_const || parameter.is_mutable {
            return Err(drift(format!(
                "parameter `{}` carries a binding mode the row does not declare",
                parameter.name.as_str(syntax_trees)
            )));
        }
        if named_spelling(parameter.type_reference) != Some(declaration.parameter) {
            return Err(drift(format!(
                "parameter `{}` is not the row's `{}` subject",
                parameter.name.as_str(syntax_trees),
                declaration.parameter
            )));
        }
        if !entry.return_type.is_valid()
            || named_spelling(entry.return_type) != Some(declaration.result)
        {
            return Err(drift(format!(
                "its result is not the row's `{}` rank",
                declaration.result
            )));
        }
        Ok(())
    }

    /*
        A sealed `FloatSemantics::<leaf>` signature must select one exact catalog
        row by its complete normalized signature; a drifted declaration rejects
        here rather than lowering as an ordinary (unsupplied) declaration.
    */
    fn require_exact_semantic_row(
        syntax_trees: &SyntaxTrees,
        machine: &syntax::item::Machine,
        entry: &syntax::item::State,
        namespace: &str,
        leaf: &str,
    ) -> Result<&'static numerics::float_semantics_catalog::FloatSemanticOperation, Diagnostic>
    {
        use numerics::float_semantics_catalog::{FloatSemanticOperation, FloatSemanticValueKind};
        let name = machine.name.as_str(syntax_trees);
        let drift = |detail: String| {
            Diagnostic::error(format!(
                "`{name}` names the compiler float-semantics catalog, but {detail}; the sealed \
             declaration must match one catalog row exactly"
            ))
            .with_source_span(machine.name.source_span())
        };
        if !machine.type_parameters.is_empty() || !machine.lifetime_parameters.is_empty() {
            return Err(drift(
                "catalog rows declare no type or lifetime parameters".to_owned(),
            ));
        }
        let kind_of =
            |handle: syntax::types::TypeReferenceHandle, position: &str| match syntax_trees
                .tables
                .type_references
                .type_reference(handle)
            {
                TypeReferenceNode::Named(identifier) => {
                    FloatSemanticValueKind::from_spelling(identifier.as_str(syntax_trees))
                        .ok_or_else(|| {
                            drift(format!(
                                "{position} type `{}` is not a catalog value kind",
                                identifier.as_str(syntax_trees)
                            ))
                        })
                }
                _ => Err(drift(format!(
                    "{position} type is not a named catalog value kind"
                ))),
            };
        let mut parameters = Vec::new();
        for handle in syntax_trees.items.state_parameters(entry.parameters) {
            let parameter = syntax_trees.items.state_parameter(*handle);
            if parameter.is_const || parameter.is_mutable {
                return Err(drift(format!(
                    "parameter `{}` carries a binding mode no catalog row declares",
                    parameter.name.as_str(syntax_trees)
                )));
            }
            parameters.push(kind_of(
                parameter.type_reference,
                &format!("parameter `{}`", parameter.name.as_str(syntax_trees)),
            )?);
        }
        if !entry.return_type.is_valid() {
            return Err(drift("it declares no result type".to_owned()));
        }
        let result = kind_of(entry.return_type, "result")?;
        FloatSemanticOperation::from_source_identity(namespace, leaf, &parameters, result)
            .ok_or_else(|| {
                drift(format!(
                    "its signature ({}) -> {} matches no catalog row",
                    parameters
                        .iter()
                        .map(|kind| kind.spelling())
                        .collect::<Vec<_>>()
                        .join(", "),
                    result.spelling()
                ))
            })
    }

    fn require_exact_atomic_fence_row(
        syntax_trees: &SyntaxTrees,
        machine: &syntax::item::Machine,
        entry: &syntax::item::State,
    ) -> Result<(), Diagnostic> {
        let valid_parameter = match syntax_trees.items.state_parameters(entry.parameters) {
            [parameter] => {
                let parameter = syntax_trees.items.state_parameter(*parameter);
                !parameter.is_const
                    && !parameter.is_mutable
                    && !parameter.relevance.is_erased()
                    && matches!(syntax_trees.tables.type_references.type_reference(parameter.type_reference),
                    TypeReferenceNode::Named(name) if name.as_str(syntax_trees) == "FenceOrdering")
            }
            _ => false,
        };
        if !machine.is_public
            || machine.spelling.is_some()
            || !machine.type_parameters.is_empty()
            || !machine.lifetime_parameters.is_empty()
            || !machine.contracts.is_empty()
            || entry.return_type.is_valid()
            || !valid_parameter
        {
            return Err(Diagnostic::error("the sealed atomic fence declaration must be exactly `public machine Atomic::fence(ordering: FenceOrdering);`").with_source_span(machine.name.source_span()));
        }
        Ok(())
    }
}
pub(crate) mod proposition {
    /*
        Proposition definitions: binders, parameters, and primitive, witness, or
        transparent bodies.
    */

    use crate::lowering::expression::lower_expression_into_table;
    use crate::lowering::state::lower_state_parameters;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::data::DataProperties;
    use symbol_resolved_trees::proposition::{
        PropositionBinder, PropositionBinderKind, PropositionBody, PropositionDefinition,
    };
    use symbols::SymbolHandle;
    use syntax::item::TypeParameter;
    use syntax::item::TypeParameterKind;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_proposition_definition(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        proposition: &syntax::item::PropositionDefinition,
    ) -> Result<PropositionDefinition, Diagnostic> {
        let binders =
            lower_proposition_binders(lowerer, syntax_trees, proposition.type_parameters)?;
        let parameters = lower_state_parameters(lowerer, syntax_trees, proposition.parameters)?;
        let body = match proposition.body {
            syntax::item::PropositionBody::Primitive => PropositionBody::Primitive,
            syntax::item::PropositionBody::Witness { evidence } => PropositionBody::Witness {
                evidence: lower_type_reference_handle(lowerer, syntax_trees, evidence)?,
            },
            syntax::item::PropositionBody::Transparent { proposition } => {
                PropositionBody::Transparent {
                    proposition: lower_expression_into_table(lowerer, syntax_trees, proposition)?,
                }
            }
        };

        Ok(PropositionDefinition {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, &proposition.name),
            is_public: proposition.is_public,
            binders,
            parameters,
            transparent_formula_source_span: proposition.transparent_formula_source_span,
            body,
        })
    }

    fn lower_proposition_binders(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        binders: HandleSpan<TypeParameter>,
    ) -> Result<HandleSpan<PropositionBinder>, Diagnostic> {
        let mut lowered = Vec::new();
        for binder in syntax_trees.items.type_parameters(binders) {
            let kind = match binder.kind {
                TypeParameterKind::Type => PropositionBinderKind::Type,
                TypeParameterKind::Const { type_reference } => PropositionBinderKind::Const {
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        type_reference,
                    )?,
                },
                TypeParameterKind::Value { .. } => {
                    return Err(Diagnostic::error(format!(
                        "proposition declaration binder `{}` cannot be a runtime-capable value binder; value binders are legal only on machine signature generics",
                        binder.name.as_str(syntax_trees)
                    )));
                }
                TypeParameterKind::Machine { .. } => PropositionBinderKind::Machine,
                TypeParameterKind::Implementation => {
                    return Err(Diagnostic::error(format!(
                        "proposition declaration binder `{}` cannot select an implementation; implementation slots belong to trait telescopes",
                        binder.name.as_str(syntax_trees)
                    )));
                }
                TypeParameterKind::Proposition { .. } => {
                    return Err(Diagnostic::error(format!(
                        "proposition declaration binder `{}` cannot itself be a generic proposition parameter; generic proposition signatures belong to trait abstraction surfaces",
                        binder.name.as_str(syntax_trees)
                    )));
                }
            };
            lowered.push(PropositionBinder {
                symbol: SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(syntax_trees, &binder.name),
                kind,
                bounds: DataProperties {
                    carry: binder.bounds.carry,
                    multiplicity: binder.bounds.multiplicity,
                },
            });
        }

        Ok(lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .proposition_binders
            .insert_many(lowered))
    }
}
pub(crate) mod state {
    /*
        Machine states: signatures, parameters, contracts, and bodies.

        Signature lowering covers invokes, service-reach names, and contracts,
        including outcome-specific contracts over a declared result sum whose
        symbols finalize after assignment. Bodies lower statement by statement
        into pending form. The guarded-arm rewrites in `statement` mint their
        continuation states here.
    */

    mod lexical_inputs {
        /*
            One-shot lexical preparation for type operands and selected call results.

            These are declaration construction records, not executable statements. They
            are never installed into a state's body, and the machine stays Deferred.
            Ordinary body lowering shares the local declaration constructor; symbol
            assignment shares its preceding-local scope and append-only allocation.
        */

        use crate::lowering::expression::lower_private_expression_into_table;
        use crate::lowering::type_reference::lower_type_reference_handle;
        use crate::preparation::generic_data::statement_call_inputs;
        use crate::preparation::generic_data::statement_type_inputs;
        use crate::resolution::lowerer::Lowerer;
        use arena::{Handle, HandleSpan};
        use diagnostics::Diagnostic;
        use symbol_resolved_trees::{
            state::State,
            statement::{LocalData, LocalDataStorage, Statement},
            types::TypeReference,
        };
        use symbols::SymbolHandle;
        use syntax_trees::{self as syntax, SyntaxTrees};

        #[derive(Clone, Copy)]
        pub(crate) enum LexicalInputDemand {
            TypeOperands,
            CallResults,
        }

        pub(crate) struct StateLexicalInputs {
            pub(crate) machine: SymbolHandle,
            pub(crate) state: Handle<State>,
            pub(crate) locals: Vec<Statement>,
            /* Each input sees only the authored locals preceding its statement. */
            pub(crate) types: Vec<(usize, TypeReference)>,
            pub(crate) calls: Vec<(usize, symbol_resolved_trees::expression::ExpressionHandle)>,
        }

        pub(crate) fn lower_local_declaration(
            lowerer: &mut Lowerer,
            syntax: &SyntaxTrees,
            local: &syntax::statement::TableLocalData,
        ) -> Result<LocalData, Diagnostic> {
            let type_reference = if local.type_reference.is_valid() {
                lower_type_reference_handle(lowerer, syntax, local.type_reference)?
            } else {
                TypeReference::Unit
            };
            /*
                A record pattern's roster keeps its authored spellings here.
                Each entry receives its field once the subject's type is known,
                in typed preparation, so no entry is resolved by this stage.
            */
            let roster = syntax
                .expressions
                .match_fields(local.pattern_fields)
                .iter()
                .map(|field| {
                    use symbol_resolved_trees::expression::{MatchFieldAction, TableMatchField};
                    TableMatchField {
                        component: field.component,
                        member: crate::lowering::name::lower_name(syntax, &field.member),
                        field_symbol: SymbolHandle::invalid(),
                        action: match &field.action {
                            syntax::expression::MatchFieldAction::Bind(binding) => {
                                MatchFieldAction::Bind(crate::lowering::name::lower_name(
                                    syntax, binding,
                                ))
                            }
                            syntax::expression::MatchFieldAction::Waive
                            | syntax::expression::MatchFieldAction::Equal(_) => {
                                MatchFieldAction::Waive
                            }
                        },
                    }
                })
                .collect::<Vec<_>>();
            let pattern_fields = lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .insert_match_fields(roster);
            Ok(LocalData {
                symbol: SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(syntax, &local.name),
                storage: LocalDataStorage {
                    role: local.role,
                    type_reference,
                    initial_value: symbol_resolved_trees::expression::ExpressionHandle::invalid(),
                    is_mutable: local.is_mutable,
                    type_is_inferred: !local.type_reference.is_valid(),
                    is_operand_capture: false,
                    relevance: local.relevance,
                    pattern_fields,
                },
            })
        }

        pub(crate) fn lower_state_lexical_inputs(
            lowerer: &mut Lowerer,
            syntax: &SyntaxTrees,
            machine: SymbolHandle,
            state: Handle<State>,
            statements: HandleSpan<syntax::statement::StatementHandle>,
            demand: LexicalInputDemand,
        ) -> Result<StateLexicalInputs, Diagnostic> {
            let declaration = lowerer.symbol_resolved_trees.machine_state(state).clone();
            super::with_state_scope(lowerer, &declaration, |lowerer| {
                let mut inputs = StateLexicalInputs {
                    machine,
                    state,
                    locals: Vec::new(),
                    types: Vec::new(),
                    calls: Vec::new(),
                };
                for &statement in syntax.items.statements(statements) {
                    let local = match syntax.statements.statement(statement) {
                        syntax::statement::StatementNode::LocalData(local) => Some(local),
                        _ => None,
                    };
                    let types = statement_type_inputs(syntax, statement)
                        .into_iter()
                        .filter(|reference| {
                            !local.is_some_and(|local| local.type_reference == *reference)
                        })
                        .collect::<Vec<_>>();
                    let calls = if matches!(demand, LexicalInputDemand::CallResults) {
                        statement_call_inputs(syntax, statement)
                    } else {
                        Vec::new()
                    };
                    if !types.is_empty() || !calls.is_empty() {
                        crate::lowering::expression::match_dispatch::with_transition_pattern_captures(
                    lowerer,
                    syntax,
                    statement,
                    |lowerer| {
                        for reference in types {
                            inputs.types.push((
                                inputs.locals.len(),
                                lower_type_reference_handle(lowerer, syntax, reference)?,
                            ));
                        }
                        for expression in calls {
                            inputs.calls.push((
                                inputs.locals.len(),
                                lower_private_expression_into_table(lowerer, syntax, expression)?,
                            ));
                        }
                        Ok(())
                    },
                )?;
                    }
                    if let Some(local) = local {
                        inputs
                            .locals
                            .push(Statement::LocalData(lower_local_declaration(
                                lowerer, syntax, local,
                            )?));
                    }
                }
                Ok(inputs)
            })
        }
    }
    pub(crate) use lexical_inputs::{
        LexicalInputDemand, StateLexicalInputs, lower_local_declaration, lower_state_lexical_inputs,
    };

    use crate::lowering::domain::lower_proof_facts;
    use crate::lowering::name::lower_names;
    use crate::lowering::statement::lower_statement_handle;
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_items;
    use arena::{Handle, HandleSpan};
    use diagnostics::Diagnostic;
    use std::mem::replace;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::{
        CrashCause, NativeCallbackParameter, SignatureContract, SignatureContractKind,
        StateParameter, StateSignature, StateSignatureStorage,
    };
    use symbol_resolved_trees::state::{State, StateStorage};
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::SymbolHandle;
    use syntax::identifier::Identifier;
    use syntax::item::CapabilityContract;
    use syntax::item::CapabilityContractKind;
    use syntax::item::NativeCallbackParameterNode;
    use syntax::item::StateParameterHandle;
    use syntax::item::TypeParameter;
    use syntax::operator_spelling::OperatorSpelling;
    use syntax::statement::StatementHandle;
    use syntax::statement::StatementNode;
    use syntax::types::TypeReferenceHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_state_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        state: &syntax::item::State,
        public_signature: bool,
    ) -> Result<State, Diagnostic> {
        lower_state_parts(
            lowerer,
            syntax_trees,
            &state.name,
            state.parameters,
            state.return_type,
            state.contracts,
            state.statements,
            public_signature,
        )
    }

    fn lower_state_parts(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        name: &Identifier,
        parameters: HandleSpan<StateParameterHandle>,
        return_type_handle: TypeReferenceHandle,
        contracts: HandleSpan<CapabilityContract>,
        statements: arena::HandleSpan<StatementHandle>,
        public_signature: bool,
    ) -> Result<State, Diagnostic> {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
        let exposure = if public_signature {
            AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            AuthoredDeclarationSelectionExposure::PrivateImplementation
        };
        let (parameters, return_type) =
            lowerer.with_authored_expression_exposure(exposure, |lowerer| {
                let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
                let return_type = return_type_handle
                    .is_valid()
                    .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
                    .transpose()?;
                Ok::<_, Diagnostic>((parameters, return_type))
            })?;
        let mut state = State {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, name),
            storage: StateStorage {
                parameters,
                return_type,
                ..StateStorage::default()
            },
        };
        state.contracts = with_state_scope(lowerer, &state, |lowerer| {
            lower_signature_contracts(lowerer, syntax_trees, contracts)
        })?;
        if !lowerer.defer_machine_bodies {
            state.statements = lower_state_body(lowerer, syntax_trees, &state, statements)?;
        }
        Ok(state)
    }

    /*
        Complete statements under their already indexed signature. The caller owns
        the pending syntax and only publishes the machine as Complete afterwards.
    */
    pub(crate) fn lower_state_body(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        declaration: &State,
        statements: HandleSpan<StatementHandle>,
    ) -> Result<HandleSpan<Statement>, Diagnostic> {
        with_state_scope(lowerer, declaration, |lowerer| {
            lower_state_statements(lowerer, syntax, statements)
        })
    }

    /*
        Signatures and statements read the same declared parameter scope.
        Restore the previous scope on both success and failure; completing one state
        must not change how a later state's body or generated continuation is lowered.
    */
    fn with_state_scope<T>(
        lowerer: &mut Lowerer,
        state: &State,
        operation: impl FnOnce(&mut Lowerer) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let previous_name = lowerer
            .current_state_name
            .replace(state.name.as_str().to_owned());
        let reference_parameters = if lowerer.current_machine_is_boundary {
            reference_struct_parameter_names(lowerer, &state.parameters)
        } else {
            Vec::new()
        };
        let parameter_names = lowerer
            .symbol_resolved_trees
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| parameter.name.as_str().to_string())
            .collect();
        let parameters = lowerer
            .symbol_resolved_trees
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| {
                (
                    parameter.name.as_str().to_string(),
                    parameter.type_reference.clone(),
                    parameter.is_mutable,
                )
            })
            .collect();
        let previous_references = replace(
            &mut lowerer.reference_struct_parameters,
            reference_parameters,
        );
        let previous_names = replace(&mut lowerer.current_state_parameter_names, parameter_names);
        let previous_parameters = replace(&mut lowerer.current_state_parameters, parameters);
        let previous_return = replace(
            &mut lowerer.current_state_return_type,
            state.return_type.clone(),
        );
        let result = operation(lowerer);
        lowerer.current_state_name = previous_name;
        lowerer.reference_struct_parameters = previous_references;
        lowerer.current_state_parameter_names = previous_names;
        lowerer.current_state_parameters = previous_parameters;
        lowerer.current_state_return_type = previous_return;
        result
    }

    pub(crate) fn lower_state_signature_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        signature: &syntax::item::StateSignature,
    ) -> Result<LoweredStateSignature, Diagnostic> {
        lower_state_signature_parts(
            lowerer,
            syntax_trees,
            StateSignatureParts {
                name: &signature.name,
                spelling: signature.spelling,
                lifetime_parameters: &signature.lifetime_parameters,
                type_parameters: signature.type_parameters,
                conformance_bounds: &signature.conformance_bounds,
                parameters: signature.parameters,
                native_callback_parameters: &signature.native_callback_parameters,
                return_type_handle: signature.return_type,
                is_default: signature.is_default,
                service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
                service_reach_keyword_source_spans: &signature.service_reach_keyword_source_spans,
                service_reaches: signature.service_reaches,
                invokes: signature.invokes,
                suspends_keyword_source_spans: &signature.suspends_keyword_source_spans,
                blocks_keyword_source_spans: &signature.blocks_keyword_source_spans,
                suspends: signature.suspends,
                blocks: signature.blocks,
                contracts: signature.contracts,
                terminates_guarantee: signature.terminates_guarantee,
                declares_no_crash_route: signature.declares_no_crash_route,
            },
        )
    }

    /*
        The authored parts of one state or structural machine-parameter signature,
        borrowed from its syntax node.
    */
    #[derive(Debug)]
    pub(crate) struct StateSignatureParts<'a> {
        pub(crate) name: &'a Identifier,
        pub(crate) spelling: Option<OperatorSpelling>,
        pub(crate) lifetime_parameters: &'a [Identifier],
        pub(crate) type_parameters: HandleSpan<TypeParameter>,
        pub(crate) conformance_bounds: &'a [syntax::item::GenericConformanceBound],
        pub(crate) parameters: HandleSpan<StateParameterHandle>,
        pub(crate) native_callback_parameters: &'a [NativeCallbackParameterNode],
        pub(crate) return_type_handle: TypeReferenceHandle,
        pub(crate) is_default: bool,
        pub(crate) service_reach_is_installation_bound: bool,
        pub(crate) service_reach_keyword_source_spans: &'a [source::SourceSpan],
        pub(crate) service_reaches: HandleSpan<Identifier>,
        pub(crate) invokes: HandleSpan<Identifier>,
        pub(crate) suspends_keyword_source_spans: &'a [source::SourceSpan],
        pub(crate) blocks_keyword_source_spans: &'a [source::SourceSpan],
        pub(crate) suspends: bool,
        pub(crate) blocks: bool,
        pub(crate) contracts: HandleSpan<CapabilityContract>,
        pub(crate) terminates_guarantee: bool,
        pub(crate) declares_no_crash_route: bool,
    }

    pub(crate) fn lower_state_signature_parts(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        inputs: StateSignatureParts<'_>,
    ) -> Result<LoweredStateSignature, Diagnostic> {
        let scope = lowerer.runtime_callable_scopes.begin(None);
        let result = lower_state_signature_contents(lowerer, syntax_trees, inputs, scope);
        lowerer.runtime_callable_scopes.end(scope);
        result
    }

    fn lower_state_signature_contents(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        inputs: StateSignatureParts<'_>,
        callable_scope: crate::runtime_callable_scopes::CallableScopeHandle,
    ) -> Result<LoweredStateSignature, Diagnostic> {
        let StateSignatureParts {
            name,
            spelling,
            lifetime_parameters,
            type_parameters,
            conformance_bounds,
            parameters,
            native_callback_parameters,
            return_type_handle,
            is_default,
            service_reach_is_installation_bound,
            service_reach_keyword_source_spans,
            service_reaches,
            invokes,
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts,
            terminates_guarantee,
            declares_no_crash_route,
        } = inputs;
        let type_parameters =
            crate::lowering::data::lower_type_parameters(lowerer, syntax_trees, type_parameters)?;
        let conformance_bounds = crate::lowering::machine::lower_generic_conformance_bounds(
            lowerer,
            syntax_trees,
            conformance_bounds,
        )?;
        let parameters = lower_state_parameters(lowerer, syntax_trees, parameters)?;
        let return_type = return_type_handle
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, return_type_handle))
            .transpose()?;
        let service_reaches = lower_service_reach_names(syntax_trees, service_reaches);
        let invokes = lower_signature_invokes(lowerer, syntax_trees, invokes);
        let contracts = lower_signature_contracts(lowerer, syntax_trees, contracts)?;

        Ok(LoweredStateSignature {
            callable_scope,
            signature: StateSignature {
                symbol: SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(syntax_trees, name),
                storage: StateSignatureStorage {
                    spelling,
                    lifetime_parameters: lower_names(syntax_trees, lifetime_parameters),
                    type_parameters,
                    conformance_bounds,
                    is_default,
                    parameters,
                    native_callback_parameters: native_callback_parameters
                        .iter()
                        .map(|parameter| NativeCallbackParameter {
                            name: crate::lowering::name::lower_name(syntax_trees, &parameter.name),
                            binder: crate::lowering::name::lower_name(
                                syntax_trees,
                                &parameter.binder,
                            ),
                            native_ordinal: parameter.native_ordinal,
                        })
                        .collect(),
                    return_type,
                    invokes,
                    service_reach_row: language_semantics::ServiceReachRowId::NULL,
                    service_reach_is_installation_bound,
                    suspends_keyword_source_spans: suspends_keyword_source_spans.to_vec(),
                    blocks_keyword_source_spans: blocks_keyword_source_spans.to_vec(),
                    suspends,
                    blocks,
                    contracts,
                    terminates_guarantee,
                    declares_no_crash_route,
                },
            },
            service_reach_keyword_source_spans: service_reach_keyword_source_spans.to_vec(),
            service_reaches,
        })
    }

    pub(crate) struct LoweredStateSignature {
        pub(crate) callable_scope: crate::runtime_callable_scopes::CallableScopeHandle,
        pub(crate) signature: StateSignature,
        pub(crate) service_reach_keyword_source_spans: Vec<source::SourceSpan>,
        pub(crate) service_reaches: Vec<DiagnosticName>,
    }

    pub(crate) fn lower_signature_invokes(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        invokes: HandleSpan<Identifier>,
    ) -> HandleSpan<DiagnosticName> {
        crate::lowering::name::lower_name_members(
            syntax_trees,
            invokes,
            &mut lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .signature_invokes,
        )
    }

    pub(crate) fn lower_service_reach_names(
        syntax_trees: &SyntaxTrees,
        service_reaches: HandleSpan<Identifier>,
    ) -> Vec<DiagnosticName> {
        lower_names(
            syntax_trees,
            syntax_trees.items.identifier_path_members(service_reaches),
        )
    }

    pub(crate) fn lower_signature_contracts(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        contracts: HandleSpan<CapabilityContract>,
    ) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
        lower_signature_contracts_with_result_sum(lowerer, syntax_trees, contracts, None)
    }

    pub(crate) fn lower_machine_signature_contracts(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        contracts: HandleSpan<CapabilityContract>,
        states: HandleSpan<Handle<symbol_resolved_trees::state::State>>,
    ) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
        let has_outcome_rows = syntax_items(syntax_trees)
            .capability_contracts(contracts)
            .iter()
            .any(|contract| {
                matches!(
                    &contract.kind,
                    CapabilityContractKind::EnsuresForResultCase { .. }
                )
            });
        if !has_outcome_rows {
            return lower_signature_contracts_with_result_sum(
                lowerer,
                syntax_trees,
                contracts,
                None,
            );
        }

        let root_state = lowerer
            .symbol_resolved_trees
            .machine_state_handles(states)
            .first()
            .copied()
            .map(|handle| lowerer.symbol_resolved_trees.machine_state(handle))
            .ok_or_else(|| {
                Diagnostic::error("outcome-specific ensures requires a machine result state")
            })?;
        let result_data_name = match root_state.return_type.as_ref() {
            Some(TypeReference::Named { name, .. }) => name,
            Some(TypeReference::Generic(generic)) => &generic.base_name,
            _ => {
                return Err(Diagnostic::error(
                    "outcome-specific ensures requires a declared nominal sum result type",
                ));
            }
        };
        let data = lowerer
            .symbol_resolved_trees
            .data_definitions
            .iter()
            .filter(|data| {
                data.name.as_str() == result_data_name.as_str()
                    && lowerer.source_reference_can_see_declaration(
                        result_data_name.source_span(),
                        data.name.source_span(),
                    )
            })
            .min_by_key(|data| {
                let declaration = data.name.source_span();
                let reference = result_data_name.source_span();
                (
                    declaration.source_id != reference.source_id,
                    lowerer.source_resolution_strata_separate(reference, declaration),
                )
            })
            .ok_or_else(|| {
                Diagnostic::error(
                    "outcome-specific ensures result does not resolve to a declared data sum",
                )
            })?;
        let variants = lowerer
            .symbol_resolved_trees
            .data_members(data.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant.name.to_string()),
                DataMember::Field(_) => None,
            })
            .collect::<Vec<_>>();
        if variants.is_empty() {
            return Err(Diagnostic::error(format!(
                "outcome-specific ensures requires a sum result; `{}` declares no cases",
                data.name
            )));
        }
        let result_sum = ResultSumContractContext {
            data_name: data.name.to_string(),
            variants,
        };
        lower_signature_contracts_with_result_sum(
            lowerer,
            syntax_trees,
            contracts,
            Some(&result_sum),
        )
    }

    struct ResultSumContractContext {
        data_name: String,
        variants: Vec<String>,
    }

    fn lower_signature_contracts_with_result_sum(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        contracts: HandleSpan<CapabilityContract>,
        result_sum: Option<&ResultSumContractContext>,
    ) -> Result<HandleSpan<SignatureContract>, Diagnostic> {
        let mut span = HandleSpan::empty();

        for contract in syntax_trees.items.capability_contracts(contracts) {
            let (kind, pending_outcome) = match &contract.kind {
                CapabilityContractKind::Requires => (SignatureContractKind::Requires, None),
                CapabilityContractKind::Ensures => (SignatureContractKind::Ensures, None),
                CapabilityContractKind::EnsuresForResultCase { result_case } => {
                    let Some(result_sum) = result_sum else {
                        return Err(Diagnostic::error(
                            "outcome-specific ensures is admitted only on a top-level machine result",
                        ));
                    };
                    let members = syntax_trees.items.identifier_path_members(*result_case);
                    let [result_name, case_name] = members else {
                        return Err(Diagnostic::error(
                            "outcome-specific ensures requires the exact path `Result::Case`",
                        ));
                    };
                    if result_name.as_str(syntax_trees) != result_sum.data_name.as_str() {
                        return Err(Diagnostic::error(format!(
                            "outcome-specific ensures case `{result_name}::{case_name}` does not belong to declared result sum `{}`",
                            result_sum.data_name,
                            result_name = result_name.as_str(syntax_trees),
                            case_name = case_name.as_str(syntax_trees)
                        )));
                    }
                    let result_case_name = result_sum
                    .variants
                    .iter()
                    .find(|name| name.as_str() == case_name.as_str(syntax_trees))
                    .cloned()
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "outcome-specific ensures names unknown case `{case_name}` of declared result sum `{}`",
                            result_sum.data_name
                        , case_name = case_name.as_str(syntax_trees)))
                    })?;
                    (
                        SignatureContractKind::EnsuresForResultCase {
                            result_data: symbols::SymbolHandle::invalid(),
                            result_case: symbols::SymbolHandle::invalid(),
                        },
                        Some((
                            result_sum.data_name.clone(),
                            result_name.source_span(),
                            result_case_name,
                        )),
                    )
                }
                CapabilityContractKind::Crashes { cause } => (
                    SignatureContractKind::Crashes {
                        cause: match cause {
                            syntax::item::CrashCause::Trap => CrashCause::Trap,
                            syntax::item::CrashCause::Abort => CrashCause::Abort,
                        },
                    },
                    None,
                ),
            };
            let facts = lower_proof_facts(lowerer, syntax_trees, contract.facts)?;
            let handle = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .signature_contracts
                .append_to_span(
                    &mut span,
                    SignatureContract {
                        kind,
                        keyword_source_span: contract.keyword_source_span,
                        binding: contract
                            .binding
                            .as_ref()
                            .map(|name| crate::lowering::name::lower_name(syntax_trees, name)),
                        facts,
                        token_count: contract.token_count,
                    },
                );
            if let Some((result_data_name, result_data_source_span, result_case_name)) =
                pending_outcome
            {
                lowerer.pending_outcome_specific_contracts.push(
                    crate::resolution::lowerer::PendingOutcomeSpecificContract {
                        contract: handle,
                        result_data_name,
                        result_data_source_span,
                        result_case_name,
                    },
                );
            }
        }

        Ok(span)
    }

    fn lower_state_statements(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        statements: HandleSpan<StatementHandle>,
    ) -> Result<HandleSpan<Statement>, Diagnostic> {
        let mut pending: Vec<Statement> = Vec::new();
        let mut run_start: Option<usize> = None;
        let mut statement_origins = Vec::new();

        for statement in syntax_trees.items.statements(statements) {
            lower_statement_into_pending(
                lowerer,
                syntax_trees,
                *statement,
                &mut pending,
                &mut run_start,
                &mut statement_origins,
            )?;
        }

        let mut span = HandleSpan::empty();
        for (index, lowered) in pending.into_iter().enumerate() {
            let handle = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .state_statements
                .append_to_span(&mut span, lowered);
            if let Some(sources) = &mut lowerer.equation_sources
                && let Some((_, original)) = statement_origins
                    .iter()
                    .find(|(offset, _)| *offset == index)
            {
                sources.statements.push((*original, handle));
            }
        }
        Ok(span)
    }

    fn lower_statement_into_pending(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        statement: StatementHandle,
        pending: &mut Vec<Statement>,
        run_start: &mut Option<usize>,
        statement_origins: &mut Vec<(usize, StatementHandle)>,
    ) -> Result<(), Diagnostic> {
        let has_preceding_transition = run_start.is_some();
        let mut lowered = lower_statement_handle(
            lowerer,
            syntax_trees,
            statement,
            pending.len(),
            has_preceding_transition,
        )?;
        let ends_in_transition = matches!(lowered.last(), Some(Statement::Transition(_)));
        if ends_in_transition {
            let lets_count = lowered.len() - 1;
            match *run_start {
                Some(start) if lets_count > 0 => {
                    for (offset, _) in statement_origins.iter_mut() {
                        if *offset >= start {
                            *offset += lets_count;
                        }
                    }
                    let transition = lowered.pop().expect("transition chunk is non-empty");
                    for (offset, hoisted) in lowered.into_iter().enumerate() {
                        pending.insert(start + offset, hoisted);
                    }
                    pending.push(transition);
                }
                _ => {
                    if run_start.is_none() {
                        *run_start = Some(pending.len() + lets_count);
                    }
                    pending.extend(lowered);
                }
            }
        } else {
            pending.extend(lowered);
            *run_start = None;
        }

        if matches!(
            syntax_trees.statements.statement(statement),
            StatementNode::Transition(transition) if transition.ends_dispatch
        ) {
            *run_start = None;
        }

        if matches!(
            syntax_trees.statements.statement(statement),
            StatementNode::Call(_) | StatementNode::LocalData(_)
        ) {
            statement_origins.push((pending.len() - 1, statement));
        }
        Ok(())
    }

    pub(crate) fn lower_state_parameters(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        parameters: HandleSpan<StateParameterHandle>,
    ) -> Result<HandleSpan<StateParameter>, Diagnostic> {
        let mut span = HandleSpan::empty();

        for parameter in syntax_trees.items.state_parameters(parameters) {
            let authored_type = syntax_items(syntax_trees)
                .state_parameter(*parameter)
                .type_reference;
            let parameter = lower_state_parameter(lowerer, syntax_trees, *parameter)?;
            let handle = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .state_parameters
                .append_to_span(&mut span, parameter);
            if let Some(sources) = &mut lowerer.equation_sources {
                sources.declared_types.push((
                    crate::preparation::machine_equations::DeclaredTypeOrigin::Parameter(handle),
                    authored_type,
                ));
            }
        }

        Ok(span)
    }

    pub(crate) fn lower_state_parameter(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        parameter: StateParameterHandle,
    ) -> Result<StateParameter, Diagnostic> {
        let parameter = syntax_trees.items.state_parameter(parameter);
        Ok(StateParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, &parameter.name),
            type_reference: lower_type_reference_handle(
                lowerer,
                syntax_trees,
                parameter.type_reference,
            )?,
            is_const: parameter.is_const,
            is_mutable: parameter.is_mutable,
            relevance: parameter.relevance,
        })
    }

    /*
        The names of parameters declared as a SHARED reference to a NAMED type
        (`table: &EfiSystemTable`) -- the shapes whose member reads must deref the
        pointer slot rather than fold flat.
    */
    fn reference_struct_parameter_names(
        lowerer: &Lowerer,
        parameters: &HandleSpan<symbol_resolved_trees::signature::StateParameter>,
    ) -> Vec<String> {
        lowerer
            .symbol_resolved_trees
            .state_parameters(*parameters)
            .iter()
            .filter_map(|parameter| {
                let TypeReference::Reference(reference) = &parameter.type_reference else {
                    return None;
                };
                if reference.access.is_exclusive() {
                    return None;
                }
                matches!(
                    lowerer
                        .symbol_resolved_trees
                        .child_type_reference(reference.referee),
                    TypeReference::Named { .. }
                )
                .then(|| parameter.name.as_str().to_string())
            })
            .collect()
    }
}
#[path = "lowering_statement.rs"]
pub(crate) mod statement;
pub(crate) mod trait_definition {
    /*
        Trait definitions: parents, requirements, conformance bounds, and machine
        signatures.
    */

    use crate::lowering::data::lower_type_parameters;
    use crate::lowering::name::lower_names;
    use crate::lowering::state::lower_state_signature_node;
    use crate::lowering::type_reference::lower_child_type_references;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_type_references;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::signature::StateSignature;
    use symbol_resolved_trees::trait_definition::{
        TraitDefinition, TraitRefinementClause, TraitRequirement, TraitStorage,
    };
    use symbols::SymbolHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_trait_definition(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        trait_definition: &syntax::item::TraitDefinition,
    ) -> Result<TraitDefinition, Diagnostic> {
        let name = crate::lowering::name::lower_name(syntax_trees, &trait_definition.name);
        let type_parameters =
            lower_type_parameters(lowerer, syntax_trees, trait_definition.type_parameters)?;
        let conformance_bounds = crate::lowering::machine::lower_generic_conformance_bounds(
            lowerer,
            syntax_trees,
            &trait_definition.conformance_bounds,
        )?;
        let requires = lower_trait_requirements(
            lowerer,
            syntax_trees,
            trait_definition.parents,
            trait_definition.requires,
        )?;
        let machines = lower_trait_machine_signatures(
            lowerer,
            syntax_trees,
            &trait_definition.name,
            trait_definition.machines,
        )?;
        let refines = trait_definition
            .refines
            .map(|base| lower_trait_requirement_node(lowerer, syntax_trees, base))
            .transpose()?;
        let mut refinement_clauses = Vec::with_capacity(trait_definition.refinement_clauses.len());
        for clause in &trait_definition.refinement_clauses {
            let clause_signature = &clause.signature;
            let lowered = crate::lowering::state::lower_state_signature_parts(
                lowerer,
                syntax_trees,
                crate::lowering::state::StateSignatureParts {
                    name: &clause_signature.name,
                    spelling: clause_signature.spelling,
                    lifetime_parameters: &clause_signature.lifetime_parameters,
                    type_parameters: clause_signature.type_parameters,
                    conformance_bounds: &clause_signature.conformance_bounds,
                    parameters: clause_signature.parameters,
                    native_callback_parameters: &clause_signature.native_callback_parameters,
                    return_type_handle: clause_signature.return_type,
                    is_default: clause_signature.is_default,
                    service_reach_is_installation_bound: clause_signature
                        .service_reach_is_installation_bound,
                    service_reach_keyword_source_spans: &clause_signature
                        .service_reach_keyword_source_spans,
                    service_reaches: clause_signature.service_reaches,
                    invokes: clause_signature.invokes,
                    suspends_keyword_source_spans: &clause_signature.suspends_keyword_source_spans,
                    blocks_keyword_source_spans: &clause_signature.blocks_keyword_source_spans,
                    suspends: clause_signature.suspends,
                    blocks: clause_signature.blocks,
                    contracts: clause_signature.contracts,
                    terminates_guarantee: clause_signature.terminates_guarantee,
                    declares_no_crash_route: clause_signature.declares_no_crash_route,
                },
            )?;
            refinement_clauses.push(TraitRefinementClause {
                requirement: clause
                    .requirement
                    .as_ref()
                    .map(|name| crate::lowering::name::lower_name(syntax_trees, name)),
                signature: lowered.signature,
                service_reach_keyword_source_spans: lowered.service_reach_keyword_source_spans,
                service_reaches: lowered.service_reaches,
            });
        }

        Ok(TraitDefinition {
            symbol: SymbolHandle::invalid(),
            is_boundary: trait_definition.is_boundary,
            is_public: trait_definition.is_public,
            name,
            storage: TraitStorage {
                lifetime_parameters: lower_names(
                    syntax_trees,
                    &trait_definition.lifetime_parameters,
                ),
                type_parameters,
                conformance_bounds,
                requires,
                machines,
                refines,
                refinement_clauses,
            },
        })
    }

    fn lower_trait_requirements(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        parents: HandleSpan<syntax::types::TypeReferenceHandle>,
        requires: HandleSpan<syntax::identifier::Identifier>,
    ) -> Result<HandleSpan<TraitRequirement>, Diagnostic> {
        let mut span = HandleSpan::empty();

        for parent_handle in syntax_trees.type_references.type_reference_handles(parents) {
            let requirement = lower_trait_requirement_node(lowerer, syntax_trees, *parent_handle)?;
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .trait_requirements
                .append_to_span(&mut span, requirement);
        }

        for required_trait in syntax_trees.items.identifier_path_members(requires) {
            lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .trait_requirements
                .append_to_span(
                    &mut span,
                    TraitRequirement {
                        symbol: SymbolHandle::invalid(),
                        name: crate::lowering::name::lower_name(syntax_trees, required_trait),
                        lifetime_arguments: Vec::new(),
                        arguments: HandleSpan::empty(),
                    },
                );
        }

        Ok(span)
    }

    /*
        One `Name` or `Name<args>` reference in trait-position (parent, `requires`,
        or refinement `= Base` head).
    */
    fn lower_trait_requirement_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        handle: syntax::types::TypeReferenceHandle,
    ) -> Result<TraitRequirement, Diagnostic> {
        let (name, lifetime_arguments, arguments) =
            match syntax_type_references(syntax_trees).type_reference(handle) {
                syntax::types::TypeReferenceNode::Named(name) => {
                    (name, Vec::new(), HandleSpan::empty())
                }
                syntax::types::TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                    ..
                } => (
                    base_name,
                    lower_names(syntax_trees, lifetime_arguments),
                    lower_child_type_references(lowerer, syntax_trees, *arguments)?,
                ),
                _ => {
                    return Err(Diagnostic::error(
                        "a trait parent must be a named trait, optionally with generic arguments",
                    ));
                }
            };
        Ok(TraitRequirement {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, name),
            lifetime_arguments,
            arguments,
        })
    }

    fn lower_trait_machine_signatures(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        trait_name: &syntax::identifier::Identifier,
        machines: HandleSpan<syntax::item::StateSignatureHandle>,
    ) -> Result<HandleSpan<StateSignature>, Diagnostic> {
        let mut span = HandleSpan::empty();

        for signature in syntax_trees.items.state_signatures(machines) {
            let lowered = lower_state_signature_node(
                lowerer,
                syntax_trees,
                syntax_trees.items.state_signature(*signature),
            )?;
            let handle = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .trait_machine_signatures
                .append_to_span(&mut span, lowered.signature);
            lowerer.runtime_callable_scopes.bind(
                lowered.callable_scope,
                crate::runtime_callable_scopes::CallableScopeOwner::Signature(
                    crate::resolution::lowerer::PendingSignatureLocation::Trait(handle),
                ),
            );
            lowerer.pending_signature_service_reaches.push(
                crate::resolution::lowerer::PendingSignatureServiceReach {
                    location: crate::resolution::lowerer::PendingSignatureLocation::Trait(handle),
                    owner: crate::resolution::lowerer::PendingSignatureOwner::Trait(
                        crate::lowering::name::lower_name(syntax_trees, trait_name),
                    ),
                    reaches:
                        crate::selection::service_reaches::PendingAuthoredServiceReach::Authored {
                            keyword_source_spans: lowered.service_reach_keyword_source_spans,
                            authored: lowered.service_reaches,
                        },
                },
            );
        }

        Ok(span)
    }
}
pub(crate) mod type_aliases {
    /* Source alias headers retain their own telescope and expansion custody. */

    use crate::lowering::data::lower_type_parameters;
    use crate::lowering::machine::lower_generic_conformance_bounds;
    use crate::lowering::name::{lower_name, lower_names};
    use crate::lowering::type_reference::lower_type_reference_handle;
    use crate::resolution::lowerer::Lowerer;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::type_aliases::TypeAliasDefinition;
    use symbols::SymbolHandle;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_type_alias_definition(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        alias: &syntax::item::TypeAliasDefinition,
    ) -> Result<TypeAliasDefinition, Diagnostic> {
        let type_parameters = lower_type_parameters(lowerer, syntax, alias.type_parameters)?;
        let conformance_bounds =
            lower_generic_conformance_bounds(lowerer, syntax, &alias.conformance_bounds)?;
        let previous = lowerer.structural_type_equations_pending;
        lowerer.structural_type_equations_pending = true;
        let expansion = lower_type_reference_handle(lowerer, syntax, alias.expansion);
        lowerer.structural_type_equations_pending = previous;
        let expansion = expansion?;
        Ok(TypeAliasDefinition {
            symbol: SymbolHandle::invalid(),
            name: lower_name(syntax, &alias.name),
            is_public: alias.is_public,
            lifetime_parameters: lower_names(syntax, &alias.lifetime_parameters),
            type_parameters,
            conformance_bounds,
            expansion,
        })
    }
}
pub(crate) mod type_reference {
    /*
        Type references, constraints, and const-argument slots.

        Normalized const arguments are validated before their expressions lower.
        Dynamic trait references and fixed array lengths lower here as well.
    */

    use crate::lowering::expression::lower_expression_into_table;
    use crate::lowering::name::lower_name;
    use crate::lowering::name::lower_names;
    use crate::resolution::lowerer::Lowerer;
    use crate::tables::syntax_type_references;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::types::{
        ConstrainedTypeReference, ConstrainedTypeReferenceStorage, DomainConstraint,
        FixedArrayLength, FixedArrayTypeReference, FixedArrayTypeReferenceStorage,
        GenericTypeReference, GenericTypeReferenceStorage, ReferenceTypeReference,
        ReferenceTypeReferenceStorage, SliceTypeReference, SliceTypeReferenceStorage,
        TypeConstraint, TypeReference,
    };
    use symbols::SymbolHandle;
    use syntax::types::TypeReferenceNode;
    use syntax_trees::{self as syntax, SyntaxTrees};

    pub(crate) fn lower_type_reference_handle(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        type_reference: syntax::types::TypeReferenceHandle,
    ) -> Result<TypeReference, Diagnostic> {
        let aliases = syntax_trees
            .type_references
            .type_alias_applications(type_reference)
            .cloned()
            .collect::<Vec<_>>();
        let prior_exposure = if aliases.is_empty() {
            None
        } else {
            Some(lowerer.current_authored_expression_exposure.take())
        };
        let result = lower_type_reference_contents(lowerer, syntax_trees, type_reference);
        if let Some(exposure) = prior_exposure {
            lowerer.current_authored_expression_exposure = exposure;
        }
        let mut lowered = result?;
        if let TypeReference::Callable { contract, .. } = &lowered {
            let definition_references = &mut lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .runtime_callable_contracts
                .get_mut(*contract)
                .definition_references;
            for alias in &aliases {
                definition_references.extend_from_slice(&alias.definition_references);
            }
        }
        for alias in aliases {
            let application = lower_alias_application(lowerer, syntax_trees, alias.application)?;
            let expanded_owner = lowered.supplied_operand_owner();
            let supplied_operand_owner = if expanded_owner.is_valid() {
                expanded_owner
            } else {
                application.supplied_operand_owner()
            };
            lowered = TypeReference::AliasUse {
                expansion: Box::new(lowered),
                application: Box::new(application),
                declaration_source: alias.declaration,
                definition_references: alias.definition_references,
                supplied_operand_owner,
            };
        }
        Ok(lowered)
    }

    fn lower_type_reference_contents(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        type_reference: syntax::types::TypeReferenceHandle,
    ) -> Result<TypeReference, Diagnostic> {
        lowerer
            .encountered_pending_type_arguments
            .extend(syntax_type_references(syntax_trees).pending_const_arguments(type_reference));
        if !lowerer.structural_type_equations_pending {
            crate::preparation::generic_data::validate_materialized_type_equations(
                syntax_trees,
                type_reference,
                lowerer.constant_selection.as_ref(),
            )?;
        }
        let mut materialization_source =
            symbol_resolved_trees::expression::ExpressionHandle::invalid();
        if let Some(normalization) =
            syntax_type_references(syntax_trees).const_argument_normalization(type_reference)
        {
            crate::constant::validate_normalized_const_argument(
                syntax_trees,
                syntax_trees.type_references.type_reference(type_reference),
                normalization,
            )?;
            crate::constant::validate_normalized_expression(syntax_trees, normalization)?;
            let derived = lowerer
                .derived_const_argument_expressions
                .contains(&normalization.authored_expression);
            if normalization.authored_expression.is_valid()
                && (!derived || normalization.materialized_expression.is_valid())
            {
                let exposure = derived.then(|| lowerer.current_authored_expression_exposure.take());
                let expression = lower_expression_into_table(
                    lowerer,
                    syntax_trees,
                    normalization.authored_expression,
                );
                if let Some(exposure) = exposure {
                    lowerer.current_authored_expression_exposure = exposure;
                }
                let expression = expression?;
                lowerer.pending_const_argument_expressions.push(expression);
                materialization_source = expression;
            }
            for origin in syntax_type_references(syntax_trees)
                .const_argument_origins(normalization.selections)
            {
                let exposure = if lowerer.derived_const_argument_origins.contains(origin) {
                    None
                } else {
                    let authored_here =
                        !syntax_trees.constant_initializer_owns_selection(origin.reference);
                    Some(if authored_here {
                        lowerer.current_authored_expression_exposure.unwrap_or(
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    )
                    } else {
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                    })
                };
                lowerer.pending_const_argument_selections.push(
                    crate::resolution::lowerer::PendingConstArgumentSelection {
                        origin: origin.clone(),
                        exposure,
                        is_argument_value: origin.reference == normalization.reference,
                    },
                );
            }
            for reference in syntax_type_references(syntax_trees)
                .const_argument_builtin_operators(normalization.builtin_operators)
            {
                if lowerer
                    .derived_const_argument_builtin_operators
                    .contains(reference)
                {
                    continue;
                }
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionExposure as Exposure,
                    AuthoredDeclarationSelectionIntrinsic as Intrinsic,
                    AuthoredDeclarationSelectionKind as Kind,
                    AuthoredDeclarationSelectionLateBinding as LateBinding,
                };
                let exposure = if syntax_trees.constant_initializer_owns_selection(*reference) {
                    Exposure::PrivateImplementation
                } else {
                    lowerer
                        .current_authored_expression_exposure
                        .unwrap_or(Exposure::PrivateImplementation)
                };
                let occurrence = lowerer
                    .symbol_resolved_trees
                    .record_late_bound_authored_declaration_selection(
                        *reference,
                        exposure,
                        Kind::Operator,
                        LateBinding::CheckedOperator,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "failed to retain normalized builtin operator: {error:?}"
                        ))
                        .with_source_span(*reference)
                    })?;
                lowerer
                    .symbol_resolved_trees
                    .finalize_intrinsic_authored_declaration_selection(
                        occurrence,
                        LateBinding::CheckedOperator,
                        Intrinsic::BuiltinOperator,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "failed to finalize normalized builtin operator: {error:?}"
                        ))
                        .with_source_span(*reference)
                    })?;
            }
        }
        let lowered = if let Some(normalization) = syntax_trees
            .type_references
            .const_argument_normalization(type_reference)
            .filter(|normalization| normalization.materialized_expression.is_valid())
        {
            let exposure = lowerer.current_authored_expression_exposure.take();
            let value = lower_expression_into_table(
                lowerer,
                syntax_trees,
                normalization.materialized_expression,
            );
            lowerer.current_authored_expression_exposure = exposure;
            let value = value?;
            lowerer.pending_const_argument_expressions.push(value);
            lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .retain_materialization_source(value, materialization_source);
            let mut lowered = TypeReference::ConstExpression(
                symbol_resolved_trees::types::ConstExpressionTypeReference::new(value),
            );
            let input = syntax_type_references(syntax_trees)
                .supplied_operand_owner(type_reference)
                .map_err(Diagnostic::error)?;
            let owner = crate::resolution::operand_owners::retain(lowerer, input);
            lowered.retain_supplied_operand_owner(owner);
            lowered
        } else {
            lower_type_reference_node(lowerer, syntax_trees, type_reference)?
        };
        let origin =
            syntax_type_references(syntax_trees).generic_application_origin(type_reference);
        if origin.is_valid() {
            let application = lower_generic_application_origin(lowerer, syntax_trees, origin)?;
            let instance = lowerer
                .declaration_child_type_references()
                .insert(lowered.clone());
            let application = lowerer
                .declaration_child_type_references()
                .insert(application);
            lowerer
                .symbol_resolved_trees
                .tables
                .types
                .generic_application_origins
                .insert(symbol_resolved_trees::types::GenericApplicationOrigin {
                    instance,
                    application,
                });
        }
        Ok(lowered)
    }

    /*
        The original alias head is source formation, not a closed nominal-data
        synthesis request. Arguments still use the ordinary lowering and const
        custody checks; alias-own carrier/bound admission occurs at the use site.
    */
    fn lower_alias_application(
        lowerer: &mut Lowerer,
        syntax: &SyntaxTrees,
        reference: syntax::types::TypeReferenceHandle,
    ) -> Result<TypeReference, Diagnostic> {
        let syntax::types::TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = syntax.type_references.type_reference(reference)
        else {
            return lower_type_reference_node(lowerer, syntax, reference);
        };
        let arguments = lower_indexed_application_arguments(lowerer, syntax, *arguments)?;
        let owner = syntax
            .type_references
            .supplied_operand_owner(reference)
            .map_err(Diagnostic::error)?;
        let supplied_operand_owner = crate::resolution::operand_owners::retain(lowerer, owner);
        Ok(TypeReference::Generic(GenericTypeReference {
            storage: GenericTypeReferenceStorage {
                supplied_operand_owner,
                base_symbol: SymbolHandle::invalid(),
                base_name: crate::lowering::name::lower_name(syntax, base_name),
                lifetime_arguments: lower_names(syntax, lifetime_arguments),
                arguments,
            },
        }))
    }

    /*
        An already synthesized instance retains its generic tuple as identity
        metadata, not as an unspecialized application. Its argument types still
        take ordinary lowering and checking; only this outer metadata node bypasses
        the requirement that executable type equations have been specialized.
    */
    pub(crate) fn lower_generic_application_origin(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        origin: syntax::types::TypeReferenceHandle,
    ) -> Result<TypeReference, Diagnostic> {
        if !matches!(
            syntax_trees.type_references.type_reference(origin),
            TypeReferenceNode::Generic { .. }
        ) {
            return Err(Diagnostic::error(
                "generic instance origin must retain a generic application",
            ));
        }
        lower_type_reference_node(lowerer, syntax_trees, origin)
    }

    fn lower_type_reference_node(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        type_reference: syntax::types::TypeReferenceHandle,
    ) -> Result<TypeReference, Diagnostic> {
        let mut lowered = match syntax_trees.type_references.type_reference(type_reference) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => Ok(TypeReference::Reference(ReferenceTypeReference {
                storage: ReferenceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    referee: lower_type_reference_child(lowerer, syntax_trees, *referee)?,
                    access: *access,
                    lifetime: lifetime.as_ref().map(|name| lower_name(syntax_trees, name)),
                },
            })),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => Ok(TypeReference::Constrained(ConstrainedTypeReference {
                storage: ConstrainedTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    base_type: lower_type_reference_child(lowerer, syntax_trees, *base_type)?,
                    constraints: lower_type_constraint_handles(
                        lowerer,
                        syntax_trees,
                        *constraints,
                    )?,
                },
            })),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => Ok(TypeReference::FixedArray(FixedArrayTypeReference {
                storage: FixedArrayTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    element_type: lower_type_reference_child(lowerer, syntax_trees, *element_type)?,
                    length: lower_fixed_array_length(lowerer, syntax_trees, length)?,
                },
            })),
            TypeReferenceNode::Slice { element_type } => {
                Ok(TypeReference::Slice(SliceTypeReference {
                    storage: SliceTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        element_type: lower_type_reference_child(
                            lowerer,
                            syntax_trees,
                            *element_type,
                        )?,
                    },
                }))
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                crate::preparation::generic_data::validate_direct_const_arguments(
                    syntax_trees,
                    base_name,
                    *arguments,
                    lowerer.constant_selection.as_ref(),
                )?;
                let lowered_arguments =
                    lower_indexed_application_arguments(lowerer, syntax_trees, *arguments)?;
                Ok(TypeReference::Generic(GenericTypeReference {
                    storage: GenericTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        base_symbol: SymbolHandle::invalid(),
                        base_name: lower_name(syntax_trees, base_name),
                        lifetime_arguments: lower_names(syntax_trees, lifetime_arguments),
                        arguments: lowered_arguments,
                    },
                }))
            }
            TypeReferenceNode::StaticSelection(_) => Err(Diagnostic::error(
                "a scoped type selection must resolve to an admitted declared type before typed lowering",
            )),
            TypeReferenceNode::ConstExpression(expression) => {
                let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
                Ok(TypeReference::ConstExpression(
                    symbol_resolved_trees::types::ConstExpressionTypeReference::new(expression),
                ))
            }
            TypeReferenceNode::Callable { signature } => {
                let contract = lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .append(Default::default());
                lowerer.runtime_callable_scopes.retain(contract);
                let lowered = crate::lowering::state::lower_state_signature_node(
                    lowerer,
                    syntax_trees,
                    syntax_trees.items.state_signature(*signature),
                )?;
                lowerer.runtime_callable_scopes.bind(
                    lowered.callable_scope,
                    crate::runtime_callable_scopes::CallableScopeOwner::Signature(
                        crate::resolution::lowerer::PendingSignatureLocation::RuntimeCallable(
                            contract,
                        ),
                    ),
                );
                let owner = lowered.signature.name.clone();
                lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .runtime_callable_contracts
                    .get_mut(contract)
                    .signature = lowered.signature;
                lowerer
                    .pending_signature_service_reaches
                    .push(crate::resolution::lowerer::PendingSignatureServiceReach {
                    location: crate::resolution::lowerer::PendingSignatureLocation::RuntimeCallable(
                        contract,
                    ),
                    owner: crate::resolution::lowerer::PendingSignatureOwner::Requirement(owner),
                    reaches:
                        crate::selection::service_reaches::PendingAuthoredServiceReach::Authored {
                            keyword_source_spans: lowered.service_reach_keyword_source_spans,
                            authored: lowered.service_reaches,
                        },
                });
                Ok(TypeReference::Callable {
                    contract,
                    supplied_operand_owner: Default::default(),
                })
            }
            TypeReferenceNode::DynamicTrait {
                trait_name,
                trait_arguments,
            } => Ok(TypeReference::DynamicTrait {
                supplied_operand_owner: Default::default(),
                symbol: SymbolHandle::invalid(),
                name: lower_name(syntax_trees, trait_name),
                arguments: lower_child_type_references(lowerer, syntax_trees, *trait_arguments)?,
            }),
            TypeReferenceNode::Named(name) => Ok(TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: SymbolHandle::invalid(),
                name: lower_name(syntax_trees, name),
            }),
            TypeReferenceNode::Unit => Ok(TypeReference::Unit),
        }?;
        let input = syntax_type_references(syntax_trees)
            .supplied_operand_owner(type_reference)
            .map_err(Diagnostic::error)?;
        let owner = crate::resolution::operand_owners::retain(lowerer, input);
        lowered.retain_supplied_operand_owner(owner);
        Ok(lowered)
    }

    pub(crate) fn lower_fixed_array_length(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        length: &syntax::types::FixedArrayLength,
    ) -> Result<FixedArrayLength, Diagnostic> {
        Ok(match length {
            syntax::types::FixedArrayLength::Expression(value) => {
                let child = lower_type_reference_child(lowerer, syntax_trees, *value)?;
                if syntax_type_references(syntax_trees)
                    .const_argument_normalization(*value)
                    .is_some_and(|normalization| !normalization.materialized_expression.is_valid())
                    && let Some(name) = syntax_type_references(syntax_trees)
                        .type_reference(*value)
                        .as_named()
                {
                    FixedArrayLength::Literal(name.as_str(syntax_trees).parse::<usize>().map_err(
                        |_| {
                            Diagnostic::error(
                                "array extent must be a nonnegative host-sized integer",
                            )
                            .with_source_span(name.source_span())
                        },
                    )?)
                } else {
                    FixedArrayLength::Expression(child)
                }
            }
            syntax::types::FixedArrayLength::Literal(value) => FixedArrayLength::Literal(*value),
            syntax::types::FixedArrayLength::ConstParameter(name) => {
                FixedArrayLength::ConstParameter {
                    symbol: SymbolHandle::invalid(),
                    name: lower_name(syntax_trees, name),
                }
            }
            syntax::types::FixedArrayLength::ConstCall(name) => FixedArrayLength::ConstCall {
                machine: SymbolHandle::invalid(),
                name: lower_name(syntax_trees, name),
            },
        })
    }

    fn lower_type_reference_child(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        type_reference: syntax::types::TypeReferenceHandle,
    ) -> Result<arena::Handle<TypeReference>, Diagnostic> {
        let type_reference = lower_type_reference_handle(lowerer, syntax_trees, type_reference)?;

        Ok(lowerer
            .declaration_child_type_references()
            .append(type_reference))
    }

    pub(crate) fn lower_child_type_references(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        arguments: HandleSpan<syntax::types::TypeReferenceHandle>,
    ) -> Result<HandleSpan<TypeReference>, Diagnostic> {
        let mut lowered = Vec::with_capacity(arguments.len());

        for argument in syntax_type_references(syntax_trees).type_reference_handles(arguments) {
            lowered.push(lower_type_reference_handle(
                lowerer,
                syntax_trees,
                *argument,
            )?);
        }

        Ok(lowerer
            .declaration_child_type_references()
            .insert_many(lowered))
    }

    pub(crate) fn lower_type_constraint_handles(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        constraints: HandleSpan<syntax::types::TypeConstraintNode>,
    ) -> Result<HandleSpan<TypeConstraint>, Diagnostic> {
        let mut lowered = Vec::with_capacity(constraints.len());

        for constraint in syntax_trees.type_references.constraints(constraints) {
            lowered.push(lower_type_constraint_handle(
                lowerer,
                syntax_trees,
                constraint,
            )?);
        }

        Ok(lowerer
            .symbol_resolved_trees
            .tables
            .types
            .constraints
            .insert_many(lowered))
    }

    fn lower_type_constraint_handle(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        constraint: &syntax::types::TypeConstraintNode,
    ) -> Result<TypeConstraint, Diagnostic> {
        match constraint {
            syntax::types::TypeConstraintNode::Named(name) => {
                Ok(TypeConstraint::Named(lower_name(syntax_trees, name)))
            }
            syntax::types::TypeConstraintNode::Domain(domain) => {
                let arguments =
                    lower_indexed_application_arguments(lowerer, syntax_trees, domain.arguments)?;
                Ok(TypeConstraint::Domain(DomainConstraint {
                    name: lower_name(syntax_trees, &domain.name),
                    arguments,
                }))
            }

            syntax::types::TypeConstraintNode::ArithmeticDomain(domain) => {
                Ok(TypeConstraint::ArithmeticDomain(*domain))
            }
        }
    }

    /*
        Lower one indexed application's argument positions: every argument becomes
        a child type reference, and the const-argument selections the arguments
        retain receive the slot custody that ties each value to the parameter that
        received it. Ordinary data applications, indexed domain constraints and
        membership facts all read their arguments through this one operation.
    */
    pub(crate) fn lower_indexed_application_arguments(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        arguments: HandleSpan<syntax::types::TypeReferenceHandle>,
    ) -> Result<HandleSpan<TypeReference>, Diagnostic> {
        let selection_start = lowerer.pending_const_argument_selections.len();
        let lowered = lower_child_type_references(lowerer, syntax_trees, arguments)?;
        retain_const_argument_slots(lowerer, syntax_trees, arguments, lowered, selection_start);
        Ok(lowered)
    }

    /*
        Both ordinary data applications and indexed domain constraints own their
        exact argument spans. Carry that relationship privately through resolution;
        source names and equal encoded atoms cannot reconstruct a receiving slot.
    */
    pub(super) fn retain_const_argument_slots(
        lowerer: &mut Lowerer,
        syntax_trees: &SyntaxTrees,
        arguments: HandleSpan<syntax::types::TypeReferenceHandle>,
        lowered_arguments: HandleSpan<TypeReference>,
        selection_start: usize,
    ) {
        for (ordinal, argument) in syntax_type_references(syntax_trees)
            .type_reference_handles(arguments)
            .iter()
            .enumerate()
        {
            let Some(normalization) =
                syntax_type_references(syntax_trees).const_argument_normalization(*argument)
            else {
                continue;
            };
            for selected in syntax_type_references(syntax_trees)
                .const_argument_origins(normalization.selections)
            {
                for (selection, pending) in lowerer
                    .pending_const_argument_selections
                    .iter()
                    .enumerate()
                    .skip(selection_start)
                {
                    if pending.origin == *selected {
                        lowerer.pending_const_argument_slots.push(
                            crate::resolution::lowerer::PendingConstArgumentSlot {
                                selection,
                                arguments: lowered_arguments,
                                ordinal,
                            },
                        );
                    }
                }
            }
        }
    }
}
pub(crate) mod wire {
    /* Wire schema derivation for lowered data definitions. */

    use crate::resolution::lowerer::Lowerer;
    use symbol_resolved_trees::data::{DataDefinition, DataMember, DataProperties};
    use symbol_resolved_trees::wire::{WireField, WireMember, WireReserved, WireSchema};

    /*
        Derive the current record codec's view from the ordinary declaration.

        A codec view exists only for records the generated codec can check against
        the public wire requirement end to end. The view is withheld for any
        declaration carrying an obligation the codec cannot establish:
        whole-record domain (`requires` facts, which cover every `zero_gated` and
        membership-gated carrier), declared properties, lifetime or type
        parameters, variant members, fields without `#n` identities, bodyless
        forms (quotient and opaque `boundary data` both lower with no members),
        and records whose supply mode is not `CheckedShape`. Those declarations
        still lower normally; only this consumer-specific view is withheld.
        Generic codec selection remains a separate consumer requirement.
    */
    pub(crate) fn derive_wire_schema(lowerer: &mut Lowerer, definition: &DataDefinition) {
        if !definition.type_parameters.is_empty()
            || !definition.lifetime_parameters.is_empty()
            || definition.properties != DataProperties::default()
            || !definition.requires_facts.is_empty()
            || definition.supply_mode != language_core::DataSupplyMode::CheckedShape
        {
            return;
        }
        let fields = lowerer
            .symbol_resolved_trees
            .data_members(definition.members);
        if fields.iter().any(|member| match member {
            DataMember::Field(field) => field.identity.is_none(),
            DataMember::Variant(_) => true,
        }) || (fields.is_empty() && definition.retired_identities.is_empty())
        {
            return;
        }
        let mut members = Vec::with_capacity(fields.len() + definition.retired_identities.len());
        for member in fields {
            if let DataMember::Field(field) = member {
                let Some(number) = field.identity else {
                    return;
                };
                members.push(WireMember::Field(WireField {
                    number,
                    name: field.name.clone(),
                    relevance: field.relevance,
                    type_reference: field.type_reference.clone(),
                }));
            }
        }
        members.extend(
            definition
                .retired_identities
                .iter()
                .map(|number| WireMember::Reserved(WireReserved { number: *number })),
        );
        let members = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .wire_members
            .insert_many(members);
        lowerer.symbol_resolved_trees.wire_schemas.push(WireSchema {
            symbol: Default::default(),
            name: definition.name.clone(),
            is_public: definition.is_public,
            encoding: None,
            members,
        });
    }
}

use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;

/* Translate every root item into the lowerer's trees, in source order. */
pub(crate) fn lower_items(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    for &item in syntax.root_item_handles() {
        item::lower_item(lowerer, syntax, item).map_err(|diagnostic| vec![diagnostic])?;
    }
    Ok(())
}
