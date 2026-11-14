/*
    Symbol assignment for top-level declarations.

    `assign_top_level_symbols` is the first assignment pass that
    `assign_symbols` and `assign_symbols_against_resolved_base` (in
    `symbols`) run after the symbol table is built. Each declaration
    already owns the handle published by its allocator. Current roots can be a
    selection of the backing graph; they never rejoin the global child stream
    by position or spelling. Child-assignment steps run in this order:

    1. domains (`domains`), then data (`data`);
    2. named conformances, done here: each takes its symbol, then
       `assign_conformance_parameter_symbols` assigns every conformance's type
       parameters and their signatures. Owned rows retain their own invocation
       telescope and read map captures through their exact row association;
    3. machines, propositions, mathematical definitions, root operators,
       measures and traits, each in its own child module;
    4. wire schemas, done here.

    A declaration that already holds a valid symbol keeps it. Every step except
    wire schemas also assigns the declaration's own children (parameters,
    fields, states) by the same positional walk over that declaration's
    children, and several resolve the type references in those children's
    signatures. Machine and trait assignment return diagnostics, which this
    function returns.

    Shared by the child modules and the conformance step rather than steps of
    their own: `next_child_of_kind` takes
    the next child and returns the invalid handle when its kind is not the
    expected one; `assign_machine_parameter_signature_symbols` and
    `assign_proposition_parameter_signature_symbols` assign the parameter
    symbols and type references of machine-parameter and proposition-parameter
    signatures.
*/

mod data {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::top_level::{SignatureSymbolTables, next_child_of_kind};
    use std::collections::HashMap;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::GenericTypeReference;
    use symbol_resolved_trees::types::GenericTypeReferenceStorage;
    use symbol_resolved_trees::types::TypeReference;

    pub(super) fn assign_data_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
        let (_, trait_proposition_slots, trait_machine_identity_slots) =
            crate::symbols::top_level::trait_parameter_tables(program);
        let type_constraints = &program.tables.types.constraints;
        let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
        let data_members = &mut program.tables.declarations.data_members;
        let data_payload_fields = &mut program.tables.declarations.data_payload_fields;
        let state_parameters = &mut program.tables.declarations.state_parameters;
        let child_type_references = &mut program.tables.declarations.child_type_references;
        let mut template_subjects: HashMap<String, SymbolHandle> = HashMap::new();
        program
            .roots
            .data_definitions
            .for_each_mut(|data_definition| {
                let data_symbol = data_definition.symbol;
                let mut data_children = symbols.child_handles(data_symbol).into_iter().flatten();

                for type_parameter in
                    data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
                {
                    let kind = match type_parameter.kind {
                        TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
                        TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                        _ => SymbolKind::TypeParameter,
                    };
                    type_parameter.symbol = next_child_of_kind(&mut data_children, symbols, kind);
                }
                let local_type_parameters = data_type_parameters
                    .span_or_empty(data_definition.type_parameters)
                    .to_vec();

                for bound in &mut data_definition.conformance_bounds {
                    bound.binder = local_type_parameters
                        .iter()
                        .find(|parameter| {
                            parameter.name == bound.binder_name
                                && parameter.kind == TypeParameterKind::Implementation
                        })
                        .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
                }
                crate::symbols::top_level::assign_conformance_bound_symbols(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    &mut data_definition.conformance_bounds,
                    &trait_proposition_slots,
                    &trait_machine_identity_slots,
                );

                for index in 0..data_definition.type_parameters.len() {
                    let (parameter_symbol, kind) = {
                        let parameter = &data_type_parameters
                            .span_or_empty(data_definition.type_parameters)[index];
                        (parameter.symbol, parameter.kind.clone())
                    };
                    let TypeParameterKind::Machine { mut contract } = kind else {
                        continue;
                    };
                    SignatureSymbolTables {
                        symbols,
                        data_type_parameters,
                        state_parameters,
                        child_type_references,
                        type_constraints,
                        trait_proposition_slots: &trait_proposition_slots,
                        trait_machine_identity_slots: &trait_machine_identity_slots,
                    }
                    .assign_parameter_contract_symbols(
                        &mut contract,
                        parameter_symbol,
                        &local_type_parameters,
                    );
                    data_type_parameters.span_mut_or_empty(data_definition.type_parameters)
                        [index]
                        .kind = TypeParameterKind::Machine { contract };
                }

                for member in data_members.span_mut_or_empty(data_definition.members) {
                    match member {
                        DataMember::Field(field) => {
                            field.symbol =
                                next_child_of_kind(&mut data_children, symbols, SymbolKind::Field);
                        }
                        DataMember::Variant(variant) => {
                            variant.symbol = next_child_of_kind(
                                &mut data_children,
                                symbols,
                                SymbolKind::Variant,
                            );
                            let mut variant_children =
                                symbols.child_handles(variant.symbol).into_iter().flatten();
                            for field in data_payload_fields.span_mut_or_empty(variant.payload) {
                                field.symbol = next_child_of_kind(
                                    &mut variant_children,
                                    symbols,
                                    SymbolKind::Field,
                                );
                            }
                        }
                    }
                }
                data_definition.predicate_subject_symbol = if data_definition
                    .requires_facts
                    .is_empty()
                {
                    SymbolHandle::invalid()
                } else if let Some(TypeReference::Generic(origin)) =
                    &data_definition.generic_instance
                {
                    template_subjects
                        .get(origin.base_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            next_child_of_kind(&mut data_children, symbols, SymbolKind::Parameter)
                        })
                } else {
                    next_child_of_kind(&mut data_children, symbols, SymbolKind::Parameter)
                };
                if data_definition.generic_instance.is_none() {
                    template_subjects.insert(
                        data_definition.name.as_str().to_owned(),
                        data_definition.predicate_subject_symbol,
                    );
                }
                data_definition.predicate_subject_type =
                    if !data_definition.predicate_subject_symbol.is_valid() {
                        TypeReference::Unit
                    } else if let Some(origin) = &data_definition.generic_instance {
                        origin.clone()
                    } else if local_type_parameters.is_empty()
                        && data_definition.lifetime_parameters.is_empty()
                    {
                        TypeReference::Named {
                            symbol: data_symbol,
                            name: DiagnosticName::generated(data_definition.name.as_str()),
                            supplied_operand_owner: Default::default(),
                        }
                    } else {
                        let arguments = local_type_parameters
                            .iter()
                            .map(|parameter| TypeReference::Named {
                                symbol: parameter.symbol,
                                name: DiagnosticName::generated(parameter.name.as_str()),
                                supplied_operand_owner: Default::default(),
                            })
                            .collect::<Vec<_>>();
                        let span = match &data_definition.predicate_subject_type {
                            TypeReference::Generic(previous)
                                if child_type_references.span_or_empty(previous.arguments)
                                    == arguments.as_slice() =>
                            {
                                previous.arguments
                            }
                            _ => child_type_references.insert_many(arguments),
                        };
                        TypeReference::Generic(GenericTypeReference {
                            storage: GenericTypeReferenceStorage {
                                base_symbol: data_symbol,
                                base_name: DiagnosticName::generated(data_definition.name.as_str()),
                                lifetime_arguments: data_definition.lifetime_parameters.clone(),
                                arguments: span,
                                ..Default::default()
                            },
                        })
                    };
            });
    }
}
mod domains {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolKind, SymbolTable};

    use crate::symbols::top_level::next_child_of_kind;
    use crate::symbols::top_level::operators::assign_operator_symbols;

    pub(super) fn assign_domain_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
        let type_constraints = &program.tables.types.constraints;
        let roots = &mut program.roots;
        let declarations = &mut program.tables.declarations;
        let operator_definitions = &mut declarations.operator_definitions;
        let data_type_parameters = &mut declarations.data_type_parameters;
        let state_parameters = &mut declarations.state_parameters;
        let child_type_references = &mut declarations.child_type_references;
        roots.domain_definitions.for_each_mut(|domain| {
            let mut domain_children = symbols.child_handles(domain.symbol).into_iter().flatten();
            for parameter in data_type_parameters.span_mut_or_empty(domain.type_parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut domain_children, symbols, SymbolKind::TypeParameter);
            }
            domain.subject_symbol =
                next_child_of_kind(&mut domain_children, symbols, SymbolKind::Parameter);
            for operator in operator_definitions.span_mut_or_empty(domain.operators) {
                assign_operator_symbols(
                    symbols,
                    &mut domain_children,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                    operator,
                );
            }
        });
    }
}
mod machines {
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::expressions::assign_expression_table_symbols;
    use crate::symbols::lookup::top_level_symbol_for_source;
    use crate::symbols::scope::{LexicalRequirements, MachineScope};
    use crate::symbols::top_level::{
        SignatureSymbolTables, next_child_of_kind, trait_parameter_tables,
    };
    use crate::symbols::type_references::{
        assign_machine_declaration_identity_argument_symbols,
        assign_proposition_family_argument_symbols,
        assign_type_reference_argument_symbols_with_constraints,
        assign_type_reference_symbol_with_locals_and_constraints,
    };
    use language_semantics::MachineSupplyMode;
    use symbol_resolved_trees::data::TypeParameterKind;

