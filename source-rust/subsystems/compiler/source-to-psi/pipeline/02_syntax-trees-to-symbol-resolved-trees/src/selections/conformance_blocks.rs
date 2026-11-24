/*
    Closed conformance blocks: exact requirement rows, and routing of the
    requirement calls conformance member bodies make.

    Each closed block's rows resolve against the trait catalog, including
    inherited defaults and same-named overloads. Calls an inline member makes
    to inherited requirements select this block's rows, never an ambient
    attached machine sharing the leaf name. Every trait default instantiation
    dispatches its authoring trait's requirements through its own
    implementation's rows.
*/

use crate::declaration_selection::ResolvedAuthoredCallBinding;
use crate::resolution::selections::body_calls::retarget_machine_calls;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::expression::StaticMachineArgument;
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::signature::StateSignature;
use crate::source_resolved::state::State;
use crate::source_resolved::trait_definition::{Conformance, ConformanceRow, ConformanceRowSource};
use crate::source_resolved::types::{TypeConstraint, TypeReference};
use crate::source_symbols::SymbolLookup;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};

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

/*
    Returns each machine's new ordinal (`None` for a dropped synthesized
    default that no row selected), so ordinal-keyed sidecars such as deferred
    machine bodies stay joined to the same declaration.
*/
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
    let discarded_roots: std::collections::HashSet<_> = synthesized_default_candidates
        .into_iter()
        .filter(|symbol| !selected_realizations.contains(symbol))
        .collect();
    let discarded: std::collections::HashSet<_> = program
        .machines
        .iter()
        .filter(|machine| discarded_roots.contains(&machine.lexical_root(&program.machines).symbol))
        .map(|machine| machine.symbol)
        .collect();
    let mut ordinals = Vec::with_capacity(program.machines.len());
    let mut retained_count = 0;
    program.machines.retain(|machine| {
        let retain = !discarded.contains(&machine.symbol);
        ordinals.push(retain.then_some(retained_count));
        retained_count += usize::from(retain);
        retain
    });
    for declaration in discarded {
        program.symbols.clear_declaration_owner(declaration);
    }
    for machine in program.machines.iter() {
        if machine.lexical_parent.is_valid() && !program.machines.contains(machine.lexical_parent) {
            return Err(Diagnostic::error(
                "retained nested machine lost its enclosing declaration",
            ));
        }
    }
    Ok(ordinals)
}

#[derive(Clone)]
struct RequirementRoute {
    requirement: SymbolHandle,
    realization_state: SymbolHandle,
    /*
        The requirement's authored formals. A call passing one operand more
        is a selector call (`Trait::member(selector, data...)`), which
        dispatches at runtime instead of naming this block's row.
    */
    parameter_count: usize,
}

#[derive(Clone)]
struct ConformanceRoutingPlan {
    machine: SymbolHandle,
    routes: Vec<RequirementRoute>,
    /* Present when `machine` instantiates a trait default body. */
    default_body: Option<DefaultBodyScope>,
}

/*
    What a default body may not name: the rows its implementation supplies
    beyond the authoring trait's requirement surface.
*/
#[derive(Clone)]
struct DefaultBodyScope {
    declaring_trait: SymbolHandle,
    /* The named implementation this instantiation serves, for diagnostics. */
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

/*
    A call through a named implementation (`Ascending::before(left, right)`)
    selects that implementation's row for the requirement it names. The row's
    public requirement remains the call target until typed preparation closes
    and retains the selected map. Replacing it with the private realization
    here would erase that source selection before its contract can be checked.
    A requirement name that matches no row, or several inherited rows, leaves
    the call unresolved so checking rejects it rather than choosing one.
    Constant initializers bind that same public requirement during constant
    finalization, before their authored selection is recorded.
*/
pub(crate) fn route_named_implementation_calls(
    program: &mut SymbolResolvedTrees,
    work: &[SymbolHandle],
) {
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
    let mut select_requirement = |site: &super::body_calls::CallSite<'_>| {
        named_implementation_requirement(&implementation_rows, site)
    };
    super::body_calls::retarget_signature_contract_calls(program, &mut select_requirement);
    let machines = program
        .machines
        .iter()
        .map(|machine| machine.symbol)
        .collect::<Vec<_>>();
    for machine in machines {
        super::body_calls::retarget_machine_calls(
            program,
            machine,
            work.contains(&machine),
            &mut select_requirement,
        );
    }
}

