/*
    Symbol assignment for type references: nominal bounds, trait binder
    arguments, cast targets and the value expressions embedded in types resolve
    in the same lexical declaration scopes as signatures and bodies.
*/

mod assignment {
    use crate::tables::resolved_declarations;
    use arena::{Arena, Handle, HandleSpan};
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

    use super::super::scope::evidence_type_parameters;
    use crate::symbols::lookup::{
        diagnostic_path_source_span, top_level_symbol_for_source, top_level_type_symbol_for_source,
    };
    use crate::symbols::targets::resolve_free_machine_entry_state_symbol;
    use std::mem::take;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::data::MachineParameterContract;
    use symbol_resolved_trees::data::TypeParameter;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::expression::StaticMachineArgument;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::proposition::PropositionBinderKind;
    use symbol_resolved_trees::proposition::PropositionBody;
    use symbol_resolved_trees::types::FixedArrayLength;
    use symbol_resolved_trees::types::TypeConstraint;
    use symbol_resolved_trees::types::TypeReference;
    use symbol_resolved_trees::wire::WireMember;

    /*
        An instantiated use and its retained application are separate type rows.
        Bind both with the same declaration-source rules before typing validates
        their identity. Completed bodies append these rows after header assignment;
        the retained prefix must not be rebound under an extension's namespace.
    */
    pub(in crate::symbols) fn assign_generic_application_origin_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
        frontier: usize,
    ) {
        let type_constraints = &program.tables.types.constraints;
        let child_type_references = &mut program.tables.declarations.child_type_references;
        for (_, origin) in program
            .tables
            .types
            .generic_application_origins
            .iter()
            .skip(frontier)
        {
            for handle in [origin.instance, origin.application] {
                let mut reference = take(child_type_references.get_mut(handle));
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &[],
                    &mut reference,
                );
                *child_type_references.get_mut(handle) = reference;
            }
        }
    }

    /*
        For each trait, the flag each of its type parameters gets from
        `predicate` — the (trait symbol, per-parameter flags) rows the later
        argument walks join on.
    */
    fn trait_parameter_slots(
        program: &SymbolResolvedTrees,
        predicate: impl Fn(&TypeParameter) -> bool,
    ) -> Vec<(SymbolHandle, Vec<bool>)> {
        program
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
                        .map(&predicate)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    }

    /*
        Resolve nominal bound arguments in the same lexical declaration scopes as
        signatures and bodies. A symbol's parent is not a capture ownership proof.
    */
    pub(crate) fn assign_nominal_machine_requirement_arguments(
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        use super::super::scope::{DeclarationTypeScopes, declaration_type_scope};
        use symbol_resolved_trees::data::{
            MachineParameterContract, TypeParameter, TypeParameterKind,
        };
        struct ArgumentScope {
            trait_definition: SymbolHandle,
            arguments: HandleSpan<TypeReference>,
            parameters: Vec<TypeParameter>,
        }
        fn collect(
            program: &SymbolResolvedTrees,
            parameters: &[TypeParameter],
            inherited: &[TypeParameter],
            scopes: &mut Vec<ArgumentScope>,
        ) {
            let lexical = declaration_type_scope(parameters, inherited);
            for parameter in parameters {
                match &parameter.kind {
                    TypeParameterKind::Machine {
                        contract:
                            MachineParameterContract::Nominal {
                                trait_definition,
                                trait_arguments,
                                ..
                            },
                    } => scopes.push(ArgumentScope {
                        trait_definition: *trait_definition,
                        arguments: *trait_arguments,
                        parameters: lexical.clone(),
                    }),
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::Structural(signature),
                    } => {
                        collect(
                            program,
                            program.data_type_parameters(signature.type_parameters),
                            &lexical,
                            scopes,
                        );
                    }
                    _ => {}
                }
            }
        }
        let captures = DeclarationTypeScopes::of(program)?;
        let mut scopes = Vec::new();
        for machine in program.machines.iter() {
            let attached = program
                .data_definitions
                .iter()
                .find(|data| data.symbol == machine.attached_data_symbol)
                .map_or(&[][..], |data| {
                    program.data_type_parameters(data.type_parameters)
                });
            let mut inherited =
                declaration_type_scope(captures.parameters(machine.symbol), attached);
            inherited.extend(evidence_type_parameters(&machine.conformance_bounds));
            collect(
                program,
                program.machine_type_parameters(machine),
                &inherited,
                &mut scopes,
            );
        }
        for owner in program.traits.iter() {
            let parameters = program.trait_type_parameters(owner);
            collect(program, parameters, &[], &mut scopes);
            for signature in program
                .trait_machine_signatures(owner.machines)
                .iter()
                .chain(
                    owner
                        .refinement_clauses
                        .iter()
                        .map(|clause| &clause.signature),
                )
            {
                collect(
                    program,
                    program.data_type_parameters(signature.type_parameters),
                    parameters,
                    &mut scopes,
                );
            }
        }
        let mut collect_root = |parameters: &[TypeParameter]| {
            collect(program, parameters, &[], &mut scopes);
        };
        for owner in program.conformances.iter() {
            collect_root(program.data_type_parameters(owner.type_parameters));
        }
        for owner in program.data_definitions.iter() {
            collect_root(program.data_type_parameters(owner.type_parameters));
        }
        for owner in program.type_aliases.iter() {
            collect_root(program.data_type_parameters(owner.type_parameters));
        }
        for owner in program.domain_definitions.iter() {
            collect_root(program.data_type_parameters(owner.type_parameters));
        }
        let propositions = trait_parameter_slots(program, |parameter| {
            matches!(parameter.kind, TypeParameterKind::Proposition { .. })
        });
        let machines = trait_parameter_slots(program, |parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::RequirementIdentity
                }
            )
        });
        for scope in scopes {
            assign_type_reference_argument_symbols_with_constraints(
                &program.symbols,
                &mut program.tables.declarations.child_type_references,
                &program.tables.types.constraints,
                &scope.parameters,
                scope.arguments,
            );
            assign_trait_bound_slot_argument_symbols(
                &program.symbols,
                &mut program.tables.declarations.child_type_references,
                &scope.parameters,
                scope.trait_definition,
                scope.arguments,
                &propositions,
                &machines,
            );
        }
        Ok(())
    }

    pub(in crate::symbols) fn assign_type_reference_symbols(
        program: &mut SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let cleanup_entries = program
            .machines
            .iter()
            .filter_map(|machine| {
                let entry = program.machine_state_handles(machine.states).first()?;
                Some((program.machine_state(*entry).symbol, machine.symbol))
            })
            .collect::<Vec<_>>();
        let trait_proposition_slots = trait_parameter_slots(program, |parameter| {
            matches!(parameter.kind, TypeParameterKind::Proposition { .. })
        });
        let trait_machine_identity_slots = trait_parameter_slots(program, |parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Machine {
                    contract: MachineParameterContract::RequirementIdentity
                }
            )
        });
        let type_constraints = &program.tables.types.constraints;
        let data_type_parameters = &mut program.tables.declarations.data_type_parameters;
        let data_members = &mut program.tables.declarations.data_members;
        let data_payload_fields = &mut program.tables.declarations.data_payload_fields;
        let child_type_references = &mut program.tables.declarations.child_type_references;
        program
            .roots
            .const_declarations
            .for_each_mut(|declaration| {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &[],
                    &mut declaration.declared_type,
                );
            });
        program
            .roots
            .data_definitions
            .for_each_mut(|data_definition| {
                let type_parameters = data_type_parameters
                    .span_or_empty(data_definition.type_parameters)
                    .to_vec();
                macro_rules! assign {
                    ($reference:expr) => {
                        assign_type_reference_symbol_with_locals_and_constraints(
                            symbols,
                            child_type_references,
                            type_constraints,
                            &type_parameters,
                            $reference,
                        )
                    };
                }
                let declaration_symbol = data_definition.symbol;
                if let Some(selection) = &mut data_definition.cleanup_selection {
                    assign_static_operand_types(
                        symbols,
                        child_type_references,
                        type_constraints,
                        &type_parameters,
                        selection,
                    );
                    crate::symbols::targets::assign_static_argument_symbols(
                        symbols,
                        declaration_symbol,
                        &type_parameters,
                        selection,
                        false,
                    );
                    if let Some((_, machine)) = cleanup_entries
                        .iter()
                        .find(|(entry, _)| *entry == selection.symbol)
                    {
                        selection.symbol = *machine;
                    }
                }
                assign_type_parameter_constraint_symbols(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &type_parameters,
                    data_type_parameters.span_mut_or_empty(data_definition.type_parameters),
                );
                assign!(&mut data_definition.predicate_subject_type);
                if let Some(generic_instance) = &mut data_definition.generic_instance {
                    assign!(generic_instance);
                }
                if let Some(quotient) = &mut data_definition.quotient {
                    assign!(&mut quotient.carrier);
                    let relation_name = quotient.relation.joined_path();
                    quotient.relation_symbol = symbols
                        .find_top_level_by_name_and_kinds_from_source(
                            &relation_name,
                            &[SymbolKind::Proposition],
                            diagnostic_path_source_span(&quotient.relation),
                        )
                        .unwrap_or_else(SymbolHandle::invalid);
                    if let Some(selection) = &mut quotient.equivalence {
                        let relation_name = selection.relation.joined_path();
                        selection.relation_symbol = symbols
                            .find_top_level_by_name_and_kinds_from_source(
                                &relation_name,
                                &[SymbolKind::Proposition],
                                diagnostic_path_source_span(&selection.relation),
                            )
                            .unwrap_or_else(SymbolHandle::invalid);
                        selection.trait_symbol = top_level_symbol_for_source(
                            symbols,
                            SymbolKind::Trait,
                            &selection.trait_name,
                        );
                        selection.conformance_symbol = top_level_symbol_for_source(
                            symbols,
                            SymbolKind::Conformance,
                            &selection.conformance_name,
                        );
                        assign_type_reference_argument_symbols_with_constraints(
                            symbols,
                            child_type_references,
                            type_constraints,
                            &type_parameters,
                            selection.trait_arguments,
                        );
                        if let Some((_, proposition_slots)) = trait_proposition_slots
                            .iter()
                            .find(|(symbol, _)| *symbol == selection.trait_symbol)
                        {
                            assign_proposition_family_argument_symbols(
                                symbols,
                                child_type_references,
                                &type_parameters,
                                selection.trait_arguments,
                                proposition_slots,
                            );
                        }
                        if let Some((_, machine_slots)) = trait_machine_identity_slots
                            .iter()
                            .find(|(symbol, _)| *symbol == selection.trait_symbol)
                        {
                            assign_machine_declaration_identity_argument_symbols(
                                symbols,
                                child_type_references,
                                &type_parameters,
                                selection.trait_arguments,
                                machine_slots,
                            );
                        }
                    }
                }
                for member in data_members.span_mut_or_empty(data_definition.members) {
                    match member {
                        DataMember::Field(field) => {
                            assign!(&mut field.type_reference);
                        }
                        DataMember::Variant(variant) => {
                            for field in data_payload_fields.span_mut_or_empty(variant.payload) {
                                assign!(&mut field.type_reference);
                            }
                        }
                    }
                }
            });

        program
            .tables
            .declarations
            .wire_members
            .for_each_mut(|_, member| {
                let WireMember::Field(field) = member else {
                    return;
                };
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &[],
                    &mut field.type_reference,
                );
            });

        program.roots.domain_definitions.for_each_mut(|domain| {
            let type_parameters = data_type_parameters
                .span_or_empty(domain.type_parameters)
                .to_vec();
            for selection in domain
                .cleanup_selection
                .iter_mut()
                .chain(domain.access_policy.iter_mut())
            {
                assign_static_operand_types(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &type_parameters,
                    selection,
                );
                crate::symbols::targets::assign_static_argument_symbols(
                    symbols,
                    domain.symbol,
                    &type_parameters,
                    selection,
                    false,
                );
            }
            if let Some(selection) = &mut domain.cleanup_selection
                && let Some((_, machine)) = cleanup_entries
                    .iter()
                    .find(|(entry, _)| *entry == selection.symbol)
            {
                selection.symbol = *machine;
            }
            assign_type_parameter_constraint_symbols(
                symbols,
                child_type_references,
                type_constraints,
                &type_parameters,
                data_type_parameters.span_mut_or_empty(domain.type_parameters),
            );
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &type_parameters,
                &mut domain.target_type,
            );
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &type_parameters,
                domain.index_arguments,
            );
        });

        let proposition_binders = &mut program.tables.declarations.proposition_binders;
        let state_parameters = &mut program.tables.declarations.state_parameters;
        program.roots.propositions.for_each_mut(|proposition| {
            let local_binders = proposition_binders
                .span_or_empty(proposition.binders)
                .iter()
                .map(|binder| TypeParameter {
                    symbol: binder.symbol,
                    name: binder.name.clone(),
                    kind: TypeParameterKind::Type,
                    bounds: binder.bounds,
                })
                .collect::<Vec<_>>();
            macro_rules! bind {
                ($reference:expr) => {
                    assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        child_type_references,
                        type_constraints,
                        &local_binders,
                        $reference,
                    )
                };
            }

            for binder in proposition_binders.span_mut_or_empty(proposition.binders) {
                let PropositionBinderKind::Const { type_reference } = &mut binder.kind else {
                    continue;
                };
                bind!(type_reference);
            }
            for parameter in state_parameters.span_mut_or_empty(proposition.parameters) {
                bind!(&mut parameter.type_reference);
            }
            if let PropositionBody::Witness { evidence } = &mut proposition.body {
                bind!(evidence);
            }
        });

        program.roots.conformances.for_each_mut(|conformance| {
            let local_type_parameters = data_type_parameters
                .span_or_empty(conformance.type_parameters)
                .to_vec();
            conformance.trait_symbol = crate::symbols::lookup::top_level_symbol_for_source(
                symbols,
                SymbolKind::Trait,
                &conformance.trait_name,
            );
            assign_type_parameter_constraint_symbols(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                data_type_parameters.span_mut_or_empty(conformance.type_parameters),
            );
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_type_parameters,
                conformance.arguments,
            );
            assign_trait_implementation_argument_symbols(
                symbols,
                child_type_references,
                &local_type_parameters,
                conformance.trait_symbol,
                conformance.arguments,
            );
            if let Some((_, proposition_slots)) = trait_proposition_slots
                .iter()
                .find(|(symbol, _)| *symbol == conformance.trait_symbol)
            {
                assign_proposition_family_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    conformance.arguments,
                    proposition_slots,
                );
            }
            if let Some((_, machine_slots)) = trait_machine_identity_slots
                .iter()
                .find(|(symbol, _)| *symbol == conformance.trait_symbol)
            {
                assign_machine_declaration_identity_argument_symbols(
                    symbols,
                    child_type_references,
                    &local_type_parameters,
                    conformance.arguments,
                    machine_slots,
                );
            }
        });
    }

    /*
        Reclassify arguments in declaration-identity machine slots after the
        selected trait has supplied their categories. Type-reference parsing keeps
        `Trait::requirement` as one delimiter-safe named leaf; this pass binds that
        leaf to the exact State symbol without pretending it is a runtime type.
        Assign the proposition and machine-identity slot symbols of one
        conformance-bound argument list: look up the bound trait in the slot
        tables, then delegate each kind to its argument walker.
    */
    pub(in crate::symbols) fn assign_trait_bound_slot_argument_symbols(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        local_type_parameters: &[TypeParameter],
        bound_trait_symbol: SymbolHandle,
        arguments: HandleSpan<TypeReference>,
        trait_proposition_slots: &[(SymbolHandle, Vec<bool>)],
        trait_machine_identity_slots: &[(SymbolHandle, Vec<bool>)],
    ) {
        assign_trait_implementation_argument_symbols(
            symbols,
            child_type_references,
            local_type_parameters,
            bound_trait_symbol,
            arguments,
        );
        if let Some((_, proposition_slots)) = trait_proposition_slots
            .iter()
            .find(|(symbol, _)| *symbol == bound_trait_symbol)
        {
            assign_proposition_family_argument_symbols(
                symbols,
                child_type_references,
                local_type_parameters,
                arguments,
                proposition_slots,
            );
        }
        if let Some((_, machine_slots)) = trait_machine_identity_slots
            .iter()
            .find(|(symbol, _)| *symbol == bound_trait_symbol)
        {
            assign_machine_declaration_identity_argument_symbols(
                symbols,
                child_type_references,
                local_type_parameters,
                arguments,
                machine_slots,
            );
        }
    }

    /*
        A trait's ordered binder children classify its written arguments. Ordinary
        type lookup excludes code maps; only an implementation slot can select one.
        Lexical binders shadow global declarations even when their kind is wrong,
        so checking diagnoses the selected binder rather than admitting a fallback.
    */
    pub(in crate::symbols) fn assign_trait_implementation_argument_symbols(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        local_type_parameters: &[TypeParameter],
        trait_symbol: SymbolHandle,
        arguments: HandleSpan<TypeReference>,
    ) {
        if !trait_symbol.is_valid() {
            return;
        }
        let parameters = symbols
            .child_handles(trait_symbol)
            .into_iter()
            .flatten()
            .take_while(|parameter| {
                matches!(
                    symbols.get(*parameter).kind,
                    SymbolKind::TypeParameter
                        | SymbolKind::MachineParameter
                        | SymbolKind::PropositionParameter
                        | SymbolKind::ConformanceParameter
                )
            });
        for (argument, parameter) in child_type_references
            .span_mut_or_empty(arguments)
            .iter_mut()
            .zip(parameters)
        {
            if symbols.get(parameter).kind != SymbolKind::ConformanceParameter {
                continue;
            }
            let (selected, name) = match argument {
                TypeReference::Named { symbol, name, .. } => (symbol, &*name),
                TypeReference::Generic(generic) => {
                    (&mut generic.storage.base_symbol, &generic.storage.base_name)
                }
                _ => continue,
            };
            *selected = local_type_parameters
                .iter()
                .find(|parameter| parameter.name.as_str() == name.as_str())
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(|| {
                    top_level_symbol_for_source(symbols, SymbolKind::Conformance, name)
                });
        }
    }

    pub(in crate::symbols) fn assign_machine_declaration_identity_argument_symbols(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        local_type_parameters: &[TypeParameter],
        arguments: HandleSpan<TypeReference>,
        machine_slots: &[bool],
    ) {
        for (argument, is_machine_identity) in child_type_references
            .span_mut_or_empty(arguments)
            .iter_mut()
            .zip(machine_slots)
        {
            if !is_machine_identity {
                continue;
            }
            let TypeReference::Named { symbol, name, .. } = argument else {
                continue;
            };
            let local = local_type_parameters
                .iter()
                .find(|parameter| parameter.name.as_str() == name.as_str())
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(SymbolHandle::invalid);
            if local.is_valid() {
                *symbol = local;
                continue;
            }

            let rendered = name.as_str();
            let exact_requirement = rendered
                .rsplit_once("::")
                .and_then(|(owner, requirement)| {
                    let owner = symbols.find_top_level_by_name_and_kinds_from_source(
                        owner,
                        &[SymbolKind::Trait],
                        name.source_span(),
                    )?;
                    symbols.find_child_by_name_and_kind(owner, requirement, SymbolKind::State)
                })
                .unwrap_or_else(SymbolHandle::invalid);
            *symbol = if exact_requirement.is_valid() {
                exact_requirement
            } else {
                resolve_free_machine_entry_state_symbol(symbols, name)
            };
        }
    }

    pub(in crate::symbols) fn assign_proposition_family_argument_symbols(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        local_type_parameters: &[TypeParameter],
        arguments: HandleSpan<TypeReference>,
        proposition_slots: &[bool],
    ) {
        for (argument, is_proposition) in child_type_references
            .span_mut_or_empty(arguments)
            .iter_mut()
            .zip(proposition_slots)
        {
            if !is_proposition {
                continue;
            }
            let TypeReference::Named { symbol, name, .. } = argument else {
                continue;
            };
            let local = local_type_parameters
                .iter()
                .find(|parameter| parameter.name.as_str() == name.as_str())
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(SymbolHandle::invalid);
            *symbol = if local.is_valid() {
                local
            } else {
                crate::symbols::lookup::top_level_symbol_for_source(
                    symbols,
                    SymbolKind::Proposition,
                    name,
                )
            };
        }
    }

    pub(in crate::symbols) fn assign_type_reference_symbol_with_locals_and_constraints(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        local_type_parameters: &[TypeParameter],
        type_reference: &mut TypeReference,
    ) {
        assign_type_reference_symbol_with_context(
            symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            type_reference,
        );
    }

    fn assign_type_reference_symbol_with_context(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        local_type_parameters: &[TypeParameter],
        type_reference: &mut TypeReference,
    ) {
        assign_type_reference_with_definitions(
            symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            type_reference,
            &[],
        );
    }

    pub(super) fn assign_type_reference_with_definitions(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
        local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
        type_reference: &mut symbol_resolved_trees::types::TypeReference,
        definition_references: &[source::SourceSpan],
    ) {
        match type_reference {
            TypeReference::Reference(reference) => {
                assign_type_reference_handle_symbol_with_context(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    reference.referee,
                    definition_references,
                );
            }
            TypeReference::Constrained(constrained) => {
                assign_type_reference_handle_symbol_with_context(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    constrained.base_type,
                    definition_references,
                );
                for constraint in type_constraints.span_or_empty(constrained.constraints) {
                    let TypeConstraint::Domain(domain) = constraint else {
                        continue;
                    };
                    assign_type_arguments_with_definitions(
                        symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        domain.arguments,
                        definition_references,
                    );
                }
            }
            TypeReference::FixedArray(fixed_array) => {
                assign_type_reference_handle_symbol_with_context(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    fixed_array.element_type,
                    definition_references,
                );
                if let FixedArrayLength::Expression(value) = fixed_array.length {
                    assign_type_reference_handle_symbol_with_context(
                        symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        value,
                        definition_references,
                    );
                }
                let from_definition = matches!(&fixed_array.length, symbol_resolved_trees::types::FixedArrayLength::ConstParameter { name, .. } if definition_references.contains(&name.source_span()));
                assign_fixed_array_length_symbol(
                    symbols,
                    if from_definition {
                        &[]
                    } else {
                        local_type_parameters
                    },
                    &mut fixed_array.length,
                );
            }
            TypeReference::Slice(slice) => {
                assign_type_reference_handle_symbol_with_context(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    slice.element_type,
                    definition_references,
                );
            }
            TypeReference::Generic(generic) => {
                generic.base_symbol =
                    if definition_references.contains(&generic.base_name.source_span()) {
                        top_level_type_symbol_for_source(symbols, &generic.base_name)
                    } else {
                        resolve_type_symbol(symbols, local_type_parameters, &generic.base_name)
                    };

                assign_type_arguments_with_definitions(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    generic.arguments,
                    definition_references,
                );
            }
            TypeReference::ConstExpression(_) => {}
            TypeReference::DynamicTrait {
                symbol,
                name,
                arguments,
                ..
            } => {
                *symbol = crate::symbols::lookup::top_level_symbol_for_source(
                    symbols,
                    SymbolKind::Trait,
                    name,
                );
                assign_type_arguments_with_definitions(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    *arguments,
                    definition_references,
                );
            }
            symbol_resolved_trees::types::TypeReference::Named { symbol, name, .. } => {
                *symbol = if definition_references.contains(&name.source_span()) {
                    top_level_type_symbol_for_source(symbols, name)
                } else {
                    resolve_type_symbol(symbols, local_type_parameters, name)
                };
            }
            symbol_resolved_trees::types::TypeReference::AliasUse {
                expansion,
                application,
                definition_references: own_references,
                ..
            } => {
                assign_type_reference_with_definitions(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    application,
                    definition_references,
                );
                let mut references = definition_references.to_vec();
                references.extend_from_slice(own_references);
                assign_type_reference_with_definitions(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    expansion,
                    &references,
                );
            }
            TypeReference::Unit => {}
        }
    }

    fn assign_fixed_array_length_symbol(
        symbols: &SymbolTable,
        local_type_parameters: &[TypeParameter],
        length: &mut FixedArrayLength,
    ) {
        match length {
            FixedArrayLength::ConstParameter { symbol, name } => {
                *symbol = local_type_parameters
                    .iter()
                    .find(|parameter| parameter.name.as_str() == name.as_str())
                    .map(|parameter| parameter.symbol)
                    .unwrap_or_else(|| {
                        top_level_symbol_for_source(symbols, SymbolKind::Const, name)
                    });
            }
            FixedArrayLength::ConstCall { machine, name } => {
                let entry = resolve_free_machine_entry_state_symbol(symbols, name);
                *machine = if entry.is_valid() {
                    symbols.get(entry).parent
                } else {
                    SymbolHandle::invalid()
                };
            }
            FixedArrayLength::Literal(_) | FixedArrayLength::Expression(_) => {}
        }
    }

    fn assign_type_parameter_constraint_symbols(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        local_type_parameters: &[TypeParameter],
        type_parameters: &mut [TypeParameter],
    ) {
        for parameter in type_parameters {
            let (TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }) = &mut parameter.kind
            else {
                continue;
            };
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                type_reference,
            );
        }
    }

    fn assign_type_reference_handle_symbol_with_context(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
        local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
        handle: Handle<symbol_resolved_trees::types::TypeReference>,
        definition_references: &[source::SourceSpan],
    ) {
        let mut type_reference = std::mem::take(child_type_references.get_mut(handle));
        assign_type_reference_with_definitions(
            symbols,
            child_type_references,
            type_constraints,
            local_type_parameters,
            &mut type_reference,
            definition_references,
        );
        *child_type_references.get_mut(handle) = type_reference;
    }

    fn assign_type_arguments_with_definitions(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
        type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
        local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
        arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
        definition_references: &[source::SourceSpan],
    ) {
        for handle in arguments.handles() {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                handle,
                definition_references,
            );
        }
    }

    pub(in crate::symbols) fn assign_type_reference_argument_symbols_with_constraints(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        type_constraints: &Arena<TypeConstraint>,
        local_type_parameters: &[TypeParameter],
        arguments: HandleSpan<TypeReference>,
    ) {
        for handle in arguments.handles() {
            let mut argument = take(child_type_references.get_mut(handle));
            assign_type_reference_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                &mut argument,
            );
            *child_type_references.get_mut(handle) = argument;
        }
    }

    /*
        Cast/zero-value target leaves use a separate semantic-domain field and do
        not own declaration constraint spans. Signature/declaration traversal uses
        the `_and_constraints` variants above so indexed arguments resolve in their
        lexical generic scope.
    */
    pub(in crate::symbols) fn assign_type_reference_symbol_with_locals(
        symbols: &SymbolTable,
        child_type_references: &mut Arena<TypeReference>,
        local_type_parameters: &[TypeParameter],
        type_reference: &mut TypeReference,
    ) {
        let constraints = Arena::new();
        assign_type_reference_symbol_with_locals_and_constraints(
            symbols,
            child_type_references,
            &constraints,
            local_type_parameters,
            type_reference,
        );
    }

    fn resolve_type_symbol(
        symbols: &SymbolTable,
        local_type_parameters: &[TypeParameter],
        name: &DiagnosticName,
    ) -> SymbolHandle {
        local_type_parameters
            .iter()
            .find(|parameter| parameter.name.as_str() == name.as_str())
            .map(|parameter| parameter.symbol)
            .unwrap_or_else(|| top_level_type_symbol_for_source(symbols, name))
    }

    /*
        A selected declaration operand may retain type handles instead of
        paths. Resolve its entire type graph in the selecting declaration's
        scope before static argument selection.
    */
    fn assign_static_operand_types(
        symbols: &SymbolTable,
        types: &mut Arena<TypeReference>,
        constraints: &Arena<TypeConstraint>,
        parameters: &[TypeParameter],
        argument: &mut StaticMachineArgument,
    ) {
        if argument.type_reference.is_valid() {
            let mut reference = take(types.get_mut(argument.type_reference));
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                types,
                constraints,
                parameters,
                &mut reference,
            );
            *types.get_mut(argument.type_reference) = reference;
        }
        if let Some(application) = &mut argument.application {
            for nested in &mut application.arguments {
                assign_static_operand_types(symbols, types, constraints, parameters, nested);
            }
        }
    }
}
mod queries {
    use arena::Arena;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::lookup::child_symbol_by_kinds;
    use crate::symbols::scope::MachineScope;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::types::TypeReference;

