use arena::{Arena, Handle, HandleSpan};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::{
    diagnostic_path_source_span, top_level_symbol_for_source, top_level_type_symbol_for_source,
};
use crate::symbols::targets::resolve_free_machine_entry_state_symbol;

/// An instantiated use and its retained application are separate type rows.
/// Bind both with the same declaration-source rules before typing validates
/// their identity. Completed bodies append these rows after header assignment;
/// the retained prefix must not be rebound under an extension's namespace.
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
            let mut reference = std::mem::take(child_type_references.get_mut(handle));
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

/// For each trait, the flag each of its type parameters gets from
/// `predicate` — the (trait symbol, per-parameter flags) rows the later
/// argument walks join on.
fn trait_parameter_slots(
    program: &SymbolResolvedTrees,
    predicate: impl Fn(&symbol_resolved_trees::data::TypeParameter) -> bool,
) -> Vec<(SymbolHandle, Vec<bool>)> {
    program
        .roots
        .traits
        .iter()
        .map(|trait_definition| {
            (
                trait_definition.symbol,
                program
                    .tables
                    .declarations
                    .data_type_parameters
                    .span_or_empty(trait_definition.type_parameters)
                    .iter()
                    .map(|parameter| predicate(parameter))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
}

/// Resolve nominal bound arguments in the same lexical declaration scopes as
/// signatures and bodies. A symbol's parent is not a capture ownership proof.
pub(crate) fn assign_nominal_machine_requirement_arguments(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    use super::super::scope::{DeclarationTypeScopes, declaration_type_scope};
    use symbol_resolved_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
    struct ArgumentScope {
        trait_definition: SymbolHandle,
        arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
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
        let mut inherited = declaration_type_scope(captures.parameters(machine.symbol), attached);
        inherited.extend(super::super::scope::evidence_type_parameters(
            &machine.conformance_bounds,
        ));
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
        for signature in program.trait_machine_signatures(owner.machines) {
            collect(
                program,
                program.data_type_parameters(signature.type_parameters),
                parameters,
                &mut scopes,
            );
        }
    }
    for owner in program.conformances.iter() {
        collect(
            program,
            program.data_type_parameters(owner.type_parameters),
            &[],
            &mut scopes,
        );
    }
    for owner in program.data_definitions.iter() {
        collect(
            program,
            program.data_type_parameters(owner.type_parameters),
            &[],
            &mut scopes,
        );
    }
    for owner in program.domain_definitions.iter() {
        collect(
            program,
            program.data_type_parameters(owner.type_parameters),
            &[],
            &mut scopes,
        );
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
    let trait_proposition_slots = trait_parameter_slots(program, |parameter| {
        matches!(
            parameter.kind,
            symbol_resolved_trees::data::TypeParameterKind::Proposition { .. }
        )
    });
    let trait_machine_identity_slots = trait_parameter_slots(program, |parameter| {
        matches!(
            parameter.kind,
            symbol_resolved_trees::data::TypeParameterKind::Machine {
                contract:
                    symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity
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
            assign_type_parameter_constraint_symbols(
                symbols,
                child_type_references,
                type_constraints,
                &type_parameters,
                data_type_parameters.span_mut_or_empty(data_definition.type_parameters),
            );
            if let Some(generic_instance) = &mut data_definition.generic_instance {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &type_parameters,
                    generic_instance,
                );
            }
            if let Some(quotient) = &mut data_definition.quotient {
                assign_type_reference_symbol_with_locals_and_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    &type_parameters,
                    &mut quotient.carrier,
                );
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
                    symbol_resolved_trees::data::DataMember::Field(field) => {
                        assign_type_reference_symbol_with_locals_and_constraints(
                            symbols,
                            child_type_references,
                            type_constraints,
                            &type_parameters,
                            &mut field.type_reference,
                        );
                    }
                    // A payload-bearing variant's fields are stored out of band (the
                    // `data_payload_fields` arena); their type references need symbols
                    // too -- a struct-typed payload `case Wrap(p: Point)` otherwise
                    // fails in the layout builder ("non-primitive type `Point` is
                    // missing a resolved symbol"). Primitive/array payloads resolved
                    // anyway (no named symbol needed); only NAMED payload types broke.
                    symbol_resolved_trees::data::DataMember::Variant(variant) => {
                        for field in data_payload_fields.span_mut_or_empty(variant.payload) {
                            assign_type_reference_symbol_with_locals_and_constraints(
                                symbols,
                                child_type_references,
                                type_constraints,
                                &type_parameters,
                                &mut field.type_reference,
                            );
                        }
                    }
                }
            }
        });

    // A numbered `data` declaration is represented both as an ordinary data
    // definition and as a wire schema. The two views own separate copies of
    // each field's outer type reference, so resolving the ordinary data copy
    // above does not resolve the schema copy used by wire validation and
    // planning. Bind every wire-owned copy through the same source-aware type
    // lookup. Historical version fields live in this arena too, so one flat
    // traversal covers both current and retained eras.
    program
        .tables
        .declarations
        .wire_members
        .for_each_mut(|_, member| {
            let symbol_resolved_trees::wire::WireMember::Field(field) = member else {
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
        // The shared type-reference walker needs only lexical name/symbol
        // pairs. Build an ephemeral view; the durable proposition telescope
        // remains its own proof-static representation and never becomes an
        // executable machine generic.
        let local_binders = proposition_binders
            .span_or_empty(proposition.binders)
            .iter()
            .map(|binder| symbol_resolved_trees::data::TypeParameter {
                symbol: binder.symbol,
                name: binder.name.clone(),
                kind: symbol_resolved_trees::data::TypeParameterKind::Type,
                bounds: binder.bounds,
            })
            .collect::<Vec<_>>();

        for binder in proposition_binders.span_mut_or_empty(proposition.binders) {
            let symbol_resolved_trees::proposition::PropositionBinderKind::Const {
                type_reference,
            } = &mut binder.kind
            else {
                continue;
            };
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_binders,
                type_reference,
            );
        }
        for parameter in state_parameters.span_mut_or_empty(proposition.parameters) {
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_binders,
                &mut parameter.type_reference,
            );
        }
        if let symbol_resolved_trees::proposition::PropositionBody::Witness { evidence } =
            &mut proposition.body
        {
            assign_type_reference_symbol_with_locals_and_constraints(
                symbols,
                child_type_references,
                type_constraints,
                &local_binders,
                evidence,
            );
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

/// Reclassify arguments in declaration-identity machine slots after the
/// selected trait has supplied their categories. Type-reference parsing keeps
/// `Trait::requirement` as one delimiter-safe named leaf; this pass binds that
/// leaf to the exact State symbol without pretending it is a runtime type.
/// Assign the proposition and machine-identity slot symbols of one
/// conformance-bound argument list: look up the bound trait in the slot
/// tables, then delegate each kind to its argument walker.
pub(in crate::symbols) fn assign_trait_bound_slot_argument_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    bound_trait_symbol: SymbolHandle,
    arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
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

/// A trait's ordered binder children classify its written arguments. Ordinary
/// type lookup excludes code maps; only an implementation slot can select one.
/// Lexical binders shadow global declarations even when their kind is wrong,
/// so checking diagnoses the selected binder rather than admitting a fallback.
pub(in crate::symbols) fn assign_trait_implementation_argument_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    trait_symbol: SymbolHandle,
    arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
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
            symbol_resolved_trees::types::TypeReference::Named { symbol, name, .. } => {
                (symbol, &*name)
            }
            symbol_resolved_trees::types::TypeReference::Generic(generic) => {
                (&mut generic.storage.base_symbol, &generic.storage.base_name)
            }
            _ => continue,
        };
        *selected = local_type_parameters
            .iter()
            .find(|parameter| parameter.name.as_str() == name.as_str())
            .map(|parameter| parameter.symbol)
            .unwrap_or_else(|| top_level_symbol_for_source(symbols, SymbolKind::Conformance, name));
    }
}

pub(in crate::symbols) fn assign_machine_declaration_identity_argument_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
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
        let symbol_resolved_trees::types::TypeReference::Named { symbol, name, .. } = argument
        else {
            continue;
        };
        let local = local_type_parameters
            .iter()
            .find(|parameter| {
                parameter.name.as_str() == name.as_str()
            })
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
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
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
        let symbol_resolved_trees::types::TypeReference::Named { symbol, name, .. } = argument
        else {
            continue;
        };
        let local = local_type_parameters
            .iter()
            .find(|parameter| {
                parameter.name.as_str() == name.as_str()
            })
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
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    type_reference: &mut symbol_resolved_trees::types::TypeReference,
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
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    type_reference: &mut symbol_resolved_trees::types::TypeReference,
) {
    match type_reference {
        symbol_resolved_trees::types::TypeReference::Reference(reference) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                reference.referee,
            );
        }
        symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                constrained.base_type,
            );
            for constraint in type_constraints.span_or_empty(constrained.constraints) {
                let symbol_resolved_trees::types::TypeConstraint::Domain(domain) = constraint
                else {
                    continue;
                };
                assign_type_reference_argument_symbols_with_constraints(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    domain.arguments,
                );
            }
        }
        symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                fixed_array.element_type,
            );
            if let symbol_resolved_trees::types::FixedArrayLength::Expression(value) =
                fixed_array.length
            {
                assign_type_reference_handle_symbol_with_context(
                    symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    value,
                );
            }
            assign_fixed_array_length_symbol(
                symbols,
                local_type_parameters,
                &mut fixed_array.length,
            );
        }
        symbol_resolved_trees::types::TypeReference::Slice(slice) => {
            assign_type_reference_handle_symbol_with_context(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                slice.element_type,
            );
        }
        symbol_resolved_trees::types::TypeReference::Generic(generic) => {
            generic.base_symbol =
                resolve_type_symbol(symbols, local_type_parameters, &generic.base_name);

            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                generic.arguments,
            );
        }
        // PDI3 index expressions retain their lexical binder spellings here;
        // typed index normalization resolves them against the enclosing const
        // telescope and records the exact selected operation separately.
        symbol_resolved_trees::types::TypeReference::ConstExpression(_) => {}
        symbol_resolved_trees::types::TypeReference::DynamicTrait {
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
            assign_type_reference_argument_symbols_with_constraints(
                symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                *arguments,
            );
        }
        symbol_resolved_trees::types::TypeReference::Named { symbol, name, .. } => {
            *symbol = resolve_type_symbol(symbols, local_type_parameters, name);
        }
        symbol_resolved_trees::types::TypeReference::Unit => {}
    }
}

