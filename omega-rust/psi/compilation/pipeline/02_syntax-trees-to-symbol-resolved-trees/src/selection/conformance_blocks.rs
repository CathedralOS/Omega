//! Closed conformance blocks: exact requirement rows, and routing of the
//! requirement calls conformance member bodies make.
//!
//! Each closed block's rows resolve against the trait catalog, including
//! inherited defaults and same-named overloads. Calls an inline member makes
//! to inherited requirements select this block's rows, never an ambient
//! attached machine sharing the leaf name. Every trait default instantiation
//! dispatches its authoring trait's requirements through its own
//! implementation's rows.

use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::signature::StateSignature;
use symbol_resolved_trees::state::State;
use symbol_resolved_trees::trait_definition::{ConformanceRow, ConformanceRowSource};
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use symbols::{SymbolHandle, SymbolKind, SymbolLookup};

#[derive(Clone)]
struct TraitCatalogEntry {
    symbol: SymbolHandle,
    parents: Vec<SymbolHandle>,
    requirements: Vec<RequirementCatalogEntry>,
}

#[derive(Clone)]
struct RequirementCatalogEntry {
    ordinal: usize,
    declaring_trait: SymbolHandle,
    declaring_trait_name: DiagnosticName,
    requirement: SymbolHandle,
    requirement_name: DiagnosticName,
    is_default: bool,
    signature: StateSignature,
}

#[derive(Clone)]
struct MachineCatalogEntry {
    ordinal: usize,
    symbol: SymbolHandle,
    name: DiagnosticName,
    states: Vec<State>,
}