/*
    A named value selects the same public member as a call through that
    implementation. Resolve it before recording its authored path selections;
    the separate typed application still owns the complete public signature.
    Restrict work to this resolution's actual expression frontier.
*/
pub(crate) fn bind_named_implementation_values(
    program: &mut SymbolResolvedTrees,
    work: &[crate::source_resolved::expression::ExpressionHandle],
) -> Result<(), Diagnostic> {
    use crate::source_resolved::expression::ExpressionNode;
    use crate::source_symbols::JoinedPath;

    let mut routing = ProvisionalBodyCallRouting::new(program);
    let mut selected = Vec::new();
    for &expression in work {
        let table = &program.tables.bodies.expressions;
        let ExpressionNode::Name(path) = table.expression(expression) else {
            continue;
        };
        if path.static_application.is_valid() || !path.head_symbol.is_valid() {
            continue;
        }
        let at = table.source_span(expression);
        let refuse = |reason: &str| Diagnostic::error(reason).with_source_span(at);
        let nodes = program.symbols.symbols().nodes();
        if !nodes.is_valid(path.head_symbol) {
            return Err(refuse("named implementation value has a stale declaration"));
        }
        let root_kind = program.symbols.get(path.head_symbol).kind;
        if path.symbol.is_valid()
            || !matches!(root_kind, SymbolKind::Module | SymbolKind::Conformance)
        {
            continue;
        }
        let names = table.name_path_members(path.members);
        if names.is_empty() || names.len() != path.members.count() as usize {
            return Err(refuse("named implementation value has a stale member path"));
        }
        let Some((member, namespace_names)) =
            names.split_last().filter(|(_, prefix)| !prefix.is_empty())
        else {
            // A bare namespace remains an ordinary static operand or receiver.
            continue;
        };
        let namespace_source = source_text::SourceSpan::new(
            namespace_names[0].source_span().source_id,
            source_text::Span::new(
                namespace_names[0].source_span().span.start,
                namespace_names
                    .last()
                    .expect("nonempty namespace")
                    .source_span()
                    .span
                    .end,
            ),
        );
        let namespace = match program
            .symbols
            .lookup_top_level_by_name_and_kinds_from_source_matching(
                &namespace_names.joined_path(),
                &[SymbolKind::Conformance],
                namespace_source,
                |_| true,
            ) {
            SymbolLookup::Unique(namespace) => namespace,
            SymbolLookup::Ambiguous { .. } => {
                return Err(refuse(
                    "named implementation value has an ambiguous namespace",
                ));
            }
            SymbolLookup::NotFound if root_kind == SymbolKind::Module => continue,
            SymbolLookup::NotFound => {
                return Err(refuse(
                    "named implementation value lost its selected namespace",
                ));
            }
        };
        // The canonical source lookup owns visibility. The retained root
        // still owns this path: a same-spelled map under another module or
        // behind a value binding cannot replace it.
        let mut namespace_path = Vec::new();
        let mut current = namespace;
        for _ in namespace_names {
            if !nodes.is_valid(current) {
                return Err(refuse(
                    "named implementation namespace has a stale declaration",
                ));
            }
            namespace_path.push(current);
            if current == path.head_symbol {
                break;
            }
            current = program.symbols.namespace_parent(current);
        }
        if namespace_path.last() != Some(&path.head_symbol) {
            return Err(refuse(
                "named implementation value changed its selected root",
            ));
        }
        namespace_path.reverse();
        let namespace_start = namespace_names.len() - namespace_path.len();
        let binding = routing
            .named_member_binding(namespace, None, member)
            .map_err(|reason| refuse(&reason))?;
        if !nodes.is_valid(binding.original) {
            return Err(refuse(
                "named implementation value differs from its exact requirement row",
            ));
        }
        let member_symbols = table.name_path_member_symbols(path.member_symbols);
        if member_symbols.len() != names.len()
            || member_symbols
                .iter()
                .any(|symbol| symbol.is_valid() && !nodes.is_valid(*symbol))
            || member_symbols[..namespace_start]
                .iter()
                .any(|symbol| symbol.is_valid())
            || member_symbols[namespace_start..namespace_names.len()]
                .iter()
                .zip(&namespace_path)
                .any(|(retained, selected)| retained.is_valid() && retained != selected)
            || member_symbols
                .last()
                .is_some_and(|symbol| symbol.is_valid())
        {
            return Err(refuse(
                "named implementation value has stale or mismatched path selections",
            ));
        }
        namespace_path.push(binding.original);
        selected.push((
            expression,
            path.member_symbols,
            namespace_start,
            namespace_path,
        ));
    }
    for (expression, members, start, namespace_path) in selected {
        let table = &mut program.tables.bodies.expressions;
        for (position, &declaration) in namespace_path.iter().enumerate() {
            let offset = u32::try_from(start + position)
                .map_err(|_| Diagnostic::error("named implementation path offset overflow"))?;
            table.set_name_path_member_symbol_at_offset(members, offset, declaration);
        }
        let requirement = *namespace_path.last().expect("selected public member");
        let ExpressionNode::Name(path) = table.expression_mut(expression) else {
            unreachable!("selected named value changed expression kind");
        };
        path.symbol = requirement;
    }
    Ok(())
}

