//! Replaying a semantic schema graph and checking authority claims.

use crate::build_time_evaluation::machine_execution::reflection::schema_graph::graph_construction::{
    NominalExpectation, describe_field, describe_type_parameter, exact_symbol_identity,
    resolve_subject, sorted,
};
use crate::build_time_evaluation::machine_execution::reflection::schema_graph::{
    CaseDescription, DeclarationDescription, FieldDescription, SchemaNode, SchemaNodeHandle,
    SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::data::{
    DataDefinition, DataField, DataMember, DataShapeKind,
};
use symbols::SymbolHandle;

/// Independently check a frozen schema graph against the typed trees it
/// claims to describe.
///
/// Replay re-resolves the bound declaration from the recorded owner identity,
/// then recomputes every member identity, qualified type, case ownership,
/// nominal edge, and retired-identity correspondence from `typed` instead of
/// trusting the producer's walk. Structural forgeries (dropped, extra,
/// mis-owned, or mis-typed members; stale stable numbers; tampered revision;
/// an unsatisfiable foreign-scope claim) reject with the exact disagreement.
pub fn replay_semantic_schema_graph(
    typed: &TypedTrees,
    graph: &SemanticSchemaGraph,
) -> Result<(), String> {
    let SchemaNode::Declaration(root) = graph.node(graph.root) else {
        return Err("schema graph root is not a declaration node".to_owned());
    };
    let data = resolve_subject(typed, &root.owner_identity)?;

    check_authority_claim(typed, data, &graph.authority, "schema", &root.name)?;
    check_subject_application(
        typed,
        data,
        &root.application_identity,
        "schema",
        &root.name,
    )?;

    // `resolve_subject` bound the declaration by `owner_identity`, but the
    // stored presentation facts beside it are still producer claims: the
    // identity-strength flag is how consumers read cross-artifact trust, and
    // the display name is what diagnostics and policy keys present. Replay
    // recomputes both rather than trusting them.
    let (_, expected_hermetic) = exact_symbol_identity(typed, data.symbol)?;
    if root.identity_is_hermetic != expected_hermetic || root.name != data.name.as_str() {
        return Err(format!(
            "schema `{}` name or identity strength does not match the bound declaration",
            root.name
        ));
    }

    let expected_shape = match DataDefinition::shape_kind_from_members(typed.data_members(data)) {
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

    // Member correspondence: recompute every member description in authored
    // order and compare to the stored nodes. This is the check that rejects
    // missing, extra, mis-typed, and mis-owned members.
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
    for (position, (member, handle)) in members.iter().zip(root.members.iter()).enumerate() {
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
                // Edge handles are checked by `check_nominal_edges`; every
                // other member fact must match exactly.
                expected_field.nominal_references = stored.nominal_references.clone();
                if *stored != expected_field {
                    return Err(format!(
                        "schema `{}` member `{name}` does not match its bound field",
                        root.name,
                        name = field.name
                    ));
                }
                check_nominal_edges(graph, root, stored, &expectations, &mut expected_nominals)?;
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
                    retired_payload_identities: sorted(variant.retired_payload_identities.clone()),
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
                    let SchemaNode::Field(stored_field) = graph.node(*payload_handle) else {
                        return Err(format!(
                            "schema `{}` case `{}` payload {payload_position} is not a field node",
                            root.name, variant.name
                        ));
                    };
                    expected_field.nominal_references = stored_field.nominal_references.clone();
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

    // Every stored member node must be reachable exactly once: payload fields
    // belong to their case, record fields to the root, and no orphan member
    // node may exist outside both lists.
    let mut reachable = vec![false; graph.nodes.len()];
    reachable[graph.root.index()] = true;
    for handle in &root.members {
        reachable[handle.index()] = true;
        if let SchemaNode::Case(case) = graph.node(*handle) {
            for payload in &case.payload_fields {
                reachable[payload.index()] = true;
            }
        }
    }
    // Edge targets are member-reachable too: nominal reference nodes sit
    // outside every member list and are reached only through field edges.
    // `check_nominal_edges` has already proven each stored edge handle lands
    // on a `NominalReference` node, so a referenced index is never a stray
    // member node.
    let mut referenced_nominals = vec![false; graph.nodes.len()];
    for node in graph.nodes.iter() {
        if let SchemaNode::Field(field) = node {
            for reference in &field.nominal_references {
                if !reference.is_valid() || reference.index() >= graph.nodes.len() {
                    return Err(format!(
                        "schema `{}` field `{}` references a node outside the owned graph",
                        root.name, field.name
                    ));
                }
                referenced_nominals[reference.index()] = true;
            }
        }
    }
    for (index, _) in graph.nodes.iter().enumerate() {
        if index > 1 && !reachable[index] && !referenced_nominals[index] {
            return Err(format!(
                "schema `{}` contains an orphan member node `{index}` reachable through no member list",
                root.name
            ));
        }
    }
    // Nominal-reference nodes must be exactly the interned expectation set:
    // deduplicated by (owner, application) and referenced by at least one
    // member.
    let mut stored_nominals: Vec<(String, String)> = Vec::new();
    for (index, node) in graph.nodes.iter().enumerate() {
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
    for key in expected_nominals {
        if !expected_unique.contains(&key) {
            expected_unique.push(key);
        }
    }
    if stored_nominals != expected_unique {
        return Err(format!(
            "schema `{}` nominal references do not match the bound member types",
            root.name
        ));
    }

    if graph.revision != graph_revision(&graph.authority, &graph.nodes) {
        return Err(format!(
            "schema `{}` report fingerprint does not match its frozen contents",
            root.name
        ));
    }
    Ok(())
}

/// Re-check the authority claim frozen beside a schema graph or a selection
/// snapshot. Authority is a claim stored beside the content; replay re-checks
/// the parts the current program can disprove. `artifact` and `subject_name`
/// name the failing artifact in diagnostics ("schema"/"selection snapshot").
pub(crate) fn check_authority_claim(
    typed: &TypedTrees,
    data: &DataDefinition,
    authority: &SchemaQueryAuthority,
    artifact: &str,
    subject_name: &str,
) -> Result<(), String> {
    match authority {
        SchemaQueryAuthority::ForeignScope { .. } => {
            let visible = symbol_resolved_trees_to_typed_trees::typed_trees::visibility::declaration_visibility(typed, data.symbol)
                .is_some_and(|visibility| visibility.is_public());
            if !visible {
                return Err(format!(
                    "{artifact} `{subject_name}` claims a foreign-scope query over a non-public subject; complete structural visibility is impossible",
                ));
            }
        }
        SchemaQueryAuthority::OwningScope { requester_identity } => {
            // A resolvable requester in a different package proves the owning
            // claim was forged; an absent requester leaves the claim
            // undisprovable at this layer (query elaboration owns admission).
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

/// Re-check a recorded selected-application identity against the bound
/// declaration's generated instance: a generated instance must record its
/// exact application, and a declared form must record none.
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
                    .package_qualified_type_identity(instance)
                    .into_string()
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

/// Recompute the expected field node contents (without edge handles) plus the
/// nominal edges its type mentions.
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
    // The stored graph owns edge handles; the expectation compares contents.
    description.nominal_references.clear();
    Ok((description, expectations))
}

/// Check one field's stored edge handles against its expected nominal edges:
/// self-edges must land on the root, other edges must resolve to a
/// `NominalReference` node whose contents match the expectation.
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
    for (handle, expectation) in stored.nominal_references.iter().zip(expectations.iter()) {
        if expectation.is_self_edge {
            if *handle != graph.root {
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

/// Report-only FNV-1a fingerprint over the frozen graph contents. The exact
/// replay above is the authority; this value is a cheap staleness gate and
/// diagnostic coordinate, never identity (matching the repository's
/// fingerprint rule).
pub(crate) fn graph_revision(authority: &SchemaQueryAuthority, nodes: &[SchemaNode]) -> u64 {
    fn byte(hash: &mut u64, value: u8) {
        *hash ^= u64::from(value);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    fn bytes(hash: &mut u64, value: &[u8]) {
        for value in value {
            byte(hash, *value);
        }
    }
    fn uint(hash: &mut u64, value: u64) {
        bytes(hash, &value.to_le_bytes());
    }
    fn text(hash: &mut u64, value: &str) {
        uint(hash, value.len() as u64);
        bytes(hash, value.as_bytes());
    }
    fn optional_text(hash: &mut u64, value: &Option<String>) {
        match value {
            Some(value) => {
                byte(hash, 1);
                text(hash, value);
            }
            None => byte(hash, 0),
        }
    }
    fn handle(hash: &mut u64, value: SchemaNodeHandle) {
        uint(hash, value.0 as u64);
    }
    fn handles(hash: &mut u64, values: &[SchemaNodeHandle]) {
        uint(hash, values.len() as u64);
        for value in values {
            handle(hash, *value);
        }
    }
    fn identities(hash: &mut u64, values: &[u64]) {
        uint(hash, values.len() as u64);
        for value in values {
            uint(hash, *value);
        }
    }

    let mut hash = 0xcbf29ce484222325u64;
    bytes(&mut hash, b"omega.reflect.schema.v1");
    match authority {
        SchemaQueryAuthority::OwningScope { requester_identity } => {
            byte(&mut hash, 0);
            text(&mut hash, requester_identity);
        }
        SchemaQueryAuthority::ForeignScope { requester_identity } => {
            byte(&mut hash, 1);
            text(&mut hash, requester_identity);
        }
    }
    uint(&mut hash, nodes.len() as u64);
    for node in nodes {
        match node {
            SchemaNode::Invalid => byte(&mut hash, 0),
            SchemaNode::Declaration(declaration) => {
                byte(&mut hash, 1);
                text(&mut hash, &declaration.owner_identity);
                byte(&mut hash, declaration.identity_is_hermetic as u8);
                text(&mut hash, &declaration.name);
                byte(
                    &mut hash,
                    match declaration.shape {
                        SchemaShape::Empty => 0,
                        SchemaShape::Record => 1,
                        SchemaShape::Sum => 2,
                        SchemaShape::Mixed => 3,
                    },
                );
                text(&mut hash, &declaration.supply);
                text(&mut hash, &declaration.multiplicity);
                optional_text(&mut hash, &declaration.carry);
                uint(&mut hash, declaration.type_parameters.len() as u64);
                for parameter in &declaration.type_parameters {
                    text(&mut hash, parameter.name.as_str());
                    text(&mut hash, parameter.kind);
                    optional_text(&mut hash, &parameter.type_identity);
                    text(&mut hash, &parameter.bounds);
                }
                uint(&mut hash, declaration.lifetime_parameters.len() as u64);
                for name in &declaration.lifetime_parameters {
                    text(&mut hash, name);
                }
                optional_text(&mut hash, &declaration.application_identity);
                handles(&mut hash, &declaration.members);
                identities(&mut hash, &declaration.retired_identities);
            }
            SchemaNode::Field(field) => {
                byte(&mut hash, 2);
                text(&mut hash, &field.member_identity);
                text(&mut hash, &field.name);
                uint(&mut hash, u64::from(field.declaration_position));
                match field.stable_number {
                    Some(number) => {
                        byte(&mut hash, 1);
                        uint(&mut hash, number);
                    }
                    None => byte(&mut hash, 0),
                }
                byte(&mut hash, field.erased as u8);
                text(&mut hash, &field.qualified_type);
                handles(&mut hash, &field.nominal_references);
                handle(&mut hash, field.owner_case);
            }
            SchemaNode::Case(case) => {
                byte(&mut hash, 3);
                text(&mut hash, &case.member_identity);
                text(&mut hash, &case.name);
                uint(&mut hash, u64::from(case.declaration_position));
                match case.stable_number {
                    Some(number) => {
                        byte(&mut hash, 1);
                        uint(&mut hash, number);
                    }
                    None => byte(&mut hash, 0),
                }
                handles(&mut hash, &case.payload_fields);
                identities(&mut hash, &case.retired_payload_identities);
            }
            SchemaNode::NominalReference(reference) => {
                byte(&mut hash, 4);
                text(&mut hash, &reference.owner_identity);
                text(&mut hash, &reference.owner_kind);
                text(&mut hash, &reference.name);
                text(&mut hash, &reference.application_identity);
            }
        }
    }
    if hash == 0 { 1 } else { hash }
}
