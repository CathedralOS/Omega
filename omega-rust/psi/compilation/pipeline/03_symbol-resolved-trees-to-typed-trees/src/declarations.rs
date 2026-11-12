/*
Declaration-family typing (`declarations`): data definitions, domains,
machines, states, traits, conformances, operators, measures, propositions,
mathematical definitions, provider defaults, type aliases, and wire schemas.
*/

pub(crate) mod conformance {
    use crate::lowerer::name::{lower_name, lower_names};
    use crate::lowerer::{Lowerer, declaration_exposure};
    use crate::signatures::type_parameters::lower_type_parameters;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::trait_definition::Conformance;

    pub(crate) fn lower_conformance(
        lowerer: &mut Lowerer,
        conformance: &Conformance,
    ) -> Result<(), Diagnostic> {
        let symbol_resolved_trees = lowerer.source_trees;
        let conformance_exposure = declaration_exposure(conformance.is_public);
        if symbol_resolved_trees
            .roots
            .trait_definition(conformance.trait_symbol)
            .is_some_and(|definition| definition.refines.is_some())
        {
            return Err(Diagnostic::error(format!(
                "conformance target `{}` is a transparent refinement — a refinement bounds existing conformance evidence, it is not implemented directly",
                conformance.trait_name.as_str(),
            )));
        }
        crate::type_reference::retain_type_reference_selection(
        symbol_resolved_trees,
        &mut lowerer.typed_trees,
        &conformance.trait_name,
        conformance.trait_symbol,
        conformance_exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
    )?;
        let source_type_parameters = conformance.type_parameters;
        let mut arguments = HandleSpan::empty();
        for argument in symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .span_or_empty(conformance.arguments)
        {
            let argument = lowerer
                .with_type_reference_exposure(conformance_exposure, |lowerer| {
                    crate::type_reference::lower_type_reference_into_table(lowerer, argument)
                })?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        let trait_lifetime_arguments = conformance
        .trait_lifetime_arguments
        .iter()
        .map(|argument| {
            let ordinal = conformance
                .lifetime_parameters
                .iter()
                .position(|parameter| parameter == argument)
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "conformance `{}` target trait lifetime `'{}' does not name an in-scope conformance lifetime binder",
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<unnamed-conformance>", |name| name.as_str()),
                        argument.as_str(),
                    ))
                })?;
            u32::try_from(ordinal).map_err(|_| {
                Diagnostic::error(format!(
                    "conformance `{}` target trait lifetime ordinal exceeds the compiler limit",
                    conformance
                        .alias
                        .as_ref()
                        .map_or("<unnamed-conformance>", |name| name.as_str()),
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
        let mut conformance = typed_trees::trait_definition::Conformance {
            symbol: conformance.symbol,
            is_public: conformance.is_public,
            lifetime_parameters: lower_names(&conformance.lifetime_parameters),
            type_parameters: HandleSpan::empty(),
            trait_name: lower_name(&conformance.trait_name),
            trait_symbol: conformance.trait_symbol,
            trait_lifetime_arguments,
            arguments,
            alias: conformance.alias.as_ref().map(lower_name),
            rows: conformance
                .rows
                .iter()
                .map(|row| {
                    typed_trees::trait_definition::ConformanceRow {
                declaring_trait: row.declaring_trait,
                declaring_trait_name: lower_name(&row.declaring_trait_name),
                requirement: row.requirement,
                requirement_name: lower_name(&row.requirement_name),
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
                realization_name: lower_name(&row.realization_name),
                source: match row.source {
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline => {
                        typed_trees::trait_definition::ConformanceRowSource::Inline
                    }
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference => {
                        typed_trees::trait_definition::ConformanceRowSource::Reference
                    }
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                        typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                    }
                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Synthesized => {
                        typed_trees::trait_definition::ConformanceRowSource::Synthesized
                    }
                },
            }
                })
                .collect(),
            value_identity: Default::default(),
        };
        conformance.type_parameters = lowerer
            .with_type_reference_exposure(conformance_exposure, |lowerer| {
                lower_type_parameters(lowerer, source_type_parameters)
            })?;
        conformance.value_identity = lowerer
            .typed_trees
            .implementation_value_identity(&conformance);
        lowerer.typed_trees.push_conformance(conformance);
        Ok(())
    }
}
pub(crate) mod data {
    use crate::contracts::proof_facts::lower_proof_facts;
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::{lower_name, lower_names};
    use crate::signatures::type_parameters::lower_type_parameters;
    use crate::type_reference::{lower_type_reference_into_table, retain_static_path_selection};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed::data::DataField;
    use typed_trees as typed;

    pub(crate) fn lower_data_definition(
        lowerer: &mut Lowerer,
        data_definition: &resolved::data::DataDefinition,
    ) -> Result<typed::data::DataDefinition, Diagnostic> {
        let exposure = data_definition
            .generic_instance
            .is_none()
            .then_some(lowerer.type_reference_exposure)
            .flatten();
        lowerer.with_type_reference_exposure(exposure, |lowerer| {
            lower_data_definition_contents(lowerer, data_definition)
        })
    }

    fn lower_data_definition_contents(
        lowerer: &mut Lowerer,
        data_definition: &resolved::data::DataDefinition,
    ) -> Result<typed::data::DataDefinition, Diagnostic> {
        let mut typed_data_definition = typed::data::DataDefinition {
        symbol: data_definition.symbol,
        name: lower_name(&data_definition.name),
        is_public: data_definition.is_public,
        predicate_subject_symbol: data_definition.predicate_subject_symbol,
        predicate_subject_type: if data_definition.predicate_subject_symbol.is_valid() {
            lowerer.with_type_reference_exposure(None, |lowerer| {
                lower_type_reference_into_table(lowerer, &data_definition.predicate_subject_type)
            })?
        } else {
            typed::types::TypeReferenceHandle::invalid()
        },
        supply_mode: data_definition.supply_mode,
        lifetime_parameters: lower_names(&data_definition.lifetime_parameters),
        type_parameters: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        generic_instance: data_definition
            .generic_instance
            .as_ref()
            .map(|origin| lower_type_reference_into_table(lowerer, origin))
            .transpose()?,
        cleanup_selection: None,
        properties: typed::data::DataProperties {
            carry: data_definition.properties.carry,
            multiplicity: data_definition.properties.multiplicity,
        },
        quotient: data_definition
            .quotient
            .as_ref()
            .map(|quotient| {
                retain_static_path_selection(
                    &mut lowerer.typed_trees,
                    &quotient.relation,
                    quotient.relation_symbol,
                    lowerer.type_reference_exposure,
                    "quotient relation",
                )?;
                if let Some(selection) = &quotient.equivalence {
                    retain_static_path_selection(
                        &mut lowerer.typed_trees,
                        &selection.relation,
                        selection.relation_symbol,
                        lowerer.type_reference_exposure,
                        "quotient equivalence subject",
                    )?;
                    crate::type_reference::retain_type_reference_selection(
                        lowerer.source_trees,
                        &mut lowerer.typed_trees,
                        &selection.trait_name,
                        selection.trait_symbol,
                        lowerer.type_reference_exposure,
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
                    )?;
                }
                Ok::<_, Diagnostic>(typed::data::QuotientDefinition {
                    carrier: lower_type_reference_into_table(lowerer, &quotient.carrier)?,
                    relation: lower_names(&quotient.relation),
                    relation_symbol: quotient.relation_symbol,
                    equivalence: quotient
                        .equivalence
                        .as_ref()
                        .map(|selection| {
                            let trait_arguments = lowerer
                                .source_trees
                                .child_type_references(selection.trait_arguments)
                                .iter()
                                .map(|argument| lower_type_reference_into_table(lowerer, argument))
                                .collect::<Result<Vec<_>, _>>()?;
                            Ok::<_, Diagnostic>(typed::data::QuotientEquivalenceSelection {
                                relation: lower_names(&selection.relation),
                                relation_symbol: selection.relation_symbol,
                                trait_name: lower_name(&selection.trait_name),
                                trait_symbol: selection.trait_symbol,
                                trait_arguments: lowerer
                                    .typed_trees
                                    .type_reference_table
                                    .insert_type_reference_handles(trait_arguments),
                                conformance_name: lower_name(
                                    &selection.conformance_name,
                                ),
                                conformance_symbol: selection.conformance_symbol,
                            })
                        })
                        .transpose()?,
                })
            })
            .transpose()?,
        requires_facts: lower_proof_facts(lowerer, data_definition.requires_facts)?,
        zero_gated: data_definition.zero_gated,
        retired_identities: data_definition.retired_identities.clone(),
        members: HandleSpan::empty(),
    };

        typed_data_definition.conformance_bounds =
            crate::signatures::evidence::lower_conformance_bounds(
                lowerer,
                &data_definition.conformance_bounds,
            )?;
        typed_data_definition.type_parameters =
            lower_type_parameters(lowerer, data_definition.type_parameters)?;

        if let Some(selection) = &data_definition.cleanup_selection {
            let subject = if let Some(origin) = typed_data_definition.generic_instance {
                origin
            } else if typed_data_definition.type_parameters.is_empty() {
                lowerer.typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Named {
                        symbol: typed_data_definition.symbol,
                        name: typed_data_definition.name.clone(),
                    },
                )
            } else {
                let parameters = lowerer
                    .typed_trees
                    .data_type_parameters(&typed_data_definition)
                    .to_vec();
                let arguments = parameters
                    .iter()
                    .map(|parameter| {
                        lowerer.typed_trees.type_reference_table.insert(
                            typed::types::TypeReferenceNode::Named {
                                symbol: parameter.symbol,
                                name: parameter.name.clone(),
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                let arguments = lowerer
                    .typed_trees
                    .type_reference_table
                    .insert_type_reference_handles(arguments);
                lowerer.typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Generic {
                        base_symbol: typed_data_definition.symbol,
                        base_name: typed_data_definition.name.clone(),
                        lifetime_arguments: typed_data_definition.lifetime_parameters.clone(),
                        arguments,
                    },
                )
            };
            typed_data_definition.cleanup_selection = Some(super::domain::lower_cleanup_selection(
                lowerer,
                selection,
                subject,
                typed_data_definition.symbol,
            )?);
        }
        for member in lowerer.source_trees.data_members(data_definition.members) {
            let member = lower_data_member(lowerer, member)?;
            lowerer
                .typed_trees
                .push_data_member(&mut typed_data_definition, member);
        }

        Ok(typed_data_definition)
    }

    fn lower_data_member(
        lowerer: &mut Lowerer,
        member: &resolved::data::DataMember,
    ) -> Result<typed::data::DataMember, Diagnostic> {
        match member {
            resolved::data::DataMember::Field(field) => {
                Ok(typed::data::DataMember::Field(DataField {
                    identity: field.identity,
                    symbol: field.symbol,
                    name: lower_name(&field.name),
                    relevance: field.relevance,
                    type_reference: lower_type_reference_into_table(
                        lowerer,
                        &field.type_reference,
                    )?,
                }))
            }
            resolved::data::DataMember::Variant(variant) => {
                let mut typed_variant = typed::data::DataVariant {
                    identity: variant.identity,
                    symbol: variant.symbol,
                    name: lower_name(&variant.name),
                    payload: HandleSpan::empty(),
                    requires_facts: lower_proof_facts(lowerer, variant.requires_facts)?,
                    retired_payload_identities: variant.retired_payload_identities.clone(),
                };
                let payload_fields = lowerer
                    .source_trees
                    .data_payload_fields(variant.payload)
                    .to_vec();
                for field in &payload_fields {
                    let lowered = DataField {
                        identity: field.identity,
                        symbol: field.symbol,
                        name: lower_name(&field.name),
                        relevance: field.relevance,
                        type_reference: lower_type_reference_into_table(
                            lowerer,
                            &field.type_reference,
                        )?,
                    };
                    lowerer
                        .typed_trees
                        .push_data_payload_field(&mut typed_variant, lowered);
                }
                Ok(typed::data::DataMember::Variant(typed_variant))
            }
        }
    }
}
pub(crate) mod domain {
    use crate::contracts::proof_facts::lower_proof_facts;
    use crate::declarations::operator::lower_operator_definition;
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::lower_name;
    use crate::tables::expression_table_mut;
    use crate::type_reference::lower_type_reference_into_table;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed::static_applications::StaticMachineLifetimeBinding as LifetimeBinding;
    use typed_trees as typed;

    pub(crate) fn lower_domain_definition(
        lowerer: &mut Lowerer,
        domain: &resolved::domain::DomainDefinition,
    ) -> Result<typed::domain::DomainDefinition, Diagnostic> {
        let facts = lower_proof_facts(lowerer, domain.facts)?;
        let alias = if let Some(alias) = domain.alias.as_ref() {
            let constituents = alias
                .constituents
                .iter()
                .map(|constituent| -> Result<_, Diagnostic> {
                    let mut path = HandleSpan::empty();
                    let members = lowerer.source_trees.domain_path_members(constituent.domain);
                    crate::type_reference::retain_static_path_selection(
                        &mut lowerer.typed_trees,
                        members,
                        constituent.domain_symbol,
                        lowerer.type_reference_exposure,
                        "domain-alias",
                    )?;
                    for member in members {
                        lowerer
                            .typed_trees
                            .domain_path_members
                            .append_to_span(&mut path, lower_name(member));
                    }
                    Ok(typed::domain::DomainAliasConstituent {
                        domain: path,
                        domain_symbol: constituent.domain_symbol,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Some(typed::domain::DomainAliasDefinition { constituents })
        } else {
            None
        };
        let target_type = lower_type_reference_into_table(lowerer, &domain.target_type)?;
        let mut typed_domain = typed::domain::DomainDefinition {
            symbol: domain.symbol,
            name: lower_name(&domain.name),
            subject_name: lower_name(&domain.subject_name),
            subject_symbol: domain.subject_symbol,
            type_parameters: HandleSpan::empty(),
            target_type,
            index_arguments: lowerer
                .source_trees
                .child_type_references(domain.index_arguments)
                .iter()
                .map(|argument| lower_type_reference_into_table(lowerer, argument))
                .collect::<Result<Vec<_>, _>>()?,
            is_public: domain.is_public,
            alias,
            cleanup_selection: domain
                .cleanup_selection
                .as_ref()
                .map(|selection| {
                    lower_cleanup_selection(lowerer, selection, target_type, domain.symbol)
                })
                .transpose()?,
            classification: domain.classification,
            predicate_presence: domain.predicate_presence,
            facts,
            operators: Default::default(),
            semantic_clause_token_count: domain.semantic_clause_token_count,
            semantic_id: domain.semantic_id,
            semantic_roles: domain.semantic_roles,
            establishment_routes: domain.establishment_routes.clone(),
        };

        typed_domain.type_parameters = crate::signatures::type_parameters::lower_type_parameters(
            lowerer,
            domain.type_parameters,
        )?;

        for operator in lowerer.source_trees.operator_definitions(domain.operators) {
            let operator = lowerer.with_type_reference_exposure(
            if operator.is_public {
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            } else {
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
            },
            |lowerer| lower_operator_definition(lowerer, operator),
        )?;
            lowerer
                .typed_trees
                .push_domain_operator(&mut typed_domain, operator);
        }

        Ok(typed_domain)
    }

    pub(crate) fn lower_cleanup_selection(
        lowerer: &mut Lowerer,
        selection: &resolved::expression::StaticMachineArgument,
        subject: typed::types::TypeReferenceHandle,
        owner: symbols::SymbolHandle,
    ) -> Result<typed::domain::CleanupSelection, Diagnostic> {
        let authorization = crate::type_reference::retain_path_selection_occurrence(
        &mut lowerer.typed_trees,
        &selection.path,
        selection.symbol,
        lowerer.type_reference_exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainCleanupSelection,
        "domain cleanup",
    )?;
        let selected = crate::expressions::expression::lower_static_machine_argument(
            Some(lowerer.source_trees),
            &mut lowerer.typed_trees,
            lowerer.type_reference_exposure,
            selection,
        )?;
        let arguments = selected
            .application
            .as_ref()
            .map_or(&[][..], |application| &application.arguments);
        let application = expression_table_mut(&mut lowerer.typed_trees)
            .retain_static_machine_application(
                selected.symbol,
                selected.path.last().cloned().unwrap_or_default(),
                arguments,
            );
        let row = expression_table_mut(&mut lowerer.typed_trees)
            .static_machine_application_mut(application);
        row.lexical_owner = owner;
        row.binding_declaration = selected.symbol;
        if let Some(authored) = &selected.application {
            let target = lowerer
                .source_trees
                .machines
                .iter()
                .find(|machine| machine.symbol == selected.symbol);
            let parameters = target.map_or(&[][..], |machine| &machine.lifetime_parameters);
            if !authored.lifetime_arguments.is_empty()
                && authored.lifetime_arguments.len() != parameters.len()
            {
                return Err(Diagnostic::error(
                    "cleanup application lifetime argument arity does not match its selected machine",
                ));
            }
            let lifetimes = parameters
                .iter()
                .zip(authored.lifetime_arguments.iter())
                .map(|(parameter, argument)| LifetimeBinding {
                    parameter: crate::lowerer::name::lower_name(parameter),
                    argument: argument.clone(),
                })
                .collect::<Vec<_>>();
            expression_table_mut(&mut lowerer.typed_trees)
                .set_static_machine_application_lifetimes(application, selected.symbol, &lifetimes);
        }
        Ok(typed::domain::CleanupSelection {
            application,
            subject,
            authorization,
            executable: symbols::SymbolHandle::invalid(),
        })
    }
}
pub(crate) mod machine {
    use crate::access_borrow_factory::access_borrow_factory;
    use crate::contracts::invocations::lower_authored_invocations;
    use crate::declarations::state::{complete_state_body, lower_state_declaration};
    use crate::expressions::equatable::EqualityScope;
    use crate::expressions::expression::lower_expression_handle;
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::{lower_name, lower_names};
    use crate::signatures::type_parameters::{lower_type_parameter, lower_type_parameters};
    use crate::tables::resolved_expressions;
    use crate::type_reference::{lower_type_reference_into_table, retain_type_reference_selection};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::MachineSupplyMode;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
    use symbol_resolved_trees as resolved;
    use symbols::SymbolHandle;
    use typed_trees as typed;

    pub(crate) mod generic_data {
        use diagnostics::Diagnostic;
        use symbol_resolved_trees::{SymbolResolvedTrees, machine::Machine, types::TypeReference};

        pub(crate) fn is_derived(
            program: &SymbolResolvedTrees,
            machine: &Machine,
        ) -> Result<bool, Diagnostic> {
            let origin = &machine.generic_data_origin;
            if !origin.template_source.is_source_backed()
                && !origin.template.is_valid()
                && !origin.closed_owner.is_valid()
            {
                return Ok(false);
            }
            let template = program.machine(origin.template);
            let owner = program.data_definition_by_symbol(origin.closed_owner);
            let valid = origin.template_source.is_source_backed()
        && origin.template.is_valid()
        && origin.closed_owner.is_valid()
        && machine.attached_data_symbol == origin.closed_owner
        && template.zip(owner).is_some_and(|(template, owner)| {
            template.symbol != machine.symbol
                && !template.generic_data_origin.template.is_valid()
                && template.name.source_span() == origin.template_source.source_span()
                && machine.name.source_span() == template.name.source_span()
                && machine.is_public == template.is_public
                && machine.supply_mode == template.supply_mode
                && machine.type_parameters.is_empty()
                && matches!(&owner.generic_instance, Some(TypeReference::Generic(application))
                    if application.base_symbol == template.attached_data_symbol)
        });
            if valid {
                Ok(true)
            } else {
                Err(Diagnostic::error(
                    "generic-data method lost its exact template and closed-owner derivation",
                )
                .with_source_span(machine.name.source_span()))
            }
        }
    }

    pub(crate) fn lower_machine(
        lowerer: &mut Lowerer,
        machine: &resolved::machine::Machine,
    ) -> Result<typed::machine::Machine, Diagnostic> {
        if let Some(factory) = access_borrow_factory(lowerer.source_trees, machine.symbol)?
            && !lowerer
                .typed_trees
                .access_borrow_factories
                .contains(&factory)
        {
            lowerer.typed_trees.access_borrow_factories.push(factory);
        }
        let derived = generic_data::is_derived(lowerer.source_trees, machine)?;
        let exposure = if derived {
            None
        } else {
            lowerer.type_reference_exposure
        };
        lowerer.with_type_reference_exposure(exposure, |lowerer| {
            lower_machine_contents(lowerer, machine)
        })
    }

    pub(crate) fn lower_token_binding_view(
        lowerer: &mut Lowerer,
        machine: &resolved::machine::Machine,
        typed_machine: &typed::machine::Machine,
    ) -> Result<Option<typed::operator::OperatorDefinition>, Diagnostic> {
        let Some(spelling) = typed_machine.spelling else {
            return Ok(None);
        };
        if generic_data::is_derived(lowerer.source_trees, machine)? {
            return Ok(None);
        }
        let entry = lowerer
            .typed_trees
            .machine_states(typed_machine)
            .first()
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "`{}` binds the fixed operator token `{}` but declares no entry state",
                    typed_machine.name,
                    spelling.symbol()
                ))
            })?;
        let mut view = typed::operator::OperatorDefinition {
            is_public: typed_machine.is_public,
            is_boundary: matches!(
                typed_machine.supply_mode,
                MachineSupplyMode::Boundary | MachineSupplyMode::TopLevelRequirement
            ),
            symbol: typed_machine.symbol,
            name: Default::default(),
            lifetime_parameters: typed_machine.lifetime_parameters.clone(),
            type_parameters: typed_machine.type_parameters,
            parameters: entry.parameters,
            return_type: entry.return_type,
            contracts: typed_machine.contracts,
            spelling: Some(spelling),
            token_count: 0,
            home_domain: attached_domain_symbol(lowerer, typed_machine),
        };
        let leaf = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or_default()
            .to_owned();
        if let Some(attached_data) = &typed_machine.attached_data {
            lowerer
                .typed_trees
                .push_operator_path_member(&mut view, attached_data.clone());
        }
        lowerer
            .typed_trees
            .push_operator_path_member(&mut view, typed::name::Identifier::generated(&leaf));
        Ok(Some(view))
    }

    fn attached_domain_symbol(
        lowerer: &Lowerer,
        typed_machine: &typed::machine::Machine,
    ) -> SymbolHandle {
        if typed_machine.attached_data_symbol.is_valid() {
            return SymbolHandle::invalid();
        }
        let Some(attached) = typed_machine.attached_data.as_ref() else {
            return SymbolHandle::invalid();
        };
        let mut matches = lowerer
            .typed_trees
            .domain_definitions()
            .iter()
            .filter(|domain| domain.name.as_str() == attached.as_str());
        match (matches.next(), matches.next()) {
            (Some(domain), None) => domain.symbol,
            _ => SymbolHandle::invalid(),
        }
    }

    fn retain_attached_data_type_reference(lowerer: &mut Lowerer, symbol: SymbolHandle) {
        if !symbol.is_valid()
            || lowerer
                .typed_trees
                .type_reference_table
                .find_named_type_reference(symbol)
                .is_some()
        {
            return;
        }
        let Some(owner) = lowerer.source_trees.data_definition_by_symbol(symbol) else {
            return;
        };
        if !owner.type_parameters.is_empty() {
            return;
        }
        let name = lower_name(&owner.name);
        lowerer
            .typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named { symbol, name });
    }

    fn lower_machine_contents(
        lowerer: &mut Lowerer,
        machine: &resolved::machine::Machine,
    ) -> Result<typed::machine::Machine, Diagnostic> {
        if let Some(attached_data) = &machine.attached_data {
            retain_type_reference_selection(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                attached_data,
                machine.attached_data_symbol,
                lowerer.type_reference_exposure,
                AuthoredDeclarationSelectionKind::TypeReference,
            )?;
        }
        retain_attached_data_type_reference(lowerer, machine.attached_data_symbol);
        let mut typed_machine = typed::machine::Machine {
            lexical_region_origin: None,
            assembly_target_requirement:
                language_core::inline_assembly::AsmTargetApplicability::Any,
            symbol: machine.symbol,
            name: lower_name(&machine.name),
            attached_data: machine.attached_data.as_ref().map(lower_name),
            attached_data_symbol: machine.attached_data_symbol,
            attached_data_application: typed::types::TypeReferenceHandle::invalid(),
            generic_data_template: machine.generic_data_origin.template,
            spelling: machine.spelling,
            is_public: machine.is_public,
            interface_exposure: machine.interface_exposure,
            supply_mode: machine.supply_mode,
            purpose: typed::machine::MachinePurpose::Runtime,
            target: machine.target.as_ref().map(lower_name),
            authored_target: machine.authored_target.as_ref().map(lower_name),
            body_is_present: machine.body_is_present,
            target_family_interface: machine.target_family_interface,
            body_status: typed::machine::MachineBodyStatus::Deferred,
            structural_type_equations_pending: machine.structural_type_equations_pending,
            termination_plan: machine.termination_plan.clone(),
            service_reach_row: machine.service_reach_row,
            service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
            admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
            declares_no_crash_route: machine.declares_no_crash_route,
            suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
            lifetime_parameters: lower_names(&machine.lifetime_parameters),
            type_parameters: HandleSpan::empty(),
            declared_type_equations: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            invokes: HandleSpan::empty(),
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        };

        typed_machine.termination_plan.implementation_witness = None;
        typed_machine.type_parameters = lower_type_parameters(lowerer, machine.type_parameters)?;
        let inherited =
            crate::signatures::lexical_context::enclosing_parameters(lowerer.source_trees, machine);
        if !inherited.is_empty() {
            let mut parameters = inherited
                .into_iter()
                .map(|parameter| lower_type_parameter(lowerer, parameter))
                .collect::<Result<Vec<_>, _>>()?;
            parameters.extend_from_slice(
                lowerer
                    .typed_trees
                    .data_type_parameters
                    .span_or_empty(typed_machine.type_parameters),
            );
            typed_machine.type_parameters = lowerer
                .typed_trees
                .data_type_parameters
                .insert_many(parameters);
        }
        let mut equations = Vec::with_capacity(machine.declared_type_equations.len());
        for equation in lowerer
            .source_trees
            .tables
            .declarations
            .declared_type_equations
            .span_or_empty(machine.declared_type_equations)
        {
            equations.push(typed::machine::DeclaredTypeEquation {
                parameter: lower_type_reference_into_table(lowerer, &equation.parameter)?,
                definition: lower_type_reference_into_table(lowerer, &equation.definition)?,
            });
        }
        typed_machine.declared_type_equations = lowerer
            .typed_trees
            .declared_type_equations
            .insert_many(equations);

        if let Some(application) = &machine.attached_data_application {
            typed_machine.attached_data_application =
                lower_type_reference_into_table(lowerer, application)?;
        }

        for owned_data in lowerer.source_trees.machine_owned_data(machine.owned_data) {
            let owned_data = lowerer.with_type_reference_exposure(
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                |lowerer| {
                    Ok::<_, Diagnostic>(typed::machine::OwnedData {
                        symbol: owned_data.symbol,
                        name: lower_name(&owned_data.name),
                        type_reference: lower_type_reference_into_table(
                            lowerer,
                            &owned_data.type_reference,
                        )?,
                        initial_value: typed::expression::ExpressionHandle::invalid(),
                    })
                },
            )?;
            lowerer
                .typed_trees
                .push_machine_owned_data(&mut typed_machine, owned_data);
        }

        for conformance in lowerer
            .source_trees
            .machine_trait_conformances(machine.satisfies)
        {
            retain_type_reference_selection(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                &conformance.name,
                conformance.symbol,
                lowerer.type_reference_exposure,
                AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            let mut arguments = HandleSpan::empty();
            for argument in lowerer
                .source_trees
                .child_type_references(conformance.arguments)
            {
                let argument =
                    crate::type_reference::lower_type_reference_into_table(lowerer, argument)?;
                lowerer
                    .typed_trees
                    .type_reference_table
                    .push_type_reference_handle(&mut arguments, argument);
            }
            let trait_lifetime_arguments = conformance
            .lifetime_arguments
            .iter()
            .map(|argument| {
                machine
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter.as_str() == argument.as_str())
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "machine `{}` satisfies `{}` with lifetime `'{}'` outside its lifetime telescope",
                            machine.name, conformance.name, argument,
                        ))
                        .with_source_span(argument.source_span())
                    })
                    .and_then(|ordinal| {
                        u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(format!(
                                "machine `{}` lifetime telescope exceeds the supported ordinal range",
                                machine.name,
                            ))
                            .with_source_span(argument.source_span())
                        })
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
            let via_expression = typed::expression::ExpressionHandle::invalid();
            lowerer.typed_trees.push_machine_trait_conformance(
                &mut typed_machine,
                typed::machine::TraitConformance {
                    symbol: conformance.symbol,
                    name: lower_name(&conformance.name),
                    trait_lifetime_arguments,
                    arguments,
                    requirement: conformance.requirement.as_ref().map(lower_name),
                    requirement_symbol: SymbolHandle::invalid(),
                    requirement_source_span: conformance
                        .requirement
                        .as_ref()
                        .filter(|requirement| requirement.is_source_backed())
                        .map(|requirement| requirement.source_span()),
                    alias: conformance.alias.as_ref().map(lower_name),
                    external_binding: conformance.external_binding,
                    via_expression,
                    external_binding_source_span: conformance.external_binding_source_span,
                },
            );
        }

        let source = lowerer.source_trees;
        typed_machine.conformance_bounds = crate::signatures::evidence::lower_conformance_bounds(
            lowerer,
            crate::signatures::lexical_context::enclosing_bounds(source, machine)
                .chain(&machine.conformance_bounds),
        )?;

        let invocation_parameters = lowerer
            .source_trees
            .machine_state_handles(machine.states)
            .first()
            .map(|state| {
                lowerer
                    .source_trees
                    .state_parameters(lowerer.source_trees.machine_state(*state).parameters)
            })
            .unwrap_or_default();
        let invocations = lower_authored_invocations(
            lowerer.source_trees,
            lowerer.source_trees.machine_invokes(machine),
            invocation_parameters,
            machine.name.as_str(),
        )?;
        for invocation in invocations {
            lowerer
                .typed_trees
                .push_machine_invoke(&mut typed_machine, invocation);
        }

        for contract in lowerer.source_trees.machine_contracts(machine) {
            let contract =
                crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
            lowerer
                .typed_trees
                .push_machine_contract(&mut typed_machine, contract);
        }

        for (state_index, state) in lowerer
            .source_trees
            .machine_state_handles(machine.states)
            .iter()
            .enumerate()
        {
            let state = lowerer.source_trees.machine_state(*state);
            lowerer.equality_scope = Some(EqualityScope::for_state_interface(
                lowerer.source_trees,
                machine,
                state,
            ));
            let exposure = lowerer.type_reference_exposure.map(|_| {
                if state_index == 0 {
                    machine.interface_exposure
                } else {
                    AuthoredDeclarationSelectionExposure::PrivateImplementation
                }
            });
            let state = lowerer.with_type_reference_exposure(exposure, |lowerer| {
                lower_state_declaration(lowerer, state)
            })?;
            lowerer.equality_scope = None;
            lowerer
                .typed_trees
                .push_machine_state(&mut typed_machine, state);
        }

        let ranking_exposure = lowerer
            .type_reference_exposure
            .map(|_| AuthoredDeclarationSelectionExposure::PrivateImplementation);
        lowerer.with_type_reference_exposure(ranking_exposure, |lowerer| {
            retain_machine_ranking_expressions(lowerer, machine)
        })?;

        if machine.body_status == resolved::machine::MachineBodyStatus::Complete {
            complete_machine_body(lowerer, machine, &mut typed_machine)?;
        }
        Ok(typed_machine)
    }

    fn retain_machine_ranking_expressions(
        lowerer: &mut Lowerer,
        machine: &resolved::machine::Machine,
    ) -> Result<(), Diagnostic> {
        let ranking_subjects = resolved_expressions(lowerer.source_trees)
            .expression_handles(machine.ranking_subjects)
            .iter()
            .map(|decrease| lower_expression_handle(lowerer, *decrease))
            .collect::<Result<Vec<_>, _>>()?;
        let ranking_view_arguments = resolved_expressions(lowerer.source_trees)
            .expression_handles(machine.ranking_view_arguments)
            .iter()
            .map(|argument| lower_expression_handle(lowerer, *argument))
            .collect::<Result<Vec<_>, _>>()?;
        let ranking_range = machine
            .ranking_range
            .is_valid()
            .then(|| lower_expression_handle(lowerer, machine.ranking_range))
            .transpose()?;
        if !ranking_subjects.is_empty()
            || !ranking_view_arguments.is_empty()
            || ranking_range.is_some()
        {
            lowerer.typed_trees.ranking_expression_custody.push(
                typed::ranking::RankingExpressionCustody {
                    machine: machine.symbol,
                    subjects: ranking_subjects,
                    view_arguments: ranking_view_arguments,
                    rank_range: ranking_range,
                },
            );
        }

        Ok(())
    }

    pub(crate) fn complete_machine_body(
        lowerer: &mut Lowerer,
        machine: &resolved::machine::Machine,
        typed_machine: &mut typed::machine::Machine,
    ) -> Result<(), Diagnostic> {
        let exposure = lowerer
            .type_reference_exposure
            .map(|_| AuthoredDeclarationSelectionExposure::PrivateImplementation);
        lowerer.with_type_reference_exposure(exposure, |lowerer| {
            lower_machine_implementation(lowerer, machine, typed_machine)
        })
    }

    fn lower_machine_implementation(
        lowerer: &mut Lowerer,
        machine: &resolved::machine::Machine,
        typed_machine: &mut typed::machine::Machine,
    ) -> Result<(), Diagnostic> {
        if machine.body_status != resolved::machine::MachineBodyStatus::Complete
            || typed_machine.body_status != typed::machine::MachineBodyStatus::Deferred
            || machine.symbol != typed_machine.symbol
        {
            return Err(Diagnostic::error(
                "machine body completion does not match its deferred declaration",
            ));
        }
        let owned = lowerer.source_trees.machine_owned_data(machine.owned_data);
        if owned.len() != lowerer.typed_trees.machine_owned_data(typed_machine).len() {
            return Err(Diagnostic::error(
                "machine body completion changed owned declarations",
            ));
        }
        for (ordinal, source) in owned.iter().enumerate() {
            let retained = &lowerer.typed_trees.machine_owned_data(typed_machine)[ordinal];
            if retained.symbol != source.symbol || retained.initial_value.is_valid() {
                return Err(Diagnostic::error(
                    "machine body completion changed an owned declaration",
                ));
            }
            let initial_value = source
                .initial_value
                .is_valid()
                .then(|| lower_expression_handle(lowerer, source.initial_value))
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
            lowerer
                .typed_trees
                .machine_owned_data
                .span_mut_or_empty(typed_machine.owned_data)[ordinal]
                .initial_value = initial_value;
        }
        let conformances = lowerer
            .source_trees
            .machine_trait_conformances(machine.satisfies);
        if conformances.len()
            != lowerer
                .typed_trees
                .machine_trait_conformances(typed_machine)
                .len()
        {
            return Err(Diagnostic::error(
                "machine body completion changed conformance declarations",
            ));
        }
        for (ordinal, source) in conformances.iter().enumerate() {
            let via_expression = source
                .via_expression
                .is_valid()
                .then(|| lower_expression_handle(lowerer, source.via_expression))
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
            lowerer
                .typed_trees
                .machine_trait_conformances
                .span_mut_or_empty(typed_machine.satisfies)[ordinal]
                .via_expression = via_expression;
        }

        let source_states = lowerer.source_trees.machine_state_handles(machine.states);
        let retained_count = lowerer.typed_trees.machine_states(typed_machine).len();
        if source_states.len() < retained_count {
            return Err(Diagnostic::error(
                "machine body completion removed a state declaration",
            ));
        }
        for (ordinal, source) in source_states.iter().enumerate() {
            let source = lowerer.source_trees.machine_state(*source);
            lowerer.equality_scope = Some(EqualityScope::for_state_interface(
                lowerer.source_trees,
                machine,
                source,
            ));
            let mut state = if ordinal < retained_count {
                lowerer.typed_trees.machine_states(typed_machine)[ordinal].clone()
            } else {
                lower_state_declaration(lowerer, source)?
            };
            lowerer.equality_scope = Some(EqualityScope::for_state(
                lowerer.source_trees,
                machine,
                source,
            ));
            complete_state_body(lowerer, machine.attached_data.as_ref(), source, &mut state)?;
            lowerer.equality_scope = None;
            if ordinal < retained_count {
                lowerer.typed_trees.machine_states_mut(typed_machine)[ordinal] = state;
            } else {
                lowerer.typed_trees.push_machine_state(typed_machine, state);
            }
        }
        typed_machine.termination_plan.implementation_witness =
            machine.termination_plan.implementation_witness.clone();
        crate::lowerer::progress::settle_machine_ranking_default(
            &lowerer.typed_trees,
            &lowerer.source_trees.symbols,
            typed_machine,
        );
        typed_machine.body_status = typed::machine::MachineBodyStatus::Complete;
        Ok(())
    }

    pub(crate) fn settle_satisfied_declarations_from(
        program: &mut typed::TypedTrees,
        machine_frontier: usize,
    ) -> Result<(), Diagnostic> {
        let mut updates = Vec::new();
        for machine in program.machines().iter().skip(machine_frontier) {
            let exposure = machine.interface_exposure;
            for (ordinal, conformance) in program
                .machine_trait_conformances(machine)
                .iter()
                .enumerate()
            {
                let Some(declaration) =
                    typed::machine::resolve_satisfied_declaration(program, machine, conformance)
                else {
                    continue;
                };
                updates.push((
                    machine.satisfies,
                    ordinal,
                    declaration.symbol(),
                    conformance.requirement_source_span,
                    exposure,
                    matches!(
                        declaration,
                        typed::machine::SatisfiedDeclaration::TopLevelRequirement(_)
                    ),
                ));
            }
        }

        for (span, ordinal, requirement_symbol, source_span, exposure, top_level_requirement) in
            updates
        {
            let conformances = program.machine_trait_conformances.span_mut_or_empty(span);
            let Some(conformance) = conformances.get_mut(ordinal) else {
                return Err(Diagnostic::error(
                    "failed to settle an exact machine `satisfies` declaration",
                ));
            };
            conformance.requirement_symbol = requirement_symbol;
            if top_level_requirement {
                conformance.symbol = requirement_symbol;
            }
            if let Some(source_span) = source_span {
                program
                .record_resolved_authored_declaration_selection_once(
                    source_span,
                    exposure,
                    AuthoredDeclarationSelectionKind::StaticPathSegment,
                    requirement_symbol,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain authored machine `satisfies` requirement selection: {error:?}"
                    ))
                    .with_source_span(source_span)
                })?;
            }
        }
        Ok(())
    }
}
pub(crate) mod mathematical {
    use crate::expressions::expression::lower_expression_handle;
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::lower_name;
    use crate::signatures::type_parameters::lower_type_parameters;
    use crate::tables::{expression_table_mut, resolved_expressions};
    use crate::type_reference::lower_type_reference_into_table;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    pub(crate) fn lower_mathematical_definition(
        lowerer: &mut Lowerer,
        definition: &resolved::mathematical::MathematicalDefinition,
    ) -> Result<typed::mathematical::MathematicalDefinition, Diagnostic> {
        let mut typed_definition = typed::mathematical::MathematicalDefinition {
            symbol: definition.symbol,
            name: lower_name(&definition.name),
            is_public: definition.is_public,
            binders: lower_type_parameters(lowerer, definition.binders)?,
            parameters: Default::default(),
            result: lower_mathematical_type(lowerer, definition.result)?,
            body: match &definition.body {
                resolved::mathematical::MathematicalBody::Assumption => {
                    typed::mathematical::MathematicalBody::Assumption
                }
                resolved::mathematical::MathematicalBody::Definition(term) => {
                    typed::mathematical::MathematicalBody::Definition(lower_expression_handle(
                        lowerer, *term,
                    )?)
                }
            },
        };

        for parameter in lowerer
            .source_trees
            .tables
            .declarations
            .mathematical_parameters
            .span_or_empty(definition.parameters)
        {
            let ty = lower_mathematical_type(lowerer, parameter.ty)?;
            lowerer.typed_trees.push_mathematical_parameter(
                &mut typed_definition,
                typed::mathematical::MathematicalParameter {
                    symbol: parameter.symbol,
                    name: lower_name(&parameter.name),
                    relevance: parameter.relevance,
                    ty,
                },
            );
        }

        Ok(typed_definition)
    }

