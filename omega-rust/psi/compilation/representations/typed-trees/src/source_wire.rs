//! Wire declaration formation and compatibility use immutable typed schema
//! identities. They issue no executable body verdict or flow facts.

pub(crate) mod compatibility {
    use diagnostics::Diagnostic;

    pub(crate) mod demands {
        use super::migration::{MigrationRouteSearch, migration_route, retired_identity_reuse};
        use super::schemas::{SchemaRow, qualified_schema_path};
        use super::{WireCompatibilityDemand, WireDemandVerdict, WireFact, WireField, WireShape};

        pub(crate) fn compatibility_demand_verdict(
            typed: &crate::TypedTrees,
            rows: &[SchemaRow],
            demand: &WireCompatibilityDemand,
        ) -> WireDemandVerdict {
            let local = select_era_path(
                &demand.local_schema,
                rows.iter().map(|row| (row.qualified_path.as_str(), row)),
            );
            let peer = select_era_path(
                &demand.peer_schema,
                rows.iter().map(|row| (row.qualified_path.as_str(), row)),
            );
            let local_row = era_resolved(&local).copied();
            let peer_row = era_resolved(&peer).copied();
            let local_schema = local_row.map(|row| &row.entry);
            let peer_schema = peer_row.map(|row| &row.entry);
            let codec = local_schema
                .and_then(|schema| schema.encoding.as_deref())
                .or_else(|| peer_schema.and_then(|schema| schema.encoding.as_deref()))
                .unwrap_or("compact_binary")
                .to_owned();
            let compact_binary = codec == "compact_binary";

            let readability_value = local_schema
                .zip(peer_schema)
                .is_some_and(|(reader, writer)| schema_accepts(reader, writer));
            let writability_value = local_schema
                .zip(peer_schema)
                .is_some_and(|(writer, reader)| schema_accepts(reader, writer));
            let readable_detail = match (local_schema, peer_schema) {
                (Some(_), Some(_)) if readability_value => {
                    "the local decoder accepts every peer shape".to_owned()
                }
                (Some(_), Some(_)) => {
                    "the strict local decoder does not accept every peer shape".to_owned()
                }
                _ => schema_selection_detail(&local, &peer, demand),
            };
            let writable_detail = match (local_schema, peer_schema) {
                (Some(_), Some(_)) if writability_value => {
                    "the peer decoder accepts every local shape".to_owned()
                }
                (Some(_), Some(_)) => {
                    "the strict peer decoder does not accept every local shape".to_owned()
                }
                _ => schema_selection_detail(&local, &peer, demand),
            };

            let migration_route = migration_route(
                typed,
                &demand.lineage,
                &demand.peer_schema,
                &demand.local_schema,
            );
            let identity_reuse = match &migration_route {
                MigrationRouteSearch::Complete(route) => retired_identity_reuse(typed, &route.eras),
                _ => None,
            };
            let migration_value = local_schema.is_some()
                && peer_schema.is_some()
                && matches!(migration_route, MigrationRouteSearch::Complete(_))
                && identity_reuse.is_none();
            let migration_detail = match (local_schema, peer_schema, &migration_route) {
                (None, _, _) | (_, None, _) => schema_selection_detail(&local, &peer, demand),
                (Some(_), Some(_), MigrationRouteSearch::Missing) => {
                    format!(
                        "no complete `{}` migration route exists from `{}` to `{}`",
                        demand.lineage, demand.peer_schema, demand.local_schema
                    )
                }
                (Some(_), Some(_), MigrationRouteSearch::Ambiguous(detail)) => detail.clone(),
                (Some(_), Some(_), MigrationRouteSearch::Complete(route)) => {
                    if let Some(reuse) = identity_reuse {
                        format!("the selected route is not a sound migration: {reuse}")
                    } else if route.machines.is_empty() {
                        "peer and local schemas are identical; no migration edge is needed"
                            .to_owned()
                    } else {
                        format!("selected checked route: {}", route.machines.join(" -> "))
                    }
                }
            };

            let readability = fact(demand.require_readable, readability_value, readable_detail);
            let writability = fact(demand.require_writable, writability_value, writable_detail);
            let preserving_decode = local_row
                .and_then(|row| row.value_symbol)
                .and_then(|symbol| published_preserving_decode(typed, symbol));
            let unknown_preservation = fact(
                demand.require_unknown_preservation,
                preserving_decode.is_some(),
                if let Some(detail) = preserving_decode {
                    detail
                } else if compact_binary {
                    "compact_binary publishes strict unknown-member behavior".to_owned()
                } else {
                    format!("codec `{codec}` publishes no preserving behavior")
                },
            );
            let canonical_value = compact_binary && local_schema.is_some() && peer_schema.is_some();
            let canonical_detail = if local_schema.is_none() || peer_schema.is_none() {
                schema_selection_detail(&local, &peer, demand)
            } else if compact_binary {
                "compact_binary emits its canonical field order and scalar encodings".to_owned()
            } else {
                format!("codec `{codec}` publishes no canonicalization guarantee")
            };
            let canonicality = fact(demand.require_canonical, canonical_value, canonical_detail);
            let migration_coverage = fact(
                demand.require_complete_migration,
                migration_value,
                migration_detail,
            );
            let satisfied = [
                &readability,
                &writability,
                &unknown_preservation,
                &canonicality,
                &migration_coverage,
            ]
            .into_iter()
            .all(|fact| !fact.required || fact.satisfied);

            WireDemandVerdict {
                edge: demand.edge.clone(),
                local_schema: demand.local_schema.clone(),
                peer_schema: demand.peer_schema.clone(),
                readability,
                writability,
                unknown_preservation,
                canonicality,
                migration_coverage,
                satisfied,
            }
        }

