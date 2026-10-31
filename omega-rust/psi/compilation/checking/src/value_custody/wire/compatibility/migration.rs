//! Find a uniquely bound migration route and check stable-identity retirement along it.

use super::demands::{EraSelection, name_leaf, select_era_path};
use super::schemas::qualified_schema_path;
use symbols::SymbolHandle;

/// Resolve an authored demand path to the exact ordinary data declaration it
/// names. Era identity is the declaration symbol: the same path rules as
/// schema dispatch apply, so a leaf must select one declaration unambiguously
/// rather than whichever era was declared first.
fn resolve_declared_era(typed: &dyn crate::CheckingInput, requested: &str) -> Option<SymbolHandle> {
    let candidates = typed
        .data_definitions()
        .iter()
        .map(|data| (typed.symbols.display_path(data.symbol, "::"), data.symbol))
        .collect::<Vec<_>>();
    match select_era_path(
        requested,
        candidates
            .iter()
            .map(|(path, symbol)| (path.as_str(), *symbol)),
    ) {
        EraSelection::Resolved(symbol) => Some(symbol),
        _ => None,
    }
}

/// The selected migration route: the bound machine names in peer-to-local
/// order for diagnostics, plus the era declarations the route traverses,
/// oldest first, so retirement obligations can be evaluated along the exact
/// chain the policy selected.
pub(super) struct MigrationRoute {
    pub(super) machines: Vec<String>,
    pub(super) eras: Vec<SymbolHandle>,
}

/// What the route search found between the selected eras. `Complete` carries
/// the uniquely bound checked conversion chain;
/// `Ambiguous` means a complete chain exists but every one traverses an edge
/// bound by more than one machine, so no unique conversion is certified;
/// `Missing` means no bound chain exists at all.
pub(super) enum MigrationRouteSearch {
    Complete(MigrationRoute),
    Ambiguous(String),
    Missing,
}

/// The checked migration route between two explicitly selected eras. Edges
/// are the machines bound to `FormatMigration<Lineage, Old, New>`; lineage,
/// old, and new are compared by declaration symbol, never by leaf name, so a
/// route bound on one module's declarations cannot satisfy another module's
/// demand even when the declarations are spelled identically. The certified
/// route may only traverse edges bound by exactly one machine: an edge bound
/// twice would certify whichever binding the search happened to reach first,
/// so such a route is reported ambiguous rather than selected.
pub(super) fn migration_route(
    typed: &dyn crate::CheckingInput,
    lineage: &str,
    peer: &str,
    local: &str,
) -> MigrationRouteSearch {
    let (Some(local_symbol), Some(peer_symbol), Some(lineage_symbol)) = (
        resolve_declared_era(typed, local),
        resolve_declared_era(typed, peer),
        resolve_declared_era(typed, lineage),
    ) else {
        return MigrationRouteSearch::Missing;
    };
    if peer_symbol == local_symbol {
        return MigrationRouteSearch::Complete(MigrationRoute {
            machines: Vec::new(),
            eras: vec![local_symbol],
        });
    }
    let mut edges = Vec::new();
    for machine in typed.machines() {
        for conformance in typed.machine_trait_conformances(machine) {
            if name_leaf(conformance.name.as_str()) != "FormatMigration"
                || conformance.requirement.as_ref().map(|name| name.as_str()) != Some("migrate")
            {
                continue;
            }
            let arguments = typed
                .type_reference_table
                .type_reference_handles(conformance.arguments);
            if arguments.len() != 3 {
                continue;
            }
            let symbol = |index: usize| typed.type_reference_table.type_symbol(arguments[index]);
            if symbol(0) != lineage_symbol {
                continue;
            }
            let (old, new) = (symbol(1), symbol(2));
            if !old.is_valid() || !new.is_valid() {
                continue;
            }
            edges.push((old, new, machine.name.as_str().to_owned()));
        }
    }

    let uniquely_bound = |(old, new): &(SymbolHandle, SymbolHandle)| {
        edges
            .iter()
            .filter(|edge| (edge.0, edge.1) == (*old, *new))
            .count()
            == 1
    };
    let certified_edges = edges
        .iter()
        .filter(|(old, new, _)| uniquely_bound(&(*old, *new)))
        .cloned()
        .collect::<Vec<_>>();
    if let Some(route) = search_route(&certified_edges, peer_symbol, local_symbol) {
        return MigrationRouteSearch::Complete(route);
    }
    if let Some(route) = search_route(&edges, peer_symbol, local_symbol) {
        let hops = route
            .eras
            .windows(2)
            .filter_map(|hop| {
                let machines = edges
                    .iter()
                    .filter(|edge| (edge.0, edge.1) == (hop[0], hop[1]))
                    .map(|edge| format!("`{}`", edge.2))
                    .collect::<Vec<_>>();
                (machines.len() > 1).then(|| {
                    format!(
                        "edge `{}` -> `{}` is bound by {}",
                        typed.symbols.display_path(hop[0], "::"),
                        typed.symbols.display_path(hop[1], "::"),
                        machines.join(", ")
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("; ");
        return MigrationRouteSearch::Ambiguous(format!(
            "no uniquely bound `{lineage}` migration route exists from `{peer}` to `{local}`: \
             {hops}; bind exactly one machine to each checked conversion edge"
        ));
    }
    MigrationRouteSearch::Missing
}

/// Depth-first search for a bound edge chain from `peer` to `local` over the
/// given edges, returning the machines in peer-to-local order and the era
/// symbols the chain traverses, oldest first.
pub(super) fn search_route(
    edges: &[(SymbolHandle, SymbolHandle, String)],
    peer: SymbolHandle,
    local: SymbolHandle,
) -> Option<MigrationRoute> {
    let mut frontier = vec![(peer, Vec::<String>::new(), vec![peer])];
    let mut visited = vec![peer];
    while let Some((current, machines, eras)) = frontier.pop() {
        for (old, new, machine) in edges {
            if *old != current {
                continue;
            }
            let mut next_machines = machines.clone();
            next_machines.push(machine.clone());
            let mut next_eras = eras.clone();
            next_eras.push(*new);
            if *new == local {
                return Some(MigrationRoute {
                    machines: next_machines,
                    eras: next_eras,
                });
            }
            if !visited.contains(new) {
                visited.push(*new);
                frontier.push((*new, next_machines, next_eras));
            }
        }
    }
    None
}

/// One numbering scope inside a published era, matching the parser's
/// declaration rules: record fields number independently of sum cases (mixed
/// data keeps both spaces and cannot tombstone), and each numbered case owns
/// a payload scope keyed by the case's stable identity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IdentityScope {
    Fields,
    Cases,
    Payload(u64),
}

/// One era's live and retired stable identities inside one numbering scope.
struct NumberScope {
    key: IdentityScope,
    live: Vec<u64>,
    retired: Vec<u64>,
}

/// The numbered scopes one era publishes: a field scope, a case scope, and a
/// payload scope per numbered case. Every era emits both member scopes even
/// when one stays empty, so a shape change on the route still observes the
/// dropped identities as retirements. `retired #N;` joins the field scope
/// when the era declares no cases and the case scope when it declares no
/// fields -- the parser's own attribution, which covers both scopes for a
/// tombstone-only era. Returns `None` when the route's era is not an ordinary
/// data declaration -- such an era declares no stable identities, so it can
/// neither retire nor reuse one.
fn era_number_scopes(
    typed: &dyn crate::CheckingInput,
    era: SymbolHandle,
) -> Option<(String, Vec<NumberScope>)> {
    let definition = typed.data_definition_by_symbol(era)?;
    let members = typed.data_members(definition);
    let has_fields = members
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Field(_)));
    let has_cases = members
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)));
    let mut scopes = vec![
        NumberScope {
            key: IdentityScope::Fields,
            live: Vec::new(),
            retired: if has_cases {
                Vec::new()
            } else {
                definition.retired_identities.clone()
            },
        },
        NumberScope {
            key: IdentityScope::Cases,
            live: Vec::new(),
            retired: if has_fields {
                Vec::new()
            } else {
                definition.retired_identities.clone()
            },
        },
    ];
    for member in members {
        match member {
            typed_trees::data::DataMember::Field(field) => {
                if let Some(identity) = field.identity {
                    scopes[0].live.push(identity);
                }
            }
            typed_trees::data::DataMember::Variant(variant) => {
                if let Some(identity) = variant.identity {
                    scopes[1].live.push(identity);
                    scopes.push(NumberScope {
                        key: IdentityScope::Payload(identity),
                        live: typed
                            .data_payload_fields(variant)
                            .iter()
                            .filter_map(|field| field.identity)
                            .collect(),
                        retired: variant.retired_payload_identities.clone(),
                    });
                }
            }
        }
    }
    Some((
        qualified_schema_path(typed, era, definition.name.as_str()),
        scopes,
    ))
}

