//! Transactional append-only typing and exact retained-base admission.

use super::{
    Lowerer, exact_field_symbol, exact_top_level_data_symbol, lower_authored_service_reach_row,
    lower_symbol_resolved_trees,
};
use crate::expressions::qualification_casts::normalize_qualification_casts_from;
use crate::type_reference::domain_constraints::normalize_domain_constraints_from;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;

mod seeded_local_instances;
mod seeded_type_application;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeededContinuationError {
    UnsupportedExtensionShape,
    CrossPairedResolvedBase,
    RetainedTypedBaseChanged,
    ResolvedSemanticTablesChanged,
    AuthoredSelectionPrefixChanged,
    AuthoredSelectionPrefixChangedDuringLowering,
    Lowering(Diagnostic),
}

/// Opaque ownership of one exact resolved/typed base pair. Capturing fails
/// when typing has minted symbols or lost the resolved selection prefix; those
/// bases require a future, broader continuation cohort.
#[derive(Debug, PartialEq, Eq)]
pub struct SeededTypingBase {
    resolved: Box<SymbolResolvedTrees>,
    typed: TypedTrees,
}

impl SeededTypingBase {
    /// Clone the exact retained predecessor for the append-only seeded
    /// resolver. The continuation later compares the resolver carrier's
    /// retained snapshot back to this owned snapshot before any mutation.
    pub fn resolved_base_for_extension(&self) -> SymbolResolvedTrees {
        (*self.resolved).clone()
    }

    pub fn typed(&self) -> &TypedTrees {
        &self.typed
    }

    pub fn typed_mut(&mut self) -> &mut TypedTrees {
        &mut self.typed
    }

    pub fn into_typed(self) -> TypedTrees {
        self.typed
    }
}

/// Run the ordinary complete resolved-to-typed lowering while retaining the
/// exact resolved predecessor inside an opaque continuation carrier.
pub fn lower_symbol_resolved_trees_to_seeded_base(
    resolved: SymbolResolvedTrees,
) -> Result<SeededTypingBase, Diagnostic> {
    let mut typed = lower_symbol_resolved_trees(&resolved)?;
    typed.symbols = resolved.symbols.clone();
    Ok(SeededTypingBase {
        resolved: Box::new(resolved),
        typed,
    })
}