    pub(super) fn assign_machine_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) -> Vec<Diagnostic> {
        let top_level_requirement_symbols = program
            .machines
            .iter()
            .filter_map(|machine| {
                matches!(machine.supply_mode, MachineSupplyMode::TopLevelRequirement)
                    .then_some(machine.symbol)
            })
            .collect::<Vec<_>>();
        let token_requirement_symbols = program
            .machines
            .iter()
            .filter_map(|machine| {
                (machine.spelling.is_some()
                    && top_level_requirement_symbols.contains(&machine.symbol))
                .then_some(machine.symbol)
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        let (refinement_symbols, trait_proposition_slots, trait_machine_identity_slots) =
            trait_parameter_tables(program);
        program.machines.for_each_mut(|machine| {
            machine.attached_data_symbol = machine
                .attached_data
                .as_ref()
                .map(|attached| top_level_symbol_for_source(symbols, SymbolKind::Data, attached))
                .unwrap_or_else(SymbolHandle::invalid);
        });
        let attached_machines = crate::symbols::scope::attached_machines(program);
        {
            let parameters = &mut program.tables.declarations.data_type_parameters;
            program.roots.machines.for_each_mut(|machine| {
                let mut children = symbols
                    .child_handles(machine.symbol)
                    .into_iter()
                    .flatten()
                    .filter(|symbol| symbols.get(*symbol).kind != SymbolKind::Machine);
                for parameter in parameters.span_mut_or_empty(machine.type_parameters) {
                    let kind = match parameter.kind {
                        TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                        _ => SymbolKind::TypeParameter,
                    };
                    let declaration = next_child_of_kind(&mut children, symbols, kind);
                    if parameter.symbol.is_valid() && parameter.symbol != declaration {
                        diagnostics.push(Diagnostic::error(
                            "retained machine binder differs from its declaration child",
                        ));
                    } else {
                        parameter.symbol = declaration;
                    }
                }
                for bound in &mut machine.conformance_bounds {
                    bound.binder = next_child_of_kind(
                        &mut children,
                        symbols,
                        SymbolKind::ConformanceParameter,
                    );
                }
            });
        }
        let conformance_scopes = match crate::symbols::scope::DeclarationTypeScopes::of(program) {
            Ok(scopes) => scopes,
            Err(errors) => {
                diagnostics.extend(errors);
                return diagnostics;
            }
        };
        let tables = &mut program.tables;
        let type_constraints = &tables.types.constraints;
        let declarations = &mut tables.declarations;
        let expression_table = &mut tables.bodies.expressions;
        let data_type_parameters = &mut declarations.data_type_parameters;
        let data_members = &declarations.data_members;
        let machine_owned_data = &mut declarations.machine_owned_data;
        let machine_trait_conformances = &mut declarations.machine_trait_conformances;
        let machine_state_handles = &declarations.machine_state_handles;
        let machine_states = &mut declarations.machine_states;
        let state_parameters = &mut declarations.state_parameters;
        let child_type_references = &mut declarations.child_type_references;
        let symbol_resolved_trees::SymbolResolvedRoots {
            data_definitions,
            type_aliases,
            conformances,
            traits,
            machines,
            ..
        } = &mut program.roots;

        machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten()
            .filter(|symbol| symbols.get(*symbol).kind != SymbolKind::Machine)
            .skip(machine.type_parameters.len() + machine.conformance_bounds.len());

        let mut local_type_parameters = crate::symbols::scope::declaration_type_scope(
            data_type_parameters.span_or_empty(machine.type_parameters),
            conformance_scopes.parameters(machine.symbol),
        );

        if machine.attached_data_application.is_none() {
        machine.attached_data_application = data_definitions
            .iter()
            .find(|definition| {
                machine.attached_data_symbol.is_valid()
                    && definition.symbol == machine.attached_data_symbol
                    && !definition.type_parameters.is_empty()
            })
            .map(|definition| {
                let arguments = data_type_parameters
                    .span_or_empty(definition.type_parameters)
                    .iter()
                    .map(|owner_parameter| {
                        let parameter = local_type_parameters
                            .iter()
                            .find(|parameter| {
                                parameter.name.as_str() == owner_parameter.name.as_str()
                                    && parameter.kind == owner_parameter.kind
                            })
                            .unwrap_or(owner_parameter);
                        symbol_resolved_trees::types::TypeReference::Named {
 supplied_operand_owner: Default::default(),
                            symbol: parameter.symbol,
                            name: parameter.name.clone(),
                        }
                    });
                symbol_resolved_trees::types::TypeReference::Generic(
                    symbol_resolved_trees::types::GenericTypeReference {
                        storage: symbol_resolved_trees::types::GenericTypeReferenceStorage {
 supplied_operand_owner: Default::default(),
                            base_symbol: definition.symbol,
                            base_name: machine
                                .attached_data
                                .clone()
                                .unwrap_or_else(|| definition.name.clone()),
                            lifetime_arguments: definition.lifetime_parameters.clone(),
                            arguments: child_type_references.insert_many(arguments),
                        },
                    },
                )
            });
        }

        if let Some(owner) = data_definitions.iter().find(|definition| {
            definition.symbol == machine.attached_data_symbol
        }) {
            local_type_parameters = crate::symbols::scope::declaration_type_scope(
                &local_type_parameters,
                data_type_parameters.span_or_empty(owner.type_parameters),
            );
        }

        local_type_parameters.extend(crate::symbols::scope::evidence_type_parameters(
            &machine.conformance_bounds,
        ));

        SignatureSymbolTables {
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
        trait_proposition_slots: &trait_proposition_slots,
trait_machine_identity_slots: &trait_machine_identity_slots,
}
        .resolve_type_parameter_kind_symbols(
            machine.type_parameters,
            &local_type_parameters,
            crate::symbols::top_level::MachineParameterArm::ResolveWithAuthoredTraitArguments,
            true,
        );

        let expression_scope = MachineScope {
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            symbol: machine_symbol,
            lexical_requirements: LexicalRequirements::none(),
            type_parameters: &local_type_parameters,
            attached_data_symbol: machine.attached_data_symbol,
            owned_data: &[],
            prior_statements: &[],
        contract_result_data: symbols::SymbolHandle::invalid(),
            inherited_data_members: None,
            data_definitions,
            type_aliases,
            conformances,
            traits,
            operand_owners: &tables.types.operand_lifetime_owners,
            data_members,
            data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints,
        };

        for equation in declarations.declared_type_equations.span_mut_or_empty(machine.declared_type_equations) {
            for reference in [&mut equation.parameter, &mut equation.definition] {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols, child_type_references, type_constraints,
                    &local_type_parameters, reference,
                );
            }
        }

        for owned_data in machine_owned_data.span_mut_or_empty(machine.owned_data) {
            owned_data.symbol =
                next_child_of_kind(&mut machine_children, symbols, SymbolKind::Field);
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                &mut owned_data.type_reference,
);
            if owned_data.initial_value.is_valid() {
                assign_expression_table_symbols(
                    symbols,
&expression_scope,
                    &[],
                    SymbolHandle::invalid(),
                    expression_table,
                    child_type_references,
                    owned_data.initial_value,
                );
            }
        }

        if !machine.trait_default_origin.trait_source.is_empty() {
            machine.trait_default_origin.declaring_trait = top_level_symbol_for_source(
                symbols,
                SymbolKind::Trait,
                &machine.trait_default_origin.trait_source,
            );
        }

        for conformance in machine_trait_conformances.span_mut_or_empty(machine.satisfies) {
            let exact_requirement = conformance.requirement.as_ref().map(|requirement| {
                let path = format!("{}::{}", conformance.name.as_str(), requirement.as_str());
                let symbol = symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &path,
                        &[SymbolKind::Machine],
                        requirement.source_span(),
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
                let token_family = !matches!(
                    symbols.lookup_signature_free_top_level_from_source_matching(
                        &path,
                        &[SymbolKind::Machine],
                        requirement.source_span(),
                        |candidate| token_requirement_symbols.contains(&candidate),
                    ),
                    symbols::SymbolLookup::NotFound
                );
                (path, symbol, requirement.source_span(), token_family)
            });
            let target_is_trait = match exact_requirement {
                Some((_, _, _, true)) => {
                    conformance.symbol = SymbolHandle::invalid();
                    false
                }
                Some((path, symbol, source_span, false)) if symbol.is_valid() => {
                    if top_level_requirement_symbols.contains(&symbol) {
                        conformance.symbol = symbol;
                    } else {
                        conformance.symbol = SymbolHandle::invalid();
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "machine satisfaction target `{path}` is an ordinary machine, not an explicit top-level `boundary requirement`"
                            ))
                            .with_source_span(source_span),
                        );
                    }
                    false
                }
                Some((path, _, source_span, false)) => {
                    conformance.symbol = top_level_symbol_for_source(
                        symbols,
                        SymbolKind::Trait,
                        &conformance.name,
                    );
                    let names_operator = !matches!(
                        symbols.lookup_signature_free_top_level_from_source_matching(
                            &path,
                            &[SymbolKind::Operator],
                            source_span,
                            |_| true,
                        ),
                        symbols::SymbolLookup::NotFound
                    );
                    if refinement_symbols.contains(&conformance.symbol) {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "machine satisfaction target `{path}` names a transparent refinement — refinements bound existing conformance evidence, they are not satisfied directly"
                            ))
                            .with_source_span(source_span),
                        );
                        conformance.symbol = SymbolHandle::invalid();
                    } else if !conformance.symbol.is_valid() && !names_operator {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "machine satisfaction target `{path}` does not resolve to an exact trait requirement or top-level `boundary requirement`"
                            ))
                            .with_source_span(source_span),
                        );
                    }
                    conformance.symbol.is_valid()
                }
                None => {
                    conformance.symbol = SymbolHandle::invalid();
                    false
                }
            };
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                conformance.arguments,
            );
            crate::symbols::type_references::assign_trait_implementation_argument_symbols(
                symbols, child_type_references, &local_type_parameters,
                conformance.symbol, conformance.arguments,
            );
            crate::symbols::type_references::assign_type_argument_value_expression_symbols(
                symbols,
                &expression_scope,
                expression_table,
                child_type_references,
                conformance.arguments,
            );
            if conformance.via_expression.is_valid() {
                assign_expression_table_symbols(
                    symbols,
&expression_scope,
                    &[],
                    SymbolHandle::invalid(),
                    expression_table,
                    child_type_references,
                    conformance.via_expression,
                );
            }
            if target_is_trait
                && let Some((_, proposition_slots)) = trait_proposition_slots
                    .iter()
                    .find(|(symbol, _)| *symbol == conformance.symbol)
            {
                assign_proposition_family_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    conformance.arguments,
                    proposition_slots,
                );
            }
            if target_is_trait
                && let Some((_, machine_slots)) = trait_machine_identity_slots
                    .iter()
                    .find(|(symbol, _)| *symbol == conformance.symbol)
            {
                assign_machine_declaration_identity_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    conformance.arguments,
                    machine_slots,
                );
            }
        }

        crate::symbols::top_level::assign_conformance_bound_symbols(
            symbols,
            child_type_references,
            type_constraints,
            &local_type_parameters,
            &mut machine.conformance_bounds,
            &trait_proposition_slots,
            &trait_machine_identity_slots,
        );

        for state in machine_state_handles
            .span_or_empty(machine.states)
            .iter()
            .copied()
        {
            let state = machine_states.get_mut(state);
            let declaration = next_child_of_kind(&mut machine_children, symbols, SymbolKind::State);
            if !state.symbol.is_valid() {
                state.symbol = declaration;
            }
            let state_symbol = state.symbol;
            let mut state_children = symbols.child_handles(state_symbol).into_iter().flatten();

            for parameter in state_parameters.span_mut_or_empty(state.parameters) {
                let declaration =
                    next_child_of_kind(&mut state_children, symbols, SymbolKind::Parameter);
                if !parameter.symbol.is_valid() {
                    parameter.symbol = declaration;
                }
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    &mut parameter.type_reference,
);
            }

            if let Some(return_type) = &mut state.return_type {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &local_type_parameters,
                    return_type,
);
            }
        }
    });

        let origins = machines
            .iter()
            .map(|machine| {
                let origin = &machine.generic_data_origin;
                if !origin.template_source.is_source_backed() {
                    return None;
                }
                let mut candidates = machines.iter().filter(|template| {
                    !template
                        .generic_data_origin
                        .template_source
                        .is_source_backed()
                        && template.name.source_span() == origin.template_source.source_span()
                        && template.target_family_interface == machine.target_family_interface
                });
                match (candidates.next(), candidates.next()) {
                    (Some(template), None) => Some((template.symbol, machine.attached_data_symbol)),
                    _ => {
                        diagnostics.push(
                            Diagnostic::error(
                                "generic-data method has no unique authored template declaration",
                            )
                            .with_source_span(origin.template_source.source_span()),
                        );
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        let mut origins = origins.into_iter();
        machines.for_each_mut(|machine| {
            if let Some((template, owner)) = origins.next().flatten() {
                machine.generic_data_origin.template = template;
                machine.generic_data_origin.closed_owner = owner;
            }
        });
        diagnostics
    }
}
mod mathematical {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolKind, SymbolTable};

    use super::super::top_level::next_child_of_kind;
    use symbol_resolved_trees::data::TypeParameterKind;

    /*
        Write back the allocated top-level symbol for each mathematical
        `let`/`boundary let`, then its binder and parameter children in the same
        order `insert_mathematical_symbol_children` registered them.
    */
    pub(super) fn assign_mathematical_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let binders = &mut program.tables.declarations.data_type_parameters;
        let parameters = &mut program.tables.declarations.mathematical_parameters;
        let definitions = &mut program.roots.mathematical_definitions;

        definitions.for_each_mut(|definition| {
            let mut children = symbols
                .child_handles(definition.symbol)
                .into_iter()
                .flatten();
            for binder in binders.span_mut_or_empty(definition.binders) {
                let kind = match binder.kind {
                    TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                    _ => SymbolKind::TypeParameter,
                };
                binder.symbol = next_child_of_kind(&mut children, symbols, kind);
            }
            for parameter in parameters.span_mut_or_empty(definition.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
            }
        });
    }
}
mod measures {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolKind, SymbolTable};

    use super::next_child_of_kind;
    use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints;

    pub(super) fn assign_measure_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
        let references = &mut program.tables.declarations.child_type_references;
        let constraints = &program.tables.types.constraints;
        program.roots.measures.for_each_mut(|measure| {
            if let Some(parameter) = &mut measure.parameter {
                let mut children = symbols.child_handles(measure.symbol).into_iter().flatten();
                parameter.symbol =
                    next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    references,
                    constraints,
                    &[],
                    &mut parameter.type_reference,
                );
            }
            if let Some(return_type) = &mut measure.return_type {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    references,
                    constraints,
                    &[],
                    return_type,
                );
            }
        });
    }
}
mod operators {
    use arena::Arena;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::top_level::next_child_of_kind;
    use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints;
    use symbol_resolved_trees::data::TypeParameterKind;

