use crate::{Carry, ContractClause, DeclarationId, Multiplicity, Site, TypeId};
use std::num::NonZeroUsize;
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DataId(NonZeroUsize);

impl DataId {
    pub fn get(self) -> usize {
        self.0.get()
    }

    pub fn from_index(index: usize) -> Self {
        Self(NonZeroUsize::new(index + 1).expect("data index belongs to a vector"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataArgument {
    Type(TypeId),
    Integer { carrier: crate::ScalarType, value: num_bigint::BigInt },
}

/// Authority for intact replacement through a write-only loan at this concrete type.
/// Descriptor retirement also owes the resident and incoming loan rosters and
/// child closure at the store. Neither capability grants copy permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOnlyCleanup {
    NoCodeAffine,
    DescriptorCleanup,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LoweredData {
    pub id: DataId,
    pub declaration: DeclarationId,
    pub carrier: TypeId,
    /// Concrete arguments of this schema, including arguments supplied by enclosing data.
    pub type_arguments: Vec<DataArgument>,
    /// Exact owned cleanup selected for this declaration, when one is authored.
    pub cleanup_target: Option<crate::MachineId>,
    pub write_only_cleanup: Option<WriteOnlyCleanup>,
    pub public: bool,
    /// The nominal declaration's multiplicity. A value also owes the obligations
    /// of its record fields or the fields of its active case.
    pub multiplicity: Multiplicity,
    /// Erased lifetime binders in declaration order. No binder adds a layout dimension.
    pub lifetime_count: usize,
    /// Carriers of runtime-capable value indices referenced by the default-domain contract.
    pub value_indices: Vec<TypeId>,
    /// Authored type-wide restrictions. The checker also composes the
    /// applicable fields of the value, including the live case of a sum.
    pub carry: Option<Carry>,
    /// Predicates that every established value of this carrier must satisfy.
    pub requires: Vec<ContractClause>,
    pub fields: Vec<LoweredField>,
    pub cases: Vec<LoweredCase>,
    /// Stable identities of this record’s fields or this sum’s cases.
    pub numbering: Option<std::sync::Arc<crate::SchemaNumbering>>,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LoweredField {
    pub declaration: DeclarationId,
    /// Lifetime of a stored reference, indexed in the enclosing data declaration.
    pub lifetime: Option<usize>,
    /// For a field that stores a record or sum taking lifetime arguments,
    /// directly or as the element of fixed arrays or of an owned sequence: the lifetime binder of the
    /// enclosing declaration that fills each of that data's lifetime binders,
    /// in its binder order. `carrier` erases those arguments, so this list is
    /// the only record of them. Empty for every other field.
    ///
    /// A field may instead refer to such a record through one shared
    /// reference, as `@'t Bytes<'t>` does. Its `carrier` is then the
    /// `NominalReference` row of that record, `lifetime` is the binder the
    /// reference itself is held at, and this list fills the referent's
    /// binders. The two are separate facts: the first indexes the enclosing
    /// declaration, and each position of the second indexes the declaration
    /// the row names. The field stores the reference only. The list states
    /// the referent's declared type; it lends no authority to read the
    /// borrows held behind the reference.
    pub aggregate_lifetimes: Vec<usize>,
    pub carrier: TypeId,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub struct LoweredCase {
    pub declaration: DeclarationId,
    /// The payload has a separate numbering scope from its enclosing sum.
    pub numbering: Option<std::sync::Arc<crate::SchemaNumbering>>,
    pub fields: Vec<LoweredField>,
    pub site: Site,
}

/// A transparent type name and the next edge in its expansion chain.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredAlias {
    pub declaration: DeclarationId,
    pub target: AliasTarget,
    /// Authored assertion checked against the expanded type; never a multiplicity override.
    pub asserted_multiplicity: Option<Multiplicity>,
    pub public: bool,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AliasTarget {
    Declaration(DeclarationId),
    Builtin(String),
    /// An anonymous type expression whose complete shape belongs to the package type table.
    Structural(TypeId),
    /// A generic alias, or a closed application whose expansion has no row in the
    /// package type table. This is a declaration handoff only: it carries no layout,
    /// constructor or ownership identity of its own.
    Expansion(AliasExpansion),
}

/// The checked expansion of a transparent alias. Uses substitute the supplied
/// arguments for `GenericParameter { owner: alias, .. }` by declaration identity;
/// the expansion and the alias spelling then denote the same type.
#[derive(Debug, Eq, PartialEq)]
pub struct AliasExpansion {
    /// Outer scopes supplying the leading arguments, as for data templates.
    pub enclosing_parameters: Vec<DeclarationId>,
    /// The alias's own binders with their checked kinds, in declaration order.
    pub parameters: Vec<TemplateBinder>,
    /// The declaration named by the authored target path, which may itself be an
    /// alias. Provenance only; `expansion` is already fully normalized.
    pub head: Option<DeclarationId>,
    pub expansion: DeclarationTerm,
}

/// A type or value in a declaration application. Static values retain their declared carrier
/// and canonical value. A forwarded binder retains its owner and position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationTerm {
    /// A type with no static value argument.
    Type(crate::TypeExpression),
    /// A declared data application with at least one static value argument.
    Applied {
        declaration: DeclarationId,
        arguments: Vec<DeclarationTerm>,
    },
    StaticInteger {
        carrier: crate::ScalarType,
        value: num_bigint::BigInt,
    },
    StaticBoolean(bool),
}

/// An open data declaration with its type arguments, private witnesses and unevaluated field types.
/// Instantiation and construction must carry its default-domain requirements forward.
#[derive(Debug, Eq, PartialEq)]
pub struct DataTemplate {
    pub declaration: DeclarationId,
    /// Outer scopes supplying the leading arguments in a flattened type application.
    pub enclosing_parameters: Vec<DeclarationId>,
    pub parameters: Vec<TemplateBinder>,
    /// Verifier inputs, not a proof of admissibility at any application.
    pub formation: Vec<(Site, DataFormation)>,
    /// Structural equations used to infer a binder from the supplied arguments.
    pub equations: Vec<(usize, crate::TypeExpression)>,
    pub witness_count: usize,
    pub public: bool,
    pub fields: Vec<TemplateField>,
    pub cases: Vec<TemplateCase>,
    pub numbering: Option<std::sync::Arc<crate::SchemaNumbering>>,
    pub multiplicity: Multiplicity,
    pub carry: Option<Carry>,
    pub requires: Vec<ContractClause>,
    pub site: Site,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataFormation {
    PositiveWidth(crate::TypeExpression),
    /// Under this template's declaration, the active application needs the next one to form.
    RecursiveApplication {
        active_arguments: Vec<crate::TypeExpression>,
        arguments: Vec<crate::TypeExpression>,
    },
    /// The authored operation and its questions, before index normalization.
    IntegerOperation {
        operation: crate::IntegerOperation,
        carrier: crate::ScalarType,
        operands: [crate::TypeExpression; 2],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateBinder {
    Type { multiplicity: Option<Multiplicity> },
    Kinded(crate::TypeExpression),
    Lifetime,
    Const(crate::TypeExpression),
}

#[derive(Debug, Eq, PartialEq)]
pub struct TemplateCase {
    pub declaration: DeclarationId,
    pub fields: Vec<TemplateField>,
    pub numbering: Option<std::sync::Arc<crate::SchemaNumbering>>,
    pub site: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TemplateField {
    pub declaration: DeclarationId,
    pub ty: crate::TypeExpression,
    pub site: Site,
}