    fn lower_mathematical_type(
        lowerer: &mut Lowerer,
        handle: resolved::mathematical::MathematicalTypeHandle,
    ) -> Result<typed::mathematical::MathematicalTypeHandle, Diagnostic> {
        let ty = match lowerer
            .source_trees
            .tables
            .declarations
            .mathematical_types
            .get(handle)
        {
            resolved::mathematical::MathematicalType::Ordinary(type_reference) => {
                typed::mathematical::MathematicalType::Ordinary(lower_type_reference_into_table(
                    lowerer,
                    type_reference,
                )?)
            }
            resolved::mathematical::MathematicalType::Arrow {
                binder,
                domain,
                codomain,
            } => typed::mathematical::MathematicalType::Arrow {
                binder: binder.as_ref().map(lower_name),
                domain: lower_mathematical_type(lowerer, *domain)?,
                codomain: lower_mathematical_type(lowerer, *codomain)?,
            },
            resolved::mathematical::MathematicalType::Application { callee, arguments } => {
                let arguments = resolved_expressions(lowerer.source_trees)
                    .expression_handles(*arguments)
                    .iter()
                    .copied()
                    .map(|argument| lower_expression_handle(lowerer, argument))
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                typed::mathematical::MathematicalType::Application {
                    callee: lower_mathematical_type(lowerer, *callee)?,
                    arguments: expression_table_mut(&mut lowerer.typed_trees)
                        .insert_expression_handles(arguments),
                }
            }
        };
        Ok(lowerer.typed_trees.insert_mathematical_type(ty))
    }
}
pub(crate) mod measure {
    use crate::expressions::expression::lower_expression_handle;
    use crate::lowerer::Lowerer;
    use crate::signatures::parameters::state_parameter;
    use crate::tables::{expression_table_mut, resolved_expressions};
    use crate::type_reference::lower_type_reference_into_table;
    use diagnostics::Diagnostic;