/// Append one supported generated-source cohort to an exact retained typed
/// base. Mutation occurs on a clone, so every error returns the input base
/// byte-for-byte; no caller is licensed to reconstruct a second frontend.
pub fn lower_seeded_extension(
    source: syntax_trees_to_symbol_resolved_trees::RebasedSeededSymbolResolvedTrees,
    retained: SeededTypingBase,
) -> Result<TypedTrees, (SeededTypingBase, SeededContinuationError)> {
    let (source, resolved_base) = source.into_typing_continuation_parts();
    if resolved_base != *retained.resolved {
        return Err((retained, SeededContinuationError::CrossPairedResolvedBase));
    }
    if retained.typed.symbols != retained.resolved.symbols
        || retained.typed.service_reaches != retained.resolved.service_reaches
        || !retained
            .typed
            .authored_declaration_selections()
            .as_slice()
            .starts_with(
                retained
                    .resolved
                    .authored_declaration_selections()
                    .as_slice(),
            )
    {
        return Err((retained, SeededContinuationError::RetainedTypedBaseChanged));
    }
    let data_frontier = resolved_base.data_definitions.len();
    let const_frontier = resolved_base.const_declarations.len();
    let machine_frontier = resolved_base.machines.len();
    let trait_frontier = resolved_base.traits.len();
    let typed_machine_frontier = retained.typed.machines().len();
    let type_reference_frontier = retained.typed.type_reference_table.type_reference_count();
    let expression_frontier = retained.typed.expression_table.expression_count();
    if !resolved_root_shape_is_supported(&source, &resolved_base)
        || !seeded_extension_shape_is_supported(
            &source,
            data_frontier,
            machine_frontier,
            trait_frontier,
        )
    {
        return Err((retained, SeededContinuationError::UnsupportedExtensionShape));
    }
    if source.service_reaches != resolved_base.service_reaches
        || !source
            .service_reach_rows
            .starts_with(&resolved_base.service_reach_rows)
        || !source
            .service_reach_rows
            .starts_with(&retained.typed.service_reach_rows)
        || !retained_authored_service_reaches_are_exact(&source, &resolved_base)
        || source.semantic_domains != resolved_base.semantic_domains
        || source.external_bindings != resolved_base.external_bindings
        || source.evidence_forwardings != resolved_base.evidence_forwardings
    {
        return Err((
            retained,
            SeededContinuationError::ResolvedSemanticTablesChanged,
        ));
    }
    let destination_ledger = retained.typed.authored_declaration_selections();
    if !source
        .authored_declaration_selections()
        .as_slice()
        .starts_with(destination_ledger.as_slice())
    {
        return Err((
            retained,
            SeededContinuationError::AuthoredSelectionPrefixChanged,
        ));
    }

    let resolved_ledger = source.authored_declaration_selections().clone();
    let mut candidate = retained.typed.clone();
    candidate.retain_authored_declaration_selections(resolved_ledger.clone());
    candidate.symbols = source.symbols.clone();
    // Resolution may append combinations of retained services, but both base
    // tables must keep their exact row IDs and meanings.
    candidate.service_reach_rows = source.service_reach_rows.clone();
    candidate.authored_service_reach_rows.extend(
        source
            .authored_service_reach_rows
            .iter()
            .filter(|row| {
                !resolved_base
                    .authored_service_reach_rows
                    .iter()
                    .any(|base_row| base_row.owner == row.owner)
            })
            .map(lower_authored_service_reach_row),
    );
    let mut lowerer = Lowerer {
        typed_trees: candidate,
        source_trees: &source,
        equality_scope: None,
        type_reference_exposure:
            Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
    };
    for declaration in source.const_declarations.iter().skip(const_frontier) {
        if let Err(error) = lowerer.lower_const_declaration(declaration) {
            return Err((retained, SeededContinuationError::Lowering(error)));
        }
    }
    for data_definition in source.data_definitions.iter().skip(data_frontier) {
        if let Err(error) = lowerer.lower_data_declaration(data_definition) {
            return Err((retained, SeededContinuationError::Lowering(error)));
        }
    }
    for machine in source.machines.iter().skip(machine_frontier) {
        if let Err(error) = lowerer.lower_machine_declaration(machine) {
            return Err((retained, SeededContinuationError::Lowering(error)));
        }
    }
    for trait_definition in source.traits.iter().skip(trait_frontier) {
        if let Err(error) = lowerer.lower_trait_declaration(trait_definition) {
            return Err((retained, SeededContinuationError::Lowering(error)));
        }
    }
    if let Err(error) = crate::declarations::machine::settle_satisfied_declarations_from(
        &mut lowerer.typed_trees,
        typed_machine_frontier,
    ) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) = lowerer.lower_const_initializer_evidence(const_frontier) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) = crate::lowerer::progress::normalize_progress_premises_from(
        &mut lowerer.typed_trees,
        typed_machine_frontier,
    ) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) = normalize_domain_constraints_from(
        &source,
        &mut lowerer.typed_trees,
        type_reference_frontier,
    ) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    // Idempotent over retained facts: an already-interned instance re-derives
    // the same name, so only facts appended past the checkpoint change.
    if let Err(error) =
        crate::contracts::proof_facts::intern_proof_membership_instances(&mut lowerer.typed_trees)
    {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) =
        normalize_qualification_casts_from(&source, &mut lowerer.typed_trees, expression_frontier)
    {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) =
        crate::expressions::fixed_byte_array_literals::land_exact_fixed_byte_array_literals_from(
            &mut lowerer.typed_trees,
            expression_frontier,
            typed_machine_frontier,
        )
    {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) =
        crate::type_reference::validate_range_arguments(&source, &lowerer.typed_trees)
    {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if !lowerer
        .typed_trees
        .authored_declaration_selections()
        .as_slice()
        .starts_with(resolved_ledger.as_slice())
    {
        return Err((
            retained,
            SeededContinuationError::AuthoredSelectionPrefixChangedDuringLowering,
        ));
    }
    if !retained_typed_base_is_exact_prefix(&retained.typed, &lowerer.typed_trees) {
        return Err((retained, SeededContinuationError::RetainedTypedBaseChanged));
    }
    Ok(lowerer.typed_trees)
}

