//! Constructing the semantic schema graph from nominal expectations.

use crate::build_time_evaluation::machine_execution::reflection::schema_graph::graph_replay::graph_revision;
use crate::build_time_evaluation::machine_execution::reflection::schema_graph::{
    CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
    SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
    TypeParameterDescription,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::data::{
    DataDefinition, DataField, DataMember, DataShapeKind, TypeParameter, TypeParameterKind,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    TypeReferenceHandle, TypeReferenceNode,
};
use symbols::{SymbolHandle, SymbolKind};

/// Exact identity of one symbol independent of compiler storage: the hermetic
/// package/toolchain identity when provenance exists, else the qualified
/// display path of focused source-free trees. Both producer and replay derive
/// through this one normalizer so the fallback cannot drift.
pub(crate) fn exact_symbol_identity(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<(String, bool), String> {
    if !symbol.is_valid() {
        return Err("a schema identity requires a resolved declaration".to_owned());
    }
    if let Ok(identity) = typed.normalized_hermetic_symbol_identity(symbol) {
        return Ok((identity, true));
    }
    let path = typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err("a schema identity requires a named declaration".to_owned());
    }
    Ok((path, false))
}

/// The expected content of one nominal-reference edge, before handles exist.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NominalExpectation {
    pub(crate) owner_identity: String,
    pub(super) owner_kind: String,
    pub(super) name: String,
    pub(super) application_identity: String,
    /// The member type references the selected application itself: the edge
    /// must reuse the root declaration handle (the pending-derivation
    /// identity), never a fresh node.
    pub(super) is_self_edge: bool,
}

fn nominal_expectations(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    root_owner_identity: &str,
    root_application_identity: Option<&str>,
) -> Result<Vec<NominalExpectation>, String> {
    let mut found = Vec::new();
    collect_nominal_expectations(
        typed,
        type_reference,
        root_owner_identity,
        root_application_identity,
        &mut found,
    )?;
    Ok(found)
}

fn collect_nominal_expectations(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    root_owner_identity: &str,
    root_application_identity: Option<&str>,
    found: &mut Vec<NominalExpectation>,
) -> Result<(), String> {
    let mut intern = |symbol: SymbolHandle,
                      name: &str,
                      application: TypeReferenceHandle,
                      bare_named: bool|
     -> Result<(), String> {
        let (owner_identity, _) = exact_symbol_identity(typed, symbol)?;
        let application_identity = typed
            .package_qualified_type_identity(application)
            .into_string();
        let is_self_edge = owner_identity == root_owner_identity
            && match root_application_identity {
                Some(root_application) => application_identity == root_application,
                // A bare nominal reference to a non-generic subject is the
                // subject itself; a generic application inside an open
                // template is a distinct edge, not the root.
                None => bare_named,
            };
        found.push(NominalExpectation {
            owner_identity,
            owner_kind: format!("{:?}", typed.symbols.get(symbol).kind),
            name: name.to_owned(),
            application_identity,
            is_self_edge,
        });
        Ok(())
    };

    match typed.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            intern(*symbol, name.as_str(), type_reference, true)?;
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            intern(*base_symbol, base_name.as_str(), type_reference, false)?;
            for argument in typed
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_nominal_expectations(
                    typed,
                    *argument,
                    root_owner_identity,
                    root_application_identity,
                    found,
                )?;
            }
        }
        TypeReferenceNode::DynamicTrait { symbol, name, .. } => {
            intern(*symbol, name.as_str(), type_reference, false)?;
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            collect_nominal_expectations(
                typed,
                *base_type,
                root_owner_identity,
                root_application_identity,
                found,
            )?;
        }
        TypeReferenceNode::Reference { referee, .. } => {
            collect_nominal_expectations(
                typed,
                *referee,
                root_owner_identity,
                root_application_identity,
                found,
            )?;
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_nominal_expectations(
                typed,
                *element_type,
                root_owner_identity,
                root_application_identity,
                found,
            )?;
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {}
    }
    Ok(())
}

/// Compute the expected description of one member field plus the nominal
/// edges its declared type mentions. Producer and replay share this
/// descriptor so the two sides cannot drift on what a member means; the
/// producer then interns edges into nodes while replay compares
/// expectations to stored nodes.
pub(crate) fn describe_field(
    typed: &TypedTrees,
    field: &DataField,
    owner_identity: &str,
    declaration_position: u32,
    owner_case: SchemaNodeHandle,
    root_application_identity: Option<&str>,
) -> Result<(FieldDescription, Vec<NominalExpectation>), String> {
    let member_identity = if field.symbol.is_valid() {
        exact_symbol_identity(typed, field.symbol)?.0
    } else {
        // A generated member may carry no resolved symbol: owner plus member
        // name is still an exact key within one declaration.
        format!("{owner_identity}::{}", field.name.as_str())
    };
    let expectations = nominal_expectations(
        typed,
        field.type_reference,
        owner_identity,
        root_application_identity,
    )?;
    Ok((
        FieldDescription {
            member_identity,
            name: field.name.as_str().to_owned(),
            declaration_position,
            stable_number: field.identity,
            erased: field.relevance.is_erased(),
            qualified_type: typed
                .package_qualified_type_identity(field.type_reference)
                .into_string(),
            nominal_references: Vec::new(),
            owner_case,
        },
        expectations,
    ))
}