    pub(crate) fn lower_measure_definition(
        lowerer: &mut Lowerer,
        measure: &symbol_resolved_trees::measure::MeasureDefinition,
    ) -> Result<typed_trees::measure::MeasureDefinition, Diagnostic> {
        let parameter = measure
            .parameter
            .as_ref()
            .map(|parameter| lower_measure_parameter(lowerer, parameter))
            .transpose()?;

        let return_type = crate::type_reference::lower_return_type(lowerer, &measure.return_type)?;

        let mut typed_measure = typed_trees::measure::MeasureDefinition {
            symbol: measure.symbol,
            name: Default::default(),
            parameter,
            return_type,
            lexicographic: measure.lexicographic,
            body: Default::default(),
        };

        for member in lowerer.source_trees.measure_path_members(measure.name) {
            lowerer.typed_trees.push_measure_path_member(
                &mut typed_measure,
                crate::lowerer::name::lower_name(member),
            );
        }

        let mut lowered_components = Vec::new();
        for expression in
            resolved_expressions(lowerer.source_trees).expression_handles(measure.body)
        {
            lowered_components.push(lower_expression_handle(lowerer, *expression)?);
        }
        typed_measure.body = expression_table_mut(&mut lowerer.typed_trees)
            .insert_expression_handles(lowered_components);

        Ok(typed_measure)
    }