/// Verify the semantic and custody-bearing base prefix after every extension
/// phase, including compiler-owned pre-check evaluation performed by Omega.
pub fn retained_typed_base_is_exact_prefix(base: &TypedTrees, candidate: &TypedTrees) -> bool {
    let roots_are_prefixes = candidate.retains_exact_root_storage(base);
    let symbol_prefix_is_exact = candidate.symbols.symbols().nodes().len()
        >= base.symbols.symbols().nodes().len()
        && candidate
            .symbols
            .symbols()
            .nodes()
            .iter()
            .take(base.symbols.symbols().nodes().len())
            .eq(base.symbols.symbols().nodes().iter())
        && candidate.symbols.names().len() >= base.symbols.names().len()
        && candidate
            .symbols
            .names()
            .iter()
            .take(base.symbols.names().len())
            .map(|(_, name)| name)
            .eq(base.symbols.names().iter().map(|(_, name)| name))
        && candidate.symbols.path_member_arena().len() >= base.symbols.path_member_arena().len()
        && candidate
            .symbols
            .path_member_arena()
            .iter()
            .take(base.symbols.path_member_arena().len())
            .map(|(_, member)| member)
            .eq(base
                .symbols
                .path_member_arena()
                .iter()
                .map(|(_, member)| member));
    roots_are_prefixes
        && symbol_prefix_is_exact
        && candidate.service_reaches == base.service_reaches
        && candidate
            .service_reach_rows
            .starts_with(&base.service_reach_rows)
        && candidate
            .authored_service_reach_rows
            .starts_with(&base.authored_service_reach_rows)
        && candidate.semantic_domains == base.semantic_domains
        && candidate.external_bindings == base.external_bindings
        && candidate
            .plan_laid_layouts
            .starts_with(&base.plan_laid_layouts)
        && candidate
            .placed_view_plans
            .starts_with(&base.placed_view_plans)
        && arena_is_exact_prefix(&base.wire_placements, &candidate.wire_placements)
        && arena_is_exact_prefix(
            &base.wire_encode_obligations,
            &candidate.wire_encode_obligations,
        )
        && candidate
            .wire_schema_plans
            .starts_with(&base.wire_schema_plans)
        && candidate
            .machine_specializations
            .starts_with(&base.machine_specializations)
        && candidate
            .boundary_calling_plans
            .starts_with(&base.boundary_calling_plans)
        && candidate
            .open_index_normalizations
            .starts_with(&base.open_index_normalizations)
        && candidate
            .evidence_forwardings
            .starts_with(&base.evidence_forwardings)
        && candidate
            .proof_output_calls
            .starts_with(&base.proof_output_calls)
        && candidate
            .ranking_expression_custody
            .starts_with(&base.ranking_expression_custody)
}

fn retained_authored_service_reaches_are_exact(
    source: &SymbolResolvedTrees,
    base: &SymbolResolvedTrees,
) -> bool {
    // Resolution groups machines before trait requirements, so generated
    // machine rows can precede retained requirement rows. Rejoin by owner;
    // preserve the typed base order and append only generated machine rows.
    let retained_owner = |row: &&symbol_resolved_trees::signature::AuthoredServiceReachRow| {
        base.authored_service_reach_rows
            .iter()
            .any(|base_row| base_row.owner == row.owner)
    };
    source
        .authored_service_reach_rows
        .iter()
        .filter(retained_owner)
        .eq(base.authored_service_reach_rows.iter())
        && source
            .authored_service_reach_rows
            .iter()
            .filter(|row| !retained_owner(row))
            .all(|row| {
                source
                    .machines
                    .iter()
                    .skip(base.machines.len())
                    .any(|machine| machine.symbol == row.owner)
            })
}

fn arena_is_exact_prefix<T: Default + PartialEq>(
    base: &arena::Arena<T>,
    candidate: &arena::Arena<T>,
) -> bool {
    candidate.len() >= base.len()
        && candidate
            .iter()
            .take(base.len())
            .map(|(_, value)| value)
            .eq(base.iter().map(|(_, value)| value))
}