pub(crate) fn describe_type_parameter(
    typed: &TypedTrees,
    parameter: &TypeParameter,
) -> TypeParameterDescription {
    let (kind, type_reference) = match &parameter.kind {
        TypeParameterKind::Type => ("type", None),
        TypeParameterKind::Const { type_reference } => ("const", Some(*type_reference)),
        TypeParameterKind::Value { type_reference } => ("value", Some(*type_reference)),
        TypeParameterKind::Machine { .. } => ("machine", None),
        TypeParameterKind::Proposition { .. } => ("proposition", None),
    };
    TypeParameterDescription {
        name: parameter.name.as_str().to_owned(),
        kind,
        type_identity: type_reference.map(|reference| {
            typed
                .package_qualified_type_identity(reference)
                .into_string()
        }),
        bounds: format!("{:?}", parameter.bounds),
    }
}

/// Locate the data definition a description binds, by exact owner identity.
/// Missing or ambiguous resolution rejects: a schema graph must bind exactly
/// one declaration in the checking program.
pub(crate) fn resolve_subject<'program>(
    typed: &'program TypedTrees,
    owner_identity: &str,
) -> Result<&'program DataDefinition, String> {
    let mut matches = typed.data_definitions().iter().filter(|data| {
        exact_symbol_identity(typed, data.symbol)
            .is_ok_and(|(identity, _)| identity == owner_identity)
    });
    let Some(data) = matches.next() else {
        return Err(format!(
            "schema owner `{owner_identity}` does not resolve to a data declaration in this program"
        ));
    };
    if matches.next().is_some() {
        return Err(format!(
            "schema owner `{owner_identity}` resolves to more than one data declaration"
        ));
    }
    Ok(data)
}