        fn published_preserving_decode(
            typed: &crate::TypedTrees,
            value_symbol: symbols::SymbolHandle,
        ) -> Option<String> {
            typed
                .machines()
                .iter()
                .flat_map(|machine| {
        typed
            .machine_trait_conformances(machine)
            .iter()
            .map(move |conformance| (machine, conformance))
                })
                .filter(|(machine, conformance)| {
        let Some(crate::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        }) = crate::machine::resolve_satisfied_declaration(
            typed, machine, conformance,
        )
        else {
            return false;
        };
        if definition.name.as_str() != "PreservingDecode"
            || requirement.name.as_str() != "decode_preserving"
        {
            return false;
        }
        typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .get(1)
            .is_some_and(|value| {
                typed.type_reference_table.type_symbol(*value) == value_symbol
            })
                })
                .map(|(machine, _)| {
        format!(
            "codec publishes preserving decode: `{}` satisfies `PreservingDecode::decode_preserving` for the local schema's value type",
            qualified_schema_path(typed, machine.symbol, machine.name.as_str())
        )
                })
                .next()
        }

        fn fact(required: bool, satisfied: bool, detail: String) -> WireFact {
            WireFact {
                required,
                satisfied,
                detail,
            }
        }

        pub(crate) enum EraSelection<T> {
            Resolved(T),
            Ambiguous(Vec<String>),
            Missing,
        }

        fn era_resolved<T>(selection: &EraSelection<T>) -> Option<&T> {
            match selection {
                EraSelection::Resolved(item) => Some(item),
                _ => None,
            }
        }

        pub(crate) fn select_era_path<'a, T>(
            requested: &str,
            candidates: impl Iterator<Item = (&'a str, T)>,
        ) -> EraSelection<T> {
            let nested_suffix = format!("::{requested}");
            let mut exact = Vec::new();
            let mut nested = Vec::new();
            for (path, item) in candidates {
                if path == requested {
                    exact.push((path, item));
                } else if path.ends_with(&nested_suffix) {
                    nested.push((path, item));
                }
            }
            let (paths, items): (Vec<&str>, Vec<T>) = if exact.is_empty() {
                nested.into_iter().unzip()
            } else {
                exact.into_iter().unzip()
            };
            match items.len() {
                0 => EraSelection::Missing,
                1 => EraSelection::Resolved(items.into_iter().next().expect("one item")),
                _ => EraSelection::Ambiguous(
                    paths
                        .into_iter()
                        .map(str::to_owned)
                        .collect::<Vec<String>>(),
                ),
            }
        }