pub(super) fn resolved_root_shape_is_supported(
    source: &SymbolResolvedTrees,
    base: &SymbolResolvedTrees,
) -> bool {
    source
        .const_declarations
        .iter()
        .take(base.const_declarations.len())
        .eq(base.const_declarations.iter())
        && source.const_declarations.len() >= base.const_declarations.len()
        && source
            .const_declarations
            .iter()
            .skip(base.const_declarations.len())
            .all(|declaration| {
                seeded_local_instances::const_declaration_is_supported(source, declaration)
            })
        && source
            .data_definitions
            .iter()
            .take(base.data_definitions.len())
            .eq(base.data_definitions.iter())
        && source.data_definitions.len() >= base.data_definitions.len()
        && source.domain_definitions == base.domain_definitions
        && source
            .machines
            .iter()
            .take(base.machines.len())
            .eq(base.machines.iter())
        && source.machines.len() >= base.machines.len()
        && source.measures == base.measures
        && source.operators == base.operators
        && source.propositions == base.propositions
        // Mathematical declarations are retained roots, never extension
        // roots: an extension can never add one, so the whole arena must
        // match the base exactly.
        && source.mathematical_definitions == base.mathematical_definitions
        && source
            .traits
            .iter()
            .take(base.traits.len())
            .eq(base.traits.iter())
        && source.traits.len() >= base.traits.len()
        && source.conformances == base.conformances
        && source.wire_schemas == base.wire_schemas
}

pub(super) fn seeded_extension_shape_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    machine_frontier: usize,
    trait_frontier: usize,
) -> bool {
    let Some(local_instances) = seeded_local_instances::validated_symbols(source, data_frontier)
    else {
        return false;
    };
    source
        .data_definitions
        .iter()
        .skip(data_frontier)
        .all(|definition| {
            if definition.generic_instance.is_some() {
                return local_instances.contains(&definition.symbol);
            }
            let type_parameters = source.data_type_parameters(definition.type_parameters);
            exact_top_level_data_symbol(source, definition)
                && type_parameters.iter().all(|parameter| {
                    seeded_local_instances::parameter_is_supported(
                        source,
                        definition.symbol,
                        parameter,
                    )
                })
                && definition.quotient.is_none()
                && definition.where_facts.is_empty()
                && !definition.zero_gated
                && source
                    .data_members(definition.members)
                    .iter()
                    .all(|member| {
                        let fields = match member {
                            symbol_resolved_trees::data::DataMember::Field(field) => {
                                std::slice::from_ref(field)
                            }
                            symbol_resolved_trees::data::DataMember::Variant(variant) => {
                                source.data_payload_fields(variant.payload)
                            }
                        };
                        fields.iter().all(|field| {
                            plain_type_is_supported(
                                source,
                                data_frontier,
                                &local_instances,
                                definition.symbol,
                                &definition.lifetime_parameters,
                                type_parameters,
                                &field.type_reference,
                            )
                        })
                    })
        })
        && source
            .machines
            .iter()
            .skip(machine_frontier)
            .all(|machine| {
                exact_extension_machine_symbol(source, data_frontier, &local_instances, machine)
            })
        && source
            .traits
            .iter()
            .skip(trait_frontier)
            .all(|trait_definition| {
                exact_extension_trait_definition(
                    source,
                    data_frontier,
                    &local_instances,
                    trait_definition,
                )
            })
}

#[cfg(test)]
pub(super) fn plain_data_extension_shape_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
) -> bool {
    seeded_extension_shape_is_supported(
        source,
        data_frontier,
        source.machines.len(),
        source.traits.len(),
    )
}

fn exact_extension_trait_definition(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
) -> bool {
    let requirements = source.trait_machine_signatures(trait_definition.machines);
    exact_flat_trait_definition(source, trait_definition)
        && !requirements.is_empty()
        && requirements.iter().all(|requirement| {
            exact_flat_trait_requirement(
                source,
                data_frontier,
                local_instances,
                trait_definition,
                requirement,
            )
        })
}

