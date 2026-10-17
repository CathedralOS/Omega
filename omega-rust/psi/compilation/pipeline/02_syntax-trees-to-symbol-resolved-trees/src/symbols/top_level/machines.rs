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

pub(super) fn assign_machine_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
) -> Vec<Diagnostic> {
    let root_machine_symbols = symbols
        .child_handles(symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| symbols.get(*symbol).kind == SymbolKind::Machine)
        .collect::<Vec<_>>();
    let top_level_requirement_symbols = program
        .machines
        .iter()
        .zip(root_machine_symbols.iter().copied())
        .filter_map(|(machine, symbol)| {
            matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::TopLevelRequirement
            )
            .then_some(symbol)
        })
        .collect::<Vec<_>>();
    let token_requirement_symbols = program
        .machines
        .iter()
        .zip(root_machine_symbols)
        .filter_map(|(machine, symbol)| {
            (machine.spelling.is_some() && top_level_requirement_symbols.contains(&symbol))
                .then_some(symbol)
        })
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    // A transparent refinement (`trait R = Base`) bounds an existing base
    // conformance; it is never itself a `satisfies` target.
    let (refinement_symbols, trait_proposition_slots, trait_machine_identity_slots) =
        trait_parameter_tables(symbols, program);
    // Resolve every attachment before visiting bodies: a method may be declared
    // after its caller or outside its carrier's module. Bodies borrow this exact
    // declaration cohort instead of rebuilding attachment identity from a name.
    program.machines.for_each_mut(|machine| {
        machine.attached_data_symbol = machine
            .attached_data
            .as_ref()
            .map(|attached| top_level_symbol_for_source(symbols, SymbolKind::Data, attached))
            .unwrap_or_else(SymbolHandle::invalid);
    });
    let attached_machines = crate::symbols::scope::attached_machines(program);
    let conformance_scopes = match crate::symbols::scope::ConformanceTypeScopes::of(program) {
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
        machines,
        ..
    } = &mut program.roots;

    machines.for_each_mut(|machine| {
        let machine_symbol = machine.symbol;
        let mut machine_children = symbols.child_handles(machine_symbol).into_iter().flatten();

        for type_parameter in data_type_parameters.span_mut_or_empty(machine.type_parameters) {
            let kind = match type_parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            // The iterator is declaration positional even on a seeded revisit.
            // Retaining an already assigned binder does not remove its child
            // slot: the following entry state must consume its own declaration.
            let declaration = next_child_of_kind(&mut machine_children, symbols, kind);
            if !type_parameter.symbol.is_valid() {
                type_parameter.symbol = declaration;
            } else if symbols.get(type_parameter.symbol).kind != kind {
                diagnostics.push(Diagnostic::error(
                    "retained machine parameter has a different declaration kind",
                ));
            }
        }
        let mut local_type_parameters = crate::symbols::scope::declaration_type_scope(
            data_type_parameters.span_or_empty(machine.type_parameters),
            conformance_scopes.parameters(machine.symbol),
        );

        // Seeded resolution revisits retained declarations. Their application
        // is already resolved; preserve its exact arena span rather than
        // replacing an unchanged prefix with freshly allocated arguments.
        if machine.attached_data_application.is_none() {
        machine.attached_data_application = data_definitions
            .iter()
            .find(|definition| {
                machine.attached_data_symbol.is_valid()
                    && definition.symbol == machine.attached_data_symbol
                    && !definition.type_parameters.is_empty()
            })
            .map(|definition| {
                // Source resolution is where repeated binder spellings acquire
                // their declaration identities. A method may strengthen its
                // own bound without changing the owner's parameter globally.
                // Unmentioned or differently kinded parameters remain the
                // owner's exact binders, not unrelated method-local names.
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

        // The attached application above and its source signature must name
        // the same owner telescope. In particular a universal cleanup hook
        // cannot introduce method-local binders to repair an unresolved T.
        if let Some(owner) = data_definitions.iter().find(|definition| {
            definition.symbol == machine.attached_data_symbol
        }) {
            local_type_parameters = crate::symbols::scope::declaration_type_scope(
                &local_type_parameters,
                data_type_parameters.span_or_empty(owner.type_parameters),
            );
        }

        for bound in &mut machine.conformance_bounds {
            bound.binder = next_child_of_kind(
                &mut machine_children,
                symbols,
                SymbolKind::ConformanceParameter,
            );
        }
        // Machine evidence lives in a separate telescope. Retain those exact
        // lexical binders in the local selection context for trait arguments.
        local_type_parameters.extend(machine.conformance_bounds.iter().map(|bound| {
            symbol_resolved_trees::data::TypeParameter {
                symbol: bound.binder,
                name: bound.binder_name.clone(),
                kind: symbol_resolved_trees::data::TypeParameterKind::Implementation,
                bounds: Default::default(),
            }
        }));

        // MP1: machine-parameter contracts are real signature data, not
        // parser-only text. Resolve their parameter/result type references in
        // the declaring machine's generic/self context. Contract-parameter
        // symbols themselves join the modular-body checker in MP3; assigning
        // types here prevents representation loss meanwhile.
        SignatureSymbolTables {
            symbols,
            data_type_parameters,
            state_parameters,
            child_type_references,
            type_constraints,
        }
        .resolve_type_parameter_kind_symbols(
            machine.type_parameters,
            &local_type_parameters,
            crate::symbols::top_level::MachineParameterArm::ResolveWithAuthoredTraitArguments,
            true,
        );

        // Machine-owned value expressions (owned-data initializers and
        // conformance `via` paths) resolve against the same enclosing scope.
        let expression_scope = MachineScope {
            attached_machines: &attached_machines,
            resolution_indexes: &program.resolution_indexes,
            symbol: machine_symbol,
            lexical_requirements: LexicalRequirements::none(),
            type_parameters: &local_type_parameters,
            attached_data: machine.attached_data.as_ref(),
            attached_data_symbol: machine.attached_data_symbol,
            owned_data: &[],
            prior_statements: &[],
            inherited_data_members: None,
            data_definitions,
            data_members,
            data_type_parameters,
            data_payload_fields: &declarations.data_payload_fields,
            type_constraints,
        };

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

        // Default synthesis spells the authoring trait's logical path, as it
        // does for the instantiation's requirement edge, so the same lookup
        // rejoins that exact declaration in its own package.
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
                    // The family is source-visible, but its exact declaration
                    // needs the complete typed signature. Do not retain an
                    // arbitrary sibling as the satisfaction target.
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
                    // Presence of a source-visible operator family is enough
                    // here. Its complete parameter/result-domain signature is
                    // selected during typed settlement, just as for token
                    // requirement families above; no sibling is chosen here.
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
                    // The generated contract and its authored prototype share
                    // provenance, not declaration identity. A closed clone
                    // retains the exact role of the declaration it instantiates.
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