fn assign_fixed_array_length_symbol(
    symbols: &SymbolTable,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    length: &mut symbol_resolved_trees::types::FixedArrayLength,
) {
    match length {
        symbol_resolved_trees::types::FixedArrayLength::ConstParameter { symbol, name } => {
            // A named extent is a value selection. Lexical binders shadow
            // declaration constants, including an invalid type-valued binder;
            // never fall through to a same-spelled global after selecting one.
            *symbol = local_type_parameters
                .iter()
                .find(|parameter| parameter.name.as_str() == name.as_str())
                .map(|parameter| parameter.symbol)
                .unwrap_or_else(|| top_level_symbol_for_source(symbols, SymbolKind::Const, name));
        }
        symbol_resolved_trees::types::FixedArrayLength::ConstCall { machine, name } => {
            // The pending call carries the same source-qualified selection as
            // an ordinary free call. Later demand and evaluation consume this
            // identity instead of looking up its diagnostic spelling again.
            let entry = resolve_free_machine_entry_state_symbol(symbols, name);
            *machine = if entry.is_valid() {
                symbols.get(entry).parent
            } else {
                SymbolHandle::invalid()
            };
        }
        symbol_resolved_trees::types::FixedArrayLength::Literal(_)
        | symbol_resolved_trees::types::FixedArrayLength::Expression(_) => {}
    }
}

