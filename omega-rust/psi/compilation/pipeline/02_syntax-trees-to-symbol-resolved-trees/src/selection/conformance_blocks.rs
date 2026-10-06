//! Closed conformance blocks: exact requirement rows, and routing of the
//! requirement calls conformance member bodies make.
//!
//! Each closed block's rows resolve against the trait catalog, including
//! inherited defaults and same-named overloads. Calls an inline member makes
//! to inherited requirements select this block's rows, never an ambient
//! attached machine sharing the leaf name. Every trait default instantiation,
//! closed or bodyless, dispatches its authoring trait's requirements through
//! its own conformance's rows.

use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::signature::StateSignature;
use symbol_resolved_trees::state::State;
use symbol_resolved_trees::trait_definition::{
    ConformanceImplementation, ConformanceRow, ConformanceRowSource,
};
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use symbols::SymbolHandle;

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
        .flat_map(|conformance| match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => Vec::new(),
            ConformanceImplementation::Closed { rows } => rows
                .iter()
                .filter(|row| row.source == ConformanceRowSource::TraitDefault)
                .filter_map(|row| {
                    let ordinal = row.provisional_realization_ordinal?;
                    machine_catalog
                        .iter()
                        .find(|machine| machine.ordinal == ordinal)
                        .map(|machine| machine.symbol)
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let normalized = program
        .conformances
        .iter()
        .map(|conformance| match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => {
                Ok(ConformanceImplementation::AttachedRequirementMachines)
            }
            ConformanceImplementation::Closed { rows } => normalize_one(
                program,
                match &conformance.subject {
                    symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(
                        type_name,
                    ) => type_name.as_str(),
                    symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => {
                        conformance
                            .alias
                            .as_ref()
                            .map_or("<subjectless>", DiagnosticName::as_str)
                    }
                },
                conformance.trait_name.as_str(),
                conformance.trait_symbol,
                rows,
                &trait_catalog,
                &machine_catalog,
            )
            .map(|rows| ConformanceImplementation::Closed { rows }),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut normalized = normalized.into_iter();
    program.conformances.for_each_mut(|conformance| {
        conformance.implementation = normalized
            .next()
            .expect("one normalized implementation per conformance");
    });
    let selected_realizations = program
        .conformances
        .iter()
        .flat_map(|conformance| match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => Vec::new(),
            ConformanceImplementation::Closed { rows } => rows
                .iter()
                .map(|row| row.realization_machine)
                .filter(|symbol| symbol.is_valid())
                .collect(),
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
}

#[derive(Clone)]
struct ConformanceRoutingPlan {
    machine: SymbolHandle,
    routes: Vec<RequirementRoute>,
    subject_states: Vec<SymbolHandle>,
    /// Whether a bare name may select a row by spelling. Only an inline
    /// member is lexically inside the block whose rows it names; a default
    /// body's names were already bound to its authoring trait's requirements.
    routes_by_name: bool,
    /// Present when `machine` instantiates a trait default body.
    default_body: Option<DefaultBodyScope>,
}

/// What a default body may not name: the rows and carrier machines its
/// conformance supplies beyond the authoring trait's requirement surface.
#[derive(Clone)]
struct DefaultBodyScope {
    declaring_trait: SymbolHandle,
    /// The conformance subject this instantiation serves, for diagnostics.
    subject: String,
    hidden_names: Vec<DiagnosticName>,
}

impl DefaultBodyScope {
    fn new(
        declaring_trait: SymbolHandle,
        subject: String,
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
            subject,
            hidden_names,
        }
    }
}

/// A call through a named implementation (`Ascending::before(left, right)`)
/// selects that implementation's row for the requirement it names. The row's
/// realization is an ordinary checked machine, so the call becomes a direct
/// static call to it with its explicit arguments. A requirement name that
/// matches no row, or several inherited rows, leaves the call unresolved so
/// checking rejects it rather than choosing one.
pub(crate) fn route_named_implementation_calls(program: &mut SymbolResolvedTrees) {
    let implementation_rows = program
        .conformances
        .iter()
        .filter_map(|conformance| match &conformance.implementation {
            ConformanceImplementation::Closed { rows } => Some((
                conformance.symbol,
                rows.iter()
                    .map(|row| (row.requirement_name.clone(), row.realization_state))
                    .collect::<Vec<_>>(),
            )),
            ConformanceImplementation::AttachedRequirementMachines => None,
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
        super::body_calls::retarget_machine_calls(program, machine, &[], &mut |site| {
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
/// own conformance's rows. A closed block supplies its row realization; a
/// bodyless conformance's row is the carrier's attached machine named by the
/// requirement, the same row conformance checking validates. A default body
/// that names a row or carrier machine outside its trait's surface is
/// rejected here, where that conformance is known: the trait author cannot
/// see what a particular conformer adds. Existing-machine reference rows
/// remain ordinary machines: they have no enclosing conformance context and
/// may be shared by several blocks.
pub(crate) fn route_conformance_requirement_calls(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let trait_catalog = build_trait_catalog(program);
    let default_body_trait = |machine: SymbolHandle| {
        program
            .machines
            .iter()
            .find(|candidate| candidate.symbol == machine)
            .map_or_else(SymbolHandle::invalid, |candidate| {
                candidate.trait_default_origin.declaring_trait
            })
    };
    let mut plans = Vec::new();
    let mut closed_realizations = Vec::new();
    for conformance in program.conformances.iter() {
        let ConformanceImplementation::Closed { rows } = &conformance.implementation else {
            continue;
        };
        let routes = rows
            .iter()
            .filter(|row| row.realization_state.is_valid())
            .map(|row| RequirementRoute {
                requirement: row.requirement,
                requirement_name: row.requirement_name.clone(),
                realization_state: row.realization_state,
            })
            .collect::<Vec<_>>();
        let subject_states = match &conformance.subject {
            symbol_resolved_trees::trait_definition::ConformanceSubject::Carrier(type_name) => {
                program
                    .machines
                    .iter()
                    .filter(|machine| {
                        machine
                            .attached_data
                            .as_ref()
                            .is_some_and(|subject| subject.as_str() == type_name.as_str())
                    })
                    .flat_map(|machine| {
                        program
                            .machine_state_handles(machine.states)
                            .iter()
                            .map(|handle| program.machine_state(*handle).symbol)
                    })
                    .collect::<Vec<_>>()
            }
            symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless => Vec::new(),
        };
        for row in rows {
            if !matches!(
                row.source,
                ConformanceRowSource::Inline | ConformanceRowSource::TraitDefault
            ) || !row.realization_machine.is_valid()
            {
                continue;
            }
            closed_realizations.push(row.realization_machine);
            let declaring_trait = default_body_trait(row.realization_machine);
            plans.push(ConformanceRoutingPlan {
                machine: row.realization_machine,
                routes: routes.clone(),
                subject_states: subject_states.clone(),
                routes_by_name: row.source == ConformanceRowSource::Inline,
                default_body: declaring_trait.is_valid().then(|| {
                    DefaultBodyScope::new(
                        declaring_trait,
                        conformance
                            .carrier_name()
                            .or(conformance.alias.as_ref())
                            .map_or_else(String::new, |name| name.as_str().to_owned()),
                        &trait_catalog,
                        rows.iter().map(|row| row.requirement_name.clone()),
                    )
                }),
            });
        }
    }

    for machine in program.machines.iter() {
        let declaring_trait = machine.trait_default_origin.declaring_trait;
        if !declaring_trait.is_valid()
            || machine.target.is_some()
            || closed_realizations.contains(&machine.symbol)
        {
            continue;
        }
        let mut surface = Vec::new();
        collect_requirement_closure(
            declaring_trait,
            &trait_catalog,
            &mut Vec::new(),
            &mut surface,
        );
        let routes = surface
            .into_iter()
            .filter_map(|requirement| {
                Some(RequirementRoute {
                    realization_state: attached_requirement_realization(
                        program,
                        machine.attached_data_symbol,
                        &closed_realizations,
                        requirement.requirement_name.as_str(),
                    )?,
                    requirement: requirement.requirement,
                    requirement_name: requirement.requirement_name,
                })
            })
            .collect();
        plans.push(ConformanceRoutingPlan {
            machine: machine.symbol,
            routes,
            subject_states: Vec::new(),
            routes_by_name: false,
            default_body: Some(DefaultBodyScope::new(
                declaring_trait,
                machine
                    .attached_data
                    .as_ref()
                    .map_or_else(String::new, |carrier| carrier.as_str().to_owned()),
                &trait_catalog,
                ambient_carrier_entries(
                    program,
                    machine.attached_data_symbol,
                    &closed_realizations,
                )
                .map(|entry| entry.name.clone()),
            )),
        });
    }

    let mut hidden_calls = Vec::new();
    for plan in plans {
        crate::selection::body_calls::retarget_machine_calls(
            program,
            plan.machine,
            &plan.subject_states,
            &mut |site| {
                let routed = routed_target(
                    site.target_symbol,
                    site.target,
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
                        scope.subject.clone(),
                        site.target.clone(),
                    ));
                }
                routed
            },
        );
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
        .map(|(declaring_trait, subject, call)| {
            let trait_name = program.symbols.display_path(declaring_trait, "::");
            Diagnostic::error(format!(
                "default body in trait `{trait_name}` calls `{}`, which is not a requirement of `{trait_name}` or a trait it inherits; its instantiation for `{subject}` cannot name what that conformance adds",
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

/// Entry states of the machines ambiently attached to `carrier`. Closed-block
/// realizations belong to their own map and target siblings to their own
/// target, so neither is an ambient member of the carrier.
fn ambient_carrier_entries<'program>(
    program: &'program SymbolResolvedTrees,
    carrier: SymbolHandle,
    closed_realizations: &'program [SymbolHandle],
) -> impl Iterator<Item = &'program State> + 'program {
    program
        .machines
        .iter()
        .filter(move |machine| {
            carrier.is_valid()
                && machine.attached_data_symbol == carrier
                && machine.target.is_none()
                && !closed_realizations.contains(&machine.symbol)
        })
        .filter_map(|machine| {
            program
                .machine_state_handles(machine.states)
                .first()
                .map(|entry| program.machine_state(*entry))
        })
}

/// A bodyless conformance's row for one requirement: the carrier's ambient
/// machine whose entry state carries the requirement's name. Several
/// candidates leave the row unrouted for conformance checking to reject.
fn attached_requirement_realization(
    program: &SymbolResolvedTrees,
    carrier: SymbolHandle,
    closed_realizations: &[SymbolHandle],
    requirement_name: &str,
) -> Option<SymbolHandle> {
    let mut matching = ambient_carrier_entries(program, carrier, closed_realizations)
        .filter(|entry| entry.name.as_str() == requirement_name)
        .map(|entry| entry.symbol);
    let realization = matching.next()?;
    matching.next().is_none().then_some(realization)
}

fn routed_target(
    current: SymbolHandle,
    target_name: &DiagnosticName,
    routes_by_name: bool,
    routes: &[RequirementRoute],
) -> Option<SymbolHandle> {
    if current.is_valid()
        && let Some(route) = routes.iter().find(|route| route.requirement == current)
    {
        return Some(route.realization_state);
    }
    if !routes_by_name {
        return None;
    }
    let mut matching = routes
        .iter()
        .filter(|route| route.requirement_name.as_str() == target_name.as_str());
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
        let realizations = resolve_realizations(authored, machine_catalog);
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
            for (machine, state) in &realizations {
                if result_dispatch_shape(program, state.return_type.as_ref()) == requirement_shape {
                    matching.push((*candidate, *machine, *state));
                }
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
        TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::Generic(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::Named { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => {}
    }
}

fn collect_declared_result_dispatch(
    program: &SymbolResolvedTrees,
    symbol: SymbolHandle,
    name: &str,
    terms: &mut Vec<String>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    let Some(definition) = program
        .domain_definitions
        .iter()
        .find(|definition| definition.symbol == symbol)
    else {
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
                .domain_definitions
                .iter()
                .find(|candidate| candidate.symbol == constituent.domain_symbol)
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
    row: &ConformanceRow,
    catalog: &'catalog [MachineCatalogEntry],
) -> Vec<(&'catalog MachineCatalogEntry, &'catalog State)> {
    catalog
        .iter()
        .filter(|machine| {
            row.provisional_realization_ordinal
                .map_or(machine.name == row.realization_name, |ordinal| {
                    machine.ordinal == ordinal
                })
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
        .collect()
}