    pub(super) fn assign_root_operator_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let type_constraints = &program.tables.types.constraints;
        let declarations = &mut program.tables.declarations;
        let data_type_parameters = &mut declarations.data_type_parameters;
        let state_parameters = &mut declarations.state_parameters;
        let child_type_references = &mut declarations.child_type_references;
        program.roots.operators.for_each_mut(|operator| {
            assign_operator_children(
                symbols,
                data_type_parameters,
                state_parameters,
                child_type_references,
                type_constraints,
                operator,
            );
        });
    }

    pub(super) fn assign_operator_symbols(
        symbols: &SymbolTable,
        siblings: &mut impl Iterator<Item = SymbolHandle>,
        data_type_parameters: &mut Arena<symbol_resolved_trees::data::TypeParameter>,
        state_parameters: &mut Arena<symbol_resolved_trees::signature::StateParameter>,
        child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
        operator: &mut symbol_resolved_trees::operator::OperatorDefinition,
    ) {
        let declaration = next_child_of_kind(siblings, symbols, SymbolKind::Operator);
        if !operator.symbol.is_valid() {
            operator.symbol = declaration;
        }
        assign_operator_children(
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
            operator,
        );
    }

    fn assign_operator_children(
        symbols: &SymbolTable,
        data_type_parameters: &mut Arena<symbol_resolved_trees::data::TypeParameter>,
        state_parameters: &mut Arena<symbol_resolved_trees::signature::StateParameter>,
        child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
        operator: &mut symbol_resolved_trees::operator::OperatorDefinition,
    ) {
        let mut operator_children = symbols.child_handles(operator.symbol).into_iter().flatten();

        for type_parameter in data_type_parameters.span_mut_or_empty(operator.type_parameters) {
            type_parameter.symbol =
                next_child_of_kind(&mut operator_children, symbols, SymbolKind::TypeParameter);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(operator.type_parameters)
            .to_vec();
        for index in 0..operator.type_parameters.len() {
            let kind = data_type_parameters.span_or_empty(operator.type_parameters)[index]
                .kind
                .clone();
            let mut resolved_kind = kind;
            let type_reference = match &mut resolved_kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => type_reference,
                _ => continue,
            };
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                type_reference,
            );
            data_type_parameters.span_mut_or_empty(operator.type_parameters)[index].kind =
                resolved_kind;
        }
        for parameter in state_parameters.span_mut_or_empty(operator.parameters) {
            parameter.symbol =
                next_child_of_kind(&mut operator_children, symbols, SymbolKind::Parameter);
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                &mut parameter.type_reference,
            );
        }
        if let Some(return_type) = &mut operator.return_type {
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                return_type,
            );
        }
    }
}
mod propositions {
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolKind, SymbolTable};

    use super::super::top_level::next_child_of_kind;
    use symbol_resolved_trees::proposition::PropositionBinderKind;

    pub(super) fn assign_proposition_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let binders = &mut program.tables.declarations.proposition_binders;
        let parameters = &mut program.tables.declarations.state_parameters;
        let propositions = &mut program.roots.propositions;

        propositions.for_each_mut(|proposition| {
            let mut children = symbols
                .child_handles(proposition.symbol)
                .into_iter()
                .flatten();
            for binder in binders.span_mut_or_empty(proposition.binders) {
                let kind = match binder.kind {
                    PropositionBinderKind::Machine => SymbolKind::PropositionMachineParameter,
                    _ => SymbolKind::TypeParameter,
                };
                binder.symbol = next_child_of_kind(&mut children, symbols, kind);
            }
            for parameter in parameters.span_mut_or_empty(proposition.parameters) {
                parameter.symbol =
                    next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
            }
        });
    }
}
mod traits {
    use diagnostics::Diagnostic;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::lookup::top_level_symbol_for_source;
    use crate::symbols::top_level::{
        SignatureSymbolTables, assign_machine_parameter_signature_symbols, next_child_of_kind,
        trait_parameter_tables,
    };
    use crate::symbols::type_references::{
        assign_trait_bound_slot_argument_symbols,
        assign_type_reference_argument_symbols_with_constraints,
    };