fn assign_type_parameter_constraint_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    type_parameters: &mut [symbol_resolved_trees::data::TypeParameter],
) {
    for parameter in type_parameters {
        let (symbol_resolved_trees::data::TypeParameterKind::Const { type_reference }
        | symbol_resolved_trees::data::TypeParameterKind::Value { type_reference }) =
            &mut parameter.kind
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
) {
    let mut type_reference = std::mem::take(child_type_references.get_mut(handle));
    assign_type_reference_symbol_with_context(
        symbols,
        child_type_references,
        type_constraints,
        local_type_parameters,
        &mut type_reference,
    );
    *child_type_references.get_mut(handle) = type_reference;
}

pub(in crate::symbols) fn assign_type_reference_argument_symbols_with_constraints(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    arguments: HandleSpan<symbol_resolved_trees::types::TypeReference>,
) {
    for handle in arguments.handles() {
        let mut argument = std::mem::take(child_type_references.get_mut(handle));
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

// Cast/zero-value target leaves use a separate semantic-domain field and do
// not own declaration constraint spans. Signature/declaration traversal uses
// the `_and_constraints` variants above so indexed arguments resolve in their
// lexical generic scope.
pub(in crate::symbols) fn assign_type_reference_symbol_with_locals(
    symbols: &SymbolTable,
    child_type_references: &mut Arena<symbol_resolved_trees::types::TypeReference>,
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    type_reference: &mut symbol_resolved_trees::types::TypeReference,
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
    local_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    name: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    local_type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol)
        .unwrap_or_else(|| top_level_type_symbol_for_source(symbols, name))
}
