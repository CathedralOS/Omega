//! Owned qualified schema-graph construction and independent replay.
//!
//! A [`SemanticSchemaGraph`] is the frozen, storage-independent description of
//! one selected data application: a root declaration node plus member nodes
//! for every field and case in authored declaration order, nominal-reference
//! edges for the type applications member types mention, and retired stable
//! numbers kept as descriptive history. Construction requires an explicit
//! [`SchemaQueryAuthority`]; replay re-derives every correspondence from the
//! typed trees so a forged or stale graph cannot stand in for the bound
//! declaration.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::{
    DataDefinition, DataField, DataMember, DataShapeKind, TypeParameter, TypeParameterKind,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// A node handle indexes only its owning graph. Index 0 is the invalid
/// sentinel, so a forged or dangling handle resolves to a dummy node instead
/// of aliasing compiler storage or another node (ZII).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaNodeHandle(u32);

impl SchemaNodeHandle {
    pub const INVALID: Self = Self(0);

    pub const fn invalid() -> Self {
        Self::INVALID
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One node of an owned schema graph.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaNode {
    /// Index-0 dummy: invalid handles land here.
    Invalid,
    Declaration(DeclarationDescription),
    Field(FieldDescription),
    Case(CaseDescription),
    /// A nominal type application a member type mentions. The edge records
    /// the referenced declaration and the exact application (its
    /// substitutions); recursive descent is a separate explicitly selected
    /// operation, never an implicit expansion of the target.
    NominalReference(NominalReferenceDescription),
}

/// The root node: one exact data declaration or concrete generic instance.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclarationDescription {
    /// Exact owner identity independent of compiler storage: the hermetic
    /// `package:<digest>::path`/`toolchain::path` identity when provenance
    /// exists, the fully qualified display path of focused source-free trees
    /// otherwise.
    pub owner_identity: String,
    /// Whether `owner_identity` is a hermetic package/toolchain identity.
    /// Consumers keying cross-artifact trust on identity strength read this
    /// flag rather than guessing from the string shape.
    pub identity_is_hermetic: bool,
    /// Presentation name; never identity.
    pub name: String,
    pub shape: SchemaShape,
    /// `DataSupplyMode`, `Multiplicity`, and `CarryPolicy` tags qualifying
    /// construction of the described type.
    pub supply: String,
    pub multiplicity: String,
    pub carry: Option<String>,
    pub type_parameters: Vec<TypeParameterDescription>,
    pub lifetime_parameters: Vec<String>,
    /// Qualified identity of the selected application for a generated
    /// concrete generic instance; `None` for the declared form itself.
    pub application_identity: Option<String>,
    /// Member node handles in authored declaration order. Source order and
    /// stable-number order remain distinct.
    pub members: Vec<SchemaNodeHandle>,
    /// Retired stable numbers are descriptive history, not live members.
    pub retired_identities: Vec<u64>,
}

/// One type or lifetime parameter of the root declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParameterDescription {
    pub name: String,
    /// `type` | `const` | `value` | `machine` | `proposition`.
    pub kind: &'static str,
    /// Qualified identity of a `const`/`value` parameter's declared type.
    /// Machine/proposition contract identities join with the typed-selection
    /// slice; this slice records their presence and bounds only.
    pub type_identity: Option<String>,
    /// `DataProperties` bounds authored on the parameter.
    pub bounds: String,
}