    pub(super) fn assign_trait_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let (refinement_symbols, trait_proposition_slots, trait_machine_identity_slots) =
            trait_parameter_tables(program);
        let type_constraints = &program.tables.types.constraints;
        let declarations = &mut program.tables.declarations;
        let data_type_parameters = &mut declarations.data_type_parameters;
        let trait_requirements = &mut declarations.trait_requirements;
        let trait_machine_signatures = &mut declarations.trait_machine_signatures;
        let state_parameters = &mut declarations.state_parameters;
        let child_type_references = &mut declarations.child_type_references;
        program.roots.traits.for_each_mut(|trait_definition| {
        let trait_symbol = trait_definition.symbol;
        let mut trait_children = symbols.child_handles(trait_symbol).into_iter().flatten();

        for type_parameter in
            data_type_parameters.span_mut_or_empty(trait_definition.type_parameters)
        {
            let kind = match type_parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Implementation => {
                    SymbolKind::ConformanceParameter
                }
                symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => {
                    SymbolKind::PropositionParameter
                }
                symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            type_parameter.symbol = next_child_of_kind(&mut trait_children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(trait_definition.type_parameters)
            .to_vec();

        let trait_name = trait_definition.name.clone();
        if let Some(base) = &mut trait_definition.refines {
            base.symbol =
                top_level_symbol_for_source(symbols, SymbolKind::Trait, &base.name);
            if refinement_symbols.contains(&base.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "transparent refinement `{}` refines `{}`, which is itself a refinement; refine the nominal base directly",
                    trait_name.as_str(),
                    base.name.as_str(),
                )));
            }
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                base.arguments,
);
            assign_trait_bound_slot_argument_symbols(
                symbols, child_type_references, &local_type_parameters, base.symbol, base.arguments,
                &trait_proposition_slots, &trait_machine_identity_slots,
            );
        }