    fn lower_measure_parameter(
        lowerer: &mut Lowerer,
        parameter: &symbol_resolved_trees::signature::StateParameter,
    ) -> Result<typed_trees::signature::StateParameter, Diagnostic> {
        let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
        Ok(state_parameter(parameter, type_reference))
    }
}
pub(crate) mod operator {
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::{lower_name, lower_names};
    use crate::signatures::parameters::state_parameter;
    use crate::signatures::type_parameters::lower_type_parameters;
    use crate::type_reference::lower_type_reference_into_table;
    use diagnostics::Diagnostic;

    pub(crate) fn lower_operator_definition(
        lowerer: &mut Lowerer,
        operator: &symbol_resolved_trees::operator::OperatorDefinition,
    ) -> Result<typed_trees::operator::OperatorDefinition, Diagnostic> {
        let mut typed_operator = typed_trees::operator::OperatorDefinition {
            is_public: operator.is_public,
            is_boundary: operator.is_boundary,
            symbol: operator.symbol,
            name: Default::default(),
            lifetime_parameters: lower_names(&operator.lifetime_parameters),
            type_parameters: Default::default(),
            parameters: Default::default(),
            return_type: crate::type_reference::lower_return_type(lowerer, &operator.return_type)?,
            contracts: Default::default(),
            spelling: operator.spelling,
            token_count: operator.token_count,
            home_domain: symbols::SymbolHandle::invalid(),
        };

        for member in lowerer.source_trees.operator_path_members(operator.name) {
            lowerer
                .typed_trees
                .push_operator_path_member(&mut typed_operator, lower_name(member));
        }

        typed_operator.type_parameters = lower_type_parameters(lowerer, operator.type_parameters)?;

        for parameter in lowerer.source_trees.state_parameters(operator.parameters) {
            let type_reference =
                lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
            lowerer.typed_trees.push_operator_parameter(
                &mut typed_operator,
                state_parameter(parameter, type_reference),
            );
        }

        for contract in lowerer.source_trees.signature_contracts(operator.contracts) {
            let contract =
                crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
            lowerer
                .typed_trees
                .push_operator_contract(&mut typed_operator, contract);
        }

        Ok(typed_operator)
    }
}
pub(crate) mod proposition {
    use crate::expressions::expression::lower_expression_handle_from_table_in_fact_position;
    use crate::lowerer::Lowerer;
    use crate::tables::resolved_expressions;
    use crate::type_reference::lower_type_reference_into_table;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    use crate::expressions::proposition::{
        is_proposition_call_target, lower_proposition_application,
    };