fn exact_flat_trait_definition(
    source: &SymbolResolvedTrees,
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
) -> bool {
    trait_definition.symbol.is_valid()
        && source.symbols.get(trait_definition.symbol).kind == symbols::SymbolKind::Trait
        && source.symbols.get(trait_definition.symbol).parent == source.symbols.root()
        && source.symbols.name(trait_definition.symbol) == trait_definition.name.as_str()
        && !trait_definition.is_boundary
        && trait_definition.lifetime_parameters.is_empty()
        && source
            .data_type_parameters(trait_definition.type_parameters)
            .is_empty()
        && trait_definition.conformance_bounds.is_empty()
        && source
            .trait_requirements(trait_definition.requires)
            .is_empty()
}

fn exact_flat_trait_requirement(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    trait_definition: &symbol_resolved_trees::trait_definition::TraitDefinition,
    requirement: &symbol_resolved_trees::signature::StateSignature,
) -> bool {
    requirement.symbol.is_valid()
        && source.symbols.get(requirement.symbol).kind == symbols::SymbolKind::State
        && source.symbols.get(requirement.symbol).parent == trait_definition.symbol
        && source.symbols.name(requirement.symbol) == requirement.name.as_str()
        && requirement.spelling.is_none()
        && requirement.lifetime_parameters.is_empty()
        && source
            .data_type_parameters(requirement.type_parameters)
            .is_empty()
        && !requirement.is_default
        && requirement.native_callback_parameters.is_empty()
        && source
            .state_parameters(requirement.parameters)
            .iter()
            .all(|value| {
                value.symbol.is_valid()
                    && source.symbols.get(value.symbol).kind == symbols::SymbolKind::Parameter
                    && source.symbols.get(value.symbol).parent == requirement.symbol
                    && source.symbols.name(value.symbol) == value.name.as_str()
                    && !value.is_self
                    && plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        symbols::SymbolHandle::invalid(),
                        &[],
                        &[],
                        &value.type_reference,
                    )
            })
        && requirement.return_type.as_ref().is_none_or(|return_type| {
            plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                symbols::SymbolHandle::invalid(),
                &[],
                &[],
                return_type,
            )
        })
        && requirement.invokes.is_empty()
        && requirement.service_reach_row == language_semantics::ServiceReachRowTable::EMPTY_ROW
        && !requirement.service_reach_is_installation_bound
        && requirement.suspends_keyword_source_spans.is_empty()
        && requirement.blocks_keyword_source_spans.is_empty()
        && !requirement.suspends
        && !requirement.blocks
        && requirement.contracts.is_empty()
        && !requirement.terminates_guarantee
}

fn exact_extension_machine_symbol(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    machine: &symbol_resolved_trees::machine::Machine,
) -> bool {
    let type_parameters = source.data_type_parameters(machine.type_parameters);
    if !machine.symbol.is_valid()
        || source.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
        || source.symbols.name(machine.symbol) != machine.name.as_str()
        || !type_parameters
            .iter()
            .all(|parameter| {
                match &parameter.kind {
                symbol_resolved_trees::data::TypeParameterKind::Type => {
                    seeded_local_instances::parameter_is_supported(
                        source,
                        machine.symbol,
                        parameter,
                    )
                }
                symbol_resolved_trees::data::TypeParameterKind::Const { .. }
                | symbol_resolved_trees::data::TypeParameterKind::Value { .. } => {
                    seeded_local_instances::const_parameter_is_supported(
                        source,
                        machine.symbol,
                        parameter,
                    )
                }
                symbol_resolved_trees::data::TypeParameterKind::Machine { contract } => {
                    match contract {
                        symbol_resolved_trees::data::MachineParameterContract::Structural(
                            _,
                        ) => exact_extension_structural_machine_parameter(
                            source,
                            data_frontier,
                            local_instances,
                            machine,
                            type_parameters,
                            parameter,
                            contract,
                        ),
                        symbol_resolved_trees::data::MachineParameterContract::Nominal {
                            ..
                        } => exact_extension_nominal_machine_parameter(
                            source,
                            data_frontier,
                            local_instances,
                            machine,
                            type_parameters,
                            parameter,
                            contract,
                        ),
                        symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity
                        | symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                            ..
                        } => false,
                    }
                }
                symbol_resolved_trees::data::TypeParameterKind::Proposition { .. } => false,
            }
            })
        || !machine.satisfies.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !machine.ranking_subjects.is_empty()
        || !machine.ranking_view.is_empty()
        || !machine.ranking_view_arguments.is_empty()
        || machine.ranking_range.is_valid()
        || machine.suspends
        || machine.blocks
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return false;
    }
    if type_parameters.iter().any(|parameter| {
        seeded_local_instances::structured_const_parameter_is_supported(
            source,
            machine.symbol,
            parameter,
        )
    }) && !source
        .machine_state_handles(machine.states)
        .iter()
        .all(|state| {
            let state = source.machine_state(*state);
            source
                .state_parameters(state.parameters)
                .iter()
                .all(|parameter| {
                    plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        machine.symbol,
                        &machine.lifetime_parameters,
                        type_parameters,
                        &parameter.type_reference,
                    )
                })
                && state.return_type.as_ref().is_none_or(|return_type| {
                    plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        machine.symbol,
                        &machine.lifetime_parameters,
                        type_parameters,
                        return_type,
                    )
                })
        })
    {
        return false;
    }
    let parent = source.symbols.get(machine.symbol).parent;
    match (&machine.attached_data, machine.attached_data_symbol) {
        (None, attached) => parent == source.symbols.root() && !attached.is_valid(),
        (Some(name), attached) => {
            parent == source.symbols.root()
                && attached.is_valid()
                && source.data_definitions.iter().any(|definition| {
                    definition.symbol == attached && definition.name.as_str() == name.as_str()
                })
        }
    }
}