fn named_implementation_requirement(
    implementation_rows: &[(SymbolHandle, Vec<(DiagnosticName, SymbolHandle)>)],
    site: &super::body_calls::CallSite<'_>,
) -> Option<SymbolHandle> {
    if !site.receiver_symbol.is_valid() {
        return None;
    }
    let (_, rows) = implementation_rows
        .iter()
        .find(|(conformance, _)| *conformance == site.receiver_symbol)?;
    Some(
        named_requirement(
            rows.iter().map(|(name, requirement)| (name, *requirement)),
            site.member.as_str(),
        )
        .unwrap_or_else(|_| SymbolHandle::invalid()),
    )
}

fn named_requirement<'name>(
    rows: impl Iterator<Item = (&'name DiagnosticName, SymbolHandle)>,
    member: &str,
) -> Result<SymbolHandle, &'static str> {
    let mut matches = rows.filter(|(name, _)| name.as_str() == member);
    let selected = matches
        .next()
        .map(|(_, requirement)| requirement)
        .ok_or("has no matching requirement row")?;
    if matches.next().is_some() {
        return Err("has more than one matching requirement row");
    }
    Ok(selected)
}

/*
    Calls a conformance's own member bodies make to its requirements select
    that conformance's exact rows, never an ambient attached machine that
    happens to share the leaf name.

    An inline member sees the implemented trait. A default sees the trait
    where it was declared. Name resolution selects a requirement or a nearer
    lexical declaration before this pass. Only requirement symbols can select
    a row. A default that names a row outside its trait's surface is rejected
    here, where that implementation is known: the
    trait author cannot see what a particular implementation adds.
    A machine named by a selection row remains that ordinary machine: it has no
    enclosing conformance context and may be shared by several blocks.
*/
pub(crate) fn route_conformance_requirement_calls(
    program: &mut SymbolResolvedTrees,
    work: &[SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    let trait_catalog = build_trait_catalog(program);
    let plans = program
        .conformances
        .iter()
        .flat_map(|conformance| {
            conformance_routing_plans(program, conformance, &conformance.rows, &trait_catalog)
        })
        .map(|plan| (plan.machine, plan))
        .collect::<std::collections::HashMap<_, _>>();
    let bodies = program
        .machines
        .iter()
        .filter_map(|machine| {
            let owner = machine.lexical_root(&program.machines).symbol;
            Some((machine.symbol, plans.get(&owner)?))
        })
        .collect::<Vec<_>>();

    let mut hidden_calls = Vec::new();
    for (machine, plan) in bodies {
        retarget_machine_calls(
            program,
            machine,
            work.contains(&machine),
            &mut |site| {
                let routed = plan.binding(site).map(|binding| binding.selected);
                if routed.is_none()
                    && !site.requirement_application.is_valid()
                    && !site.target_symbol.is_valid()
                    && site.local_receiver
                    && let Some(scope) = &plan.default_body
                    && scope
                        .hidden_names
                        .iter()
                        .any(|hidden| hidden.as_str() == site.member.as_str())
                {
                    hidden_calls.push((
                        scope.declaring_trait,
                        scope.implementation.clone(),
                        site.member.clone(),
                    ));
                }
                routed
            },
        );
    }
    let diagnostics = hidden_calls
    .into_iter()
    .filter(|(_, _, call)| {
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

impl ConformanceRoutingPlan {
    fn binding(
        &self,
        site: &super::body_calls::CallSite<'_>,
    ) -> Option<ResolvedAuthoredCallBinding> {
        if site.requirement_application.is_valid() {
            return None;
        }
        let route = self.routes.iter().find(|route| {
            route.requirement == site.target_symbol
                && site.argument_count != route.parameter_count + 1
        })?;
        Some(ResolvedAuthoredCallBinding {
            selected: route.realization_state,
            original: route.requirement,
        })
    }
}

fn conformance_routing_plans(
    program: &SymbolResolvedTrees,
    conformance: &Conformance,
    rows: &[ConformanceRow],
    trait_catalog: &[TraitCatalogEntry],
) -> Vec<ConformanceRoutingPlan> {
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
            realization_state: row.realization_state,
            parameter_count: parameter_count(row.requirement),
        })
        .collect::<Vec<_>>();
    rows.iter()
        .filter(|row| row.source.owns_realization() && row.realization_machine.is_valid())
        .map(|row| {
            let declaring_trait = program
                .machine(row.realization_machine)
                .map_or_else(SymbolHandle::invalid, |machine| {
                    machine.trait_default_origin.declaring_trait
                });
            ConformanceRoutingPlan {
                machine: row.realization_machine,
                routes: routes.clone(),
                default_body: declaring_trait.is_valid().then(|| {
                    DefaultBodyScope::new(
                        declaring_trait,
                        conformance
                            .alias
                            .as_ref()
                            .map_or_else(String::new, |name| name.as_str().to_owned()),
                        trait_catalog,
                        rows.iter().map(|row| row.requirement_name.clone()),
                    )
                }),
            }
        })
        .collect()
}

/*
    Source dependencies can need a member body before executable call routing.
    Prepare the same declaration-owned plans against one immutable program.
    Only reached conformances are normalized; unrelated unfinished rows do not
    become dependencies. The caller checks retained typed selections separately.
*/
pub(crate) struct ProvisionalBodyCallRouting<'program> {
    program: &'program SymbolResolvedTrees,
    catalogs: Option<(Vec<TraitCatalogEntry>, Vec<MachineCatalogEntry>)>,
    owner_namespaces: Vec<(SymbolHandle, SymbolHandle)>,
    conformances: Vec<PreparedConformanceRouting>,
}

struct PreparedConformanceRouting {
    namespace: SymbolHandle,
    rows: Vec<ConformanceRow>,
    plans: Vec<ConformanceRoutingPlan>,
}

impl<'program> ProvisionalBodyCallRouting<'program> {
    pub(crate) fn new(program: &'program SymbolResolvedTrees) -> Self {
        Self {
            program,
            catalogs: None,
            owner_namespaces: Vec::new(),
            conformances: Vec::new(),
        }
    }

    pub(crate) fn binding(
        &mut self,
        machine: SymbolHandle,
        site: &super::body_calls::CallSite<'_>,
    ) -> Result<Option<ResolvedAuthoredCallBinding>, String> {
        let machine = self.program.machine(machine).map_or(machine, |body| {
            body.lexical_root(&self.program.machines).symbol
        });
        let namespace = if let Some((_, namespace)) = self
            .owner_namespaces
            .iter()
            .find(|(owner, _)| *owner == machine)
        {
            *namespace
        } else {
            let namespace =
                crate::resolution::names::owned_conformance_namespace(self.program, machine)?;
            self.owner_namespaces.push((machine, namespace));
            namespace
        };
        if !namespace.is_valid() {
            return Ok(None);
        }
        let conformance = self.prepared_conformance(namespace)?;
        let mut selected = conformance
            .plans
            .iter()
            .filter(|plan| plan.machine == machine);
        let plan = selected
            .next()
            .ok_or("source call has no owned implementation body")?;
        if selected.next().is_some() {
            return Err("source call has ambiguous owned implementation bodies".into());
        }
        Ok(plan.binding(site))
    }

    /*
        A named map selects a public requirement, while its exact row supplies
        the separate body demand. Keep that source identity before and after
        executable routing, including calls in constant initializers.
    */
    pub(crate) fn named_binding(
        &mut self,
        site: &super::body_calls::CallSite<'_>,
        static_arguments: &[StaticMachineArgument],
    ) -> Result<Option<ResolvedAuthoredCallBinding>, String> {
        let program = self.program;
        let target = program.symbols.get(site.target_symbol);
        let parent_kind = program.symbols.get(target.parent).kind;
        let (implementation, explicit_requirement) =
            if program.symbols.get(site.receiver_symbol).kind == SymbolKind::Conformance {
                (site.receiver_symbol, None)
            } else if target.kind == SymbolKind::State && parent_kind == SymbolKind::Trait {
                let Some(argument) = static_arguments.last() else {
                    return Ok(None);
                };
                if program.symbols.get(argument.symbol).kind != SymbolKind::Conformance {
                    return Ok(None);
                }
                (argument.symbol, Some(site.target_symbol))
            } else {
                return Ok(None);
            };
        self.named_member_binding(implementation, explicit_requirement, site.member)
            .map(Some)
    }

    fn named_member_binding(
        &mut self,
        implementation: SymbolHandle,
        explicit_requirement: Option<SymbolHandle>,
        member: &DiagnosticName,
    ) -> Result<ResolvedAuthoredCallBinding, String> {
        let program = self.program;
        let rows = &self.prepared_conformance(implementation)?.rows;
        let requirement = if let Some(requirement) = explicit_requirement {
            requirement
        } else {
            named_requirement(
                rows.iter()
                    .map(|row| (&row.requirement_name, row.requirement)),
                member.as_str(),
            )
            .map_err(|reason| {
                format!(
                    "named implementation `{}` {reason} for `{}`",
                    program.symbols.display_path(implementation, "::"),
                    member,
                )
            })?
        };
        let mut matches = rows.iter().filter(|row| row.requirement == requirement);
        let selected = matches
            .next()
            .map(|row| row.realization_state)
            .filter(|state| state.is_valid())
            .unwrap_or(requirement);
        if matches.next().is_some() {
            return Err("named implementation call has ambiguous exact requirement rows".into());
        }
        Ok(ResolvedAuthoredCallBinding {
            selected,
            original: requirement,
        })
    }

    fn prepared_conformance(
        &mut self,
        namespace: SymbolHandle,
    ) -> Result<&PreparedConformanceRouting, String> {
        if !self
            .conformances
            .iter()
            .any(|entry| entry.namespace == namespace)
        {
            self.prepare_namespace(namespace)?;
        }
        self.conformances
            .iter()
            .find(|entry| entry.namespace == namespace)
            .ok_or_else(|| "source call has no prepared implementation declaration".into())
    }

    fn prepare_namespace(&mut self, namespace: SymbolHandle) -> Result<(), String> {
        let mut conformances = self
            .program
            .conformances
            .iter()
            .filter(|candidate| candidate.symbol == namespace);
        let conformance = conformances
            .next()
            .ok_or("source call has no owning implementation declaration")?;
        if conformances.next().is_some() {
            return Err("source call has ambiguous owning implementation declarations".into());
        }
        let program = self.program;
        let (trait_catalog, machine_catalog) = self
            .catalogs
            .get_or_insert_with(|| (build_trait_catalog(program), build_machine_catalog(program)));
        let rows = normalize_one(
            self.program,
            conformance
                .alias
                .as_ref()
                .map_or("<unnamed implementation>", DiagnosticName::as_str),
            conformance.trait_name.as_str(),
            conformance.trait_symbol,
            &conformance.rows,
            trait_catalog,
            machine_catalog,
        )
        .map_err(|diagnostic| diagnostic.message)?;
        let plans = conformance_routing_plans(self.program, conformance, &rows, trait_catalog);
        self.conformances.push(PreparedConformanceRouting {
            namespace,
            rows,
            plans,
        });
        Ok(())
    }
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
    for authored in authored_rows
        .iter()
        .filter(|row| row.source != ConformanceRowSource::TraitDefault)
        .chain(
            authored_rows
                .iter()
                .filter(|row| row.source == ConformanceRowSource::TraitDefault),
        )
    {
        let inherited_qualifiers = (authored.source == ConformanceRowSource::Inline).then(|| {
            let member = authored
                .provisional_realization_ordinal
                .filter(|ordinal| *ordinal < program.machines.len())
                .map(|ordinal| &program.machines[ordinal])
                .or_else(|| {
                    program
                        .machines
                        .iter()
                        .find(|machine| machine.symbol == authored.realization_machine)
                });
            let qualifiers = member
                .into_iter()
                .flat_map(|machine| program.machine_trait_conformances(machine.satisfies))
                .filter(|edge| edge.requirement.as_ref() == Some(&authored.requirement_name))
                .map(|edge| edge.symbol)
                .collect::<Vec<_>>();
            let inherited = qualifiers
                .iter()
                .copied()
                .filter(|qualifier| {
                    requirements.iter().any(|requirement| {
                        requirement.declaring_trait == *qualifier
                            && requirement.requirement_name == authored.requirement_name
                    })
                })
                .collect::<Vec<_>>();
            // A member's own `satisfies Trait::name` names the slot
            // it fills. When every such qualifier names a trait
            // outside this implementation's requirement closure, the
            // member fills no slot here; an empty list would instead
            // read as unqualified and let an equal name substitute a
            // different requirement owner.
            if inherited.is_empty() {
                qualifiers
            } else {
                inherited
            }
        });
        let candidates = requirements
            .iter()
            .filter(|instance| {
                let requirement = *instance;
                requirement.requirement_name == authored.requirement_name
                    && inherited_qualifiers.as_ref().map_or_else(
                        || {
                            if authored.source == ConformanceRowSource::Reference {
                                return requirement.declaring_trait == authored.declaring_trait;
                            }
                            authored.declaring_trait_name.as_str().is_empty()
                                || requirement.declaring_trait_name == authored.declaring_trait_name
                                || program
                                    .symbols
                                    .display_path(requirement.declaring_trait, "::")
                                    == authored.declaring_trait_name.as_str()
                        },
                        |qualifiers| {
                            qualifiers.is_empty()
                                || qualifiers.contains(&requirement.declaring_trait)
                        },
                    )
                    && authored
                        .provisional_requirement_ordinal
                        .is_none_or(|ordinal| requirement.ordinal == ordinal)
            })
            .collect::<Vec<_>>();
        let realizations = resolve_realizations(program, authored, machine_catalog, None);
        let mut matching = Vec::new();
        for candidate in &candidates {
            let requirement_shape =
                result_dispatch_shape(program, candidate.signature.return_type.as_ref());
            for (machine, state) in
                resolve_realizations(program, authored, machine_catalog, Some(&requirement_shape))
            {
                matching.push((*candidate, machine, state));
            }
        }
        // Two overloads can share result domains while accepting different
        // types. If several rows remain, compare their declared input and
        // result types to identify one slot; later checking still judges
        // the selected member's complete refinement obligations.
        if matching.len() > 1 {
            let mut complete = Vec::with_capacity(matching.len());
            for (requirement, machine, state) in matching {
                let expected = program.state_parameters(requirement.signature.parameters);
                let supplied = program.state_parameters(state.parameters);
                if expected.len() != supplied.len()
                    || expected.iter().zip(supplied).any(|(left, right)| {
                        left.is_const != right.is_const || left.relevance != right.relevance
                    })
                {
                    continue;
                }
                let expected = expected
                    .iter()
                    .map(|parameter| parameter.type_reference.clone())
                    .chain(std::iter::once(
                        requirement
                            .signature
                            .return_type
                            .clone()
                            .unwrap_or(TypeReference::Unit),
                    ))
                    .collect();
                let supplied = supplied
                    .iter()
                    .map(|parameter| parameter.type_reference.clone())
                    .chain(std::iter::once(
                        state.return_type.clone().unwrap_or(TypeReference::Unit),
                    ))
                    .collect();
                if super::callable_inference::same_type_values(
                    program,
                    expected,
                    supplied,
                    authored.requirement_name.source_span(),
                )? {
                    complete.push((requirement, machine, state));
                }
            }
            matching = complete;
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
        TypeReference::AliasUse { expansion, .. } => {
            collect_result_dispatch(program, expansion, terms, alias_stack);
        }
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
                    TypeConstraint::Named(_) => {}
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
    if definition.predicate_presence.is_present()
        && definition.semantic_roles.is_empty()
        && definition.establishment_routes.is_empty()
        && definition.access_policy.is_none()
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
            (!row.realization_machine.is_valid() || machine.symbol == row.realization_machine)
                && row
                    .provisional_realization_ordinal
                    .is_none_or(|ordinal| machine.ordinal == ordinal)
        })
        .filter_map(|machine| {
            if row.realization_state.is_valid() {
                return machine
                    .states
                    .iter()
                    .find(|state| state.symbol == row.realization_state)
                    .map(|state| (machine, state));
            }
            let leaf = machine
                .name
                .as_str()
                .rsplit_once("::")
                .map_or(machine.name.as_str(), |(_, leaf)| leaf);
            crate::resolution::names::select_machine_entry(leaf, |name| {
                machine
                    .states
                    .iter()
                    .find(|state| state.name.as_str() == name)
            })
            .map(|state| (machine, state))
        })
        .filter(|(_, state)| {
            requirement_shape.is_none_or(|shape| {
                result_dispatch_shape(program, state.return_type.as_ref()) == shape
            })
        })
        .collect::<Vec<_>>();
    if row.realization_machine.is_valid() || row.provisional_realization_ordinal.is_some() {
        return candidates;
    }
    if let Some(reference) = row.authored_realization_source_span {
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