        for bound in &mut trait_definition.conformance_bounds {
            bound.binder = local_type_parameters.iter()
                .find(|parameter| parameter.name == bound.binder_name
                    && parameter.kind == symbol_resolved_trees::data::TypeParameterKind::Implementation)
                .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
        }

        SignatureSymbolTables {
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
        trait_proposition_slots: &trait_proposition_slots,
trait_machine_identity_slots: &trait_machine_identity_slots,
}
        .resolve_type_parameter_kind_symbols(
            trait_definition.type_parameters,
            &local_type_parameters,
            crate::symbols::top_level::MachineParameterArm::ResolveWithAuthoredTraitArguments,
            true,
        );

        crate::symbols::top_level::assign_conformance_bound_symbols(
            symbols,
            child_type_references,
            type_constraints,
            &local_type_parameters,
            &mut trait_definition.conformance_bounds,
            &trait_proposition_slots,
            &trait_machine_identity_slots,
        );

        for requirement in trait_requirements.span_mut_or_empty(trait_definition.requires) {
            requirement.symbol =
                top_level_symbol_for_source(symbols, SymbolKind::Trait, &requirement.name);
            if refinement_symbols.contains(&requirement.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "trait requirement `{}` is a transparent refinement — refinements bound evidence, they are not conformance targets",
                    requirement.name.as_str(),
                )));
            }
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                requirement.arguments,
            );
            assign_trait_bound_slot_argument_symbols(
                symbols,
                child_type_references,
                &local_type_parameters,
                requirement.symbol,
                requirement.arguments,
                &trait_proposition_slots,
                &trait_machine_identity_slots,
            );
        }

        for machine in trait_machine_signatures.span_mut_or_empty(trait_definition.machines).iter_mut()
            .chain(trait_definition.refinement_clauses.iter_mut().map(|clause| &mut clause.signature)) {
            machine.symbol = next_child_of_kind(&mut trait_children, symbols, SymbolKind::State);
            let machine_symbol = machine.symbol;
            assign_machine_parameter_signature_symbols(
                &mut SignatureSymbolTables {
                    symbols,
                    data_type_parameters,
                    state_parameters,
                    child_type_references,
                    type_constraints,
                trait_proposition_slots: &trait_proposition_slots,
trait_machine_identity_slots: &trait_machine_identity_slots,
},
                machine,
                machine_symbol,
                &local_type_parameters,
);
        }
    });
        diagnostics
    }
}
mod type_aliases {
    /* Assign alias-owned parameters and references without a nominal data owner. */