    pub(crate) fn lower_proposition_definition(
        lowerer: &mut Lowerer,
        proposition: &resolved::proposition::PropositionDefinition,
    ) -> Result<typed::proposition::PropositionDefinition, Diagnostic> {
        let mut typed_proposition = typed::proposition::PropositionDefinition {
            symbol: proposition.symbol,
            name: crate::lowerer::name::lower_name(&proposition.name),
            is_public: proposition.is_public,
            binders: Default::default(),
            parameters: Default::default(),
            transparent_formula_source_span: proposition.transparent_formula_source_span,
            body: typed::proposition::PropositionBody::Primitive,
        };

        for binder in lowerer
            .source_trees
            .tables
            .declarations
            .proposition_binders
            .span_or_empty(proposition.binders)
        {
            let kind = match &binder.kind {
                resolved::proposition::PropositionBinderKind::Type => {
                    typed::proposition::PropositionBinderKind::Type
                }
                resolved::proposition::PropositionBinderKind::Const { type_reference } => {
                    typed::proposition::PropositionBinderKind::Const {
                        type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                    }
                }
                resolved::proposition::PropositionBinderKind::Machine => {
                    typed::proposition::PropositionBinderKind::Machine
                }
            };
            lowerer.typed_trees.push_proposition_binder(
                &mut typed_proposition,
                typed::proposition::PropositionBinder {
                    symbol: binder.symbol,
                    name: crate::lowerer::name::lower_name(&binder.name),
                    kind,
                    bounds: typed::data::DataProperties {
                        carry: binder.bounds.carry,
                        multiplicity: binder.bounds.multiplicity,
                    },
                },
            );
        }

        for parameter in lowerer
            .source_trees
            .state_parameters(proposition.parameters)
        {
            let parameter =
                crate::signatures::parameters::lower_state_parameter(lowerer, parameter)?;
            lowerer
                .typed_trees
                .push_proposition_parameter(&mut typed_proposition, parameter);
        }

        typed_proposition.body = match &proposition.body {
            resolved::proposition::PropositionBody::Primitive => {
                typed::proposition::PropositionBody::Primitive
            }
            resolved::proposition::PropositionBody::Witness { evidence } => {
                typed::proposition::PropositionBody::Witness {
                    evidence: lower_type_reference_into_table(lowerer, evidence)?,
                }
            }
            resolved::proposition::PropositionBody::Transparent { proposition } => {
                let formula = if let resolved::expression::ExpressionNode::Call(call) =
                    resolved_expressions(lowerer.source_trees).expression(*proposition)
                    && is_proposition_call_target(&lowerer.source_trees.symbols, call.target_symbol)
                {
                    typed::proposition::PropositionFormula::Application(
                        lower_proposition_application(lowerer, call)?,
                    )
                } else {
                    typed::proposition::PropositionFormula::BooleanExpression(
                        lower_expression_handle_from_table_in_fact_position(
                            lowerer.source_trees,
                            &lowerer.source_trees.tables.bodies.expressions,
                            &mut lowerer.typed_trees,
                            *proposition,
                        )?,
                    )
                };
                typed::proposition::PropositionBody::Transparent {
                    proposition: formula,
                }
            }
        };

        Ok(typed_proposition)
    }
}
pub(crate) mod provider_defaults {
    use crate::tables::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic, BuildOperation,
    };
    use typed_trees::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding as Binding,
        AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget as Target,
        TypedTrees,
    };

    pub(crate) fn finalize_provider_default_calls(
        typed: &mut TypedTrees,
    ) -> Result<(), Diagnostic> {
        for machine in typed.machines_mut() {
            if machine.name.as_str().ends_with("::provider_defaults") {
                machine.purpose = typed_trees::machine::MachinePurpose::ProviderDefaults;
            }
        }
        let occurrences = typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
            })
            .flat_map(|machine| provider_default_call_occurrences(typed, machine))
            .collect::<Vec<_>>();
        if occurrences.is_empty() {
            return Ok(());
        }
        let mut selections = typed.authored_declaration_selections().clone();
        for occurrence in occurrences {
            if selections.get(occurrence).is_some_and(|selection| {
                selection.target() == Target::Intrinsic(Intrinsic::BuildProviderSelection)
            }) {
                continue;
            }
            selections
                .finalize_intrinsic(
                    occurrence,
                    Binding::CheckedCall,
                    Intrinsic::BuildProviderSelection,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "provider-default call lost its authored selection custody: {error:?}"
                    ))
                })?;
        }
        typed.retain_authored_declaration_selections(selections);
        Ok(())
    }

    fn provider_default_call_occurrences(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<AuthoredDeclarationSelectionOccurrenceId> {
        let mut occurrences = Vec::new();
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                match statement {
                typed_trees::statement::StatementNode::Call(call)
                    if language_semantics::declaration_selection::is_unresolved_provider_selection_call(
                        call.target.as_str(),
                        call.target_symbol,
                    ) =>
                {
                    occurrences.extend(call.authored_call_selection);
                }
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                        && BuildOperation::from_call_target(call.target.as_str())
                            == Some(BuildOperation::ProviderSelection)
                        && !call.target_symbol.is_valid()
                    {
                        occurrences.extend(
                            expression_table(typed).authored_selection_occurrences(*expression).filter(|occurrence| {
                                    typed
                                        .authored_declaration_selections()
                                        .get(*occurrence)
                                        .is_some_and(|selection| {
                                            selection.kind()
                                                == AuthoredDeclarationSelectionKind::Call
                                        })
                                }),
                        );
                    }
                }
                _ => {}
            }
            }
        }
        occurrences
    }
}
pub(crate) mod state {
    use crate::expressions::statement::lower_statement;
    use crate::lowerer::Lowerer;
    use crate::tables::{expression_table, resolved_statements, statement_table_mut};
    use diagnostics::Diagnostic;
    use language_semantics::CallOperationalAcknowledgementOrigin;
    use symbol_resolved_trees as resolved;
    use symbols::SymbolHandle;
    use typed::expression::ExpressionHandle;
    use typed_trees as typed;