    fn type_reference_symbol(
        child_type_references: &Arena<TypeReference>,
        type_reference: &TypeReference,
    ) -> SymbolHandle {
        match type_reference {
            symbol_resolved_trees::types::TypeReference::AliasUse { expansion, .. } => {
                type_reference_symbol(child_type_references, expansion)
            }
            symbol_resolved_trees::types::TypeReference::Reference(reference) => {
                type_reference_symbol(
                    child_type_references,
                    child_type_references.get(reference.referee),
                )
            }
            TypeReference::Constrained(constrained) => type_reference_symbol(
                child_type_references,
                child_type_references.get(constrained.base_type),
            ),
            TypeReference::FixedArray(fixed_array) => type_reference_symbol(
                child_type_references,
                child_type_references.get(fixed_array.element_type),
            ),
            TypeReference::Slice(slice) => type_reference_symbol(
                child_type_references,
                child_type_references.get(slice.element_type),
            ),
            TypeReference::Generic(generic) => generic.base_symbol,
            TypeReference::ConstExpression(_) => SymbolHandle::invalid(),
            TypeReference::DynamicTrait { symbol, .. } => *symbol,
            TypeReference::Named { symbol, .. } => *symbol,
            TypeReference::Unit => SymbolHandle::invalid(),
        }
    }

