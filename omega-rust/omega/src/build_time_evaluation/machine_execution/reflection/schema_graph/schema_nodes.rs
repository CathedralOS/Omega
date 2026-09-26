//! Schema node handles, nodes, descriptions, shapes and query authorities.

use crate::build_time_evaluation::machine_execution::reflection::schema_graph::graph_construction::exact_symbol_identity;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbols::SymbolHandle;

/// A node handle indexes only its owning graph. Index 0 is the invalid
/// sentinel, so a forged or dangling handle resolves to a dummy node instead
/// of aliasing compiler storage or another node (ZII).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaNodeHandle(pub(crate) u32);

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