        fn schema_selection_detail<T>(
            local: &EraSelection<T>,
            peer: &EraSelection<T>,
            demand: &WireCompatibilityDemand,
        ) -> String {
            if matches!(local, EraSelection::Missing) && matches!(peer, EraSelection::Missing) {
                return format!(
                    "neither local schema `{}` nor peer schema `{}` is published",
                    demand.local_schema, demand.peer_schema
                );
            }
            [
                ("local", demand.local_schema.as_str(), local),
                ("peer", demand.peer_schema.as_str(), peer),
            ]
            .into_iter()
            .filter_map(|(role, authored, selection)| match selection {
                EraSelection::Resolved(_) => None,
                EraSelection::Missing => {
        Some(format!("{role} schema `{authored}` is not published"))
                }
                EraSelection::Ambiguous(candidates) => Some(format!(
        "{role} schema `{authored}` is ambiguous across published eras {}; qualify the era path in the demand",
        candidates
            .iter()
            .map(|candidate| format!("`{candidate}`"))
            .collect::<Vec<_>>()
            .join(", ")
                )),
            })
            .collect::<Vec<_>>()
            .join("; ")
        }

        pub(crate) fn name_leaf(name: &str) -> &str {
            name.rsplit("::").next().unwrap_or(name)
        }

        pub(crate) fn schema_accepts(reader: &WireShape, writer: &WireShape) -> bool {
            if !reader.cases.is_empty() || !writer.cases.is_empty() {
                return writer.cases.iter().all(|writer_case| {
                    reader.cases.iter().any(|reader_case| {
                        reader_case.number == writer_case.number
                            && fields_equal(
                                &reader_case.payload_fields,
                                &writer_case.payload_fields,
                            )
                    })
                });
            }
            fields_equal(&reader.fields, &writer.fields)
        }