    pub(in crate::symbols) fn call_target_for_type_reference(
        machine: &MachineScope<'_>,
        symbols: &SymbolTable,
        child_type_references: &Arena<TypeReference>,
        type_reference: &TypeReference,
        target: &DiagnosticName,
    ) -> SymbolHandle {
        let type_symbol = type_reference_symbol(child_type_references, type_reference);
        let direct_child =
            child_symbol_by_kinds(symbols, type_symbol, &[SymbolKind::State], target.as_str());
        if direct_child.is_valid() {
            return direct_child;
        }

        if type_symbol.is_valid() && matches!(symbols.get(type_symbol).kind, SymbolKind::Data) {
            return machine.attached_call_target(symbols, type_symbol, target);
        }

        SymbolHandle::invalid()
    }
}
mod value_expressions {
    /*
        Type-bound value expressions use their declaration's ordinary value namespace.
        Type/domain names retain declaration lookup; index expressions must resolve
        lexical subjects before a later phase can establish that their value is static.
    */

    use arena::{Arena, Handle};
    use symbol_resolved_trees::{
        expression::ExpressionTable,
        signature::StateParameter,
        types::{FixedArrayLength, TypeConstraint, TypeReference},
    };
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use crate::symbols::expressions::lexical_value_head;
    use crate::symbols::scope::MachineScope;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::name::DiagnosticName;