fn exact_extension_structural_machine_parameter(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    machine: &symbol_resolved_trees::machine::Machine,
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    parameter: &symbol_resolved_trees::data::TypeParameter,
    contract: &symbol_resolved_trees::data::MachineParameterContract,
) -> bool {
    let symbol_resolved_trees::data::MachineParameterContract::Structural(signature) = contract
    else {
        return false;
    };
    parameter.bounds == symbol_resolved_trees::data::DataProperties::default()
        && parameter.symbol.is_valid()
        && source.symbols.get(parameter.symbol).kind == symbols::SymbolKind::MachineParameter
        && source.symbols.get(parameter.symbol).parent == machine.symbol
        && source.symbols.name(parameter.symbol) == parameter.name.as_str()
        && signature.symbol == parameter.symbol
        && signature.name.as_str() == parameter.name.as_str()
        && signature.lifetime_parameters.is_empty()
        && signature.type_parameters.is_empty()
        && !signature.is_default
        && signature.native_callback_parameters.is_empty()
        && signature.invokes.is_empty()
        && !signature.service_reach_is_installation_bound
        && signature.suspends_keyword_source_spans.is_empty()
        && signature.blocks_keyword_source_spans.is_empty()
        && !signature.suspends
        && !signature.blocks
        && signature.contracts.is_empty()
        && !signature.terminates_guarantee
        && source
            .state_parameters(signature.parameters)
            .iter()
            .all(|value| {
                value.symbol.is_valid()
                    && source.symbols.get(value.symbol).kind == symbols::SymbolKind::Parameter
                    && source.symbols.get(value.symbol).parent == parameter.symbol
                    && source.symbols.name(value.symbol) == value.name.as_str()
                    && !value.is_self
                    && plain_type_is_supported(
                        source,
                        data_frontier,
                        local_instances,
                        machine.symbol,
                        &machine.lifetime_parameters,
                        owner_type_parameters,
                        &value.type_reference,
                    )
            })
        && signature.return_type.as_ref().is_none_or(|return_type| {
            plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                machine.symbol,
                &machine.lifetime_parameters,
                owner_type_parameters,
                return_type,
            )
        })
}

