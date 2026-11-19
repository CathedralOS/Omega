use crate::ParsedSource;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;

/// The package identity and logical path are supplied by stage admission.
pub struct StageSource<'a, 'parse> {
    pub package: &'a str,
    pub source: ParsedSource<'a, 'parse>,
}

/// Only direct, requester-local edges admitted for this stage are visible.
#[derive(Clone, Copy)]
pub struct Dependency<'a> {
    pub requester: &'a str,
    pub alias: &'a str,
    pub package: &'a str,
}

/// Names admitted by the selected architecture. The target checker owns instruction semantics.
#[derive(Default)]
pub struct AssemblyVocabulary<'a> {
    pub instructions: &'a [&'a str],
    pub registers: &'a [&'a str],
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DeclarationId {
    pub kind: NodeKind,
    pub name: std::sync::Arc<DeclarationName>,
}

impl Ord for DeclarationId {
    fn cmp(
        &self,
        other: &Self,
    ) -> std::cmp::Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| (self.kind as usize).cmp(&(other.kind as usize)))
    }
}

impl PartialOrd for DeclarationId {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The resolver shares one qualified name and overload identity across all its occurrences.
/// Independent package interfaces compare by these contents.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeclarationName {
    pub package: String,
    pub path: String,
    pub operands: Vec<String>,
    pub result_domains: Vec<String>,
    pub signature_owners: Vec<(String, String)>,
    pub enclosing_machine: Option<DeclarationId>,
}

impl std::ops::Deref for DeclarationId {
    type Target = DeclarationName;
    fn deref(&self) -> &DeclarationName {
        &self.name
    }
}

/// A generic argument's namespace follows its receiving parameter's typed bound.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NameContext {
    Type,
    Value,
    GenericArgument,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Symbol {
    Declaration(DeclarationId),
    Candidates(Vec<DeclarationId>),
    /// These visible domains compete only if their typed carriers agree with the written qualifier.
    DomainCandidates {
        candidates: Vec<DomainCandidate>,
        ambiguity: String,
    },
    /// Visible bindings in lexical precedence order for a name whose namespace depends on typing.
    ScopedNames {
        context: NameContext,
        names: Vec<Symbol>,
    },
    Binding {
        /// The node that introduces this binding in the occurrence's file.
        binder_node: usize,
        span: SourceSpan,
        kind: NodeKind,
    },
    Builtin(String),
    /// L3 selects the member using the resolved receiver's type.
    Member(String),
    /// A member selected through a lexical static binder. Typing interprets its normalized bound.
    StaticMember {
        binder: SourceSpan,
        member: String,
    },
    TargetInstruction(String),
    TargetRegister(String),
    Module {
        package: String,
        path: String,
    },
}

impl Symbol {
    /// The exact declaration or overload set bound by ordinary name lookup.
    pub fn declaration_candidates(&self) -> &[DeclarationId] {
        match self {
            Self::Declaration(id) => std::slice::from_ref(id),
            Self::Candidates(ids) => ids,
            _ => &[],
        }
    }
    pub fn names_domain(&self) -> bool {
        matches!(self, Self::Declaration(id) if id.kind == NodeKind::Domain) || matches!(self, Self::DomainCandidates { .. })
    }

    pub fn visible_names(&self) -> &[Symbol] {
        match self {
            Self::ScopedNames { names, .. } => names,
            _ => std::slice::from_ref(self),
        }
    }
}

/// One spelling can occupy several roles, as in a shorthand field initializer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NameRole {
    Reference,
    Member,
    Qualifier,
    ApplicationPrefix,
    /// The name being declared on an indexed domain head. Its final argument list binds the domain indices.
    Definition,
    /// A lexical binder shares a qualifier spelling with another visible static root.
    CompetingStaticRoot,
    Register,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameUse {
    pub node: crate::TreeNodeId,
    pub role: NameRole,
    pub span: SourceSpan,
    pub target: Symbol,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedStage {
    pub(crate) direct_dependencies: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    pub(crate) domain_carriers: std::collections::HashMap<usize, super::DomainCarrier>,
    pub(crate) direct_trait_parents: std::collections::BTreeMap<usize, Vec<usize>>,
    pub(super) trait_parents: std::collections::HashMap<(String, String), std::collections::BTreeSet<(String, String)>>,
    pub declarations: Vec<DeclarationId>,
    pub carrier_visibility: Vec<CarrierVisibility>,
    pub operator_declarations: Vec<OperatorDeclaration>,
}
impl ResolvedStage {
    /// Qualified names of transitive parent traits in the admitted declaration graph.
    pub fn trait_ancestors(
        &self,
        declaration: &DeclarationId,
    ) -> impl Iterator<Item = (&str, &str)> {
        (declaration.kind == NodeKind::Trait)
            .then(|| {
                self.trait_parents
                    .get(&(declaration.package.clone(), declaration.path.clone()))
            })
            .flatten()
            .into_iter()
            .flatten()
            .map(|(package, path)| (package.as_str(), path.as_str()))
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DomainCandidate {
    pub declaration: DeclarationId,
    /// This attachment matches only when the named domain's binder is a type.
    pub type_binder: Option<(DeclarationId, usize)>,
}

/// Naming the domain reaches an inaccessible carrier only when every listed binder names a value.
#[derive(Debug, Eq, PartialEq)]
pub struct CarrierVisibility {
    pub site: crate::TreeNodeId,
    pub domain: DeclarationId,
    pub binders: Vec<(DeclarationId, usize)>,
    pub error: NameError,
}

#[derive(Debug, Eq, PartialEq)]
pub struct OperatorDeclaration {
    /// The operator-token node in the declaring machine.
    pub node: crate::TreeNodeId,
    /// Effective visibility, including that inherited from an enclosing trait.
    pub public: bool,
    pub token: String,
    pub machine: DeclarationId,
    /// A declaration that qualifies the binding, or the enclosing trait or conformance. A module supplies no semantic home.
    pub declared_home: Option<Symbol>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct NameError {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub message: String,
}
impl NameError {
    pub(crate) fn new(
        package: &str,
        file: &str,
        span: SourceSpan,
        message: impl Into<String>,
    ) -> Self {
        Self {
            package: package.into(),
            file: file.into(),
            span,
            message: message.into(),
        }
    }
}
