/*
Reference and borrow lifetime contract validation (`borrow_contracts`) and
compile-time semantic reflection schema graphs, selection snapshots, and
runtime field visitation plans (`reflection`).
*/

pub mod borrow_contracts {
    pub use typed_trees::source_borrow_contracts::*;
}

pub mod reflection {
    pub(crate) mod schema_graph {
        pub(crate) mod graph_replay {
            use crate::reflection::extend_unique;
            use symbols::SymbolHandle;
            use typed_trees::TypedTrees;
            use typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind};
            use typed_trees::reflection::construction::{
                NominalExpectation, describe_field, describe_type_parameter, exact_symbol_identity,
                resolve_subject, sorted,
            };
            use typed_trees::reflection::graph_revision;
            use typed_trees::reflection::{
                CaseDescription, DeclarationDescription, FieldDescription, SchemaNode,
                SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
            };

            pub fn replay_semantic_schema_graph(
                typed: &TypedTrees,
                graph: &SemanticSchemaGraph,
            ) -> Result<(), String> {
                let SchemaNode::Declaration(root) = graph.node(graph.root()) else {
                    return Err("schema graph root is not a declaration node".to_owned());
                };
                let data = resolve_subject(typed, &root.owner_identity)?;

                check_authority_claim(typed, data, graph.authority(), "schema", &root.name)?;
                check_subject_application(
                    typed,
                    data,
                    &root.application_identity,
                    "schema",
                    &root.name,
                )?;

                let (_, expected_hermetic) = exact_symbol_identity(typed, data.symbol)?;
                if root.identity_is_hermetic != expected_hermetic || root.name != data.name.as_str()
                {
                    return Err(format!(
                        "schema `{}` name or identity strength does not match the bound declaration",
                        root.name
                    ));
                }

                let expected_shape =
                    match DataDefinition::shape_kind_from_members(typed.data_members(data)) {
                        DataShapeKind::Empty => SchemaShape::Empty,
                        DataShapeKind::Record => SchemaShape::Record,
                        DataShapeKind::Enum => SchemaShape::Sum,
                        DataShapeKind::Mixed => SchemaShape::Mixed,
                    };
                if root.shape != expected_shape {
                    return Err(format!(
                        "schema `{}` claims shape `{:?}` but the bound declaration is `{:?}`",
                        root.name, root.shape, expected_shape
                    ));
                }
                if root.supply != format!("{:?}", data.supply_mode)
                    || root.multiplicity != format!("{:?}", data.properties.multiplicity)
                    || root.carry != data.properties.carry.map(|carry| format!("{carry:?}"))
                {
                    return Err(format!(
                        "schema `{}` does not match the bound declaration's construction qualifications",
                        root.name
                    ));
                }
                let expected_parameters: Vec<_> = typed
                    .data_type_parameters(data)
                    .iter()
                    .map(|parameter| describe_type_parameter(typed, parameter))
                    .collect();
                if root.type_parameters != expected_parameters {
                    return Err(format!(
                        "schema `{}` type parameters do not match the bound declaration",
                        root.name
                    ));
                }
                let expected_lifetimes: Vec<_> = data
                    .lifetime_parameters
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect();
                if root.lifetime_parameters != expected_lifetimes {
                    return Err(format!(
                        "schema `{}` lifetime parameters do not match the bound declaration",
                        root.name
                    ));
                }
                if root.retired_identities != sorted(data.retired_identities.clone()) {
                    return Err(format!(
                        "schema `{}` retired identities do not match the bound declaration's tombstones",
                        root.name
                    ));
                }

                let members = typed.data_members(data);
                if root.members.len() != members.len() {
                    return Err(format!(
                        "schema `{}` describes {} top-level members but the bound declaration has {}",
                        root.name,
                        root.members.len(),
                        members.len()
                    ));
                }
                let mut expected_nominals: Vec<(String, String)> = Vec::new();
                for (position, (member, handle)) in
                    members.iter().zip(root.members.iter()).enumerate()
                {
                    if !handle.is_valid() {
                        return Err(format!(
                            "schema `{}` member {position} carries an invalid node handle",
                            root.name
                        ));
                    }
                    match member {
                        DataMember::Field(field) => {
                            let (mut expected_field, expectations) = expect_field_node(
                                typed,
                                field,
                                &root.owner_identity,
                                position as u32,
                                SchemaNodeHandle::INVALID,
                                root.application_identity.as_deref(),
                            )?;
                            let SchemaNode::Field(stored) = graph.node(*handle) else {
                                return Err(format!(
                                    "schema `{}` member {position} `{}` is not a field node",
                                    root.name, field.name
                                ));
                            };
                            expected_field.nominal_references = stored.nominal_references.clone();
                            if *stored != expected_field {
                                return Err(format!(
                                    "schema `{}` member `{name}` does not match its bound field",
                                    root.name,
                                    name = field.name
                                ));
                            }
                            check_nominal_edges(
                                graph,
                                root,
                                stored,
                                &expectations,
                                &mut expected_nominals,
                            )?;
                        }
                        DataMember::Variant(variant) => {
                            let SchemaNode::Case(stored_case) = graph.node(*handle) else {
                                return Err(format!(
                                    "schema `{}` member {position} `{}` is not a case node",
                                    root.name, variant.name
                                ));
                            };
                            let expected_case = CaseDescription {
                                member_identity: member_identity_or_name(
                                    typed,
                                    variant.symbol,
                                    &root.owner_identity,
                                    variant.name.as_str(),
                                ),
                                name: variant.name.as_str().to_owned(),
                                declaration_position: position as u32,
                                stable_number: variant.identity,
                                payload_fields: stored_case.payload_fields.clone(),
                                retired_payload_identities: sorted(
                                    variant.retired_payload_identities.clone(),
                                ),
                            };
                            if *stored_case != expected_case {
                                return Err(format!(
                                    "schema `{}` case `{}` does not match its bound variant",
                                    root.name, variant.name
                                ));
                            }
                            let payload = typed.data_payload_fields(variant);
                            if stored_case.payload_fields.len() != payload.len() {
                                return Err(format!(
                                    "schema `{}` case `{}` describes {} payload members but the bound case has {}",
                                    root.name,
                                    variant.name,
                                    stored_case.payload_fields.len(),
                                    payload.len()
                                ));
                            }
                            for (payload_position, (payload_field, payload_handle)) in payload
                                .iter()
                                .zip(stored_case.payload_fields.iter())
                                .enumerate()
                            {
                                if !payload_handle.is_valid() {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload {payload_position} carries an invalid node handle",
                                        root.name, variant.name
                                    ));
                                }
                                let (mut expected_field, expectations) = expect_field_node(
                                    typed,
                                    payload_field,
                                    &root.owner_identity,
                                    payload_position as u32,
                                    *handle,
                                    root.application_identity.as_deref(),
                                )?;
                                let SchemaNode::Field(stored_field) = graph.node(*payload_handle)
                                else {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload {payload_position} is not a field node",
                                        root.name, variant.name
                                    ));
                                };
                                expected_field.nominal_references =
                                    stored_field.nominal_references.clone();
                                if *stored_field != expected_field {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload member `{}` does not match its bound field",
                                        root.name, variant.name, payload_field.name
                                    ));
                                }
                                check_nominal_edges(
                                    graph,
                                    root,
                                    stored_field,
                                    &expectations,
                                    &mut expected_nominals,
                                )?;
                            }
                        }
                    }
                }

                let mut reachable = vec![false; graph.nodes().len()];
                reachable[graph.root().index()] = true;
                for handle in &root.members {
                    reachable[handle.index()] = true;
                    if let SchemaNode::Case(case) = graph.node(*handle) {
                        for payload in &case.payload_fields {
                            reachable[payload.index()] = true;
                        }
                    }
                }
                let mut referenced_nominals = vec![false; graph.nodes().len()];
                for node in graph.nodes().iter() {
                    if let SchemaNode::Field(field) = node {
                        for reference in &field.nominal_references {
                            if !reference.is_valid() || reference.index() >= graph.nodes().len() {
                                return Err(format!(
                                    "schema `{}` field `{}` references a node outside the owned graph",
                                    root.name, field.name
                                ));
                            }
                            referenced_nominals[reference.index()] = true;
                        }
                    }
                }
                for (index, _) in graph.nodes().iter().enumerate() {
                    if index > 1 && !reachable[index] && !referenced_nominals[index] {
                        return Err(format!(
                            "schema `{}` contains an orphan member node `{index}` reachable through no member list",
                            root.name
                        ));
                    }
                }
                let mut stored_nominals: Vec<(String, String)> = Vec::new();
                for (index, node) in graph.nodes().iter().enumerate() {
                    let SchemaNode::NominalReference(reference) = node else {
                        continue;
                    };
                    if !referenced_nominals[index] {
                        return Err(format!(
                            "schema `{}` nominal reference `{}` is not referenced by any member",
                            root.name, reference.name
                        ));
                    }
                    let key = (
                        reference.owner_identity.clone(),
                        reference.application_identity.clone(),
                    );
                    if stored_nominals.contains(&key) {
                        return Err(format!(
                            "schema `{}` duplicates nominal reference `{}`",
                            root.name, reference.name
                        ));
                    }
                    stored_nominals.push(key);
                }
                let mut expected_unique: Vec<(String, String)> = Vec::new();
                extend_unique(&mut expected_unique, expected_nominals);
                if stored_nominals != expected_unique {
                    return Err(format!(
                        "schema `{}` nominal references do not match the bound member types",
                        root.name
                    ));
                }

                if graph.revision() != graph_revision(graph.authority(), graph.nodes()) {
                    return Err(format!(
                        "schema `{}` report fingerprint does not match its frozen contents",
                        root.name
                    ));
                }
                Ok(())
            }

            pub(crate) fn check_authority_claim(
                typed: &TypedTrees,
                data: &DataDefinition,
                authority: &SchemaQueryAuthority,
                artifact: &str,
                subject_name: &str,
            ) -> Result<(), String> {
                match authority {
                    SchemaQueryAuthority::ForeignScope { .. } => {
                        let visible =
                            typed_trees::visibility::declaration_visibility(typed, data.symbol)
                                .is_some_and(|visibility| visibility.is_public());
                        if !visible {
                            return Err(format!(
                                "{artifact} `{subject_name}` claims a foreign-scope query over a non-public subject; complete structural visibility is impossible",
                            ));
                        }
                    }
                    SchemaQueryAuthority::OwningScope { requester_identity } => {
                        if let Some(requester) = resolve_requester(typed, requester_identity)?
                            && !typed
                                .symbols
                                .same_symbol_source_package(requester, data.symbol)
                        {
                            return Err(format!(
                                "{artifact} `{subject_name}` claims an owning scope but its requester is outside the subject's package",
                            ));
                        }
                    }
                }
                Ok(())
            }

            pub(crate) fn check_subject_application(
                typed: &TypedTrees,
                data: &DataDefinition,
                recorded: &Option<String>,
                artifact: &str,
                subject_name: &str,
            ) -> Result<(), String> {
                if let Some(application_identity) = recorded {
                    let expected = data
            .generic_instance
            .map(|instance| {
                typed
                    .package_qualified_type_identity_text(instance)
            })
            .ok_or_else(|| {
                format!(
                    "{artifact} `{subject_name}` claims application `{application_identity}` but the bound declaration is not a generated instance",
                )
            })?;
                    if *application_identity != expected {
                        return Err(format!(
                            "{artifact} `{subject_name}` application `{application_identity}` does not match the bound instance `{expected}`",
                        ));
                    }
                } else if data.generic_instance.is_some() {
                    return Err(format!(
                        "{artifact} `{subject_name}` records no application for a generated instance",
                    ));
                }
                Ok(())
            }

            pub(crate) fn member_identity_or_name(
                typed: &TypedTrees,
                symbol: SymbolHandle,
                owner_identity: &str,
                name: &str,
            ) -> String {
                if symbol.is_valid() {
                    exact_symbol_identity(typed, symbol)
                        .map(|(identity, _)| identity)
                        .unwrap_or_else(|_| format!("{owner_identity}::{name}"))
                } else {
                    format!("{owner_identity}::{name}")
                }
            }

            fn expect_field_node(
                typed: &TypedTrees,
                field: &DataField,
                owner_identity: &str,
                declaration_position: u32,
                owner_case: SchemaNodeHandle,
                root_application_identity: Option<&str>,
            ) -> Result<(FieldDescription, Vec<NominalExpectation>), String> {
                let (mut description, expectations) = describe_field(
                    typed,
                    field,
                    owner_identity,
                    declaration_position,
                    owner_case,
                    root_application_identity,
                )?;
                description.nominal_references.clear();
                Ok((description, expectations))
            }

            fn check_nominal_edges(
                graph: &SemanticSchemaGraph,
                root: &DeclarationDescription,
                stored: &FieldDescription,
                expectations: &[NominalExpectation],
                expected_nominals: &mut Vec<(String, String)>,
            ) -> Result<(), String> {
                if stored.nominal_references.len() != expectations.len() {
                    return Err(format!(
                        "schema `{}` field `{}` records {} nominal edges but its bound type has {}",
                        root.name,
                        stored.name,
                        stored.nominal_references.len(),
                        expectations.len()
                    ));
                }
                for (handle, expectation) in
                    stored.nominal_references.iter().zip(expectations.iter())
                {
                    if expectation.is_self_edge {
                        if *handle != graph.root() {
                            return Err(format!(
                                "schema `{}` field `{}` self-references the selected application through a non-root handle",
                                root.name, stored.name
                            ));
                        }
                        continue;
                    }
                    expected_nominals.push((
                        expectation.owner_identity.clone(),
                        expectation.application_identity.clone(),
                    ));
                    let SchemaNode::NominalReference(reference) = graph.node(*handle) else {
                        return Err(format!(
                            "schema `{}` field `{}` references `{}` through a non-nominal node",
                            root.name, stored.name, expectation.name
                        ));
                    };
                    if reference.owner_identity != expectation.owner_identity
                        || reference.owner_kind != expectation.owner_kind
                        || reference.name != expectation.name
                        || reference.application_identity != expectation.application_identity
                    {
                        return Err(format!(
                            "schema `{}` field `{}` nominal edge `{}` does not match its bound type",
                            root.name, stored.name, expectation.name
                        ));
                    }
                }
                Ok(())
            }

            fn resolve_requester(
                typed: &TypedTrees,
                requester_identity: &str,
            ) -> Result<Option<SymbolHandle>, String> {
                let mut found = None;
                for index in 0..typed.symbols.symbols().len() {
                    let symbol = SymbolHandle::from_arena_index(index as u32);
                    if !symbol.is_valid() {
                        continue;
                    }
                    if let Ok((identity, _)) = exact_symbol_identity(typed, symbol)
                        && identity == requester_identity
                    {
                        if found.is_some() {
                            return Err(format!(
                                "schema requester `{requester_identity}` resolves to more than one symbol"
                            ));
                        }
                        found = Some(symbol);
                    }
                }
                Ok(found)
            }
        }
        pub use graph_replay::replay_semantic_schema_graph;
        pub(crate) use graph_replay::{
            check_authority_claim, check_subject_application, member_identity_or_name,
        };
    }
    fn extend_unique<T: PartialEq>(target: &mut Vec<T>, additions: impl IntoIterator<Item = T>) {
        for addition in additions {
            if !target.contains(&addition) {
                target.push(addition);
            }
        }
    }
    pub(crate) mod selection {
        pub(crate) mod requirement_resolution {
            use crate::reflection::selection::scoped_members::ScopedMember;
            use crate::reflection::selection::{
                DemandedTraitApplication, SelectionChoice, SelectionRequirement,
                first_occurrence_normalized,
            };
            use symbols::SymbolHandle;
            use typed_trees::TypedTrees;
            use typed_trees::machine::Machine;
            use typed_trees::reflection::construction::exact_symbol_identity;
            use typed_trees::trait_definition::{Conformance, TraitDefinition};
            use typed_trees::types::TypeReferenceHandle;

            pub(crate) struct ResolvedRequirement<'program> {
                definition: &'program TraitDefinition,
                requirement_symbol: Option<SymbolHandle>,
                trait_application: Option<DemandedTraitApplication>,
            }

            pub(crate) fn resolve_requirement<'program>(
                typed: &'program TypedTrees,
                requirement: &SelectionRequirement,
            ) -> Result<ResolvedRequirement<'program>, String> {
                let mut matches = typed.traits().iter().filter(|definition| {
                    exact_symbol_identity(typed, definition.symbol)
                        .is_ok_and(|(identity, _)| identity == requirement.trait_identity)
                });
                let Some(definition) = matches.next() else {
                    return Err(format!(
                        "selection requirement `{}` does not resolve to a trait declaration in this program",
                        requirement.trait_identity
                    ));
                };
                if matches.next().is_some() {
                    return Err(format!(
                        "selection requirement `{}` resolves to more than one trait declaration",
                        requirement.trait_identity
                    ));
                }
                let requirement_symbol = match &requirement.requirement_identity {
        Some(identity) => Some(
            typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| {
                    exact_symbol_identity(typed, signature.symbol)
                        .is_ok_and(|(candidate, _)| candidate == *identity)
                })
                .map(|signature| signature.symbol)
                .ok_or_else(|| {
                    format!(
                        "selection requirement `{identity}` is not a requirement of trait `{}`",
                        requirement.trait_identity
                    )
                })?,
        ),
        None => None,
    };
                Ok(ResolvedRequirement {
                    definition,
                    requirement_symbol,
                    trait_application: requirement.trait_application.clone(),
                })
            }

            fn resolve_unique<'program, T>(
                items: impl Iterator<Item = &'program T>,
                symbol_of: impl Fn(&'program T) -> SymbolHandle,
                typed: &'program TypedTrees,
                identity: &str,
                domain: &str,
            ) -> Result<&'program T, String> {
                let mut matches = items.filter(|item| {
                    exact_symbol_identity(typed, symbol_of(item))
                        .is_ok_and(|(candidate, _)| candidate == identity)
                });
                match (matches.next(), matches.next()) {
                    (Some(item), None) => Ok(item),
                    (Some(_), Some(_)) => Err(format!(
                        "selection {domain} `{identity}` resolves to more than one declaration"
                    )),
                    _ => Err(format!(
                        "selection {domain} `{identity}` does not resolve to a {domain} in this program"
                    )),
                }
            }

            fn resolve_conformance<'program>(
                typed: &'program TypedTrees,
                identity: &str,
            ) -> Result<&'program Conformance, String> {
                resolve_unique(
                    typed.conformances().iter(),
                    |conformance| conformance.symbol,
                    typed,
                    identity,
                    "conformance",
                )
            }

            fn resolve_machine<'program>(
                typed: &'program TypedTrees,
                identity: &str,
            ) -> Result<&'program Machine, String> {
                resolve_unique(
                    typed.machines().iter(),
                    |machine| machine.symbol,
                    typed,
                    identity,
                    "machine",
                )
            }

            fn check_machine_owner_covers_member(
                typed: &TypedTrees,
                machine: &Machine,
                member: &ScopedMember,
            ) -> Result<(), String> {
                let Some(head) = &member.head_nominal_identity else {
                    return Err(format!(
                        "selection for `{}` requires an operation on the member's declared type, which has no nominal head",
                        member.member_identity
                    ));
                };
                if !machine.attached_data_symbol.is_valid() {
                    return Err(format!(
                        "selection for `{}` names an operation owned by no data declaration",
                        member.member_identity
                    ));
                }
                let owner_identity = exact_symbol_identity(typed, machine.attached_data_symbol)?.0;
                if owner_identity != *head {
                    return Err(format!(
                        "selection for `{}` names an operation on `{owner_identity}` which does not cover the member's declared type",
                        member.member_identity
                    ));
                }
                Ok(())
            }

            fn application_satisfied(
                typed: &TypedTrees,
                demanded: &DemandedTraitApplication,
                type_arguments: &[TypeReferenceHandle],
                lifetime_arguments: &[u32],
            ) -> bool {
                let carried: Vec<String> = type_arguments
                    .iter()
                    .map(|argument| typed.package_qualified_type_identity_text(*argument))
                    .collect();
                carried == demanded.type_argument_identities
                    && first_occurrence_normalized(lifetime_arguments)
                        == demanded.lifetime_arguments
            }

            fn carried_application_matches(
                typed: &TypedTrees,
                demanded: &Option<DemandedTraitApplication>,
                arguments: arena::HandleSpan<TypeReferenceHandle>,
                lifetime_arguments: &[u32],
            ) -> bool {
                match demanded {
                    None => true,
                    Some(demanded) => application_satisfied(
                        typed,
                        demanded,
                        typed.type_reference_table.type_reference_handles(arguments),
                        lifetime_arguments,
                    ),
                }
            }

            pub(crate) fn check_conformance_refines(
                typed: &TypedTrees,
                conformance_identity: &str,
                requirement: &ResolvedRequirement,
                member: &ScopedMember,
            ) -> Result<(), String> {
                let conformance = resolve_conformance(typed, conformance_identity)?;
                if conformance.trait_symbol != requirement.definition.symbol {
                    return Err(format!(
                        "selection for `{}` names conformance `{conformance_identity}` of a different trait; insufficient contract",
                        member.member_identity
                    ));
                }
                if !carried_application_matches(
                    typed,
                    &requirement.trait_application,
                    conformance.arguments,
                    &conformance.trait_lifetime_arguments,
                ) {
                    return Err(format!(
                        "selection for `{}` names conformance `{conformance_identity}` at a different trait application than demanded",
                        member.member_identity
                    ));
                }
                if let Some(requirement_symbol) = requirement.requirement_symbol
                    && !conformance.rows.iter().any(|row| {
                        row.requirement == requirement_symbol && row.realization_machine.is_valid()
                    })
                {
                    return Err(format!(
                        "selection for `{}` names conformance `{conformance_identity}` which does not realize the pinned requirement",
                        member.member_identity
                    ));
                }
                Err(format!(
                    "selection for `{}` names named implementation `{conformance_identity}`, which has no data subject; select the realizing machine whose owner covers the member",
                    member.member_identity
                ))
            }

            pub(crate) fn check_machine_refines(
                typed: &TypedTrees,
                machine_identity: &str,
                requirement: &ResolvedRequirement,
                member: &ScopedMember,
            ) -> Result<(), String> {
                let machine = resolve_machine(typed, machine_identity)?;
                if typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|edge| {
                        edge.symbol == requirement.definition.symbol
                            && requirement
                                .requirement_symbol
                                .is_none_or(|required| edge.requirement_symbol == required)
                            && carried_application_matches(
                                typed,
                                &requirement.trait_application,
                                edge.arguments,
                                &edge.trait_lifetime_arguments,
                            )
                    })
                {
                    return check_machine_owner_covers_member(typed, machine, member);
                }
                for conformance in typed
                    .conformances()
                    .iter()
                    .filter(|conformance| conformance.trait_symbol == requirement.definition.symbol)
                    .filter(|conformance| {
                        carried_application_matches(
                            typed,
                            &requirement.trait_application,
                            conformance.arguments,
                            &conformance.trait_lifetime_arguments,
                        )
                    })
                {
                    let realizes = conformance.rows.iter().any(|row| {
                        row.realization_machine == machine.symbol
                            && requirement
                                .requirement_symbol
                                .is_none_or(|required| row.requirement == required)
                    });
                    if realizes {
                        return check_machine_owner_covers_member(typed, machine, member);
                    }
                }
                Err(format!(
                    "selection for `{}` names machine `{machine_identity}` which does not realize the required contract",
                    member.member_identity
                ))
            }

            pub(crate) fn check_choice_refines(
                typed: &TypedTrees,
                requirement: &SelectionRequirement,
                member: &ScopedMember,
                choice: &SelectionChoice,
            ) -> Result<(), String> {
                let requirement = resolve_requirement(typed, requirement)?;
                match choice {
                    SelectionChoice::Conformance {
                        conformance_identity,
                    } => {
                        check_conformance_refines(typed, conformance_identity, &requirement, member)
                    }
                    SelectionChoice::Machine { machine_identity } => {
                        check_machine_refines(typed, machine_identity, &requirement, member)
                    }
                    SelectionChoice::Excluded => Ok(()),
                }
            }
        }
        pub(crate) mod scoped_members {
            use crate::reflection::schema_graph::member_identity_or_name;
            use crate::reflection::selection::{
                SelectionChoice, SelectionProjection, SelectionRecord,
            };
            use typed_trees::TypedTrees;
            use typed_trees::data::{DataDefinition, DataMember};
            use typed_trees::reflection::construction::describe_field;
            use typed_trees::reflection::{
                FieldDescription, SchemaNode, SchemaNodeHandle, SemanticSchemaGraph,
            };

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct MemberSelectionKey {
                pub(crate) member_identity: String,
                pub(crate) qualified_type: Option<String>,
                owner_case_identity: Option<String>,
                erased: bool,
            }

            impl MemberSelectionKey {
                pub fn member_identity(&self) -> &str {
                    &self.member_identity
                }

                pub fn qualified_type(&self) -> Option<&str> {
                    self.qualified_type.as_deref()
                }

                pub fn is_erased(&self) -> bool {
                    self.erased
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum MemberKind {
                Field,
                Case,
                PayloadField,
            }

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub(crate) struct ScopedMember {
                pub(crate) member_identity: String,
                pub(crate) name: String,
                pub(crate) kind: MemberKind,
                pub(crate) declaration_position: u32,
                pub(crate) stable_number: Option<u64>,
                pub(crate) qualified_type: Option<String>,
                pub(crate) owner_case_identity: Option<String>,
                pub(crate) erased: bool,
                pub(crate) head_nominal_identity: Option<String>,
            }

            impl ScopedMember {
                pub(crate) fn key(&self) -> MemberSelectionKey {
                    MemberSelectionKey {
                        member_identity: self.member_identity.clone(),
                        qualified_type: self.qualified_type.clone(),
                        owner_case_identity: self.owner_case_identity.clone(),
                        erased: self.erased,
                    }
                }

                pub(crate) fn matches_key(&self, key: &MemberSelectionKey) -> bool {
                    self.member_identity == key.member_identity
                        && self.qualified_type == key.qualified_type
                        && self.owner_case_identity == key.owner_case_identity
                        && self.erased == key.erased
                }

                pub(crate) fn record(&self, choice: SelectionChoice) -> SelectionRecord {
                    SelectionRecord {
                        member_identity: self.member_identity.clone(),
                        qualified_type: self.qualified_type.clone(),
                        owner_case_identity: self.owner_case_identity.clone(),
                        erased: self.erased,
                        choice,
                    }
                }
            }

            pub(crate) fn record_matches(record: &SelectionRecord, member: &ScopedMember) -> bool {
                record.member_identity == member.member_identity
                    && record.qualified_type == member.qualified_type
                    && record.owner_case_identity == member.owner_case_identity
                    && record.erased == member.erased
            }

            pub(crate) fn project(
                members: &[ScopedMember],
                projection: &SelectionProjection,
            ) -> Result<Vec<ScopedMember>, String> {
                match projection {
                    SelectionProjection::DeclaredMembers => Ok(members.to_vec()),
                    SelectionProjection::RuntimeMembers => Ok(members
                        .iter()
                        .filter(|member| !member.erased)
                        .cloned()
                        .collect()),
                    SelectionProjection::Members(identities) => {
                        let mut scoped = Vec::with_capacity(identities.len());
                        let mut seen: Vec<&str> = Vec::with_capacity(identities.len());
                        for identity in identities {
                            if seen.contains(&identity.as_str()) {
                                return Err(format!(
                                    "selection projection lists member `{identity}` twice"
                                ));
                            }
                            seen.push(identity.as_str());
                            let mut matches = members
                                .iter()
                                .filter(|member| member.member_identity == *identity);
                            match (matches.next(), matches.next()) {
                                (Some(member), None) => scoped.push(member.clone()),
                                (Some(_), Some(_)) => {
                                    return Err(format!(
                                        "selection projection member `{identity}` is ambiguous in the bound schema"
                                    ));
                                }
                                _ => {
                                    return Err(format!(
                                        "selection projection member `{identity}` is not a member of the bound schema"
                                    ));
                                }
                            }
                        }
                        Ok(scoped)
                    }
                }
            }

            pub(crate) fn graph_members(
                graph: &SemanticSchemaGraph,
            ) -> Result<Vec<ScopedMember>, String> {
                let declaration = graph.declaration();
                let mut members = Vec::new();
                for handle in &declaration.members {
                    match graph.node(*handle) {
                        SchemaNode::Field(field) => {
                            members.push(graph_field_member(graph, field, MemberKind::Field, None)?)
                        }
                        SchemaNode::Case(case) => {
                            members.push(ScopedMember {
                                member_identity: case.member_identity.clone(),
                                name: case.name.clone(),
                                kind: MemberKind::Case,
                                declaration_position: case.declaration_position,
                                stable_number: case.stable_number,
                                qualified_type: None,
                                owner_case_identity: None,
                                erased: false,
                                head_nominal_identity: Some(declaration.owner_identity.clone()),
                            });
                            for payload in &case.payload_fields {
                                let SchemaNode::Field(field) = graph.node(*payload) else {
                                    return Err(format!(
                                        "schema `{}` case `{}` payload is not a field node",
                                        declaration.name, case.name
                                    ));
                                };
                                members.push(graph_field_member(
                                    graph,
                                    field,
                                    MemberKind::PayloadField,
                                    Some(case.member_identity.clone()),
                                )?);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(members)
            }

            fn graph_field_member(
                graph: &SemanticSchemaGraph,
                field: &FieldDescription,
                kind: MemberKind,
                owner_case_identity: Option<String>,
            ) -> Result<ScopedMember, String> {
                let head_nominal_identity = match field.nominal_references.first() {
                    None => None,
                    Some(handle) if *handle == graph.root() => {
                        Some(graph.declaration().owner_identity.clone())
                    }
                    Some(handle) => match graph.node(*handle) {
                        SchemaNode::NominalReference(reference) => {
                            Some(reference.owner_identity.clone())
                        }
                        _ => {
                            return Err(format!(
                                "schema member `{}` nominal edge is not a reference node",
                                field.name
                            ));
                        }
                    },
                };
                Ok(ScopedMember {
                    member_identity: field.member_identity.clone(),
                    name: field.name.clone(),
                    kind,
                    declaration_position: field.declaration_position,
                    stable_number: field.stable_number,
                    qualified_type: Some(field.qualified_type.clone()),
                    owner_case_identity,
                    erased: field.erased,
                    head_nominal_identity,
                })
            }

            pub(crate) fn expected_members(
                typed: &TypedTrees,
                data: &DataDefinition,
                owner_identity: &str,
                application_identity: Option<&str>,
            ) -> Result<Vec<ScopedMember>, String> {
                let mut members = Vec::new();
                for (position, member) in typed.data_members(data).iter().enumerate() {
                    match member {
                        DataMember::Field(field) => {
                            let (description, expectations) = describe_field(
                                typed,
                                field,
                                owner_identity,
                                position as u32,
                                SchemaNodeHandle::INVALID,
                                application_identity,
                            )?;
                            members.push(ScopedMember {
                                member_identity: description.member_identity,
                                name: description.name,
                                kind: MemberKind::Field,
                                declaration_position: description.declaration_position,
                                stable_number: description.stable_number,
                                qualified_type: Some(description.qualified_type),
                                owner_case_identity: None,
                                erased: description.erased,
                                head_nominal_identity: expectations
                                    .first()
                                    .map(|expectation| expectation.owner_identity.clone()),
                            });
                        }
                        DataMember::Variant(variant) => {
                            let case_identity = member_identity_or_name(
                                typed,
                                variant.symbol,
                                owner_identity,
                                variant.name.as_str(),
                            );
                            members.push(ScopedMember {
                                member_identity: case_identity.clone(),
                                name: variant.name.as_str().to_owned(),
                                kind: MemberKind::Case,
                                declaration_position: position as u32,
                                stable_number: variant.identity,
                                qualified_type: None,
                                owner_case_identity: None,
                                erased: false,
                                head_nominal_identity: Some(owner_identity.to_owned()),
                            });
                            for (payload_position, payload_field) in
                                typed.data_payload_fields(variant).iter().enumerate()
                            {
                                let (description, expectations) = describe_field(
                                    typed,
                                    payload_field,
                                    owner_identity,
                                    payload_position as u32,
                                    SchemaNodeHandle::INVALID,
                                    application_identity,
                                )?;
                                members.push(ScopedMember {
                                    member_identity: description.member_identity,
                                    name: description.name,
                                    kind: MemberKind::PayloadField,
                                    declaration_position: description.declaration_position,
                                    stable_number: description.stable_number,
                                    qualified_type: Some(description.qualified_type),
                                    owner_case_identity: Some(case_identity.clone()),
                                    erased: description.erased,
                                    head_nominal_identity: expectations
                                        .first()
                                        .map(|expectation| expectation.owner_identity.clone()),
                                });
                            }
                        }
                    }
                }
                Ok(members)
            }
        }
        pub(crate) mod selection_receiver {
            use crate::reflection::selection::requirement_resolution::{
                check_choice_refines, resolve_requirement,
            };
            use crate::reflection::selection::scoped_members::{
                ScopedMember, graph_members, project, record_matches,
            };
            use crate::reflection::selection::snapshots::snapshot_revision;
            use crate::reflection::selection::{
                MemberSelectionKey, SelectionChoice, SelectionCoverage, SelectionProjection,
                SelectionRecord, SelectionRequirement, SelectionSnapshot,
            };
            use typed_trees::TypedTrees;
            use typed_trees::reflection::{SchemaQueryAuthority, SemanticSchemaGraph};

            #[derive(Debug)]
            pub struct ScopedSelectionReceiver<'program> {
                typed: &'program TypedTrees,
                subject_owner_identity: String,
                subject_application_identity: Option<String>,
                authority: SchemaQueryAuthority,
                projection: SelectionProjection,
                coverage: SelectionCoverage,
                requirement: SelectionRequirement,
                members: Vec<ScopedMember>,
                records: Vec<SelectionRecord>,
            }

            impl<'program> ScopedSelectionReceiver<'program> {
                pub fn new(
                    typed: &'program TypedTrees,
                    graph: &SemanticSchemaGraph,
                    projection: SelectionProjection,
                    coverage: SelectionCoverage,
                    requirement: SelectionRequirement,
                ) -> Result<Self, String> {
                    resolve_requirement(typed, &requirement)?;
                    let declaration = graph.declaration();
                    let members = project(&graph_members(graph)?, &projection)?;
                    Ok(Self {
                        typed,
                        subject_owner_identity: declaration.owner_identity.clone(),
                        subject_application_identity: declaration.application_identity.clone(),
                        authority: graph.authority().clone(),
                        projection,
                        coverage,
                        requirement,
                        members,
                        records: Vec::new(),
                    })
                }

                pub fn member(&self, name: &str) -> Result<MemberSelectionKey, String> {
                    let mut matches = self.members.iter().filter(|member| member.name == name);
                    match (matches.next(), matches.next()) {
                        (Some(member), None) => Ok(member.key()),
                        (Some(_), Some(_)) => Err(format!(
                            "selection member name `{name}` is ambiguous inside the projection"
                        )),
                        _ => Err(format!(
                            "no member named `{name}` is in the receiver's scoped projection"
                        )),
                    }
                }

                pub fn members(&self) -> impl Iterator<Item = MemberSelectionKey> + '_ {
                    self.members.iter().map(ScopedMember::key)
                }

                pub fn select(
                    &mut self,
                    key: &MemberSelectionKey,
                    choice: SelectionChoice,
                ) -> Result<(), String> {
                    if matches!(choice, SelectionChoice::Excluded) {
                        return Err(
                            "an explicit exclusion is recorded with `exclude`, not `select`"
                                .to_owned(),
                        );
                    }
                    let member = self.scoped_member_for_key(key)?;
                    self.check_unrecorded(&member)?;
                    check_choice_refines(self.typed, &self.requirement, &member, &choice)?;
                    self.records.push(member.record(choice));
                    Ok(())
                }

                pub fn exclude(&mut self, key: &MemberSelectionKey) -> Result<(), String> {
                    if self.coverage == SelectionCoverage::Complete {
                        return Err(format!(
                            "member `{}` cannot be excluded under a complete-coverage contract; every in-scope member needs exactly one selection",
                            key.member_identity
                        ));
                    }
                    let member = self.scoped_member_for_key(key)?;
                    self.check_unrecorded(&member)?;
                    self.records.push(member.record(SelectionChoice::Excluded));
                    Ok(())
                }

                fn scoped_member_for_key(
                    &self,
                    key: &MemberSelectionKey,
                ) -> Result<ScopedMember, String> {
                    if let Some(member) = self.members.iter().find(|member| member.matches_key(key))
                    {
                        return Ok(member.clone());
                    }
                    if self
                        .members
                        .iter()
                        .any(|member| member.member_identity == key.member_identity)
                    {
                        Err(format!(
                            "selection key for `{}` is stale: it does not match the projected member's type and scope",
                            key.member_identity
                        ))
                    } else {
                        Err(format!(
                            "selection member `{}` is outside the receiver's scoped projection",
                            key.member_identity
                        ))
                    }
                }

                fn check_unrecorded(&self, member: &ScopedMember) -> Result<(), String> {
                    if self
                        .records
                        .iter()
                        .any(|record| record_matches(record, member))
                    {
                        return Err(format!(
                            "member `{}` already has a recorded choice; selecting twice is not last-write-wins",
                            member.member_identity
                        ));
                    }
                    Ok(())
                }

                pub fn freeze(self) -> Result<SelectionSnapshot, String> {
                    let mut records = self.records;
                    let mut ordered = Vec::with_capacity(self.members.len());
                    for member in &self.members {
                        let Some(position) = records
                            .iter()
                            .position(|record| record_matches(record, member))
                        else {
                            return Err(format!(
                                "member `{}` has no recorded selection or exclusion; a scoped selection covers every in-scope member exactly once",
                                member.member_identity
                            ));
                        };
                        ordered.push(records.swap_remove(position));
                    }
                    if !records.is_empty() {
                        return Err(
                            "selection records name members outside the projection".to_owned()
                        );
                    }
                    let mut snapshot = SelectionSnapshot {
                        subject_owner_identity: self.subject_owner_identity,
                        subject_application_identity: self.subject_application_identity,
                        authority: self.authority,
                        projection: self.projection,
                        coverage: self.coverage,
                        requirement: self.requirement,
                        records: ordered,
                        revision: 0,
                    };
                    snapshot.revision = snapshot_revision(&snapshot);
                    Ok(snapshot)
                }
            }
        }
        pub(crate) mod snapshots {
            use crate::reflection::schema_graph::{
                check_authority_claim, check_subject_application,
            };
            use crate::reflection::selection::requirement_resolution::{
                check_conformance_refines, check_machine_refines, resolve_requirement,
            };
            use crate::reflection::selection::scoped_members::{
                expected_members, project, record_matches,
            };
            use crate::reflection::selection::{
                SelectionChoice, SelectionCoverage, SelectionProjection, SelectionRecord,
                SelectionRequirement,
            };
            use typed_trees::TypedTrees;
            use typed_trees::reflection::SchemaQueryAuthority;
            use typed_trees::reflection::construction::resolve_subject;

            #[derive(Debug, Clone, PartialEq)]
            pub struct SelectionSnapshot {
                pub(crate) subject_owner_identity: String,
                pub(crate) subject_application_identity: Option<String>,
                pub(crate) authority: SchemaQueryAuthority,
                pub(crate) projection: SelectionProjection,
                pub(crate) coverage: SelectionCoverage,
                pub(crate) requirement: SelectionRequirement,
                pub(crate) records: Vec<SelectionRecord>,
                pub(crate) revision: u64,
            }

            impl SelectionSnapshot {
                pub fn subject_owner_identity(&self) -> &str {
                    &self.subject_owner_identity
                }

                pub fn subject_application_identity(&self) -> Option<&str> {
                    self.subject_application_identity.as_deref()
                }

                pub fn authority(&self) -> &SchemaQueryAuthority {
                    &self.authority
                }

                pub fn projection(&self) -> &SelectionProjection {
                    &self.projection
                }

                pub fn coverage(&self) -> SelectionCoverage {
                    self.coverage
                }

                pub fn requirement(&self) -> &SelectionRequirement {
                    &self.requirement
                }

                pub fn records(&self) -> &[SelectionRecord] {
                    &self.records
                }

                pub fn revision(&self) -> u64 {
                    self.revision
                }
            }

            pub fn replay_selection_snapshot(
                typed: &TypedTrees,
                snapshot: &SelectionSnapshot,
            ) -> Result<(), String> {
                let data = resolve_subject(typed, &snapshot.subject_owner_identity)?;
                if data.quotient.is_some() {
                    return Err(format!(
                        "selection snapshot `{}` binds a quotient declaration, which exposes no representative to reflection",
                        data.name.as_str()
                    ));
                }
                check_authority_claim(
                    typed,
                    data,
                    &snapshot.authority,
                    "selection snapshot",
                    data.name.as_str(),
                )?;
                check_subject_application(
                    typed,
                    data,
                    &snapshot.subject_application_identity,
                    "selection snapshot",
                    data.name.as_str(),
                )?;
                let requirement = resolve_requirement(typed, &snapshot.requirement)?;
                let expected = expected_members(
                    typed,
                    data,
                    &snapshot.subject_owner_identity,
                    snapshot.subject_application_identity.as_deref(),
                )?;
                let in_scope = project(&expected, &snapshot.projection).map_err(|error| {
        format!(
            "selection snapshot `{}` projection does not resolve against the bound declaration: {error}",
            data.name.as_str()
        )
    })?;
                if snapshot.records.len() != in_scope.len() {
                    return Err(format!(
                        "selection snapshot `{}` records {} members but its projection covers {}",
                        data.name.as_str(),
                        snapshot.records.len(),
                        in_scope.len()
                    ));
                }
                for (record, member) in snapshot.records.iter().zip(in_scope.iter()) {
                    if !record_matches(record, member) {
                        return Err(format!(
                            "selection snapshot `{}` record for `{}` does not match the bound member",
                            data.name.as_str(),
                            record.member_identity
                        ));
                    }
                    match &record.choice {
                        SelectionChoice::Excluded => {
                            if snapshot.coverage == SelectionCoverage::Complete {
                                return Err(format!(
                                    "selection snapshot `{}` excludes member `{}` under a complete-coverage contract",
                                    data.name.as_str(),
                                    record.member_identity
                                ));
                            }
                        }
                        SelectionChoice::Conformance {
                            conformance_identity,
                        } => check_conformance_refines(
                            typed,
                            conformance_identity,
                            &requirement,
                            member,
                        )?,
                        SelectionChoice::Machine { machine_identity } => {
                            check_machine_refines(typed, machine_identity, &requirement, member)?
                        }
                    }
                }
                if snapshot.revision != snapshot_revision(snapshot) {
                    return Err(format!(
                        "selection snapshot `{}` report fingerprint does not match its frozen contents",
                        data.name.as_str()
                    ));
                }
                Ok(())
            }

            pub(crate) fn snapshot_revision(snapshot: &SelectionSnapshot) -> u64 {
                use typed_trees::reflection::fingerprint::{
                    byte, bytes, optional_text, query_authority, text, uint,
                };

                let mut hash = typed_trees::reflection::fingerprint::OFFSET_BASIS;
                bytes(&mut hash, b"omega.reflect.selection.v1");
                text(&mut hash, &snapshot.subject_owner_identity);
                optional_text(&mut hash, &snapshot.subject_application_identity);
                query_authority(&mut hash, &snapshot.authority);
                match &snapshot.projection {
                    SelectionProjection::DeclaredMembers => byte(&mut hash, 0),
                    SelectionProjection::RuntimeMembers => byte(&mut hash, 1),
                    SelectionProjection::Members(identities) => {
                        byte(&mut hash, 2);
                        uint(&mut hash, identities.len() as u64);
                        for identity in identities {
                            text(&mut hash, identity);
                        }
                    }
                }
                byte(
                    &mut hash,
                    match snapshot.coverage {
                        SelectionCoverage::Complete => 0,
                        SelectionCoverage::Partial => 1,
                    },
                );
                text(&mut hash, &snapshot.requirement.trait_identity);
                optional_text(&mut hash, &snapshot.requirement.requirement_identity);
                match &snapshot.requirement.trait_application {
                    Some(application) => {
                        byte(&mut hash, 1);
                        uint(&mut hash, application.type_argument_identities.len() as u64);
                        for identity in &application.type_argument_identities {
                            text(&mut hash, identity);
                        }
                        uint(&mut hash, application.lifetime_arguments.len() as u64);
                        for ordinal in &application.lifetime_arguments {
                            uint(&mut hash, u64::from(*ordinal));
                        }
                    }
                    None => byte(&mut hash, 0),
                }
                uint(&mut hash, snapshot.records.len() as u64);
                for record in &snapshot.records {
                    text(&mut hash, &record.member_identity);
                    optional_text(&mut hash, &record.qualified_type);
                    optional_text(&mut hash, &record.owner_case_identity);
                    byte(&mut hash, record.erased as u8);
                    match &record.choice {
                        SelectionChoice::Conformance {
                            conformance_identity,
                        } => {
                            byte(&mut hash, 0);
                            text(&mut hash, conformance_identity);
                        }
                        SelectionChoice::Machine { machine_identity } => {
                            byte(&mut hash, 1);
                            text(&mut hash, machine_identity);
                        }
                        SelectionChoice::Excluded => byte(&mut hash, 2),
                    }
                }
                if hash == 0 { 1 } else { hash }
            }
        }

        pub use scoped_members::{MemberKind, MemberSelectionKey};
        pub(crate) use scoped_members::{ScopedMember, expected_members, project};
        pub use selection_receiver::ScopedSelectionReceiver;
        pub use snapshots::{SelectionSnapshot, replay_selection_snapshot};

        use symbols::SymbolHandle;
        use typed_trees::TypedTrees;

        use typed_trees::reflection::construction::exact_symbol_identity;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum SelectionProjection {
            DeclaredMembers,
            RuntimeMembers,
            Members(Vec<String>),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SelectionCoverage {
            Complete,
            Partial,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct DemandedTraitApplication {
            pub type_argument_identities: Vec<String>,
            pub lifetime_arguments: Vec<u32>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct SelectionRequirement {
            pub trait_identity: String,
            pub requirement_identity: Option<String>,
            pub trait_application: Option<DemandedTraitApplication>,
        }

        pub(crate) fn first_occurrence_normalized(ordinals: &[u32]) -> Vec<u32> {
            let mut seen: Vec<u32> = Vec::with_capacity(ordinals.len());
            ordinals
                .iter()
                .map(
                    |ordinal| match seen.iter().position(|known| known == ordinal) {
                        Some(position) => position as u32,
                        None => {
                            seen.push(*ordinal);
                            (seen.len() - 1) as u32
                        }
                    },
                )
                .collect()
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum SelectionChoice {
            Conformance { conformance_identity: String },
            Machine { machine_identity: String },
            Excluded,
        }

        impl SelectionChoice {
            pub fn conformance(typed: &TypedTrees, symbol: SymbolHandle) -> Result<Self, String> {
                if !typed.conformance_by_symbol(symbol).is_some() {
                    return Err(
                        "a conformance selection names an exact conformance declaration".to_owned(),
                    );
                }
                Ok(Self::Conformance {
                    conformance_identity: exact_symbol_identity(typed, symbol)?.0,
                })
            }

            pub fn machine(typed: &TypedTrees, symbol: SymbolHandle) -> Result<Self, String> {
                if !typed.machine(symbol).is_some() {
                    return Err("a machine selection names an exact machine declaration".to_owned());
                }
                Ok(Self::Machine {
                    machine_identity: exact_symbol_identity(typed, symbol)?.0,
                })
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct SelectionRecord {
            pub member_identity: String,
            pub qualified_type: Option<String>,
            pub owner_case_identity: Option<String>,
            pub erased: bool,
            pub choice: SelectionChoice,
        }
    }
    pub(crate) mod visitation {
        use typed_trees::TypedTrees;

        use super::selection::{
            MemberKind, ScopedMember, SelectionChoice, SelectionRecord, SelectionRequirement,
            SelectionSnapshot, expected_members, project, replay_selection_snapshot,
        };
        use typed_trees::reflection::SchemaQueryAuthority;
        use typed_trees::reflection::construction::resolve_subject;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct FieldInfo {
            member_identity: String,
            name: String,
            kind: MemberKind,
            declaration_position: u32,
            stable_number: Option<u64>,
            qualified_type: Option<String>,
            owner_case_identity: Option<String>,
            carrier_identity: Option<String>,
        }

        impl FieldInfo {
            pub fn member_identity(&self) -> &str {
                &self.member_identity
            }

            pub fn name(&self) -> &str {
                &self.name
            }

            pub fn kind(&self) -> MemberKind {
                self.kind
            }

            pub fn declaration_position(&self) -> u32 {
                self.declaration_position
            }

            pub fn stable_number(&self) -> Option<u64> {
                self.stable_number
            }

            pub fn qualified_type(&self) -> Option<&str> {
                self.qualified_type.as_deref()
            }

            pub fn owner_case_identity(&self) -> Option<&str> {
                self.owner_case_identity.as_deref()
            }

            pub fn carrier_identity(&self) -> Option<&str> {
                self.carrier_identity.as_deref()
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum VisitationOperation {
            Conformance { conformance_identity: String },
            Machine { machine_identity: String },
        }

        impl VisitationOperation {
            pub fn identity(&self) -> &str {
                match self {
                    Self::Conformance {
                        conformance_identity,
                    } => conformance_identity,
                    Self::Machine { machine_identity } => machine_identity,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct VisitationCall {
            field: FieldInfo,
            operation: VisitationOperation,
        }

        impl VisitationCall {
            pub fn field(&self) -> &FieldInfo {
                &self.field
            }

            pub fn operation(&self) -> &VisitationOperation {
                &self.operation
            }
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct RuntimeVisitationPlan {
            subject_owner_identity: String,
            subject_application_identity: Option<String>,
            authority: SchemaQueryAuthority,
            requirement: SelectionRequirement,
            calls: Vec<VisitationCall>,
            revision: u64,
        }

        impl RuntimeVisitationPlan {
            pub fn compose(
                typed: &TypedTrees,
                snapshot: &SelectionSnapshot,
            ) -> Result<Self, String> {
                replay_selection_snapshot(typed, snapshot)?;
                let data = resolve_subject(typed, snapshot.subject_owner_identity())?;
                let members = expected_members(
                    typed,
                    data,
                    snapshot.subject_owner_identity(),
                    snapshot.subject_application_identity(),
                )?;
                let in_scope = project(&members, snapshot.projection())?;
                let calls = expand_calls(&in_scope, snapshot.records())?;
                let mut plan = Self {
                    subject_owner_identity: snapshot.subject_owner_identity().to_owned(),
                    subject_application_identity: snapshot
                        .subject_application_identity()
                        .map(str::to_owned),
                    authority: snapshot.authority().clone(),
                    requirement: snapshot.requirement().clone(),
                    calls,
                    revision: 0,
                };
                plan.revision = plan_revision(&plan);
                Ok(plan)
            }

            pub fn subject_owner_identity(&self) -> &str {
                &self.subject_owner_identity
            }

            pub fn subject_application_identity(&self) -> Option<&str> {
                self.subject_application_identity.as_deref()
            }

            pub fn authority(&self) -> &SchemaQueryAuthority {
                &self.authority
            }

            pub fn requirement(&self) -> &SelectionRequirement {
                &self.requirement
            }

            pub fn calls(&self) -> &[VisitationCall] {
                &self.calls
            }

            pub fn revision(&self) -> u64 {
                self.revision
            }
        }

        fn field_info(member: &ScopedMember) -> FieldInfo {
            FieldInfo {
                member_identity: member.member_identity.clone(),
                name: member.name.clone(),
                kind: member.kind,
                declaration_position: member.declaration_position,
                stable_number: member.stable_number,
                qualified_type: member.qualified_type.clone(),
                owner_case_identity: member.owner_case_identity.clone(),
                carrier_identity: member.head_nominal_identity.clone(),
            }
        }

        fn expand_calls(
            members: &[ScopedMember],
            records: &[SelectionRecord],
        ) -> Result<Vec<VisitationCall>, String> {
            let mut calls = Vec::new();
            for (member, record) in members.iter().zip(records.iter()) {
                let operation = match &record.choice {
                    SelectionChoice::Conformance {
                        conformance_identity,
                    } => VisitationOperation::Conformance {
                        conformance_identity: conformance_identity.clone(),
                    },
                    SelectionChoice::Machine { machine_identity } => VisitationOperation::Machine {
                        machine_identity: machine_identity.clone(),
                    },
                    SelectionChoice::Excluded => continue,
                };
                if member.erased {
                    return Err(format!(
                        "member `{}` is erased and receives no runtime borrow; its selected operation cannot be visited",
                        member.member_identity
                    ));
                }
                calls.push(VisitationCall {
                    field: field_info(member),
                    operation,
                });
            }
            Ok(calls)
        }

        pub fn replay_runtime_visitation_plan(
            typed: &TypedTrees,
            snapshot: &SelectionSnapshot,
            plan: &RuntimeVisitationPlan,
        ) -> Result<(), String> {
            if plan.subject_owner_identity != snapshot.subject_owner_identity()
                || plan.subject_application_identity.as_deref()
                    != snapshot.subject_application_identity()
                || plan.authority != *snapshot.authority()
                || plan.requirement != *snapshot.requirement()
            {
                return Err(
                    "visitation plan header does not match the snapshot it claims to expand"
                        .to_owned(),
                );
            }
            replay_selection_snapshot(typed, snapshot)?;
            let data = resolve_subject(typed, snapshot.subject_owner_identity())?;
            let members = expected_members(
                typed,
                data,
                snapshot.subject_owner_identity(),
                snapshot.subject_application_identity(),
            )?;
            let in_scope = project(&members, snapshot.projection()).map_err(|error| {
        format!(
            "visitation plan projection does not resolve against the bound declaration: {error}"
        )
    })?;
            let expected = expand_calls(&in_scope, snapshot.records())?;
            if plan.calls != expected {
                return Err(format!(
                    "visitation plan for `{}` does not match the snapshot's checked call expansion",
                    data.name.as_str()
                ));
            }
            if plan.revision != plan_revision(plan) {
                return Err(format!(
                    "visitation plan for `{}` report fingerprint does not match its frozen contents",
                    data.name.as_str()
                ));
            }
            Ok(())
        }

        fn plan_revision(plan: &RuntimeVisitationPlan) -> u64 {
            use typed_trees::reflection::fingerprint::{
                byte, bytes, optional_text, optional_uint, query_authority, text, uint,
            };

            let mut hash = typed_trees::reflection::fingerprint::OFFSET_BASIS;
            bytes(&mut hash, b"omega.reflect.visitation.v1");
            text(&mut hash, &plan.subject_owner_identity);
            optional_text(&mut hash, &plan.subject_application_identity);
            query_authority(&mut hash, &plan.authority);
            text(&mut hash, &plan.requirement.trait_identity);
            optional_text(&mut hash, &plan.requirement.requirement_identity);
            uint(&mut hash, plan.calls.len() as u64);
            for call in &plan.calls {
                let field = &call.field;
                text(&mut hash, &field.member_identity);
                text(&mut hash, &field.name);
                byte(
                    &mut hash,
                    match field.kind {
                        MemberKind::Field => 0,
                        MemberKind::Case => 1,
                        MemberKind::PayloadField => 2,
                    },
                );
                uint(&mut hash, u64::from(field.declaration_position));
                optional_uint(&mut hash, field.stable_number);
                optional_text(&mut hash, &field.qualified_type);
                optional_text(&mut hash, &field.owner_case_identity);
                optional_text(&mut hash, &field.carrier_identity);
                match &call.operation {
                    VisitationOperation::Conformance {
                        conformance_identity,
                    } => {
                        byte(&mut hash, 0);
                        text(&mut hash, conformance_identity);
                    }
                    VisitationOperation::Machine { machine_identity } => {
                        byte(&mut hash, 1);
                        text(&mut hash, machine_identity);
                    }
                }
            }
            if hash == 0 { 1 } else { hash }
        }
    }

    pub use schema_graph::replay_semantic_schema_graph;
    pub use selection::{
        MemberKind, MemberSelectionKey, ScopedSelectionReceiver, SelectionChoice,
        SelectionCoverage, SelectionProjection, SelectionRecord, SelectionRequirement,
        SelectionSnapshot, replay_selection_snapshot,
    };
    pub use visitation::{
        FieldInfo, RuntimeVisitationPlan, VisitationCall, VisitationOperation,
        replay_runtime_visitation_plan,
    };
}
