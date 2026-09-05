use crate::data::lower_data_definition;
use crate::domain::lower_domain_definition;
use crate::domain_constraints::{normalize_domain_constraints, normalize_domain_constraints_from};
use crate::machine::lower_machine;
use crate::operator::lower_operator_definition;
use crate::qualification_casts::{
    normalize_qualification_casts, normalize_qualification_casts_from,
};
use crate::trait_definition::lower_trait_definition;
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
        let declared_type = match lowerer.with_type_reference_exposure(
            declaration_exposure(declaration.is_public),
            |lowerer| {
                crate::type_reference::lower_type_reference_into_table(
                    lowerer,
                    &declaration.declared_type,
                )
            },
        ) {
            Ok(declared_type) => declared_type,
            Err(error) => {
                return Err((retained, SeededContinuationError::Lowering(error)));
            }
        };
        lowerer
            .typed_trees
            .push_const_declaration(typed_trees::constant::ConstDeclaration {
                symbol: declaration.symbol,
                is_public: declaration.is_public,
                declared_type,
                initializer_source_span: declaration.initializer_source_span,
                canonical_value_encoding: declaration.canonical_value_encoding.clone(),
            });
    }
    for data_definition in source.data_definitions.iter().skip(data_frontier) {
        let lowered = match lowerer.with_type_reference_exposure(
            declaration_exposure(data_definition.is_public),
            |lowerer| lower_data_definition(lowerer, data_definition),
        ) {
            Ok(lowered) => lowered,
            Err(error) => {
                return Err((retained, SeededContinuationError::Lowering(error)));
            }
        };
        lowerer.typed_trees.push_data_definition(lowered);
    }
    for machine in source.machines.iter().skip(machine_frontier) {
        let lowered = match lowerer
            .with_type_reference_exposure(machine_interface_exposure(machine), |lowerer| {
                lower_machine(lowerer, machine)
            }) {
            Ok(lowered) => lowered,
            Err(error) => {
                return Err((retained, SeededContinuationError::Lowering(error)));
            }
        };
        lowerer.typed_trees.push_machine(lowered);
    }
    for trait_definition in source.traits.iter().skip(trait_frontier) {
        let lowered = match lowerer.with_type_reference_exposure(
            declaration_exposure(trait_definition.is_public),
            |lowerer| lower_trait_definition(lowerer, trait_definition),
        ) {
            Ok(lowered) => lowered,
            Err(error) => {
                return Err((retained, SeededContinuationError::Lowering(error)));
            }
        };
        lowerer.typed_trees.push_trait_definition(lowered);
    }
    if let Err(error) = crate::machine::settle_satisfied_declarations_from(
        &mut lowerer.typed_trees,
        typed_machine_frontier,
    ) {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) = crate::progress::normalize_progress_premises_from(
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
    if let Err(error) =
        normalize_qualification_casts_from(&mut lowerer.typed_trees, expression_frontier)
    {
        return Err((retained, SeededContinuationError::Lowering(error)));
    }
    if let Err(error) = crate::fixed_byte_array_literals::land_exact_fixed_byte_array_literals_from(
        &mut lowerer.typed_trees,
        expression_frontier,
        typed_machine_frontier,
    ) {
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
    let base_snapshot = base.snapshot();
    let candidate_snapshot = candidate.snapshot();
    let roots_are_prefixes = candidate_snapshot
        .roots
        .const_declarations
        .starts_with(&base_snapshot.roots.const_declarations)
        && candidate_snapshot
            .roots
            .data_definitions
            .starts_with(&base_snapshot.roots.data_definitions)
        && candidate_snapshot
            .roots
            .domain_definitions
            .starts_with(&base_snapshot.roots.domain_definitions)
        && candidate_snapshot
            .roots
            .machines
            .starts_with(&base_snapshot.roots.machines)
        && candidate_snapshot
            .roots
            .operators
            .starts_with(&base_snapshot.roots.operators)
        && candidate_snapshot
            .roots
            .propositions
            .starts_with(&base_snapshot.roots.propositions)
        && candidate_snapshot
            .roots
            .traits
            .starts_with(&base_snapshot.roots.traits)
        && candidate_snapshot
            .roots
            .conformances
            .starts_with(&base_snapshot.roots.conformances)
        && candidate_snapshot
            .roots
            .wire_schemas
            .starts_with(&base_snapshot.roots.wire_schemas)
        && candidate.measures().starts_with(base.measures());
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
        && candidate
            .authored_declaration_selections()
            .as_slice()
            .starts_with(base.authored_declaration_selections().as_slice())
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

fn lower_authored_service_reach_row(
    row: &symbol_resolved_trees::signature::AuthoredServiceReachRow,
) -> typed_trees::signature::AuthoredServiceReachRow {
    typed_trees::signature::AuthoredServiceReachRow {
        owner: row.owner,
        keyword_source_spans: row.keyword_source_spans.clone(),
        targets: row
            .targets
            .iter()
            .map(
                |target| typed_trees::signature::AuthoredServiceReachTarget {
                    service: target.service,
                    source_span: target.source_span,
                },
            )
            .collect(),
        installation_bound: row.installation_bound,
    }
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

fn resolved_root_shape_is_supported(
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
        && source
            .traits
            .iter()
            .take(base.traits.len())
            .eq(base.traits.iter())
        && source.traits.len() >= base.traits.len()
        && source.conformances == base.conformances
        && source.wire_schemas == base.wire_schemas
}

fn seeded_extension_shape_is_supported(
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
fn plain_data_extension_shape_is_supported(
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
                symbol_resolved_trees::data::TypeParameterKind::Const { .. } => {
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

fn exact_top_level_data_symbol(
    source: &SymbolResolvedTrees,
    definition: &symbol_resolved_trees::data::DataDefinition,
) -> bool {
    definition.symbol.is_valid()
        && source.symbols.get(definition.symbol).kind == symbols::SymbolKind::Data
        && source.symbols.get(definition.symbol).parent == source.symbols.root()
        && source.symbols.name(definition.symbol) == definition.name.as_str()
}

fn exact_field_symbol(
    source: &SymbolResolvedTrees,
    owner: symbols::SymbolHandle,
    field: &symbol_resolved_trees::data::DataField,
) -> bool {
    field.symbol.is_valid()
        && source.symbols.get(field.symbol).kind == symbols::SymbolKind::Field
        && source.symbols.get(field.symbol).parent == owner
        && source.symbols.name(field.symbol) == field.name.as_str()
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

pub fn lower_symbol_resolved_trees(
    symbol_resolved_trees: &SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    // Decision 11: user-written `==` against bare payload-bearing case names
    // must be rejected BEFORE membership lowering synthesizes its internal
    // tag-equality compares, which are deliberately the same typed shape.
    crate::equality::validate_equality_operands(symbol_resolved_trees)?;

    // Equatable conformance prerequisites error at the conformance item,
    // before any `==` site tries to expand against a malformed type.
    crate::equatable::validate_equatable_conformances(symbol_resolved_trees)?;

    // Exhaustiveness counting over case domains also needs the resolved
    // trees: membership is still a distinct node here, so case arms and
    // domain arms are recognizable before lowering erases them into tag
    // compares and classifier expansions.
    crate::exhaustiveness::validate_case_dispatch_exhaustiveness(symbol_resolved_trees)?;

    let mut lowerer = Lowerer {
        typed_trees: TypedTrees::default(),
        source_trees: symbol_resolved_trees,
        equality_scope: None,
        type_reference_exposure:
            Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation),
    };
    lowerer.typed_trees.service_reaches = symbol_resolved_trees.service_reaches.clone();
    lowerer.typed_trees.service_reach_rows = symbol_resolved_trees.service_reach_rows.clone();
    lowerer.typed_trees.authored_service_reach_rows = symbol_resolved_trees
        .authored_service_reach_rows
        .iter()
        .map(lower_authored_service_reach_row)
        .collect();
    lowerer.typed_trees.semantic_domains = symbol_resolved_trees.semantic_domains.clone();
    lowerer.typed_trees.external_bindings = symbol_resolved_trees.external_bindings.clone();
    lowerer.typed_trees.retain_authored_declaration_selections(
        symbol_resolved_trees
            .authored_declaration_selections()
            .clone(),
    );
    for declaration in &symbol_resolved_trees.const_declarations {
        let declared_type = lowerer.with_type_reference_exposure(
            declaration_exposure(declaration.is_public),
            |lowerer| {
                crate::type_reference::lower_type_reference_into_table(
                    lowerer,
                    &declaration.declared_type,
                )
            },
        )?;
        lowerer
            .typed_trees
            .push_const_declaration(typed_trees::constant::ConstDeclaration {
                symbol: declaration.symbol,
                is_public: declaration.is_public,
                declared_type,
                initializer_source_span: declaration.initializer_source_span,
                canonical_value_encoding: declaration.canonical_value_encoding.clone(),
            });
    }
    lowerer.typed_trees.evidence_forwardings = symbol_resolved_trees
        .evidence_forwardings
        .iter()
        .map(|forwarding| typed_trees::typed_trees::EvidenceForwarding {
            machine_symbol: forwarding.machine_symbol,
            state_symbol: forwarding.state_symbol,
            statement_index: forwarding.statement_index,
            source_statement_index: forwarding.statement_index,
            target: crate::name::lower_name(&forwarding.target),
            source: crate::name::lower_name(&forwarding.source),
            source_conformance: forwarding.source_conformance,
        })
        .collect();

    for data_definition in &symbol_resolved_trees.data_definitions {
        let data_definition = lowerer.with_type_reference_exposure(
            declaration_exposure(data_definition.is_public),
            |lowerer| lower_data_definition(lowerer, data_definition),
        )?;
        lowerer.typed_trees.push_data_definition(data_definition);
    }

    for domain_definition in &symbol_resolved_trees.domain_definitions {
        let domain_definition = lowerer.with_type_reference_exposure(
            declaration_exposure(domain_definition.is_public),
            |lowerer| lower_domain_definition(lowerer, domain_definition),
        )?;
        lowerer
            .typed_trees
            .push_domain_definition(domain_definition);
    }

    for proposition in &symbol_resolved_trees.propositions {
        let proposition = lowerer.with_type_reference_exposure(
            declaration_exposure(proposition.is_public),
            |lowerer| crate::proposition::lower_proposition_definition(lowerer, proposition),
        )?;
        lowerer.typed_trees.push_proposition(proposition);
    }

    for machine in &symbol_resolved_trees.machines {
        let machine = lowerer
            .with_type_reference_exposure(machine_interface_exposure(machine), |lowerer| {
                lower_machine(lowerer, machine)
            })?;
        lowerer.typed_trees.push_machine(machine);
    }

    for measure in &symbol_resolved_trees.measures {
        let measure = crate::measure::lower_measure_definition(&mut lowerer, measure)?;
        lowerer.typed_trees.push_measure(measure);
    }

    for operator in &symbol_resolved_trees.operators {
        let operator = lowerer
            .with_type_reference_exposure(declaration_exposure(operator.is_public), |lowerer| {
                lower_operator_definition(lowerer, operator)
            })?;
        lowerer.typed_trees.push_operator(operator);
    }

    for trait_definition in &symbol_resolved_trees.traits {
        let trait_definition = lowerer.with_type_reference_exposure(
            declaration_exposure(trait_definition.is_public),
            |lowerer| lower_trait_definition(lowerer, trait_definition),
        )?;
        lowerer.typed_trees.push_trait_definition(trait_definition);
    }

    for conformance in &symbol_resolved_trees.conformances {
        let conformance_exposure = declaration_exposure(conformance.is_public);
        if let symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(carrier_name) =
            &conformance.subject
        {
            crate::type_reference::retain_type_reference_selection(
                symbol_resolved_trees,
                &mut lowerer.typed_trees,
                carrier_name,
                conformance.carrier_symbol,
                conformance_exposure,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
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
        let mut arguments = arena::HandleSpan::empty();
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
            lifetime_parameters: conformance
                .lifetime_parameters
                .iter()
                .map(crate::name::lower_name)
                .collect(),
            type_parameters: arena::HandleSpan::empty(),
            subject: match &conformance.subject {
                symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                    type_name,
                ) => typed_trees::trait_definition::ConformanceSubject::Carrier(
                    crate::name::lower_name(type_name),
                ),
                symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                    typed_trees::trait_definition::ConformanceSubject::Subjectless
                }
            },
            carrier_symbol: conformance.carrier_symbol,
            trait_name: crate::name::lower_name(&conformance.trait_name),
            trait_symbol: conformance.trait_symbol,
            trait_lifetime_arguments,
            arguments,
            alias: conformance.alias.as_ref().map(crate::name::lower_name),
            implementation: match &conformance.implementation {
                symbol_resolved_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                    typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines
                }
                symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } => {
                    typed_trees::trait_definition::ConformanceImplementation::Closed {
                        rows: rows
                            .iter()
                            .map(|row| typed_trees::trait_definition::ConformanceRow {
                                declaring_trait: row.declaring_trait,
                                declaring_trait_name: crate::name::lower_name(&row.declaring_trait_name),
                                requirement: row.requirement,
                                requirement_name: crate::name::lower_name(&row.requirement_name),
                                realization_machine: row.realization_machine,
                                realization_state: row.realization_state,
                                realization_name: crate::name::lower_name(&row.realization_name),
                                source: match row.source {
                                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Inline => typed_trees::trait_definition::ConformanceRowSource::Inline,
                                    symbol_resolved_trees::trait_definition::ConformanceRowSource::Reference => typed_trees::trait_definition::ConformanceRowSource::Reference,
                                    symbol_resolved_trees::trait_definition::ConformanceRowSource::TraitDefault => typed_trees::trait_definition::ConformanceRowSource::TraitDefault,
                                },
                            })
                            .collect(),
                    }
                }
            },
        };
        for parameter in symbol_resolved_trees.data_type_parameters(source_type_parameters) {
            let parameter = lowerer
                .with_type_reference_exposure(conformance_exposure, |lowerer| {
                    crate::data::lower_type_parameter(lowerer, parameter)
                })?;
            lowerer
                .typed_trees
                .push_conformance_type_parameter(&mut conformance, parameter);
        }
        // Inline/default realization machines close over the conformance
        // name's telescope. Publish that telescope as the machine template's
        // own generic surface as well, so ordinary specialization can clone
        // and substitute the selected row instead of leaving family symbols
        // such as `Element` in executable checked code. Referenced external
        // machines keep their independently declared telescope.
        let realization_machines = match &conformance.implementation {
            typed_trees::trait_definition::ConformanceImplementation::Closed { rows } => rows
                .iter()
                .filter(|row| {
                    matches!(
                        row.source,
                        typed_trees::trait_definition::ConformanceRowSource::Inline
                            | typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                    )
                })
                .map(|row| row.realization_machine)
                .collect::<Vec<_>>(),
            typed_trees::trait_definition::ConformanceImplementation::AttachedRequirementMachines => {
                Vec::new()
            }
        };
        for machine in lowerer.typed_trees.machines_mut() {
            if realization_machines.contains(&machine.symbol) {
                machine.lifetime_parameters = conformance.lifetime_parameters.clone();
                machine.type_parameters = conformance.type_parameters;
            }
        }
        lowerer.typed_trees.push_conformance(conformance);
    }

    for wire_schema in &symbol_resolved_trees.wire_schemas {
        let wire_schema = lowerer.with_type_reference_exposure(
            declaration_exposure(wire_schema.is_public),
            |lowerer| crate::wire::lower_wire_schema(lowerer, wire_schema),
        )?;
        lowerer.typed_trees.push_wire_schema(wire_schema);
    }

    lowerer.finish()
}

fn declaration_exposure(
    is_public: bool,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;

    if is_public {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
}

fn machine_interface_exposure(
    machine: &symbol_resolved_trees::machine::Machine,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    let is_exported_boundary = matches!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::AdmissionClaim
    );
    declaration_exposure(machine.is_public || is_exported_boundary)
}

pub fn lower_symbol_resolved_trees_owned(
    symbol_resolved_trees: SymbolResolvedTrees,
) -> Result<TypedTrees, Diagnostic> {
    let mut typed_trees = lower_symbol_resolved_trees(&symbol_resolved_trees)?;
    typed_trees.symbols = symbol_resolved_trees.symbols;
    Ok(typed_trees)
}

pub(crate) struct Lowerer<'source> {
    pub(crate) typed_trees: TypedTrees,
    pub(crate) source_trees: &'source SymbolResolvedTrees,
    /// The value-typing scope of the state body currently being lowered;
    /// `==` expansion uses it to find an operand's data type.
    pub(crate) equality_scope: Option<crate::equatable::EqualityScope>,
    /// None lowers a compiler-derived type without inventing an authored
    /// occurrence. Original generic applications retain their own exposure.
    pub(crate) type_reference_exposure:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
}

impl Lowerer<'_> {
    pub(crate) fn with_type_reference_exposure<T>(
        &mut self,
        exposure: impl Into<
            Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
        >,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let previous = std::mem::replace(&mut self.type_reference_exposure, exposure.into());
        let result = operation(self);
        self.type_reference_exposure = previous;
        result
    }

    pub(crate) fn finish(mut self) -> Result<TypedTrees, Diagnostic> {
        self.typed_trees.symbols = self.source_trees.symbols.clone();
        crate::machine::settle_satisfied_declarations(&mut self.typed_trees)?;
        crate::progress::normalize_progress_premises(&mut self.typed_trees)?;
        let TypedTrees {
            roots,
            tables,
            symbols,
            service_reaches,
            service_reach_rows,
            authored_service_reach_rows,
            semantic_domains,
            external_bindings,
            plan_laid_layouts: _,
            placed_view_plans: _,
            wire_placements: _,
            wire_encode_obligations: _,
            wire_schema_plans: _,
            machine_specializations: _,
            boundary_calling_plans: _,
            fused_service_erasures: _,
            open_index_normalizations: _,
            evidence_forwardings,
            proof_output_calls,
            ranking_expression_custody,
        } = self.typed_trees;

        let mut trees = TypedTrees::with_roots(roots, tables, symbols);
        // The copied semantic interners survive the rebuild.
        trees.service_reaches = service_reaches;
        trees.service_reach_rows = service_reach_rows;
        trees.authored_service_reach_rows = authored_service_reach_rows;
        trees.semantic_domains = semantic_domains;
        trees.external_bindings = external_bindings;
        trees.evidence_forwardings = evidence_forwardings
            .into_iter()
            .map(|mut forwarding| {
                let erased_before = proof_output_calls
                    .iter()
                    .filter(|package| {
                        package.machine_symbol == forwarding.machine_symbol
                            && package.state_symbol == forwarding.state_symbol
                            && package.source_statement_index < forwarding.statement_index
                    })
                    .count();
                forwarding.statement_index =
                    forwarding.statement_index.saturating_sub(erased_before);
                forwarding
            })
            .collect();
        trees.proof_output_calls = proof_output_calls;
        trees.ranking_expression_custody = ranking_expression_custody;
        normalize_domain_constraints(self.source_trees, &mut trees)?;
        normalize_qualification_casts(&mut trees)?;
        crate::fixed_byte_array_literals::land_exact_fixed_byte_array_literals(&mut trees)?;
        Ok(trees)
    }
}

#[cfg(test)]
mod tests;