        pub(crate) fn fields_equal(left: &[WireField], right: &[WireField]) -> bool {
            let left_relevant = left
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            let right_relevant = right
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            left_relevant.len() == right_relevant.len()
                && left_relevant.iter().all(|left_field| {
                    right_relevant.iter().any(|right_field| {
                        left_field.number == right_field.number
                            && left_field.type_display == right_field.type_display
                    })
                })
        }
    }
    pub(crate) mod migration {
        use super::demands::{EraSelection, name_leaf, select_era_path};
        use super::schemas::qualified_schema_path;
        use symbols::SymbolHandle;

        fn resolve_declared_era(
            typed: &crate::TypedTrees,
            requested: &str,
        ) -> Option<SymbolHandle> {
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

        pub(crate) struct MigrationRoute {
            pub(crate) machines: Vec<String>,
            pub(crate) eras: Vec<SymbolHandle>,
        }

        pub(crate) enum MigrationRouteSearch {
            Complete(MigrationRoute),
            Ambiguous(String),
            Missing,
        }

        pub(crate) fn migration_route(
            typed: &crate::TypedTrees,
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
                        || conformance.requirement.as_ref().map(|name| name.as_str())
                            != Some("migrate")
                    {
                        continue;
                    }
                    let arguments = typed
                        .type_reference_table
                        .type_reference_handles(conformance.arguments);
                    if arguments.len() != 3 {
                        continue;
                    }
                    let symbol =
                        |index: usize| typed.type_reference_table.type_symbol(arguments[index]);
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

        pub(crate) fn search_route(
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

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum IdentityScope {
            Fields,
            Cases,
            Payload(u64),
        }

        pub(crate) struct NumberScope {
            key: IdentityScope,
            live: Vec<u64>,
            retired: Vec<u64>,
        }

        fn era_number_scopes(
            typed: &crate::TypedTrees,
            era: SymbolHandle,
        ) -> Option<(String, Vec<NumberScope>)> {
            let definition = typed.data_definition_by_symbol(era)?;
            let members = typed.data_members(definition);
            let has_fields = members
                .iter()
                .any(|member| matches!(member, crate::data::DataMember::Field(_)));
            let has_cases = members
                .iter()
                .any(|member| matches!(member, crate::data::DataMember::Variant(_)));
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
                    crate::data::DataMember::Field(field) => {
                        if let Some(identity) = field.identity {
                            scopes[0].live.push(identity);
                        }
                    }
                    crate::data::DataMember::Variant(variant) => {
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

        pub(crate) fn retired_identity_reuse(
            typed: &crate::TypedTrees,
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
                    if let Some((_, dropped)) =
                        previous_live.iter().find(|(key, _)| *key == scope.key)
                    {
                        for identity in dropped {
                            if !scope.live.contains(identity)
                                && !retired.iter().any(|(key, candidate, _)| {
                                    *key == scope.key && candidate == identity
                                })
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
    }
    pub(crate) mod observations {
        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        pub(crate) struct WireShape {
            pub name: String,
            pub encoding: Option<String>,
            pub fields: Vec<WireField>,
            pub cases: Vec<WireCase>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct WireField {
            pub number: u64,
            pub relevance: language_core::BindingRelevance,
            pub type_display: String,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct WireCase {
            pub number: u64,
            pub payload_fields: Vec<WireField>,
        }

        pub(crate) struct WireDemandVerdict {
            pub edge: String,
            pub local_schema: String,
            pub peer_schema: String,
            pub readability: WireFact,
            pub writability: WireFact,
            pub unknown_preservation: WireFact,
            pub canonicality: WireFact,
            pub migration_coverage: WireFact,
            pub satisfied: bool,
        }

        pub(crate) struct WireFact {
            pub required: bool,
            pub satisfied: bool,
            pub detail: String,
        }
    }
    pub(crate) mod schemas {
        use super::demands::compatibility_demand_verdict;
        use super::{WireCase, WireCompatibilityDemand, WireDemandVerdict, WireField, WireShape};
        use crate::TypedTrees;
        use crate::data::DataDefinition;
        use crate::wire::{WireMember, WireSchema};
        use arena::HandleSpan;

        pub(crate) struct SchemaRow {
            pub(crate) qualified_path: String,
            pub(crate) entry: WireShape,
            pub(crate) value_symbol: Option<symbols::SymbolHandle>,
        }

        pub(crate) fn check_wire_demands(
            typed: &TypedTrees,
            compatibility_demands: &[WireCompatibilityDemand],
        ) -> Vec<WireDemandVerdict> {
            if compatibility_demands.is_empty() {
                return Vec::new();
            }
            let mut rows = typed
                .wire_schemas()
                .iter()
                .map(|schema| SchemaRow {
                    qualified_path: qualified_schema_path(
                        typed,
                        schema.symbol,
                        schema.name.as_str(),
                    ),
                    entry: schema_shape(typed, schema),
                    value_symbol: None,
                })
                .collect::<Vec<_>>();
            for (ordinary_path, ordinary, value_symbol) in
                typed.data_definitions().iter().filter_map(|data| {
                    ordinary_data_shape(typed, data).map(|entry| {
                        (
                            qualified_schema_path(typed, data.symbol, data.name.as_str()),
                            entry,
                            data.symbol,
                        )
                    })
                })
            {
                if let Some(generated) = rows
                    .iter_mut()
                    .find(|row| row.qualified_path == ordinary_path)
                {
                    generated.value_symbol = Some(value_symbol);
                    if generated.entry.fields.is_empty() {
                        generated.entry.fields = ordinary.fields;
                    }
                    generated.entry.cases = ordinary.cases;
                } else {
                    rows.push(SchemaRow {
                        qualified_path: ordinary_path,
                        entry: ordinary,
                        value_symbol: Some(value_symbol),
                    });
                }
            }
            rows.sort_by(|left, right| left.entry.name.cmp(&right.entry.name));
            compatibility_demands
                .iter()
                .map(|demand| compatibility_demand_verdict(typed, &rows, demand))
                .collect()
        }

        pub(crate) fn qualified_schema_path(
            typed: &TypedTrees,
            symbol: symbols::SymbolHandle,
            fallback_name: &str,
        ) -> String {
            let path = typed.symbols.display_path(symbol, "::");
            if path.is_empty() {
                fallback_name.to_owned()
            } else {
                path
            }
        }

        fn ordinary_data_shape(typed: &TypedTrees, data: &DataDefinition) -> Option<WireShape> {
            use crate::data::DataMember;

            let members = typed.data_members(data);
            let has_identity_metadata = !data.retired_identities.is_empty()
                || members.iter().any(|member| match member {
                    DataMember::Field(field) => field.identity.is_some(),
                    DataMember::Variant(variant) => {
                        variant.identity.is_some()
                            || !variant.retired_payload_identities.is_empty()
                            || typed
                                .data_payload_fields(variant)
                                .iter()
                                .any(|field| field.identity.is_some())
                    }
                });
            if !has_identity_metadata {
                return None;
            }
            let fields = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(WireField {
                        number: field.identity?,
                        relevance: field.relevance,
                        type_display: typed.display_type_reference(field.type_reference),
                    }),
                    DataMember::Variant(_) => None,
                })
                .collect();
            let cases = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(WireCase {
                        number: variant.identity?,
                        payload_fields: typed
                            .data_payload_fields(variant)
                            .iter()
                            .filter_map(|field| {
                                Some(WireField {
                                    number: field.identity?,
                                    relevance: field.relevance,
                                    type_display: typed
                                        .display_type_reference(field.type_reference),
                                })
                            })
                            .collect(),
                    }),
                    DataMember::Field(_) => None,
                })
                .collect();
            Some(WireShape {
                name: data.name.to_string(),
                encoding: None,
                fields,
                cases,
            })
        }

        fn collect_fields(typed: &TypedTrees, members: HandleSpan<WireMember>) -> Vec<WireField> {
            typed
                .wire_members(members)
                .iter()
                .filter_map(|member| match member {
                    WireMember::Field(field) => Some(WireField {
                        number: field.number,
                        relevance: field.relevance,
                        type_display: typed.display_type_reference(field.type_reference),
                    }),
                    WireMember::Reserved(_) | WireMember::Version(_) => None,
                })
                .collect()
        }

        fn schema_shape(typed: &TypedTrees, schema: &WireSchema) -> WireShape {
            WireShape {
                name: schema.name.to_string(),
                encoding: schema
                    .encoding
                    .as_ref()
                    .map(|encoding| encoding.to_string()),
                fields: collect_fields(typed, schema.members),
                cases: Vec::new(),
            }
        }
    }

    use language_semantics::wire::WireCompatibilityDemand;
    use observations::{WireCase, WireDemandVerdict, WireFact, WireField, WireShape};
    use schemas::check_wire_demands;

    pub fn validate_wire_protocol(
        typed: &crate::TypedTrees,
        compatibility_demands: &[WireCompatibilityDemand],
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        super::schema_validation::validate_wire_schemas(typed, &mut diagnostics);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        validate_wire_demands(&check_wire_demands(typed, compatibility_demands))
    }

    fn validate_wire_demands(demands: &[WireDemandVerdict]) -> Result<(), Vec<Diagnostic>> {
        let diagnostics = demands
            .iter()
            .filter(|demand| !demand.satisfied)
            .map(|demand| {
                let failed = [
                    ("readability", &demand.readability),
                    ("writability", &demand.writability),
                    ("unknown preservation", &demand.unknown_preservation),
                    ("canonicality", &demand.canonicality),
                    ("migration coverage", &demand.migration_coverage),
                ]
                .into_iter()
                .filter(|(_, fact)| fact.required && !fact.satisfied)
                .map(|(name, fact)| format!("{name} ({})", fact.detail))
                .collect::<Vec<_>>()
                .join(", ");
                Diagnostic::error(format!(
                    "wire compatibility demand `{}` is unsatisfied for local schema `{}` and peer \
             schema `{}`: {}",
                    demand.edge, demand.local_schema, demand.peer_schema, failed
                ))
            })
            .collect::<Vec<_>>();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}
pub(crate) mod schema_validation {
    use crate::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
    use crate::wire::{WireField, WireMember, WireReserved, WireSchema, WireVersion};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;

    pub fn validate_wire_schemas(program: &crate::TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
        for (schema_index, schema) in program.wire_schemas().iter().enumerate() {
            if program.wire_schemas()[..schema_index]
                .iter()
                .any(|previous| {
                    previous.name == schema.name
                        && !program
                            .symbols
                            .source_scopes_separate(previous.symbol, schema.symbol)
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate data `{}`",
                    schema.name
                )));
            }

            validate_schema(program, schema, diagnostics);
        }
    }

    pub(crate) struct WireScope<'program> {
        version: Option<&'program str>,
        fields: Vec<&'program WireField>,
        reserved: Vec<&'program WireReserved>,
        versions: Vec<&'program WireVersion>,
    }

    fn collect_scope<'program>(
        program: &'program crate::TypedTrees,
        version: Option<&'program str>,
        members: HandleSpan<WireMember>,
    ) -> WireScope<'program> {
        let mut scope = WireScope {
            version,
            fields: Vec::new(),
            reserved: Vec::new(),
            versions: Vec::new(),
        };

        for member in program.wire_members(members) {
            match member {
                WireMember::Field(field) => scope.fields.push(field),
                WireMember::Reserved(reserved) => scope.reserved.push(reserved),
                WireMember::Version(nested) => scope.versions.push(nested),
            }
        }

        scope
    }

    fn scope_label(schema: &WireSchema, scope: &WireScope<'_>) -> String {
        match scope.version {
            Some(version) => format!("data `{}` version `{version}`", schema.name),
            None => format!("data `{}`", schema.name),
        }
    }

    fn validate_schema(
        program: &crate::TypedTrees,
        schema: &WireSchema,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let current = collect_scope(program, None, schema.members);

        for (version_index, version) in current.versions.iter().enumerate() {
            if current.versions[..version_index]
                .iter()
                .any(|previous| previous.name == version.name)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` declares duplicate version `{}`",
                    schema.name, version.name
                )));
            }
        }

        let version_scopes = current
            .versions
            .iter()
            .map(|version| collect_scope(program, Some(version.name.as_str()), version.members))
            .collect::<Vec<_>>();

        for scope in version_scopes.iter().chain(std::iter::once(&current)) {
            validate_scope_numbering(schema, scope, diagnostics);
            validate_scope_field_types(program, schema, scope, diagnostics);

            if scope.version.is_some() && !scope.versions.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
            "{} nests a version block inside a version block; protocol eras must be declared at the schema level",
            scope_label(schema, scope)
        )));
            }
        }

        let mut chain: Vec<&WireScope<'_>> = version_scopes.iter().collect();
        chain.push(&current);

        for pair in chain.windows(2) {
            validate_adjacent_eras(schema, pair[0], pair[1], diagnostics);
        }
    }

    fn validate_scope_numbering(
        schema: &WireSchema,
        scope: &WireScope<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (field_index, field) in scope.fields.iter().enumerate() {
            if scope.fields[..field_index]
                .iter()
                .any(|previous| previous.number == field.number)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{} declares duplicate field number {} for field `{}`",
                    scope_label(schema, scope),
                    field.number,
                    field.name
                )));
            }

            if scope
                .reserved
                .iter()
                .any(|reserved| reserved.number == field.number)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{} field `{}` reuses retired identity number {}",
                    scope_label(schema, scope),
                    field.name,
                    field.number
                )));
            }
        }

        for (reserved_index, reserved) in scope.reserved.iter().enumerate() {
            if scope.reserved[..reserved_index]
                .iter()
                .any(|previous| previous.number == reserved.number)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{} retires identity number {} more than once",
                    scope_label(schema, scope),
                    reserved.number
                )));
            }
        }
    }

    fn validate_scope_field_types(
        program: &crate::TypedTrees,
        schema: &WireSchema,
        scope: &WireScope<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for field in &scope.fields {
            validate_field_type_reference(
                program,
                schema,
                scope,
                field,
                field.type_reference,
                diagnostics,
            );
            if field.relevance.is_erased() {
                continue;
            }
        }
    }

    fn validate_field_type_reference(
        program: &crate::TypedTrees,
        schema: &WireSchema,
        scope: &WireScope<'_>,
        field: &WireField,
        type_reference: TypeReferenceHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                validate_field_type_reference(program, schema, scope, field, *referee, diagnostics)
            }
            TypeReferenceNode::Constrained { base_type, .. } => validate_field_type_reference(
                program,
                schema,
                scope,
                field,
                *base_type,
                diagnostics,
            ),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => validate_field_type_reference(
                program,
                schema,
                scope,
                field,
                *element_type,
                diagnostics,
            ),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                if !wire_type_resolves(program, *base_symbol, base_name.as_str()) {
                    push_unknown_field_type(schema, scope, field, base_name.as_str(), diagnostics);
                }

                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    if matches!(
                        program.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::Named { name, .. }
                            if name.as_str().parse::<u64>().is_ok()
                    ) {
                        continue;
                    }
                    validate_field_type_reference(
                        program,
                        schema,
                        scope,
                        field,
                        *argument,
                        diagnostics,
                    );
                }
            }
            TypeReferenceNode::DynamicTrait { symbol, name, .. }
            | TypeReferenceNode::Named { symbol, name } => {
                if !wire_type_resolves(program, *symbol, name.as_str()) {
                    push_unknown_field_type(schema, scope, field, name.as_str(), diagnostics);
                }
            }
            TypeReferenceNode::Callable { .. } => diagnostics.push(Diagnostic::error(format!(
                "{} field `{}` uses a local machine value without a wire codec",
                scope_label(schema, scope),
                field.name,
            ))),
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
        }
    }

    fn wire_type_resolves(
        program: &crate::TypedTrees,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> bool {
        let declared_symbol = symbol.is_valid()
            && (program
                .data_definitions()
                .iter()
                .any(|row| row.symbol == symbol)
                || program.machines().iter().any(|row| row.symbol == symbol)
                || program.traits().iter().any(|row| row.symbol == symbol)
                || program
                    .symbols
                    .child_handles(program.symbols.root())
                    .is_some_and(|mut children| {
                        children.any(|candidate| {
                            candidate == symbol
                                && program.symbols.get(candidate).kind
                                    == symbols::SymbolKind::BuiltinType
                        })
                    }));
        declared_symbol
            || PrimitiveType::from_name(name).is_some()
            || program.wire_schemas().iter().any(|schema| {
                schema.symbol == symbol
                    || (!program.symbols.has_source_metadata() && schema.name.as_str() == name)
            })
            || (!program.symbols.has_source_metadata()
                && (program
                    .data_definitions()
                    .iter()
                    .any(|row| row.name.as_str() == name)
                    || program
                        .machines()
                        .iter()
                        .any(|row| row.name.as_str() == name)
                    || program.traits().iter().any(|row| row.name.as_str() == name)))
    }

    fn push_unknown_field_type(
        schema: &WireSchema,
        scope: &WireScope<'_>,
        field: &WireField,
        type_name: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{} field `{}` references unknown type `{type_name}`",
            scope_label(schema, scope),
            field.name
        )));
    }

    fn validate_adjacent_eras(
        schema: &WireSchema,
        predecessor: &WireScope<'_>,
        successor: &WireScope<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let predecessor_name = predecessor.version.unwrap_or_default();

        for field in &predecessor.fields {
            let successor_field = successor
                .fields
                .iter()
                .find(|candidate| candidate.number == field.number);

            if successor_field.is_none() {
                let is_reserved = successor
                    .reserved
                    .iter()
                    .any(|reserved| reserved.number == field.number);

                if !is_reserved {
                    diagnostics.push(Diagnostic::error(format!(
                "{} drops field number {} (`{}` in version `{predecessor_name}`) without tombstoning it; add `retired {};`",
                scope_label(schema, successor),
                field.number,
                field.name,
                field.number
            )));
                }
            }
        }
    }
}
pub use compatibility::validate_wire_protocol;
pub use schema_validation::validate_wire_schemas;