/// One field member: a record/common field or a case payload member.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDescription {
    /// Exact member identity: the field's own symbol identity, or
    /// `<owner>::<name>` when a generated member carries no resolved symbol.
    pub member_identity: String,
    /// Presentation name; member identity, not this name, is the durable key.
    pub name: String,
    /// Authored declaration order within its owning scope.
    pub declaration_position: u32,
    /// Optional authored stable number; retired numbers live on the owner.
    pub stable_number: Option<u64>,
    /// Erased members remain fully described; they receive no runtime borrow.
    pub erased: bool,
    /// Complete declared type under canonical package-qualified identity:
    /// domain atoms, arithmetic policy, references, arrays, and nominal
    /// applications normalize to their semantic form.
    pub qualified_type: String,
    /// Nominal applications referenced by the declared type, in discovery
    /// order. A member whose type is the selected application itself reuses
    /// the root declaration handle — recursive references are edges, not
    /// expansion.
    pub nominal_references: Vec<SchemaNodeHandle>,
    /// Owning case for payload members; invalid for record/common fields.
    pub owner_case: SchemaNodeHandle,
}

/// One sum case member.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseDescription {
    pub member_identity: String,
    pub name: String,
    /// Authored declaration order within the shared member sequence, so two
    /// nullary cases remain distinguishable.
    pub declaration_position: u32,
    pub stable_number: Option<u64>,
    /// Payload member handles in declaration order.
    pub payload_fields: Vec<SchemaNodeHandle>,
    /// Retired payload stable numbers are descriptive history.
    pub retired_payload_identities: Vec<u64>,
}

/// A nominal type application referenced by a member type.
#[derive(Debug, Clone, PartialEq)]
pub struct NominalReferenceDescription {
    /// Exact identity of the referenced declaration.
    pub owner_identity: String,
    /// Symbol kind of the referenced declaration (`Data`, `Trait`, ...).
    pub owner_kind: String,
    /// Presentation name; never identity.
    pub name: String,
    /// Qualified identity of the full referenced application, including its
    /// substitutions.
    pub application_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaShape {
    Empty,
    Record,
    Sum,
    Mixed,
}

/// The explicit authorized scope a schema query was elaborated under.
///
/// This is context data supplied by the reflection occurrence's lexical
/// author, not ambient permission: the graph freezes which scope authorized
/// the query so replay can re-check the claim it was produced under.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaQueryAuthority {
    /// The lexical author shares the subject's package (or the program has
    /// no package boundary): complete structural visibility includes
    /// non-public members.
    OwningScope { requester_identity: String },
    /// A foreign package: the subject must be public, because the query may
    /// not hide inaccessible members while claiming complete coverage.
    ForeignScope { requester_identity: String },
}

impl SchemaQueryAuthority {
    /// Derive the scope `requester` holds over `subject` under ordinary
    /// package visibility. Source-free focused trees model one package and
    /// therefore derive the owning scope.
    pub fn for_requester(
        typed: &TypedTrees,
        requester: SymbolHandle,
        subject: SymbolHandle,
    ) -> Result<Self, String> {
        if !requester.is_valid() {
            return Err(
                "a schema query requires a resolved lexical author as its requester".to_owned(),
            );
        }
        let (requester_identity, _) = exact_symbol_identity(typed, requester)?;
        Ok(
            if typed.symbols.same_symbol_source_package(requester, subject) {
                Self::OwningScope { requester_identity }
            } else {
                Self::ForeignScope { requester_identity }
            },
        )
    }
}

/// An owned, frozen semantic schema graph for one selected type application.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticSchemaGraph {
    /// Node 0 is `SchemaNode::Invalid`; `root` is node 1.
    nodes: Vec<SchemaNode>,
    root: SchemaNodeHandle,
    authority: SchemaQueryAuthority,
    /// Report-only FNV revision over the frozen contents. This value indexes
    /// nothing and authorizes nothing: replay reconstructs the exact
    /// correspondence instead of trusting the fingerprint (the repository's
    /// report-only-fingerprint rule).
    revision: u64,
}

impl SemanticSchemaGraph {
    pub fn root(&self) -> SchemaNodeHandle {
        self.root
    }

    pub fn authority(&self) -> &SchemaQueryAuthority {
        &self.authority
    }