/// Retirement joins the selected route: an era retires a stable identity by
/// tombstoning it (`retired #N;`) or by dropping it from the published shape
/// the previous era on the route carried. Either way the identity is dead to
/// the lineage from that era on -- `FormatMigration` adds no era
/// discriminator to the payload, so a later era redeclaring the number would
/// decode stored old-era bytes under the new meaning. Returns the first reuse
/// found along the chain, or `None` when every era's identities are fresh.
pub(super) fn retired_identity_reuse(
    typed: &dyn crate::CheckingInput,
    eras: &[SymbolHandle],
) -> Option<String> {
    let mut retired: Vec<(IdentityScope, u64, String)> = Vec::new();
    let mut previous_live: Vec<(IdentityScope, Vec<u64>)> = Vec::new();
    for era in eras {
        let Some((era_name, scopes)) = era_number_scopes(typed, *era) else {
            continue;
        };
        for scope in &scopes {
            for identity in &scope.live {
                if let Some((_, _, retiring)) = retired
                    .iter()
                    .find(|(key, candidate, _)| *key == scope.key && candidate == identity)
                {
                    return Some(match scope.key {
                        IdentityScope::Fields => format!(
                            "era `{era_name}` redeclares stable identity #{identity} retired in era `{retiring}`"
                        ),
                        IdentityScope::Cases => format!(
                            "era `{era_name}` redeclares stable case identity #{identity} retired in era `{retiring}`"
                        ),
                        IdentityScope::Payload(case) => format!(
                            "era `{era_name}` redeclares stable payload identity #{identity} of case #{case} retired in era `{retiring}`"
                        ),
                    });
                }
            }
        }
        for scope in &scopes {
            for identity in &scope.retired {
                if !retired
                    .iter()
                    .any(|(key, candidate, _)| *key == scope.key && candidate == identity)
                {
                    retired.push((scope.key, *identity, era_name.clone()));
                }
            }
            if let Some((_, dropped)) = previous_live.iter().find(|(key, _)| *key == scope.key) {
                for identity in dropped {
                    if !scope.live.contains(identity)
                        && !retired
                            .iter()
                            .any(|(key, candidate, _)| *key == scope.key && candidate == identity)
                    {
                        retired.push((scope.key, *identity, era_name.clone()));
                    }
                }
            }
        }
        for scope in scopes {
            match previous_live.iter_mut().find(|(key, _)| *key == scope.key) {
                Some((_, live)) => *live = scope.live,
                None => previous_live.push((scope.key, scope.live)),
            }
        }
    }
    None
}