/// Construct the owned schema graph for one selected data declaration under
/// an explicit authorized scope.
///
/// `subject` is the exact resolved data declaration symbol — for
/// `schema<Box<u32>>` the generated concrete instance definition, not the
/// open template. Type-application resolution belongs to the query
/// elaboration slice.
pub fn construct_semantic_schema_graph(
    typed: &TypedTrees,
    subject: SymbolHandle,
    authority: SchemaQueryAuthority,
) -> Result<SemanticSchemaGraph, String> {
    if !subject.is_valid() || typed.symbols.get(subject).kind != SymbolKind::Data {
        return Err(
            "a semantic schema query selects an exact data declaration, not another symbol kind"
                .to_owned(),
        );
    }
    let data = typed
        .data_definitions()
        .iter()
        .find(|data| data.symbol == subject)
        .ok_or_else(|| {
            format!(
                "schema subject `{}` has no retained data definition",
                typed.symbols.display_path(subject, "::")
            )
        })?;
    if data.quotient.is_some() {
        return Err(format!(
            "schema reflection cannot observe quotient `{}`: a quotient does not expose its representative outside its checked quotient contract",
            data.name.as_str()
        ));
    }
    let (owner_identity, identity_is_hermetic) = exact_symbol_identity(typed, subject)?;
    if let SchemaQueryAuthority::ForeignScope { .. } = &authority {
        let visible =
            symbol_resolved_trees_to_typed_trees::typed_trees::visibility::declaration_visibility(
                typed, subject,
            )
            .is_some_and(|visibility| visibility.is_public());
        if !visible {
            return Err(format!(
                "schema subject `{}` is not public to a foreign package; a query may not hide inaccessible members while claiming complete coverage",
                data.name.as_str()
            ));
        }
    }
    let application_identity = data.generic_instance.map(|instance| {
        typed
            .package_qualified_type_identity(instance)
            .into_string()
    });

    let mut nodes = vec![SchemaNode::Invalid, SchemaNode::Invalid];
    let root = SchemaNodeHandle(1);
    // (owner_identity, application_identity) -> interned edge node. Linear
    // dedup keeps construction deterministic without a map keyed by
    // compiler-owned data.
    let mut interned: Vec<(String, String, SchemaNodeHandle)> = Vec::new();
    let mut member_handles = Vec::new();
    for (position, member) in typed.data_members(data).iter().enumerate() {
        match member {
            DataMember::Field(field) => {
                let handle = push_field_node(
                    typed,
                    field,
                    &owner_identity,
                    position as u32,
                    SchemaNodeHandle::INVALID,
                    application_identity.as_deref(),
                    root,
                    &mut interned,
                    &mut nodes,
                )?;
                member_handles.push(handle);
            }
            DataMember::Variant(variant) => {
                let case_handle = nodes.len() as u32;
                nodes.push(SchemaNode::Invalid);
                let case_handle = SchemaNodeHandle(case_handle);
                let mut payload_handles = Vec::new();
                for (payload_position, payload_field) in
                    typed.data_payload_fields(variant).iter().enumerate()
                {
                    payload_handles.push(push_field_node(
                        typed,
                        payload_field,
                        &owner_identity,
                        payload_position as u32,
                        case_handle,
                        application_identity.as_deref(),
                        root,
                        &mut interned,
                        &mut nodes,
                    )?);
                }
                let (member_identity, _) = exact_symbol_identity(typed, variant.symbol)
                    .unwrap_or_else(|_| {
                        (
                            format!("{owner_identity}::{}", variant.name.as_str()),
                            false,
                        )
                    });
                nodes[case_handle.index()] = SchemaNode::Case(CaseDescription {
                    member_identity,
                    name: variant.name.as_str().to_owned(),
                    declaration_position: position as u32,
                    stable_number: variant.identity,
                    payload_fields: payload_handles,
                    retired_payload_identities: sorted(variant.retired_payload_identities.clone()),
                });
                member_handles.push(case_handle);
            }
        }
    }

    nodes[root.index()] = SchemaNode::Declaration(DeclarationDescription {
        owner_identity,
        identity_is_hermetic,
        name: data.name.as_str().to_owned(),
        shape: match DataDefinition::shape_kind_from_members(typed.data_members(data)) {
            DataShapeKind::Empty => SchemaShape::Empty,
            DataShapeKind::Record => SchemaShape::Record,
            DataShapeKind::Enum => SchemaShape::Sum,
            DataShapeKind::Mixed => SchemaShape::Mixed,
        },
        supply: format!("{:?}", data.supply_mode),
        multiplicity: format!("{:?}", data.properties.multiplicity),
        carry: data.properties.carry.map(|carry| format!("{carry:?}")),
        type_parameters: typed
            .data_type_parameters(data)
            .iter()
            .map(|parameter| describe_type_parameter(typed, parameter))
            .collect(),
        lifetime_parameters: data
            .lifetime_parameters
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
        application_identity,
        members: member_handles,
        retired_identities: sorted(data.retired_identities.clone()),
    });
    let revision = graph_revision(&authority, &nodes);
    Ok(SemanticSchemaGraph {
        nodes,
        root,
        authority,
        revision,
    })
}

#[allow(clippy::too_many_arguments)]
fn push_field_node(
    typed: &TypedTrees,
    field: &DataField,
    owner_identity: &str,
    declaration_position: u32,
    owner_case: SchemaNodeHandle,
    root_application_identity: Option<&str>,
    root: SchemaNodeHandle,
    interned: &mut Vec<(String, String, SchemaNodeHandle)>,
    nodes: &mut Vec<SchemaNode>,
) -> Result<SchemaNodeHandle, String> {
    let (mut description, expectations) = describe_field(
        typed,
        field,
        owner_identity,
        declaration_position,
        owner_case,
        root_application_identity,
    )?;
    description.nominal_references = expectations
        .iter()
        .map(|expectation| {
            if expectation.is_self_edge {
                return Ok(root);
            }
            if let Some((_, _, handle)) = interned.iter().find(|(owner, application, _)| {
                owner == &expectation.owner_identity
                    && application == &expectation.application_identity
            }) {
                return Ok(*handle);
            }
            let handle = SchemaNodeHandle(nodes.len() as u32);
            nodes.push(SchemaNode::NominalReference(NominalReferenceDescription {
                owner_identity: expectation.owner_identity.clone(),
                owner_kind: expectation.owner_kind.clone(),
                name: expectation.name.clone(),
                application_identity: expectation.application_identity.clone(),
            }));
            interned.push((
                expectation.owner_identity.clone(),
                expectation.application_identity.clone(),
                handle,
            ));
            Ok(handle)
        })
        .collect::<Result<_, String>>()?;
    let handle = SchemaNodeHandle(nodes.len() as u32);
    nodes.push(SchemaNode::Field(description));
    Ok(handle)
}

pub(crate) fn sorted(mut identities: Vec<u64>) -> Vec<u64> {
    identities.sort_unstable();
    identities
}