    /// Report-only content fingerprint; `replay_semantic_schema_graph` is the
    /// authority, never this value.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Resolve a graph-local handle. Invalid and out-of-range handles borrow
    /// the dummy node so callers cannot alias storage outside the graph.
    pub fn node(&self, handle: SchemaNodeHandle) -> &SchemaNode {
        self.nodes
            .get(handle.index())
            .unwrap_or(&SchemaNode::Invalid)
    }

    /// The root declaration description.
    pub fn declaration(&self) -> &DeclarationDescription {
        let SchemaNode::Declaration(declaration) = self.node(self.root) else {
            unreachable!("the schema root is always a declaration node");
        };
        declaration
    }

    /// Borrowed record/common field descriptions in authored declaration
    /// order — the `schema.fields()` projection. Erased members are included:
    /// declaration inspection describes them even though runtime visitation
    /// will not borrow them.
    pub fn fields(&self) -> impl Iterator<Item = (SchemaNodeHandle, &FieldDescription)> {
        self.declaration()
            .members
            .iter()
            .copied()
            .filter_map(|handle| match self.node(handle) {
                SchemaNode::Field(field) => Some((handle, field)),
                _ => None,
            })
    }

    /// Borrowed case descriptions in authored declaration order — the
    /// `schema.cases()` projection. Case payload members borrow through
    /// `CaseDescription::payload_fields`.
    pub fn cases(&self) -> impl Iterator<Item = (SchemaNodeHandle, &CaseDescription)> {
        self.declaration()
            .members
            .iter()
            .copied()
            .filter_map(|handle| match self.node(handle) {
                SchemaNode::Case(case) => Some((handle, case)),
                _ => None,
            })
    }
}