    pub(crate) fn lower_state_declaration(
        lowerer: &mut Lowerer,
        state: &resolved::state::State,
    ) -> Result<typed::state::State, Diagnostic> {
        let mut typed_state = typed::state::State {
            symbol: state.symbol,
            name: crate::lowerer::name::lower_name(&state.name),
            parameters: Default::default(),
            return_type: crate::type_reference::lower_return_type(lowerer, &state.return_type)?,
            contracts: Default::default(),
            statement_nodes: Default::default(),
        };

        let domain_constrained_parameters =
        crate::contracts::parameter_domains::lower_state_parameters_collecting_domain_constraints(
            lowerer,
            state.parameters,
            |typed_trees, parameter| typed_trees.push_state_parameter(&mut typed_state, parameter),
        )?;

        for contract in lowerer.source_trees.signature_contracts(state.contracts) {
            let contract =
                crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
            lowerer
                .typed_trees
                .push_state_contract(&mut typed_state, contract);
        }

        crate::contracts::parameter_domains::push_domain_membership_contracts(
            lowerer,
            domain_constrained_parameters,
            |typed_trees, contract| typed_trees.push_state_contract(&mut typed_state, contract),
        );

        Ok(typed_state)
    }

    pub(crate) fn complete_state_body(
        lowerer: &mut Lowerer,
        attached_data: Option<&resolved::name::DiagnosticName>,
        state: &resolved::state::State,
        typed_state: &mut typed::state::State,
    ) -> Result<(), Diagnostic> {
        if typed_state.symbol != state.symbol || !typed_state.statement_nodes.is_empty() {
            return Err(Diagnostic::error(
                "state body completion does not match its retained declaration",
            ));
        }
        let statements =
            resolved_statements(lowerer.source_trees).statements(state.statement_nodes);
        let mut pending = Vec::new();
        for (index, statement) in statements.iter().enumerate() {
            let closes_run = matches!(statement,
                resolved::statement::StatementNode::Transition(transition) if transition.ends_dispatch
            ) || !matches!(
                statements.get(index + 1),
                Some(resolved::statement::StatementNode::Transition(_))
            );
            if let resolved::statement::StatementNode::ProofOutputBindingStatement(package) =
                statement
            {
                let call =
                    crate::expressions::expression::lower_expression_handle(lowerer, package.call)?;
                let runtime_call_statement_index = package
                    .bindings
                    .iter()
                    .any(|binding| {
                        binding.output_field.as_str() == "value" && binding.binding.as_str() != "_"
                    })
                    .then(|| {
                        pending
                            .len()
                            .checked_sub(1)
                            .expect("a proof-output Type value has its synthesized local first")
                    });
                let runtime_call_statement_index = if runtime_call_statement_index.is_some() {
                    runtime_call_statement_index
                } else if proof_output_call_requires_execution(lowerer, call) {
                    let statement_index = pending.len();
                    let call = proof_output_runtime_call(lowerer, call)?;
                    pending.push(typed::statement::StatementNode::Call(call));
                    Some(statement_index)
                } else {
                    None
                };
                lowerer
                    .typed_trees
                    .proof_output_calls
                    .push(typed::typed_trees::ProofOutputCall {
                        machine_symbol: package.machine_symbol,
                        state_symbol: package.state_symbol,
                        statement_index: pending.len(),
                        source_statement_index: package.statement_index,
                        runtime_call_statement_index,
                        bindings: package
                            .bindings
                            .iter()
                            .map(|binding| typed::typed_trees::ProofOutputSelector {
                                output_field: crate::lowerer::name::lower_name(
                                    &binding.output_field,
                                ),
                                binding: crate::lowerer::name::lower_name(&binding.binding),
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        call,
                    });
                continue;
            }
            pending.extend(lower_statement(
                lowerer,
                attached_data,
                state,
                statement,
                closes_run,
            )?);
        }

        typed_state.statement_nodes =
            statement_table_mut(&mut lowerer.typed_trees).insert_statements(pending);
        Ok(())
    }

    fn proof_output_call_requires_execution(lowerer: &Lowerer, call: ExpressionHandle) -> bool {
        let typed::expression::ExpressionNode::Call(call) =
            lowerer.typed_trees.expression_table.expression(call)
        else {
            return true;
        };
        let Some((machine, state)) = lowerer.source_trees.machines.iter().find_map(|machine| {
            lowerer
                .source_trees
                .machine_state_handles(machine.states)
                .iter()
                .map(|state| lowerer.source_trees.machine_state(*state))
                .find(|state| state.symbol == call.target_symbol)
                .map(|state| (machine, state))
        }) else {
            return true;
        };

        state.return_type.is_none()
            && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || lowerer
                    .source_trees
                    .machine_state_handles(machine.states)
                    .len()
                    != 1
                || !state.statement_nodes.is_empty())
    }

    fn proof_output_runtime_call(
        lowerer: &mut Lowerer,
        call_handle: ExpressionHandle,
    ) -> Result<typed::statement::TableCall, Diagnostic> {
        let source_span = expression_table(&lowerer.typed_trees).source_span(call_handle);
        let typed::expression::ExpressionNode::Call(call) = expression_table(&lowerer.typed_trees)
            .expression(call_handle)
            .clone()
        else {
            return Err(Diagnostic::error(
                "proof-output binding requires a direct call",
            ));
        };
        let mut call_occurrences = Vec::new();
        for occurrence in
            expression_table(&lowerer.typed_trees).authored_selection_occurrences(call_handle)
        {
            let Some(selection) = lowerer
                .typed_trees
                .authored_declaration_selections()
                .get(occurrence)
            else {
                return Err(Diagnostic::error(format!(
                    "proof-output call retains unknown authored selection occurrence {}",
                    occurrence.ordinal(),
                )));
            };
            if selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
            {
                call_occurrences.push(occurrence);
            }
        }
        let authored_call_selection = match call.operational_acknowledgement.origin {
            CallOperationalAcknowledgementOrigin::Source => {
                let [occurrence] = call_occurrences.as_slice() else {
                    return Err(Diagnostic::error(format!(
                        "proof-output call retains {} authored call selections; expected one",
                        call_occurrences.len(),
                    )));
                };
                Some(*occurrence)
            }
            CallOperationalAcknowledgementOrigin::CompilerSynthesized => None,
        };
        let (receiver_root_symbol, receiver_symbol, receiver_members) =
            proof_output_receiver_parts(&lowerer.typed_trees, call.receiver).ok_or_else(|| {
                Diagnostic::error("proof-output call receiver must be a name path")
            })?;
        let mut receiver = arena::HandleSpan::empty();
        for member in receiver_members {
            statement_table_mut(&mut lowerer.typed_trees)
                .push_name_path_member(&mut receiver, member);
        }
        let arguments = lowerer
            .typed_trees
            .tables
            .statement_table
            .insert_expression_handles(
                lowerer
                    .typed_trees
                    .tables
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        Ok(typed::statement::TableCall {
            installed_interrupt_operation: None,
            authored_machine_application: Default::default(),
            receiver_root_symbol,
            receiver_symbol,
            target_symbol: call.target_symbol,
            receiver,
            target: call.target,
            static_machine_parameter: call.static_machine_parameter,
            static_requirement_dispatch: None,
            machine_arguments: call.machine_arguments,
            arguments,
            evidence_arguments: call.evidence_arguments,
            operational_acknowledgement: call.operational_acknowledgement,
            discards_result: false,
            source_span,
            authored_call_selection,
            reflected_field_origin: None,
        })
    }

    fn proof_output_receiver_parts(
        program: &typed::TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, SymbolHandle, Vec<typed::name::Identifier>)> {
        if !expression.is_valid() {
            return Some((SymbolHandle::invalid(), SymbolHandle::invalid(), Vec::new()));
        }
        match program.expression_table.expression(expression) {
            typed::expression::ExpressionNode::Borrow(inner) => {
                proof_output_receiver_parts(program, inner.target)
            }
            typed::expression::ExpressionNode::Name(path) => Some((
                path.head_symbol,
                path.symbol,
                expression_table(program)
                    .name_path_members(path.members)
                    .to_vec(),
            )),
            typed::expression::ExpressionNode::Member(member) => {
                let (root, _, mut members) = proof_output_receiver_parts(program, member.receiver)?;
                members.push(member.member.clone());
                Some((root, member.member_symbol, members))
            }
            _ => None,
        }
    }
}
pub(crate) mod trait_definition {
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::{lower_name, lower_names};
    use crate::signatures::callable_signature::lower_state_signature;
    use crate::signatures::type_parameters::lower_type_parameters;
    use crate::type_reference::{lower_type_reference_into_table, retain_type_reference_selection};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
    use symbol_resolved_trees as resolved;
    use symbol_resolved_trees::signature::StateSignature;
    use typed::trait_definition::TraitRefinementClause;
    use typed_trees as typed;

    pub(crate) fn lower_trait_definition(
        lowerer: &mut Lowerer,
        trait_definition: &resolved::trait_definition::TraitDefinition,
    ) -> Result<typed::trait_definition::TraitDefinition, Diagnostic> {
        let mut typed_trait = typed::trait_definition::TraitDefinition {
            symbol: trait_definition.symbol,
            is_boundary: trait_definition.is_boundary,
            is_public: trait_definition.is_public,
            name: lower_name(&trait_definition.name),
            lifetime_parameters: lower_names(&trait_definition.lifetime_parameters),
            type_parameters: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            requires: HandleSpan::empty(),
            machines: HandleSpan::empty(),
            refines: None,
            refinement_clauses: Vec::new(),
        };

        typed_trait.type_parameters =
            lower_type_parameters(lowerer, trait_definition.type_parameters)?;

        typed_trait.conformance_bounds = crate::signatures::evidence::lower_conformance_bounds(
            lowerer,
            &trait_definition.conformance_bounds,
        )?;

        for requirement in lowerer
            .source_trees
            .trait_requirements(trait_definition.requires)
        {
            retain_type_reference_selection(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                &requirement.name,
                requirement.symbol,
                lowerer.type_reference_exposure,
                AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            let mut arguments = HandleSpan::empty();
            let source_arguments = lowerer
                .source_trees
                .child_type_references(requirement.arguments)
                .to_vec();
            for argument in &source_arguments {
                let argument = lower_type_reference_into_table(lowerer, argument)?;
                lowerer
                    .typed_trees
                    .type_reference_table
                    .push_type_reference_handle(&mut arguments, argument);
            }
            lowerer.typed_trees.push_trait_requirement(
                &mut typed_trait,
                typed::trait_definition::TraitRequirement {
                    symbol: requirement.symbol,
                    name: lower_name(&requirement.name),
                    lifetime_arguments: lower_names(&requirement.lifetime_arguments),
                    arguments,
                    source_span: requirement.name.source_span(),
                },
            );
        }

        for signature in lowerer
            .source_trees
            .trait_machine_signatures(trait_definition.machines)
        {
            let signature = lower_state_signature(lowerer, signature)?;
            lowerer
                .typed_trees
                .push_trait_machine_signature(&mut typed_trait, signature);
        }

        if let Some(base) = &trait_definition.refines {
            let base_definition = lowerer.source_trees.roots.trait_definition(base.symbol);
            let Some(base_definition) = base_definition else {
                return Err(Diagnostic::error(format!(
                    "transparent refinement `{}` base `{}` does not name a trait",
                    trait_definition.name.as_str(),
                    base.name.as_str(),
                )));
            };
            retain_type_reference_selection(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                &base.name,
                base.symbol,
                lowerer.type_reference_exposure,
                AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            let mut base_arguments = HandleSpan::empty();
            for argument in lowerer.source_trees.child_type_references(base.arguments) {
                let argument = lower_type_reference_into_table(lowerer, argument)?;
                lowerer
                    .typed_trees
                    .type_reference_table
                    .push_type_reference_handle(&mut base_arguments, argument);
            }
            typed_trait.refines = Some(typed::trait_definition::TraitRequirement {
                symbol: base.symbol,
                name: lower_name(&base.name),
                lifetime_arguments: lower_names(&base.lifetime_arguments),
                arguments: base_arguments,
                source_span: base.name.source_span(),
            });
            let base_machines = lowerer
                .source_trees
                .trait_machine_signatures(base_definition.machines)
                .to_vec();
            let mut seen_named = std::collections::HashSet::new();
            for clause in &trait_definition.refinement_clauses {
                let signature = lower_state_signature(lowerer, &clause.signature)?;
                let covered: Vec<&StateSignature> = match &clause.requirement {
                    Some(requirement) => {
                        let matching = base_machines
                            .iter()
                            .filter(|machine| machine.name == *requirement)
                            .collect::<Vec<_>>();
                        if matching.is_empty() {
                            return Err(Diagnostic::error(format!(
                                "refinement clause `machine {}::{}` names no requirement of base trait `{}`",
                                base.name.as_str(),
                                requirement.as_str(),
                                base.name.as_str(),
                            )));
                        }
                        if matching.len() > 1 {
                            return Err(Diagnostic::error(format!(
                                "refinement clause `machine {}::{}` names {} overloads of base trait `{}`; name one exact requirement",
                                base.name.as_str(),
                                requirement.as_str(),
                                matching.len(),
                                base.name.as_str(),
                            )));
                        }
                        if !seen_named.insert(requirement.as_str().to_owned()) {
                            return Err(Diagnostic::error(format!(
                                "duplicate refinement clause for `{}`",
                                requirement.as_str(),
                            )));
                        }
                        matching
                    }
                    None => base_machines.iter().collect(),
                };
                if clause.signature.suspends
                    && let Some(machine) = covered.iter().find(|machine| !machine.suspends)
                {
                    return Err(Diagnostic::error(format!(
                        "refinement clause asserts `suspends` on `{}`, but base trait `{}` does not declare it — a refinement narrows, it cannot add an axis",
                        machine.name.as_str(),
                        base.name.as_str(),
                    )));
                }
                if clause.signature.blocks
                    && let Some(machine) = covered.iter().find(|machine| !machine.blocks)
                {
                    return Err(Diagnostic::error(format!(
                        "refinement clause asserts `blocks` on `{}`, but base trait `{}` does not declare it — a refinement narrows, it cannot add an axis",
                        machine.name.as_str(),
                        base.name.as_str(),
                    )));
                }
                if !clause.service_reaches.is_empty()
                    && let Some(machine) = covered.iter().find(|machine| {
                        machine.service_reach_row
                            == language_semantics::ServiceReachRowTable::EMPTY_ROW
                            || machine.service_reach_row
                                == language_semantics::ServiceReachRowId::default()
                    })
                {
                    return Err(Diagnostic::error(format!(
                        "refinement clause narrows `reaches` on `{}`, but base trait `{}` declares no reach set to narrow",
                        machine.name.as_str(),
                        base.name.as_str(),
                    )));
                }
                let wildcard = clause
                    .service_reaches
                    .iter()
                    .any(|reach| reach.as_str() == "_");
                if wildcard && clause.service_reaches.len() > 1 {
                    return Err(Diagnostic::error(
                        "refinement clause `reaches _;` is an independent abstract row and does not combine with named services",
                    ));
                }
                let mut clause_services = Vec::new();
                for reach in &clause.service_reaches {
                    if reach.as_str() == "_" {
                        continue;
                    }
                    let service = lowerer
                        .source_trees
                        .symbols
                        .find_top_level_by_name_and_kinds_from_source(
                            reach.as_str(),
                            &[symbols::SymbolKind::Trait],
                            reach.source_span(),
                        )
                        .and_then(|symbol| {
                            lowerer.source_trees.service_reaches.id_for_symbol(symbol)
                        });
                    let Some(service) = service else {
                        return Err(Diagnostic::error(format!(
                            "refinement clause `reaches` names `{reach}`, which is not a boundary service",
                        )));
                    };
                    if let Some(machine) = covered.iter().find(|machine| {
                        !lowerer
                            .source_trees
                            .service_reach_rows
                            .services(machine.service_reach_row)
                            .contains(&service)
                    }) {
                        return Err(Diagnostic::error(format!(
                            "refinement clause narrows `reaches` to `{reach}`, but `{}` of base trait `{}` does not reach it — a refinement narrows, it cannot add a reach",
                            machine.name.as_str(),
                            base.name.as_str(),
                        )));
                    }
                    clause_services.push(service);
                }
                let service_reach = if clause.service_reach_keyword_source_spans.is_empty() {
                    typed::trait_definition::TraitRefinementReach::Inherited
                } else if wildcard {
                    typed::trait_definition::TraitRefinementReach::IndependentBounded(
                        covered
                            .iter()
                            .map(|machine| typed::trait_definition::ClauseAbstractReachRow {
                                requirement: lower_name(&machine.name),
                                row: lowerer
                                    .typed_trees
                                    .service_reach_rows
                                    .intern_abstract(machine.service_reach_row),
                            })
                            .collect(),
                    )
                } else {
                    typed::trait_definition::TraitRefinementReach::Concrete(
                        lowerer
                            .typed_trees
                            .service_reach_rows
                            .intern(clause_services),
                    )
                };
                typed_trait.refinement_clauses.push(TraitRefinementClause {
                    requirement: clause.requirement.as_ref().map(lower_name),
                    signature,
                    service_reaches: lower_names(&clause.service_reaches),
                    service_reach,
                });
            }
        }

        Ok(typed_trait)
    }
}
pub(crate) mod type_aliases {
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::{lower_name, lower_names};
    use crate::signatures::type_parameters::lower_type_parameters;
    use crate::type_reference::lower_type_reference_into_table;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed_trees as typed;

    pub(crate) fn lower_type_alias_definition(
        lowerer: &mut Lowerer,
        alias: &resolved::type_aliases::TypeAliasDefinition,
    ) -> Result<typed::type_aliases::TypeAliasDefinition, Diagnostic> {
        let type_parameters = lower_type_parameters(lowerer, alias.type_parameters)?;
        let conformance_bounds = crate::signatures::evidence::lower_conformance_bounds(
            lowerer,
            &alias.conformance_bounds,
        )?;
        let expansion = lower_type_reference_into_table(lowerer, &alias.expansion)?;
        Ok(typed::type_aliases::TypeAliasDefinition {
            symbol: alias.symbol,
            name: lower_name(&alias.name),
            is_public: alias.is_public,
            lifetime_parameters: lower_names(&alias.lifetime_parameters),
            type_parameters,
            conformance_bounds,
            expansion,
        })
    }
}
pub(crate) mod wire {
    use crate::lowerer::Lowerer;
    use crate::lowerer::name::lower_name;
    use crate::type_reference::lower_type_reference_into_table;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use symbol_resolved_trees as resolved;
    use typed::wire::WireField;
    use typed_trees as typed;

    pub(crate) fn lower_wire_schema(
        lowerer: &mut Lowerer,
        wire_schema: &resolved::wire::WireSchema,
    ) -> Result<typed::wire::WireSchema, Diagnostic> {
        let members = lower_wire_members(lowerer, wire_schema.members)?;

        Ok(typed::wire::WireSchema {
            symbol: wire_schema.symbol,
            name: lower_name(&wire_schema.name),
            is_public: wire_schema.is_public,
            encoding: wire_schema.encoding.as_ref().map(lower_name),
            members,
        })
    }

    fn lower_wire_members(
        lowerer: &mut Lowerer,
        members: HandleSpan<resolved::wire::WireMember>,
    ) -> Result<HandleSpan<typed::wire::WireMember>, Diagnostic> {
        let mut lowered = Vec::new();

        for member in lowerer.source_trees.wire_members(members) {
            lowered.push(match member {
                resolved::wire::WireMember::Field(field) => {
                    typed::wire::WireMember::Field(WireField {
                        number: field.number,
                        name: lower_name(&field.name),
                        relevance: field.relevance,
                        type_reference: lower_type_reference_into_table(
                            lowerer,
                            &field.type_reference,
                        )?,
                    })
                }
                resolved::wire::WireMember::Reserved(reserved) => {
                    typed::wire::WireMember::Reserved(typed::wire::WireReserved {
                        number: reserved.number,
                    })
                }
                resolved::wire::WireMember::Version(version) => {
                    let members = lower_wire_members(lowerer, version.members)?;
                    typed::wire::WireMember::Version(typed::wire::WireVersion {
                        name: lower_name(&version.name),
                        members,
                    })
                }
            });
        }

        Ok(lowerer.typed_trees.append_wire_members(lowered))
    }
}