/// Returns each machine's new ordinal (`None` for a dropped synthesized
/// default that no row selected), so ordinal-keyed sidecars such as deferred
/// machine bodies stay joined to the same declaration.
pub(crate) fn normalize_closed_conformance_blocks(
    program: &mut SymbolResolvedTrees,
) -> Result<Vec<Option<usize>>, Diagnostic> {
    let trait_catalog = build_trait_catalog(program);
    let machine_catalog = build_machine_catalog(program);
    let synthesized_default_candidates = program
        .conformances
        .iter()
        .flat_map(|conformance| {
            conformance
                .rows
                .iter()
                .filter(|row| row.source == ConformanceRowSource::TraitDefault)
                .filter_map(|row| {
                    let ordinal = row.provisional_realization_ordinal?;
                    machine_catalog
                        .iter()
                        .find(|machine| machine.ordinal == ordinal)
                        .map(|machine| machine.symbol)
                })
        })
        .collect::<Vec<_>>();
    let normalized = program
        .conformances
        .iter()
        .map(|conformance| {
            normalize_one(
                program,
                conformance
                    .alias
                    .as_ref()
                    .map_or("<unnamed implementation>", DiagnosticName::as_str),
                conformance.trait_name.as_str(),
                conformance.trait_symbol,
                &conformance.rows,
                &trait_catalog,
                &machine_catalog,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut normalized = normalized.into_iter();
    program.conformances.for_each_mut(|conformance| {
        conformance.rows = normalized
            .next()
            .expect("one normalized row map per conformance");
    });
    let selected_realizations = program
        .conformances
        .iter()
        .flat_map(|conformance| {
            conformance
                .rows
                .iter()
                .map(|row| row.realization_machine)
                .filter(|symbol| symbol.is_valid())
        })
        .collect::<Vec<_>>();
    let mut ordinals = Vec::with_capacity(program.machines.len());
    let mut retained_machines = Vec::with_capacity(program.machines.len());
    for machine in program.machines.iter() {
        if !synthesized_default_candidates.contains(&machine.symbol)
            || selected_realizations.contains(&machine.symbol)
        {
            ordinals.push(Some(retained_machines.len()));
            retained_machines.push(machine.clone());
        } else {
            ordinals.push(None);
        }
    }
    program.machines = Default::default();
    for machine in retained_machines {
        program.machines.push(machine);
    }
    Ok(ordinals)
}

#[derive(Clone)]
struct RequirementRoute {
    requirement: SymbolHandle,
    requirement_name: DiagnosticName,
    realization_state: SymbolHandle,
    /// The requirement's authored formals. A call passing one operand more
    /// is a selector call (`Trait::member(selector, data...)`), which
    /// dispatches at runtime instead of naming this block's row.
    parameter_count: usize,
}

#[derive(Clone)]
struct ConformanceRoutingPlan {
    machine: SymbolHandle,
    routes: Vec<RequirementRoute>,
    /// Whether a bare name may select a row by spelling. Only an inline
    /// member is lexically inside the block whose rows it names; a default
    /// body's names were already bound to its authoring trait's requirements.
    routes_by_name: bool,
    /// Present when `machine` instantiates a trait default body.
    default_body: Option<DefaultBodyScope>,
}

/// What a default body may not name: the rows its implementation supplies
/// beyond the authoring trait's requirement surface.
#[derive(Clone)]
struct DefaultBodyScope {
    declaring_trait: SymbolHandle,
    /// The named implementation this instantiation serves, for diagnostics.
    implementation: String,
    hidden_names: Vec<DiagnosticName>,
}

impl DefaultBodyScope {
    fn new(
        declaring_trait: SymbolHandle,
        implementation: String,
        trait_catalog: &[TraitCatalogEntry],
        conformance_names: impl IntoIterator<Item = DiagnosticName>,
    ) -> Self {
        let mut surface = Vec::new();
        collect_requirement_closure(
            declaring_trait,
            trait_catalog,
            &mut Vec::new(),
            &mut surface,
        );
        let mut hidden_names = Vec::<DiagnosticName>::new();
        for name in conformance_names {
            if !surface
                .iter()
                .any(|requirement| requirement.requirement_name.as_str() == name.as_str())
                && !hidden_names
                    .iter()
                    .any(|hidden| hidden.as_str() == name.as_str())
            {
                hidden_names.push(name);
            }
        }
        Self {
            declaring_trait,
            implementation,
            hidden_names,
        }
    }
}

/// A call through a named implementation (`Ascending::before(left, right)`)
/// selects that implementation's row for the requirement it names. The row's
/// public requirement remains the call target until typed preparation closes
/// and retains the selected map. Replacing it with the private realization
/// here would erase that source selection before its contract can be checked.
/// A requirement name that
/// matches no row, or several inherited rows, leaves the call unresolved so
/// checking rejects it rather than choosing one.
pub(crate) fn route_named_implementation_calls(program: &mut SymbolResolvedTrees) {
    let implementation_rows = program
        .conformances
        .iter()
        .map(|conformance| {
            (
                conformance.symbol,
                conformance
                    .rows
                    .iter()
                    .map(|row| (row.requirement_name.clone(), row.requirement))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    if implementation_rows.is_empty() {
        return;
    }
    let machines = program
        .machines
        .iter()
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    for machine in machines {
        super::body_calls::retarget_machine_calls(program, machine, &mut |site| {
            if !site.receiver_symbol.is_valid() {
                return None;
            }
            let (_, rows) = implementation_rows
                .iter()
                .find(|(conformance, _)| *conformance == site.receiver_symbol)?;
            let mut matches = rows
                .iter()
                .filter(|(requirement, _)| requirement.as_str() == site.target.as_str());
            let selected = matches.next().map(|(_, state)| *state);
            if matches.next().is_some() {
                return Some(SymbolHandle::invalid());
            }
            Some(selected.unwrap_or_else(SymbolHandle::invalid))
        });
    }
}

/// Calls a conformance's own member bodies make to its requirements select
/// that conformance's exact rows, never an ambient attached machine that
/// happens to share the leaf name.
///
/// An inline member is lexically part of one closed block, so its bare names
/// select the block's rows by spelling. A trait default body is lexically part
/// of its authoring trait instead: symbol assignment already bound its bare
/// names to that trait's exact requirements (`LexicalRequirements` in the
/// symbols scope), so every instantiation re-points those requirements to its
/// own implementation's rows. A default body that names a row outside its
/// trait's surface is rejected here, where that implementation is known: the
/// trait author cannot see what a particular implementation adds.
/// Existing-machine reference rows remain ordinary machines: they have no
/// enclosing conformance context and may be shared by several blocks.
pub(crate) fn route_conformance_requirement_calls(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let trait_catalog = build_trait_catalog(program);
    let default_body_trait = |machine: SymbolHandle| {
        program
            .machine(machine)
            .map_or_else(SymbolHandle::invalid, |candidate| {
                candidate.trait_default_origin.declaring_trait
            })
    };
    let mut plans = Vec::new();
    let mut closed_realizations = Vec::new();
    for conformance in program.conformances.iter() {
        let rows = &conformance.rows;
        let parameter_count = |requirement: SymbolHandle| {
            trait_catalog
                .iter()
                .flat_map(|entry| &entry.requirements)
                .find(|entry| entry.requirement == requirement)
                .map_or(0, |entry| entry.signature.parameters.len())
        };
        let routes = rows
            .iter()
            .filter(|row| row.realization_state.is_valid())
            .map(|row| RequirementRoute {
                requirement: row.requirement,
                requirement_name: row.requirement_name.clone(),
                realization_state: row.realization_state,
                parameter_count: parameter_count(row.requirement),
            })
            .collect::<Vec<_>>();
        for row in rows {
            if !row.source.owns_realization() || !row.realization_machine.is_valid() {
                continue;
            }
            closed_realizations.push(row.realization_machine);
            let declaring_trait = default_body_trait(row.realization_machine);
            plans.push(ConformanceRoutingPlan {
                machine: row.realization_machine,
                routes: routes.clone(),
                routes_by_name: row.source == ConformanceRowSource::Inline,
                default_body: declaring_trait.is_valid().then(|| {
                    DefaultBodyScope::new(
                        declaring_trait,
                        conformance
                            .alias
                            .as_ref()
                            .map_or_else(String::new, |name| name.as_str().to_owned()),
                        &trait_catalog,
                        rows.iter().map(|row| row.requirement_name.clone()),
                    )
                }),
            });
        }
    }

    let mut hidden_calls = Vec::new();
    for plan in plans {
        crate::selection::body_calls::retarget_machine_calls(program, plan.machine, &mut |site| {
            let routed = routed_target(
                site.target_symbol,
                site.target,
                site.argument_count,
                plan.routes_by_name && site.local_receiver,
                &plan.routes,
            );
            if routed.is_none()
                && !site.target_symbol.is_valid()
                && site.local_receiver
                && let Some(scope) = &plan.default_body
                && scope
                    .hidden_names
                    .iter()
                    .any(|hidden| hidden.as_str() == site.target.as_str())
            {
                hidden_calls.push((
                    scope.declaring_trait,
                    scope.implementation.clone(),
                    site.target.clone(),
                ));
            }
            routed
        });
    }
    let diagnostics = hidden_calls
        .into_iter()
        .filter(|(_, _, call)| {
            // A visible declaration of the same spelling may still be
            // selected by typed arity selection; only a name nothing else
            // can supply is a default body reaching outside its trait.
            program
                .symbols
                .find_top_level_declaration_or_operator_family_from_source(
                    call.as_str(),
                    &[
                        symbols::SymbolKind::Machine,
                        symbols::SymbolKind::Operator,
                        symbols::SymbolKind::BuiltinFunction,
                    ],
                    call.source_span(),
                )
                .is_none()
        })
        .map(|(declaring_trait, implementation, call)| {
            let trait_name = program.symbols.display_path(declaring_trait, "::");
            Diagnostic::error(format!(
                "default body in trait `{trait_name}` calls `{}`, which is not a requirement of `{trait_name}` or a trait it inherits; its instantiation for `{implementation}` cannot name what that conformance adds",
                call.as_str()
            ))
            .with_source_span(call.source_span())
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn routed_target(
    current: SymbolHandle,
    target_name: &DiagnosticName,
    argument_count: usize,
    routes_by_name: bool,
    routes: &[RequirementRoute],
) -> Option<SymbolHandle> {
    // A selector call passes its selector before the requirement's operands;
    // it reaches whichever implementation the selector names, not this row.
    let static_routes = || {
        routes
            .iter()
            .filter(move |route| argument_count != route.parameter_count + 1)
    };
    if current.is_valid()
        && let Some(route) = static_routes().find(|route| route.requirement == current)
    {
        return Some(route.realization_state);
    }
    if !routes_by_name {
        return None;
    }
    let mut matching =
        static_routes().filter(|route| route.requirement_name.as_str() == target_name.as_str());
    let route = matching.next()?;
    matching.next().is_none().then_some(route.realization_state)
}

fn build_trait_catalog(program: &SymbolResolvedTrees) -> Vec<TraitCatalogEntry> {
    program
        .traits
        .iter()
        .map(|trait_definition| TraitCatalogEntry {
            symbol: trait_definition.symbol,
            parents: program
                .trait_requirements(trait_definition.requires)
                .iter()
                .map(|parent| parent.symbol)
                .collect(),
            requirements: program
                .trait_machine_signatures(trait_definition.machines)
                .iter()
                .enumerate()
                .map(|(ordinal, requirement)| RequirementCatalogEntry {
                    ordinal,
                    declaring_trait: trait_definition.symbol,
                    declaring_trait_name: trait_definition.name.clone(),
                    requirement: requirement.symbol,
                    requirement_name: requirement.name.clone(),
                    is_default: requirement.is_default,
                    signature: requirement.clone(),
                })
                .collect(),
        })
        .collect()
}

fn build_machine_catalog(program: &SymbolResolvedTrees) -> Vec<MachineCatalogEntry> {
    program
        .machines
        .iter()
        .enumerate()
        // A target sibling never realizes a conformance of this realization.
        .filter(|(_, machine)| machine.target.is_none())
        .map(|(ordinal, machine)| MachineCatalogEntry {
            ordinal,
            symbol: machine.symbol,
            name: machine.name.clone(),
            states: program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| program.machine_state(*handle).clone())
                .collect(),
        })
        .collect()
}

fn normalize_one(
    program: &SymbolResolvedTrees,
    subject_name: &str,
    trait_name: &str,
    trait_symbol: SymbolHandle,
    authored_rows: &[ConformanceRow],
    trait_catalog: &[TraitCatalogEntry],
    machine_catalog: &[MachineCatalogEntry],
) -> Result<Vec<ConformanceRow>, Diagnostic> {
    let root = trait_catalog
        .iter()
        .find(|entry| entry.symbol == trait_symbol)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` names unknown trait `{trait_name}`"
            ))
        })?;
    let mut requirements = Vec::new();
    collect_requirement_closure(
        root.symbol,
        trait_catalog,
        &mut Vec::new(),
        &mut requirements,
    );

    let mut normalized = Vec::new();
    // Authored rows own their exact slots. Synthesized defaults are fallback
    // candidates and normalize only after every authored row, independent of
    // incidental source-table order.
    for authored in authored_rows
        .iter()
        .filter(|row| row.source != ConformanceRowSource::TraitDefault)
        .chain(
            authored_rows
                .iter()
                .filter(|row| row.source == ConformanceRowSource::TraitDefault),
        )
    {
        let candidates = requirements
            .iter()
            .filter(|instance| {
                let requirement = *instance;
                // A declaring-trait qualifier may spell the declaration's
                // exact logical path (synthesized rows always do) as well as
                // the authored leaf; the leaf match stays for compatibility
                // but the path match is what distinguishes same-spelled
                // module owners.
                requirement.requirement_name == authored.requirement_name
                    && (authored.declaring_trait_name.as_str().is_empty()
                        || requirement.declaring_trait_name == authored.declaring_trait_name
                        || program
                            .symbols
                            .display_path(requirement.declaring_trait, "::")
                            == authored.declaring_trait_name.as_str())
                    && authored
                        .provisional_requirement_ordinal
                        .is_none_or(|ordinal| requirement.ordinal == ordinal)
            })
            .collect::<Vec<_>>();
        let realizations = resolve_realizations(program, authored, machine_catalog, None);
        let mut matching = Vec::new();
        for candidate in &candidates {
            // Legal same-path overload families share one normalized
            // parameter signature and differ by dispatch-bearing result
            // domains. The typed conformance validator remains authoritative
            // for the complete parameter/contract compatibility check; this
            // pre-typed phase uses the result set only to retain the exact
            // declaration symbol rather than collapsing to the leaf name.
            let requirement_shape =
                result_dispatch_shape(program, candidate.signature.return_type.as_ref());
            for (machine, state) in
                resolve_realizations(program, authored, machine_catalog, Some(&requirement_shape))
            {
                matching.push((*candidate, machine, state));
            }
        }
        let (requirement, machine, state) = match (
            candidates.as_slice(),
            realizations.as_slice(),
            matching.as_slice(),
        ) {
            ([], _, _) => {
                let qualified = if authored.declaring_trait_name.as_str().is_empty() {
                    authored.requirement_name.as_str().to_owned()
                } else {
                    format!(
                        "{}::{}",
                        authored.declaring_trait_name, authored.requirement_name
                    )
                };
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` has no inherited requirement slot `{qualified}`"
                )));
            }
            // A sole declaration is checked against the fully applied trait
            // later. Its raw result spelling may be a generic parameter whose
            // domain is supplied by this conformance's arguments.
            ([requirement], [(machine, state)], _) => (*requirement, *machine, *state),
            (_, _, [(requirement, machine, state)]) => (*requirement, *machine, *state),
            _ => {
                let inherited_collision = authored.declaring_trait_name.as_str().is_empty()
                    && candidates.first().is_some_and(|first| {
                        candidates
                            .iter()
                            .skip(1)
                            .any(|candidate| candidate.declaring_trait != first.declaring_trait)
                    });
                if inherited_collision {
                    return Err(Diagnostic::error(format!(
                        "closed conformance `{subject_name} satisfies {trait_name}` member `{}` is ambiguous across inherited traits; use `DeclaringTrait::{}`",
                        authored.requirement_name, authored.requirement_name
                    )));
                }
                return Err(Diagnostic::error(format!(
                    "closed conformance `{subject_name} satisfies {trait_name}` member `{}` does not identify one exact inherited overload; qualify the declaring trait and match the complete parameter/result-domain signature",
                    authored.requirement_name
                )));
            }
        };

        if normalized.iter().any(|row: &ConformanceRow| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            if authored.source == ConformanceRowSource::TraitDefault {
                continue;
            }
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` fills `{}::{}` more than once",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }

        let mut row = authored.clone();
        row.declaring_trait = requirement.declaring_trait;
        row.declaring_trait_name = requirement.declaring_trait_name.clone();
        row.requirement = requirement.requirement;
        row.provisional_requirement_ordinal = None;
        row.realization_machine = machine.symbol;
        row.realization_state = state.symbol;
        row.provisional_realization_ordinal = None;
        normalized.push(row);
    }

    for requirement in requirements {
        if normalized.iter().any(|row| {
            row.declaring_trait == requirement.declaring_trait
                && row.requirement == requirement.requirement
        }) {
            continue;
        }
        if !requirement.is_default {
            return Err(Diagnostic::error(format!(
                "closed conformance `{subject_name} satisfies {trait_name}` is incomplete: missing `{}::{}`",
                requirement.declaring_trait_name, requirement.requirement_name
            )));
        }
        normalized.push(ConformanceRow {
            declaring_trait: requirement.declaring_trait,
            declaring_trait_name: requirement.declaring_trait_name.clone(),
            requirement: requirement.requirement,
            requirement_name: requirement.requirement_name.clone(),
            provisional_requirement_ordinal: None,
            realization_machine: SymbolHandle::invalid(),
            realization_state: SymbolHandle::invalid(),
            realization_name: DiagnosticName::generated(format!(
                "{}::{}#default",
                requirement.declaring_trait_name, requirement.requirement_name
            )),
            authored_realization_source_span: None,
            provisional_realization_ordinal: None,
            source: ConformanceRowSource::TraitDefault,
        });
    }

    normalized.sort_by_key(|row| {
        (
            row.declaring_trait.arena_index(),
            row.requirement.arena_index(),
        )
    });
    Ok(normalized)
}

fn collect_requirement_closure(
    trait_symbol: SymbolHandle,
    catalog: &[TraitCatalogEntry],
    visited: &mut Vec<SymbolHandle>,
    output: &mut Vec<RequirementCatalogEntry>,
) {
    if !trait_symbol.is_valid() || visited.contains(&trait_symbol) {
        return;
    }
    visited.push(trait_symbol);
    let Some(entry) = catalog.iter().find(|entry| entry.symbol == trait_symbol) else {
        return;
    };
    for requirement in &entry.requirements {
        if !output.iter().any(|existing| {
            existing.declaring_trait == requirement.declaring_trait
                && existing.requirement == requirement.requirement
        }) {
            output.push(requirement.clone());
        }
    }
    for parent in &entry.parents {
        collect_requirement_closure(*parent, catalog, visited, output);
    }
}

fn result_dispatch_shape(
    program: &SymbolResolvedTrees,
    return_type: Option<&TypeReference>,
) -> String {
    let mut result_dispatch = Vec::new();
    if let Some(return_type) = return_type {
        collect_result_dispatch(program, return_type, &mut result_dispatch, &mut Vec::new());
    }
    result_dispatch.sort();
    result_dispatch.dedup();
    result_dispatch.join("&")
}

fn collect_result_dispatch(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    terms: &mut Vec<String>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    match type_reference {
        TypeReference::Reference(reference) => collect_result_dispatch(
            program,
            program.child_type_reference(reference.referee),
            terms,
            alias_stack,
        ),
        TypeReference::Constrained(constrained) => {
            collect_result_dispatch(
                program,
                program.child_type_reference(constrained.base_type),
                terms,
                alias_stack,
            );
            for constraint in program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                match constraint {
                    TypeConstraint::ArithmeticDomain(domain) => {
                        terms.push(format!("arithmetic:{}", domain.name()));
                    }
                    TypeConstraint::Domain(domain) => {
                        let symbol = program
                            .symbols
                            .find_top_level_by_name_and_kinds_from_source(
                                domain.name.as_str(),
                                &[symbols::SymbolKind::Domain],
                                domain.name.source_span(),
                            )
                            .unwrap_or_else(SymbolHandle::invalid);
                        collect_declared_result_dispatch(
                            program,
                            symbol,
                            domain.name.as_str(),
                            terms,
                            alias_stack,
                        );
                    }
                    TypeConstraint::Named(_) | TypeConstraint::Range { .. } => {}
                }
            }
        }
        super::leaf_carrier_type_reference_pattern!() => {}
    }
}

fn collect_declared_result_dispatch(
    program: &SymbolResolvedTrees,
    symbol: SymbolHandle,
    name: &str,
    terms: &mut Vec<String>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    let Some(definition) = program.domain_definition_by_symbol(symbol) else {
        terms.push(format!("declared:{name}"));
        return;
    };
    if alias_stack.contains(&definition.symbol) {
        return;
    }
    if let Some(alias) = definition.alias.as_ref() {
        alias_stack.push(definition.symbol);
        for constituent in &alias.constituents {
            let constituent_name = program
                .domain_definition_by_symbol(constituent.domain_symbol)
                .map(|candidate| candidate.name.as_str())
                .unwrap_or("<unresolved-domain>");
            collect_declared_result_dispatch(
                program,
                constituent.domain_symbol,
                constituent_name,
                terms,
                alias_stack,
            );
        }
        alias_stack.pop();
        return;
    }
    if definition.predicate_body.is_present()
        && definition.semantic_roles.is_empty()
        && definition.establishment_routes.is_empty()
    {
        return;
    }
    terms.push(format!("declared:{}", definition.name));
}

fn resolve_realizations<'catalog>(
    program: &SymbolResolvedTrees,
    row: &ConformanceRow,
    catalog: &'catalog [MachineCatalogEntry],
    requirement_shape: Option<&str>,
) -> Vec<(&'catalog MachineCatalogEntry, &'catalog State)> {
    let mut candidates = catalog
        .iter()
        .filter(|machine| {
            row.provisional_realization_ordinal
                .is_none_or(|ordinal| machine.ordinal == ordinal)
        })
        .filter_map(|machine| {
            let leaf = machine
                .name
                .as_str()
                .rsplit_once("::")
                .map_or(machine.name.as_str(), |(_, leaf)| leaf);
            machine
                .states
                .iter()
                .find(|state| state.name.as_str() == leaf)
                .or_else(|| {
                    machine
                        .states
                        .iter()
                        .find(|state| state.name.as_str() == "entry")
                })
                .map(|state| (machine, state))
        })
        .filter(|(_, state)| {
            requirement_shape.is_none_or(|shape| {
                result_dispatch_shape(program, state.return_type.as_ref()) == shape
            })
        })
        .collect::<Vec<_>>();
    if row.provisional_realization_ordinal.is_some() {
        return candidates;
    }
    if let Some(reference) = row.authored_realization_source_span {
        // A referenced machine belongs to the row's declaration-site namespace,
        // not to a global pool of diagnostic spellings. Apply ordinary source
        // lookup after overload eligibility, retaining ambiguity rather than
        // choosing whichever package happened to be loaded first.
        let selected = program
            .symbols
            .lookup_signature_free_top_level_from_source_matching(
                row.realization_name.as_str(),
                &[SymbolKind::Machine],
                reference,
                |symbol| {
                    candidates
                        .iter()
                        .any(|(machine, _)| machine.symbol == symbol)
                },
            );
        candidates.retain(|(machine, _)| match selected {
            SymbolLookup::Unique(symbol) => machine.symbol == symbol,
            SymbolLookup::Ambiguous { first, second } => {
                machine.symbol == first || machine.symbol == second
            }
            SymbolLookup::NotFound => false,
        });
    } else {
        candidates.retain(|(machine, _)| machine.name == row.realization_name);
    }
    candidates
}