    /*
        Constant and field types contain the same value expressions as parameter types.
        Resolve them after declaration and type identities exist, before typing can
        copy an unresolved call into a constant position. Data owns its generic and
        field names but has no machine activation: no state parameters or locals are
        in scope. Field references remain symbolic, not same-spelled global constants.
    */
    pub(in crate::symbols) fn assign_declaration_type_value_expression_symbols(
        program: &mut symbol_resolved_trees::SymbolResolvedTrees,
        symbols: &SymbolTable,
    ) {
        let attached_machines = crate::symbols::scope::attached_machines(program);
        let declarations = &mut program.tables.declarations;
        let applications = program
            .roots
            .data_definitions
            .iter()
            .filter_map(|declaration| {
                declaration.cleanup_selection.as_ref().map(|selection| {
                    (
                        declaration.symbol,
                        declaration.type_parameters,
                        selection.clone(),
                    )
                })
            })
            .chain(
                program
                    .roots
                    .domain_definitions
                    .iter()
                    .flat_map(|declaration| {
                        declaration
                            .cleanup_selection
                            .iter()
                            .chain(declaration.access_policy.iter())
                            .map(move |selection| {
                                (
                                    declaration.symbol,
                                    declaration.type_parameters,
                                    selection.clone(),
                                )
                            })
                    }),
            )
            .collect::<Vec<_>>();
        for (owner, parameters, selection) in applications {
            let scope = MachineScope {
                symbol: owner,
                lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
                attached_machines: &attached_machines,
                resolution_indexes: &program.resolution_indexes,
                type_parameters: declarations.data_type_parameters.span_or_empty(parameters),
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: &[],
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                traits: &program.roots.traits,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &declarations.data_members,
                data_type_parameters: &declarations.data_type_parameters,
                data_payload_fields: &declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            crate::symbols::expressions::assign_static_arguments_type_symbols(
                symbols,
                &scope,
                &[],
                SymbolHandle::invalid(),
                &mut program.tables.bodies.expressions,
                &mut declarations.child_type_references,
                std::slice::from_ref(&selection),
            );
        }
        for conformance in program.roots.conformances.iter() {
            let scope = MachineScope {
                symbol: conformance.symbol,
                lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
                attached_machines: &attached_machines,
                resolution_indexes: &program.resolution_indexes,
                type_parameters: declarations
                    .data_type_parameters
                    .span_or_empty(conformance.type_parameters),
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: &[],
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                traits: &program.roots.traits,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &declarations.data_members,
                data_type_parameters: &declarations.data_type_parameters,
                data_payload_fields: &declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            assign_type_argument_value_expression_symbols(
                symbols,
                &scope,
                &mut program.tables.bodies.expressions,
                &mut declarations.child_type_references,
                conformance.arguments,
            );
        }
        program
            .roots
            .const_declarations
            .for_each_mut(|declaration| {
                let scope = MachineScope {
                    symbol: declaration.symbol,
                    lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
                    attached_machines: &attached_machines,
                    resolution_indexes: &program.resolution_indexes,
                    type_parameters: &[],
                    attached_data_symbol: SymbolHandle::invalid(),
                    inherited_data_members: None,
                    owned_data: &[],
                    prior_statements: &[],
                    data_definitions: &program.roots.data_definitions,
                    type_aliases: &program.roots.type_aliases,
                    conformances: &program.roots.conformances,
                    traits: &program.roots.traits,
                    operand_owners: &program.tables.types.operand_lifetime_owners,
                    data_members: &declarations.data_members,
                    data_type_parameters: &declarations.data_type_parameters,
                    data_payload_fields: &declarations.data_payload_fields,
                    type_constraints: &program.tables.types.constraints,
                };
                assign_type_value_expression_symbols(
                    symbols,
                    &scope,
                    &[],
                    SymbolHandle::invalid(),
                    &mut program.tables.bodies.expressions,
                    &mut declarations.child_type_references,
                    &mut declaration.declared_type,
                );
            });
        let mut alias_expansions = Vec::new();
        for alias in program.roots.type_aliases.iter() {
            let parameters = declarations
                .data_type_parameters
                .span_or_empty(alias.type_parameters)
                .to_vec();
            let scope = MachineScope {
                symbol: alias.symbol,
                lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
                attached_machines: &attached_machines,
                resolution_indexes: &program.resolution_indexes,
                type_parameters: &parameters,
                attached_data_symbol: SymbolHandle::invalid(),
                inherited_data_members: None,
                owned_data: &[],
                prior_statements: &[],
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                traits: &program.roots.traits,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &declarations.data_members,
                data_type_parameters: &declarations.data_type_parameters,
                data_payload_fields: &declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            let mut expansion = alias.expansion.clone();
            assign_type_value_expression_symbols(
                symbols,
                &scope,
                &[],
                SymbolHandle::invalid(),
                &mut program.tables.bodies.expressions,
                &mut declarations.child_type_references,
                &mut expansion,
            );
            alias_expansions.push(expansion);
        }
        let mut alias_expansions = alias_expansions.into_iter();
        program.roots.type_aliases.for_each_mut(|alias| {
            alias.expansion = alias_expansions.next().expect("unchanged alias roster");
        });
        let mut field_updates = Vec::new();
        let mut payload_updates = Vec::new();
        for definition in program.roots.data_definitions.iter() {
            let members = declarations.data_members.span_or_empty(definition.members);
            let mut scope = MachineScope {
                symbol: definition.symbol,
                lexical_requirements: crate::symbols::scope::LexicalRequirements::none(),
                attached_machines: &attached_machines,
                resolution_indexes: &program.resolution_indexes,
                type_parameters: declarations
                    .data_type_parameters
                    .span_or_empty(definition.type_parameters),
                attached_data_symbol: definition.symbol,
                inherited_data_members: Some(members),
                owned_data: &[],
                prior_statements: &[],
                data_definitions: &program.roots.data_definitions,
                type_aliases: &program.roots.type_aliases,
                conformances: &program.roots.conformances,
                traits: &program.roots.traits,
                operand_owners: &program.tables.types.operand_lifetime_owners,
                data_members: &declarations.data_members,
                data_type_parameters: &declarations.data_type_parameters,
                data_payload_fields: &declarations.data_payload_fields,
                type_constraints: &program.tables.types.constraints,
            };
            for (member_offset, member) in members.iter().enumerate() {
                let (fields, payload) = match member {
                    DataMember::Field(field) => {
                        scope.symbol = definition.symbol;
                        (std::slice::from_ref(field), None)
                    }
                    DataMember::Variant(variant) => {
                        scope.symbol = variant.symbol;
                        (
                            declarations
                                .data_payload_fields
                                .span_or_empty(variant.payload),
                            Some(variant.payload),
                        )
                    }
                };
                for (field_offset, field) in fields.iter().enumerate() {
                    let mut reference = field.type_reference.clone();
                    assign_type_value_expression_symbols(
                        symbols,
                        &scope,
                        &[],
                        SymbolHandle::invalid(),
                        &mut program.tables.bodies.expressions,
                        &mut declarations.child_type_references,
                        &mut reference,
                    );
                    if let Some(payload) = payload {
                        payload_updates.push((payload.handle_at(field_offset as u32), reference));
                    } else {
                        field_updates.push((
                            definition.members.handle_at(member_offset as u32),
                            reference,
                        ));
                    }
                }
            }
        }
        for (handle, reference) in field_updates {
            if let DataMember::Field(field) = declarations.data_members.get_mut(handle) {
                field.type_reference = reference;
            }
        }
        for (handle, reference) in payload_updates {
            declarations
                .data_payload_fields
                .get_mut(handle)
                .type_reference = reference;
        }
    }

    /*
        Bind value expressions in an argument list under its declaration owner.
        Type-name binding alone does not select calls or values inside an argument.
    */
    pub(in crate::symbols) fn assign_type_argument_value_expression_symbols(
        symbols: &SymbolTable,
        scope: &MachineScope<'_>,
        expressions: &mut ExpressionTable,
        child_types: &mut Arena<TypeReference>,
        arguments: arena::HandleSpan<TypeReference>,
    ) {
        for argument in arguments.handles() {
            let mut reference = child_types.get(argument).clone();
            assign_type_value_expression_symbols(
                symbols,
                scope,
                &[],
                SymbolHandle::invalid(),
                expressions,
                child_types,
                &mut reference,
            );
            *child_types.get_mut(argument) = reference;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::symbols) fn assign_type_value_expression_symbols(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expressions: &mut ExpressionTable,
        child_types: &mut Arena<TypeReference>,
        type_reference: &mut TypeReference,
    ) {
        assign_type_value_expressions_with_declarations(
            symbols,
            machine,
            parameters,
            state_symbol,
            expressions,
            child_types,
            type_reference,
            &[],
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn assign_type_value_expressions_with_declarations(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expressions: &mut ExpressionTable,
        child_types: &mut Arena<TypeReference>,
        type_reference: &mut TypeReference,
        declaration_references: &[source::SourceSpan],
    ) {
        let owner = type_reference.supplied_operand_owner();
        let supplied_scope = owner
            .is_valid()
            .then(|| machine.supplied_scope(owner))
            .flatten();
        if owner.is_valid() && supplied_scope.is_none() {
            return;
        }
        let (machine, parameters, state_symbol) = supplied_scope
            .as_ref()
            .map_or((machine, parameters, state_symbol), |scope| {
                (scope, &[][..], SymbolHandle::invalid())
            });
        if supplied_scope.is_some() {
            super::assignment::assign_type_reference_with_definitions(
                symbols,
                child_types,
                machine.type_constraints,
                machine.type_parameters,
                type_reference,
                declaration_references,
            );
        }
        let mut child = |handle: Handle<TypeReference>| {
            let mut reference = child_types.get(handle).clone();
            assign_type_value_expressions_with_declarations(
                symbols,
                machine,
                parameters,
                state_symbol,
                expressions,
                child_types,
                &mut reference,
                declaration_references,
            );
            *child_types.get_mut(handle) = reference;
        };
        match type_reference {
            TypeReference::Reference(reference) => child(reference.referee),
            TypeReference::FixedArray(array) => {
                child(array.element_type);
                if let FixedArrayLength::Expression(value) = array.length {
                    child(value);
                }
                if let FixedArrayLength::ConstParameter { symbol, name } = &mut array.length
                    && let Some(selected) =
                        resolve_index_subject(symbols, machine, parameters, name)
                {
                    *symbol = selected;
                }
            }
            TypeReference::Slice(slice) => child(slice.element_type),
            TypeReference::Generic(generic) => {
                for argument in generic.arguments.handles() {
                    child(argument);
                }
                let telescope = machine
                    .data_definitions
                    .iter()
                    .find(|data| data.symbol == generic.base_symbol)
                    .map(|data| {
                        machine
                            .data_type_parameters
                            .span_or_empty(data.type_parameters)
                    })
                    .or_else(|| {
                        if symbols.get(generic.base_symbol).kind != SymbolKind::TypeAlias {
                            return None;
                        }
                        let mut aliases = machine
                            .type_aliases
                            .iter()
                            .filter(|alias| alias.symbol == generic.base_symbol);
                        let alias = aliases.next()?;
                        if aliases.next().is_some() {
                            return None;
                        }
                        Some(
                            machine
                                .data_type_parameters
                                .span_or_empty(alias.type_parameters),
                        )
                    });
                if let Some(telescope) = telescope {
                    for (parameter, argument) in telescope.iter().zip(generic.arguments.handles()) {
                        if !matches!(
                            parameter.kind,
                            symbol_resolved_trees::data::TypeParameterKind::Value { .. }
                                | symbol_resolved_trees::data::TypeParameterKind::Const { .. }
                        ) {
                            continue;
                        }
                        if let TypeReference::Named { symbol, name, .. } =
                            child_types.get_mut(argument)
                            && let Some(selected) =
                                resolve_index_subject(symbols, machine, parameters, name)
                        {
                            *symbol = selected;
                        }
                    }
                }
            }
            TypeReference::Constrained(constrained) => {
                child(constrained.base_type);
                for constraint in machine
                    .type_constraints
                    .span_or_empty(constrained.constraints)
                {
                    match constraint {
                        TypeConstraint::Domain(domain) => {
                            for handle in domain.arguments.handles() {
                                let mut reference = child_types.get(handle).clone();
                                assign_type_value_expressions_with_declarations(
                                    symbols,
                                    machine,
                                    parameters,
                                    state_symbol,
                                    expressions,
                                    child_types,
                                    &mut reference,
                                    declaration_references,
                                );
                                *child_types.get_mut(handle) = reference;
                            }
                        }
                        TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReference::ConstExpression(expression) => {
                if !expression.supplied_operand_owner.is_valid()
                    && !expression.lexical_owner.is_valid()
                {
                    expression.lexical_owner = machine.symbol;
                }
                crate::symbols::expressions::assign_expression_with_declaration_references(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expressions,
                    child_types,
                    expression.expression,
                    declaration_references,
                );
            }
            TypeReference::AliasUse {
                expansion,
                application,
                definition_references,
                ..
            } => {
                assign_type_value_expressions_with_declarations(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expressions,
                    child_types,
                    application,
                    declaration_references,
                );
                let mut references = declaration_references.to_vec();
                references.extend_from_slice(definition_references);
                assign_type_value_expressions_with_declarations(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expressions,
                    child_types,
                    expansion,
                    &references,
                );
            }
            TypeReference::Named { .. }
            | TypeReference::DynamicTrait { .. }
            | TypeReference::Unit => {}
        }
    }

    /*
        Re-assign symbols and value expressions on a cloned child type reference,
        then write it back — the tail every Cast/argument reassign site repeats.
    */
    pub(in crate::symbols) fn reassign_type_reference_symbols_and_values(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expressions: &mut ExpressionTable,
        child_types: &mut Arena<TypeReference>,
        handle: Handle<TypeReference>,
    ) {
        let mut type_reference = child_types.get(handle).clone();
        crate::symbols::type_references::assign_type_reference_symbol_with_locals(
            symbols,
            child_types,
            machine.type_parameters,
            &mut type_reference,
        );
        assign_type_value_expression_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expressions,
            child_types,
            &mut type_reference,
        );
        *child_types.get_mut(handle) = type_reference;
    }

    /*
        Re-assign symbols on a cloned child type reference, then write it back —
        the tail every symbol-only reassign site repeats.
    */
    pub(in crate::symbols) fn reassign_type_reference_symbols(
        symbols: &SymbolTable,
        child_types: &mut Arena<TypeReference>,
        local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
        handle: Handle<TypeReference>,
    ) {
        let mut type_reference = child_types.get(handle).clone();
        crate::symbols::type_references::assign_type_reference_symbol_with_locals(
            symbols,
            child_types,
            local_type_parameters,
            &mut type_reference,
        );
        *child_types.get_mut(handle) = type_reference;
    }

    /*
        Type-bound indices use the declaration's value scope. Active payload
        binders precede enclosing generic names. A common descriptor synthesized
        from a value binder keeps that binder identity for application substitution;
        ordinary common fields still resolve through declaration lookup. Machine
        parameters and preceding locals retain their normal frontier. Namespace-headed paths
        keep the declaration identity already selected by ordinary type resolution.
    */
    fn resolve_index_subject(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        name: &DiagnosticName,
    ) -> Option<SymbolHandle> {
        let members = name
            .as_str()
            .split("::")
            .map(|member| DiagnosticName::from_str(member, name.source_span()))
            .collect::<Vec<_>>();
        let (head_name, suffix) = members.split_first()?;
        let (_, declared) = crate::symbols::scoped_paths::resolve_state_scoped_members(
            symbols,
            machine.symbol,
            SymbolHandle::invalid(),
            std::slice::from_ref(head_name),
            None,
        );
        let payload_subject = symbols.get(machine.symbol).kind == SymbolKind::Variant
            && declared.is_valid()
            && symbols.get(declared).kind == SymbolKind::Field
            && symbols.get(declared).parent == machine.symbol;
        let lexical = if payload_subject {
            declared
        } else {
            let frontier =
                lexical_value_head(machine, parameters, head_name.as_str(), !suffix.is_empty());
            if frontier.is_valid() {
                frontier
            } else {
                machine
                    .type_parameters
                    .iter()
                    .find(|parameter| parameter.name.as_str() == head_name.as_str())
                    .map(|parameter| parameter.symbol)
                    .unwrap_or_else(SymbolHandle::invalid)
            }
        };
        if lexical.is_valid() {
            return Some(
                crate::symbols::scoped_paths::resolve_path_members_from_head(
                    symbols, lexical, suffix,
                )
                .last()
                .copied()
                .unwrap_or_else(SymbolHandle::invalid),
            );
        }
        if suffix.is_empty() {
            return Some(declared);
        }
        if declared.is_valid()
            && !matches!(
                symbols.get(declared).kind,
                SymbolKind::Module
                    | SymbolKind::Data
                    | SymbolKind::Machine
                    | SymbolKind::Trait
                    | SymbolKind::BuiltinType
            )
        {
            return Some(
                crate::symbols::scoped_paths::resolve_path_members_from_head(
                    symbols, declared, suffix,
                )
                .last()
                .copied()
                .unwrap_or_else(SymbolHandle::invalid),
            );
        }
        None
    }

    #[cfg(test)]
    mod subject_binding_tests {
        /* Alias arguments bind source subjects under the same frontier as data. */

        use super::*;
        use crate::symbols::scope::{LexicalRequirements, attached_machines};
        use source::SourceSpan;
        use symbol_resolved_trees::{
            SymbolResolvedTrees,
            data::{DataDefinition, DataDefinitionStorage, TypeParameter, TypeParameterKind},
            statement::{LocalData, LocalDataStorage, Statement},
            type_aliases::TypeAliasDefinition,
            types::{GenericTypeReference, GenericTypeReferenceStorage},
        };
        use symbols::{SymbolNameRef, SymbolTableBuilder};

        fn name(value: &str) -> DiagnosticName {
            DiagnosticName::from_str(value, SourceSpan::default())
        }

        fn named(symbol: SymbolHandle, value: &str) -> TypeReference {
            TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol,
                name: name(value),
            }
        }

        #[test]
        fn alias_subjects_use_exact_headers_roles_and_lexical_frontiers() {
            for (data, copies, other_alias) in [
                (true, 1, false),
                (false, 0, false),
                (false, 1, false),
                (false, 2, false),
                (false, 1, true),
            ] {
                let mut builder = SymbolTableBuilder::default();
                let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
                let roots = builder.insert_children(
                    root,
                    symbols::builtin_type_symbols().into_iter().chain([
                        (SymbolKind::Data, SymbolNameRef::Static("Record")),
                        (SymbolKind::TypeAlias, SymbolNameRef::Static("Alias")),
                        (SymbolKind::Module, SymbolNameRef::Static("other")),
                        (SymbolKind::Const, SymbolNameRef::Static("index")),
                    ]),
                );
                let u64_symbol = roots.handle_at(symbols::BuiltinTypeAtom::U64.ordinal() as u32);
                let record = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
                let alias = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
                let other = roots.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
                let descendants = builder.insert_children(
                    other,
                    [
                        (SymbolKind::TypeAlias, SymbolNameRef::Static("Alias")),
                        (SymbolKind::Machine, SymbolNameRef::Static("owner")),
                    ],
                );
                let absent_alias = descendants.handle_at(0);
                let owner = descendants.handle_at(1);
                let state = builder
                    .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("entry"))])
                    .start();
                let subjects = builder.insert_children(
                    state,
                    [
                        (SymbolKind::Parameter, SymbolNameRef::Static("index")),
                        (SymbolKind::Local, SymbolNameRef::Static("index")),
                    ],
                );
                let parameter = subjects.handle_at(0);
                let local = subjects.handle_at(1);
                let mut header_symbols = Vec::new();
                for declaration in [record, alias] {
                    header_symbols.push(builder.insert_children(
                        declaration,
                        [
                            (
                                SymbolKind::TypeParameter,
                                SymbolNameRef::Static("ValueSlot"),
                            ),
                            (
                                SymbolKind::TypeParameter,
                                SymbolNameRef::Static("ConstSlot"),
                            ),
                            (SymbolKind::TypeParameter, SymbolNameRef::Static("TypeSlot")),
                        ],
                    ));
                }
                let symbols = builder.finish();
                let mut program = SymbolResolvedTrees {
                    symbols: symbols.clone(),
                    ..Default::default()
                };
                let carrier = named(u64_symbol, "u64");
                let mut spans = Vec::new();
                for symbols in header_symbols {
                    spans.push(
                        program
                            .tables
                            .declarations
                            .data_type_parameters
                            .insert_many([
                                TypeParameter {
                                    symbol: symbols.handle_at(0),
                                    name: name("ValueSlot"),
                                    kind: TypeParameterKind::Value {
                                        type_reference: carrier.clone(),
                                    },
                                    ..Default::default()
                                },
                                TypeParameter {
                                    symbol: symbols.handle_at(1),
                                    name: name("ConstSlot"),
                                    kind: TypeParameterKind::Const {
                                        type_reference: carrier.clone(),
                                    },
                                    ..Default::default()
                                },
                                TypeParameter {
                                    symbol: symbols.handle_at(2),
                                    name: name("TypeSlot"),
                                    ..Default::default()
                                },
                            ]),
                    );
                }
                program.roots.data_definitions.push(DataDefinition {
                    symbol: record,
                    name: name("Record"),
                    storage: DataDefinitionStorage {
                        type_parameters: spans[0],
                        ..Default::default()
                    },
                    ..Default::default()
                });
                for _ in 0..copies {
                    program.roots.type_aliases.push(TypeAliasDefinition {
                        symbol: alias,
                        name: name("Alias"),
                        type_parameters: spans[1],
                        ..Default::default()
                    });
                }
                let attached = attached_machines(&program);
                let parameter = StateParameter {
                    symbol: parameter,
                    name: name("index"),
                    type_reference: carrier.clone(),
                    ..Default::default()
                };
                let prior = [Statement::LocalData(LocalData {
                    symbol: local,
                    name: name("index"),
                    storage: LocalDataStorage {
                        type_reference: carrier.clone(),
                        ..Default::default()
                    },
                })];
                for has_prior in [false, true] {
                    let arguments = program
                        .tables
                        .declarations
                        .child_type_references
                        .insert_many([
                            named(SymbolHandle::invalid(), "index"),
                            named(SymbolHandle::invalid(), "index"),
                            named(SymbolHandle::invalid(), "index"),
                        ]);
                    let target = if data {
                        record
                    } else if other_alias {
                        absent_alias
                    } else {
                        alias
                    };
                    let mut application = TypeReference::Generic(GenericTypeReference {
                        storage: GenericTypeReferenceStorage {
                            base_symbol: target,
                            base_name: name(if data { "Record" } else { "Alias" }),
                            arguments,
                            ..Default::default()
                        },
                    });
                    let scope = MachineScope {
                        symbol: owner,
                        lexical_requirements: LexicalRequirements::none(),
                        attached_machines: &attached,
                        resolution_indexes: &program.resolution_indexes,
                        type_parameters: &[],
                        attached_data_symbol: SymbolHandle::invalid(),
                        inherited_data_members: None,
                        owned_data: &[],
                        prior_statements: if has_prior { &prior } else { &[] },
                        data_definitions: &program.roots.data_definitions,
                        type_aliases: &program.roots.type_aliases,
                        conformances: &program.roots.conformances,
                        traits: &program.roots.traits,
                        operand_owners: &program.tables.types.operand_lifetime_owners,
                        data_members: &program.tables.declarations.data_members,
                        data_type_parameters: &program.tables.declarations.data_type_parameters,
                        data_payload_fields: &program.tables.declarations.data_payload_fields,
                        type_constraints: &program.tables.types.constraints,
                    };
                    assign_type_value_expression_symbols(
                        &symbols,
                        &scope,
                        std::slice::from_ref(&parameter),
                        state,
                        &mut program.tables.bodies.expressions,
                        &mut program.tables.declarations.child_type_references,
                        &mut application,
                    );
                    let expected = if data || (copies == 1 && !other_alias) {
                        if has_prior { local } else { parameter.symbol }
                    } else {
                        SymbolHandle::invalid()
                    };
                    for (ordinal, expected) in [expected, expected, SymbolHandle::invalid()]
                        .into_iter()
                        .enumerate()
                    {
                        assert!(
                            matches!(program.tables.declarations.child_type_references.get(arguments.handle_at(ordinal as u32)),
                    TypeReference::Named { symbol, .. } if *symbol == expected)
                        );
                    }
                }
            }
        }
    }
}

pub(super) use value_expressions::{
    assign_declaration_type_value_expression_symbols,
    assign_type_argument_value_expression_symbols, assign_type_value_expression_symbols,
    reassign_type_reference_symbols, reassign_type_reference_symbols_and_values,
};

pub(super) use assignment::{
    assign_generic_application_origin_symbols,
    assign_machine_declaration_identity_argument_symbols,
    assign_proposition_family_argument_symbols, assign_trait_bound_slot_argument_symbols,
    assign_trait_implementation_argument_symbols,
    assign_type_reference_argument_symbols_with_constraints,
    assign_type_reference_symbol_with_locals,
    assign_type_reference_symbol_with_locals_and_constraints, assign_type_reference_symbols,
};
pub(super) use queries::call_target_for_type_reference;

pub(crate) use assignment::assign_nominal_machine_requirement_arguments;