fn exact_extension_nominal_machine_parameter(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    machine: &symbol_resolved_trees::machine::Machine,
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    parameter: &symbol_resolved_trees::data::TypeParameter,
    contract: &symbol_resolved_trees::data::MachineParameterContract,
) -> bool {
    let symbol_resolved_trees::data::MachineParameterContract::Nominal {
        trait_definition,
        requirement,
        authored_path,
    } = contract
    else {
        return false;
    };
    let [trait_path @ .., requirement_name] = authored_path.as_slice() else {
        return false;
    };
    let trait_definitions = source
        .traits
        .iter()
        .filter(|candidate| candidate.symbol == *trait_definition)
        .collect::<Vec<_>>();
    let [trait_definition] = trait_definitions.as_slice() else {
        return false;
    };
    let requirements = source
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|candidate| candidate.symbol == *requirement)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return false;
    };
    parameter.bounds == symbol_resolved_trees::data::DataProperties::default()
        && parameter.symbol.is_valid()
        && source.symbols.get(parameter.symbol).kind == symbols::SymbolKind::MachineParameter
        && source.symbols.get(parameter.symbol).parent == machine.symbol
        && source.symbols.name(parameter.symbol) == parameter.name.as_str()
        && owner_type_parameters
            .iter()
            .filter(|candidate| {
                matches!(
                    candidate.kind,
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. }
                )
            })
            .count()
            == 1
        && exact_flat_trait_definition(source, trait_definition)
        && !trait_path.is_empty()
        && trait_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::")
            == trait_definition.name.as_str()
        && requirement_name.as_str() == requirement.name.as_str()
        && exact_flat_trait_requirement(
            source,
            data_frontier,
            local_instances,
            trait_definition,
            requirement,
        )
}

fn plain_type_is_supported(
    source: &SymbolResolvedTrees,
    data_frontier: usize,
    local_instances: &[symbols::SymbolHandle],
    owner: symbols::SymbolHandle,
    owner_lifetimes: &[symbol_resolved_trees::name::DiagnosticName],
    owner_type_parameters: &[symbol_resolved_trees::data::TypeParameter],
    type_reference: &symbol_resolved_trees::types::TypeReference,
) -> bool {
    use symbol_resolved_trees::types::TypeReference;
    match type_reference {
        TypeReference::Named { symbol, .. } if !symbol.is_valid() => false,
        TypeReference::Named { symbol, name } if source.symbols.name(*symbol) != name.as_str() => {
            false
        }
        TypeReference::Named { symbol, .. } => match source.symbols.get(*symbol).kind {
            symbols::SymbolKind::BuiltinType => true,
            symbols::SymbolKind::Data => source.data_definitions.iter().any(|definition| {
                definition.symbol == *symbol
                    && definition.lifetime_parameters.is_empty()
                    && definition.type_parameters.is_empty()
                    && (definition.generic_instance.is_none() || local_instances.contains(symbol))
            }),
            symbols::SymbolKind::TypeParameter => owner_type_parameters.iter().any(|parameter| {
                parameter.symbol == *symbol
                    && matches!(
                        parameter.kind,
                        symbol_resolved_trees::data::TypeParameterKind::Type
                    )
            }),
            _ => false,
        },
        TypeReference::SelfType { symbol } => *symbol == owner,
        TypeReference::Unit => true,
        TypeReference::Reference(reference) => {
            reference.lifetime.as_ref().is_none_or(|lifetime| {
                owner_lifetimes
                    .iter()
                    .any(|parameter| parameter.as_str() == lifetime.as_str())
            }) && plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                source.child_type_reference(reference.referee),
            )
        }
        TypeReference::Slice(slice) => plain_type_is_supported(
            source,
            data_frontier,
            local_instances,
            owner,
            owner_lifetimes,
            owner_type_parameters,
            source.child_type_reference(slice.element_type),
        ),
        TypeReference::FixedArray(array) => {
            seeded_local_instances::array_length_is_supported(
                source,
                owner,
                owner_type_parameters,
                &array.length,
            ) && plain_type_is_supported(
                source,
                data_frontier,
                local_instances,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                source.child_type_reference(array.element_type),
            )
        }
        TypeReference::Generic(generic) => {
            seeded_local_instances::instance_application_is_supported(
                source,
                local_instances,
                owner_lifetimes,
                generic,
            ) || seeded_local_instances::template_application_is_supported(
                source,
                data_frontier,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                generic,
            ) || seeded_type_application::is_supported(
                source,
                data_frontier,
                local_instances,
                owner,
                owner_lifetimes,
                owner_type_parameters,
                generic,
            )
        }
        TypeReference::Constrained(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. } => false,
    }
}