    use super::{
        MachineParameterArm, SignatureSymbolTables, next_child_of_kind, trait_parameter_tables,
    };
    use crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    pub(super) fn assign_type_alias_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let (_, trait_proposition_slots, trait_machine_identity_slots) =
            trait_parameter_tables(program);
        let constraints = &program.tables.types.constraints;
        let declarations = &mut program.tables.declarations;
        let parameters = &mut declarations.data_type_parameters;
        let state_parameters = &mut declarations.state_parameters;
        let references = &mut declarations.child_type_references;
        program.roots.type_aliases.for_each_mut(|alias| {
            let mut children = symbols.child_handles(alias.symbol).into_iter().flatten();
            for parameter in parameters.span_mut_or_empty(alias.type_parameters) {
                let kind = match parameter.kind {
                    TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
                    TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                    TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
                    _ => SymbolKind::TypeParameter,
                };
                parameter.symbol = next_child_of_kind(&mut children, symbols, kind);
            }
            let locals = parameters.span_or_empty(alias.type_parameters).to_vec();
            SignatureSymbolTables {
                symbols,
                data_type_parameters: parameters,
                state_parameters,
                child_type_references: references,
                type_constraints: constraints,
                trait_proposition_slots: &trait_proposition_slots,
                trait_machine_identity_slots: &trait_machine_identity_slots,
            }
            .resolve_type_parameter_kind_symbols(
                alias.type_parameters,
                &locals,
                MachineParameterArm::ResolveWithAuthoredTraitArguments,
                true,
            );
            for bound in &mut alias.conformance_bounds {
                bound.binder = locals
                    .iter()
                    .find(|parameter| {
                        parameter.name == bound.binder_name
                            && parameter.kind == TypeParameterKind::Implementation
                    })
                    .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
            }
            super::assign_conformance_bound_symbols(
                symbols,
                references,
                constraints,
                &locals,
                &mut alias.conformance_bounds,
                &trait_proposition_slots,
                &trait_machine_identity_slots,
            );
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                references,
                constraints,
                &locals,
                &mut alias.expansion,
            );
        });
    }
}

use crate::tables::resolved_declarations;
use arena::Arena;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::top_level::data::assign_data_symbols;
use super::top_level::domains::assign_domain_symbols;
use super::top_level::machines::assign_machine_symbols;
use super::top_level::mathematical::assign_mathematical_symbols;
use super::top_level::operators::assign_root_operator_symbols;
use super::top_level::propositions::assign_proposition_symbols;
use super::top_level::traits::assign_trait_symbols;
use crate::symbols::lookup::top_level_symbol_for_source;
use std::collections::HashSet;
use symbol_resolved_trees::data::MachineParameterContract;
use symbol_resolved_trees::data::PropositionParameterSignature;
use symbol_resolved_trees::data::TypeParameter;
use symbol_resolved_trees::data::TypeParameterKind;
use symbol_resolved_trees::machine::GenericConformanceBound;
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::signature::StateSignature;
use symbol_resolved_trees::types::TypeConstraint;
use symbol_resolved_trees::types::TypeReference;