/// Exact identity of one symbol independent of compiler storage: the hermetic
/// package/toolchain identity when provenance exists, else the qualified
/// display path of focused source-free trees. Both producer and replay derive
/// through this one normalizer so the fallback cannot drift.
pub(super) fn exact_symbol_identity(
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
pub(super) struct NominalExpectation {
    pub(super) owner_identity: String,
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
pub(super) fn describe_field(
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

fn describe_type_parameter(
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
pub(super) fn resolve_subject<'program>(
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
        let visible = typed_trees::visibility::declaration_visibility(typed, subject)
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

fn sorted(mut identities: Vec<u64>) -> Vec<u64> {
    identities.sort_unstable();
    identities
}

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
pub(super) fn check_authority_claim(
    typed: &TypedTrees,
    data: &DataDefinition,
    authority: &SchemaQueryAuthority,
    artifact: &str,
    subject_name: &str,
) -> Result<(), String> {
    match authority {
        SchemaQueryAuthority::ForeignScope { .. } => {
            let visible = typed_trees::visibility::declaration_visibility(typed, data.symbol)
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
pub(super) fn check_subject_application(
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

pub(super) fn member_identity_or_name(
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
fn graph_revision(authority: &SchemaQueryAuthority, nodes: &[SchemaNode]) -> u64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn typed(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn subject(typed: &TypedTrees, name: &str) -> SymbolHandle {
        typed
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
            .unwrap_or_else(|| panic!("data `{name}` exists"))
            .symbol
    }

    fn owning() -> SchemaQueryAuthority {
        SchemaQueryAuthority::OwningScope {
            requester_identity: "test::requester".to_owned(),
        }
    }

    fn foreign() -> SchemaQueryAuthority {
        SchemaQueryAuthority::ForeignScope {
            requester_identity: "pkg::consumer".to_owned(),
        }
    }

    fn field<'graph>(graph: &'graph SemanticSchemaGraph, name: &str) -> &'graph FieldDescription {
        graph
            .fields()
            .find(|(_, field)| field.name == name)
            .unwrap_or_else(|| panic!("field `{name}` exists"))
            .1
    }

    #[test]
    fn record_graph_describes_qualified_members() {
        let typed = typed("data Player { health: u32; speed: f32; }");
        let graph = construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
            .expect("record schema constructs");

        let declaration = graph.declaration();
        assert_eq!(declaration.name, "Player");
        assert_eq!(declaration.shape, SchemaShape::Record);
        assert_eq!(declaration.members.len(), 2);

        let health = field(&graph, "health");
        assert_eq!(health.member_identity, "Player::health");
        assert_eq!(health.declaration_position, 0);
        assert!(!health.erased);
        assert!(health.qualified_type.contains("u32"));
        // `u32` is a nominal domain declaration, so the field carries one
        // interned edge node rather than expanding the domain's members.
        assert_eq!(health.nominal_references.len(), 1);
        let SchemaNode::NominalReference(reference) = graph.node(health.nominal_references[0])
        else {
            panic!("the `u32` edge resolves to a nominal reference node")
        };
        assert_eq!(reference.name, "u32");
        let speed = field(&graph, "speed");
        assert_eq!(speed.declaration_position, 1);
        assert_eq!(speed.nominal_references.len(), 1);

        replay_semantic_schema_graph(&typed, &graph).expect("constructed graph replays");
    }

    #[test]
    fn erased_and_empty_members_remain_described() {
        let typed = typed("data Rec { proof [erased]: i32; tag: u8; } data Empty {}");
        let graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Rec"), owning()).expect("ok");
        let proof = field(&graph, "proof");
        assert!(proof.erased, "semantic schema retains erased members");
        let empty = construct_semantic_schema_graph(&typed, subject(&typed, "Empty"), owning())
            .expect("ok");
        assert_eq!(empty.declaration().shape, SchemaShape::Empty);
        assert!(empty.declaration().members.is_empty());
        replay_semantic_schema_graph(&typed, &graph).expect("replays");
        replay_semantic_schema_graph(&typed, &empty).expect("empty replays");
    }

    #[test]
    fn sum_graph_keeps_nullary_cases_distinct_and_owns_payloads() {
        let typed = typed("data Outcome { case Ready(value: u32); case Empty; case Done; }");
        let graph = construct_semantic_schema_graph(&typed, subject(&typed, "Outcome"), owning())
            .expect("sum schema constructs");
        let cases: Vec<_> = graph.cases().collect();
        assert_eq!(cases.len(), 3);
        assert_eq!(cases[0].1.name, "Ready");
        assert_eq!(cases[1].1.name, "Empty");
        assert_eq!(cases[2].1.name, "Done");
        assert_ne!(
            cases[1].1.member_identity, cases[2].1.member_identity,
            "two nullary cases stay distinguishable"
        );
        assert_eq!(cases[0].1.payload_fields.len(), 1);
        assert!(cases[1].1.payload_fields.is_empty());
        let payload_handle = cases[0].1.payload_fields[0];
        let SchemaNode::Field(payload) = graph.node(payload_handle) else {
            panic!("payload member is a field node")
        };
        assert_eq!(payload.name, "value");
        assert_eq!(payload.owner_case, cases[0].0);
        assert_eq!(payload.declaration_position, 0);
        replay_semantic_schema_graph(&typed, &graph).expect("replays");
    }

    #[test]
    fn recursive_reference_is_an_edge_not_expansion() {
        let typed = typed("data List { case Nil; case Cons(head: u32, tail: List); }");
        let graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "List"), owning()).expect("ok");
        let cases: Vec<_> = graph.cases().collect();
        let cons = cases
            .iter()
            .find(|(_, case)| case.name == "Cons")
            .expect("Cons");
        assert_eq!(cons.1.payload_fields.len(), 2);
        let SchemaNode::Field(tail) = graph.node(cons.1.payload_fields[1]) else {
            panic!("tail is a field")
        };
        assert_eq!(
            tail.nominal_references,
            vec![graph.root()],
            "a member typed as the selected application reuses the pending root identity"
        );
        replay_semantic_schema_graph(&typed, &graph).expect("replays");
    }

    #[test]
    fn foreign_references_deduplicate_into_shared_nodes() {
        let typed =
            typed("data Inner { x: u8; } data Outer { inner: Inner; items: [Inner; 2]; tag: u8; }");
        let graph = construct_semantic_schema_graph(&typed, subject(&typed, "Outer"), owning())
            .expect("ok");
        let inner = field(&graph, "inner");
        let items = field(&graph, "items");
        assert_eq!(inner.nominal_references.len(), 1);
        assert_eq!(
            inner.nominal_references, items.nominal_references,
            "the same application interns one reference node"
        );
        let SchemaNode::NominalReference(reference) = graph.node(inner.nominal_references[0])
        else {
            panic!("nominal edge resolves to a reference node")
        };
        assert_eq!(reference.name, "Inner");
        assert!(reference.owner_identity.contains("Inner"));
        replay_semantic_schema_graph(&typed, &graph).expect("replays");
    }

    #[test]
    fn foreign_scope_requires_a_public_subject() {
        let typed = typed("data Secret { x: u8; } pub data Open { x: u8; }");
        let denied = construct_semantic_schema_graph(&typed, subject(&typed, "Secret"), foreign());
        let error = denied.expect_err("private subject must reject under a foreign scope");
        assert!(error.contains("not public"), "{error}");
        let open = construct_semantic_schema_graph(&typed, subject(&typed, "Open"), foreign())
            .expect("public subject admits a foreign scope");
        replay_semantic_schema_graph(&typed, &open).expect("replays");
    }

    #[test]
    fn quotient_subjects_reject() {
        let typed = typed(
            "data Carrier { case Unit; } proposition same(left: Carrier, right: Carrier) = left == right; data Bucket = Carrier % same;",
        );
        let error = construct_semantic_schema_graph(&typed, subject(&typed, "Bucket"), owning())
            .expect_err("a quotient does not expose its representative");
        assert!(error.contains("quotient"), "{error}");
    }

    #[test]
    fn non_data_subjects_reject() {
        let typed = typed("machine helper() -> u8 { 7 } data Rec { x: u8; }");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "helper")
            .expect("machine")
            .symbol;
        let error = construct_semantic_schema_graph(&typed, machine, owning())
            .expect_err("a machine is not a schema subject");
        assert!(error.contains("data declaration"), "{error}");
    }

    #[test]
    fn replay_rejects_forged_member_identity() {
        let typed = typed("data Player { health: u32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        let handle = graph.declaration().members[0];
        let SchemaNode::Field(stored) = &mut graph.nodes[handle.index()] else {
            panic!("member is a field")
        };
        stored.member_identity = "Player::stamina".to_owned();
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("a forged member identity must reject");
        assert!(error.contains("does not match its bound field"), "{error}");
    }

    #[test]
    fn replay_rejects_wrong_qualified_type() {
        let typed = typed("data Player { health: u32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        let handle = graph.declaration().members[0];
        let SchemaNode::Field(stored) = &mut graph.nodes[handle.index()] else {
            panic!("member is a field")
        };
        stored.qualified_type = "f32".to_owned();
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("a wrong qualified type must reject");
        assert!(error.contains("does not match its bound field"), "{error}");
    }

    #[test]
    fn replay_rejects_dropped_member() {
        let typed = typed("data Player { health: u32; speed: f32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        graph.nodes[graph.root.index()] = {
            let SchemaNode::Declaration(mut root) = graph.node(graph.root()).clone() else {
                panic!("root is a declaration")
            };
            root.members.pop();
            SchemaNode::Declaration(root)
        };
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error =
            replay_semantic_schema_graph(&typed, &graph).expect_err("a dropped member must reject");
        assert!(
            error.contains("top-level members") || error.contains("orphan"),
            "{error}"
        );
    }

    #[test]
    fn replay_rejects_stale_stable_number() {
        let typed = typed("data Rec { #3 x: u8; #7 y: u8; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Rec"), owning()).expect("ok");
        let handle = graph.declaration().members[0];
        let SchemaNode::Field(stored) = &mut graph.nodes[handle.index()] else {
            panic!("member is a field")
        };
        stored.stable_number = Some(9);
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("a stale stable number must reject");
        assert!(error.contains("does not match its bound field"), "{error}");
    }

    #[test]
    fn replay_rejects_misowned_payload() {
        let typed = typed("data Outcome { case Ready(value: u32); case Empty; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Outcome"), owning())
                .expect("ok");
        let empty_case = graph
            .cases()
            .find(|(_, case)| case.name == "Empty")
            .expect("Empty case")
            .0;
        let payload_handle = graph
            .cases()
            .find(|(_, case)| case.name == "Ready")
            .expect("Ready case")
            .1
            .payload_fields[0];
        let SchemaNode::Field(stored) = &mut graph.nodes[payload_handle.index()] else {
            panic!("payload is a field")
        };
        stored.owner_case = empty_case;
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("a mis-owned payload must reject");
        assert!(error.contains("does not match its bound field"), "{error}");
    }

    #[test]
    fn replay_rejects_extra_orphan_node() {
        let typed = typed("data Player { health: u32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        let SchemaNode::Field(orphan) = graph.node(graph.declaration().members[0]).clone() else {
            panic!("member is a field")
        };
        graph.nodes.push(SchemaNode::Field(orphan));
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("an orphan member node must reject");
        assert!(error.contains("orphan"), "{error}");
    }

    #[test]
    fn replay_rejects_tampered_revision() {
        let typed = typed("data Player { health: u32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        graph.revision = graph.revision.wrapping_add(1);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("a tampered revision must reject");
        assert!(error.contains("fingerprint"), "{error}");
    }

    #[test]
    fn replay_rejects_unresolvable_owner() {
        let typed = typed("data Player { health: u32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        let SchemaNode::Declaration(root) = &mut graph.nodes[graph.root.index()] else {
            panic!("root is a declaration")
        };
        root.owner_identity = "Forged::Owner".to_owned();
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error =
            replay_semantic_schema_graph(&typed, &graph).expect_err("an unbound owner must reject");
        assert!(error.contains("does not resolve"), "{error}");
    }

    #[test]
    fn replay_rejects_forged_root_presentation() {
        let typed = typed("data Player { health: u32; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Player"), owning())
                .expect("ok");
        let SchemaNode::Declaration(root) = &mut graph.nodes[graph.root.index()] else {
            panic!("root is a declaration")
        };
        root.name = "Impostor".to_owned();
        root.identity_is_hermetic = !root.identity_is_hermetic;
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("forged root presentation facts must reject");
        assert!(error.contains("name or identity strength"), "{error}");
    }

    #[test]
    fn replay_rejects_foreign_scope_over_private_subject() {
        let typed = typed("data Secret { x: u8; }");
        let mut graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Secret"), owning())
                .expect("owning scope admits a private subject");
        graph.authority = foreign();
        graph.revision = graph_revision(&graph.authority, &graph.nodes);
        let error = replay_semantic_schema_graph(&typed, &graph)
            .expect_err("a foreign claim over a private subject must reject");
        assert!(error.contains("non-public"), "{error}");
    }

    #[test]
    fn requester_derives_owning_scope_in_source_free_trees() {
        let typed = typed("machine helper() -> u8 { 7 } data Rec { x: u8; }");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "helper")
            .expect("machine")
            .symbol;
        let authority =
            SchemaQueryAuthority::for_requester(&typed, machine, subject(&typed, "Rec"))
                .expect("requester resolves");
        assert!(
            matches!(authority, SchemaQueryAuthority::OwningScope { .. }),
            "source-free trees model one package"
        );
        let graph =
            construct_semantic_schema_graph(&typed, subject(&typed, "Rec"), authority).expect("ok");
        replay_semantic_schema_graph(&typed, &graph).expect("replays");
    }
}