/*
    Trait-side tables both machine and trait assignment consult: which
    traits are transparent refinements, and each trait's per-parameter
    proposition/machine-identity kind slots keyed by its top-level symbol.
*/
pub(super) fn trait_parameter_tables(
    program: &SymbolResolvedTrees,
) -> (
    HashSet<SymbolHandle>,
    Vec<(SymbolHandle, Vec<bool>)>,
    Vec<(SymbolHandle, Vec<bool>)>,
) {
    let refinement_symbols = program
        .roots
        .traits
        .iter()
        .filter(|definition| definition.refines.is_some())
        .map(|definition| definition.symbol)
        .collect();
    let trait_proposition_slots = program
        .roots
        .traits
        .iter()
        .map(|trait_definition| {
            (
                trait_definition.symbol,
                resolved_declarations(program)
                    .data_type_parameters
                    .span_or_empty(trait_definition.type_parameters)
                    .iter()
                    .map(|parameter| {
                        matches!(parameter.kind, TypeParameterKind::Proposition { .. })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let trait_machine_identity_slots = program
        .roots
        .traits
        .iter()
        .map(|definition| {
            (
                definition.symbol,
                resolved_declarations(program)
                    .data_type_parameters
                    .span_or_empty(definition.type_parameters)
                    .iter()
                    .map(|parameter| {
                        matches!(
                            parameter.kind,
                            TypeParameterKind::Machine {
                                contract: MachineParameterContract::RequirementIdentity
                            }
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    (
        refinement_symbols,
        trait_proposition_slots,
        trait_machine_identity_slots,
    )
}

pub(super) fn assign_top_level_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) -> Vec<diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    assign_domain_symbols(program, symbols);
    assign_data_symbols(program, symbols);
    type_aliases::assign_type_alias_symbols(program, symbols);
    assign_conformance_parameter_symbols(program, symbols);
    diagnostics.extend(assign_machine_symbols(program, symbols));
    assign_proposition_symbols(program, symbols);
    assign_mathematical_symbols(program, symbols);
    assign_root_operator_symbols(program, symbols);
    measures::assign_measure_symbols(program, symbols);
    diagnostics.extend(assign_trait_symbols(program, symbols));

    diagnostics
}

fn assign_conformance_parameter_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let (_, trait_proposition_slots, trait_machine_identity_slots) =
        trait_parameter_tables(program);
    let type_constraints = &program.tables.types.constraints;
    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;

    program.roots.conformances.for_each_mut(|conformance| {
        let mut children = symbols
            .child_handles(conformance.symbol)
            .into_iter()
            .flatten();
        for parameter in data_type_parameters.span_mut_or_empty(conformance.type_parameters) {
            let kind = match parameter.kind {
                TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
                TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
                _ => SymbolKind::TypeParameter,
            };
            parameter.symbol = next_child_of_kind(&mut children, symbols, kind);
        }
        let local_type_parameters = data_type_parameters
            .span_or_empty(conformance.type_parameters)
            .to_vec();

        SignatureSymbolTables {
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
            trait_proposition_slots: &trait_proposition_slots,
            trait_machine_identity_slots: &trait_machine_identity_slots,
        }
        .resolve_type_parameter_kind_symbols(
            conformance.type_parameters,
            &local_type_parameters,
            MachineParameterArm::ResolveWithAuthoredTraitArguments,
            false,
        );
    });
}

pub(super) fn next_child_of_kind(
    children: &mut impl Iterator<Item = SymbolHandle>,
    symbols: &SymbolTable,
    kind: SymbolKind,
) -> SymbolHandle {
    let Some(child) = children.next() else {
        return SymbolHandle::invalid();
    };

    if symbols.get(child).kind == kind {
        child
    } else {
        SymbolHandle::invalid()
    }
}

/*
    The symbol table plus the four arenas every machine-parameter signature
    walk borrows together; the bundle never splits between callers, so the
    walk takes it as one context instead of a nine-position argument list.
*/
pub(super) struct SignatureSymbolTables<'a> {
    pub symbols: &'a SymbolTable,
    pub data_type_parameters: &'a mut Arena<TypeParameter>,
    pub state_parameters: &'a mut Arena<StateParameter>,
    pub child_type_references: &'a mut Arena<TypeReference>,
    pub type_constraints: &'a Arena<TypeConstraint>,
    pub trait_proposition_slots: &'a [(SymbolHandle, Vec<bool>)],
    pub trait_machine_identity_slots: &'a [(SymbolHandle, Vec<bool>)],
}

impl SignatureSymbolTables<'_> {
    /*
        Assigns a machine parameter's structural contract signature, when the
        contract carries one — the guard every parameter-kind site performs.
    */
    pub(super) fn assign_parameter_contract_symbols(
        &mut self,
        contract: &mut MachineParameterContract,
        owner_symbol: SymbolHandle,
        inherited_type_parameters: &[TypeParameter],
    ) {
        if let Some(signature) = contract.structural_mut() {
            assign_machine_parameter_signature_symbols(
                self,
                signature,
                owner_symbol,
                inherited_type_parameters,
            );
        }
    }

    /*
        Resolves each type parameter's kind payload in place: Const and Value
        type references, Machine and Proposition contract signatures. Machines,
        traits, and conformances run the identical pass over
        `type_parameters`; their Machine-arm coverage differs
        (`machine_parameter_arm`), and conformances skip the Const/Value arms
        entirely (`resolve_reference_arms`).
    */
    pub(super) fn resolve_type_parameter_kind_symbols(
        &mut self,
        span: arena::HandleSpan<TypeParameter>,
        local_type_parameters: &[TypeParameter],
        machine_parameter_arm: MachineParameterArm,
        resolve_reference_arms: bool,
    ) {
        let symbols = self.symbols;
        for index in 0..span.len() {
            let (parameter_symbol, kind) = {
                let parameter = &self.data_type_parameters.span_or_empty(span)[index];
                (parameter.symbol, parameter.kind.clone())
            };
            let resolved_kind = match kind {
                TypeParameterKind::Const { mut type_reference } if resolve_reference_arms => {
                    crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        self.child_type_references,
                        self.type_constraints,
                        local_type_parameters,
                        &mut type_reference,
                    );
                    TypeParameterKind::Const { type_reference }
                }
                TypeParameterKind::Value { mut type_reference } if resolve_reference_arms => {
                    crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        self.child_type_references,
                        self.type_constraints,
                        local_type_parameters,
                        &mut type_reference,
                    );
                    TypeParameterKind::Value { type_reference }
                }
                TypeParameterKind::Machine { mut contract } => {
                    if matches!(
                        machine_parameter_arm,
                        MachineParameterArm::ResolveWithAuthoredTraitArguments
                    ) && let MachineParameterContract::AuthoredNominal {
                        trait_arguments, ..
                    } = &contract
                    {
                        crate::symbols::type_references::assign_type_reference_argument_symbols_with_constraints(
                            symbols,
                            self.child_type_references,
                            self.type_constraints,
                            local_type_parameters,
                            *trait_arguments,
                        );
                    }
                    self.assign_parameter_contract_symbols(
                        &mut contract,
                        parameter_symbol,
                        local_type_parameters,
                    );
                    TypeParameterKind::Machine { contract }
                }
                TypeParameterKind::Proposition { mut contract } => {
                    assign_proposition_parameter_signature_symbols(
                        symbols,
                        self.state_parameters,
                        self.child_type_references,
                        self.type_constraints,
                        &mut contract,
                        parameter_symbol,
                        local_type_parameters,
                    );
                    TypeParameterKind::Proposition { contract }
                }
                other => other,
            };
            self.data_type_parameters.span_mut_or_empty(span)[index].kind = resolved_kind;
        }
    }
}

/*
    How much of the shared type-parameter pass a site's Machine arm performs:
    every current site resolves the contract plus its authored-nominal
    trait-argument symbols.
*/
#[derive(Clone, Copy)]
pub(super) enum MachineParameterArm {
    ResolveWithAuthoredTraitArguments,
}

pub(super) fn assign_machine_parameter_signature_symbols(
    tables: &mut SignatureSymbolTables<'_>,
    contract: &mut StateSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[TypeParameter],
) {
    use symbol_resolved_trees::data::TypeParameterKind;

    contract.symbol = owner_symbol;
    let mut children = tables
        .symbols
        .child_handles(owner_symbol)
        .into_iter()
        .flatten();

    for parameter in tables
        .data_type_parameters
        .span_mut_or_empty(contract.type_parameters)
    {
        let kind = match parameter.kind {
            TypeParameterKind::Implementation => SymbolKind::ConformanceParameter,
            TypeParameterKind::Machine { .. } => SymbolKind::MachineParameter,
            TypeParameterKind::Proposition { .. } => SymbolKind::PropositionParameter,
            _ => SymbolKind::TypeParameter,
        };
        parameter.symbol = next_child_of_kind(&mut children, tables.symbols, kind);
    }

    let mut local_type_parameters = tables
        .data_type_parameters
        .span_or_empty(contract.type_parameters)
        .to_vec();
    local_type_parameters.extend_from_slice(inherited_type_parameters);

    for bound in &mut contract.conformance_bounds {
        bound.binder = local_type_parameters
            .iter()
            .find(|parameter| {
                parameter.kind == TypeParameterKind::Implementation
                    && parameter.name == bound.binder_name
            })
            .map_or(SymbolHandle::invalid(), |parameter| parameter.symbol);
    }
    assign_conformance_bound_symbols(
        tables.symbols,
        tables.child_type_references,
        tables.type_constraints,
        &local_type_parameters,
        &mut contract.conformance_bounds,
        tables.trait_proposition_slots,
        tables.trait_machine_identity_slots,
    );

    tables.resolve_type_parameter_kind_symbols(
        contract.type_parameters,
        &local_type_parameters,
        MachineParameterArm::ResolveWithAuthoredTraitArguments,
        true,
    );

    for parameter in tables
        .state_parameters
        .span_mut_or_empty(contract.parameters)
    {
        parameter.symbol = next_child_of_kind(&mut children, tables.symbols, SymbolKind::Parameter);
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
            tables.symbols,
            tables.child_type_references,
            tables.type_constraints,
            &local_type_parameters,
            &mut parameter.type_reference,
        );
    }
    if let Some(return_type) = &mut contract.return_type {
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
            tables.symbols,
            tables.child_type_references,
            tables.type_constraints,
            &local_type_parameters,
            return_type,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn assign_proposition_parameter_signature_symbols(
    symbols: &SymbolTable,
    state_parameters: &mut Arena<StateParameter>,
    child_type_references: &mut Arena<TypeReference>,
    type_constraints: &Arena<TypeConstraint>,
    contract: &mut PropositionParameterSignature,
    owner_symbol: SymbolHandle,
    inherited_type_parameters: &[TypeParameter],
) {
    let mut children = symbols.child_handles(owner_symbol).into_iter().flatten();
    for parameter in state_parameters.span_mut_or_empty(contract.parameters) {
        parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
        crate::symbols::type_references::assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            type_constraints,
            inherited_type_parameters,
            &mut parameter.type_reference,
        );
    }
}

/*
    Assigns every conformance bound's trait symbol plus its type-reference
    and bound-slot argument symbols; machine and trait declarations run the
    identical pass over `conformance_bounds`.
*/
pub(super) fn assign_conformance_bound_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<TypeReference>,
    type_constraints: &Arena<TypeConstraint>,
    local_type_parameters: &[TypeParameter],
    bounds: &mut [GenericConformanceBound],
    trait_proposition_slots: &[(SymbolHandle, Vec<bool>)],
    trait_machine_identity_slots: &[(SymbolHandle, Vec<bool>)],
) {
    for bound in bounds {
        bound.trait_symbol =
            top_level_symbol_for_source(symbols, SymbolKind::Trait, &bound.trait_name);
        crate::symbols::type_references::assign_type_reference_argument_symbols_with_constraints(
            symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            bound.arguments,
        );
        crate::symbols::type_references::assign_trait_bound_slot_argument_symbols(
            symbols,
            child_type_references,
            local_type_parameters,
            bound.trait_symbol,
            bound.arguments,
            trait_proposition_slots,
            trait_machine_identity_slots,
        );
    }
}
