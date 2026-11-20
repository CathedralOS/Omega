use omega_syntax::NodeKind;
use omega_tokens::SourceSpan;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub type DeclarationId = omega_program_contracts::DeclarationId<NodeKind>;
pub type DeclarationName = omega_program_contracts::DeclarationName<NodeKind>;

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
/// Package and declaration relationships computed during resolution.
#[derive(Debug, Eq, PartialEq)]
pub struct ResolutionIndex {
    pub direct_dependencies: BTreeMap<String, BTreeSet<String>>,
    pub domain_carriers: HashMap<usize, DomainCarrier>,
    pub direct_trait_parents: BTreeMap<usize, Vec<usize>>,
    pub trait_parents: HashMap<(String, String), BTreeSet<(String, String)>>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedStage {
    pub(crate) index: ResolutionIndex,
    pub declarations: Vec<DeclarationId>,
    pub carrier_visibility: Vec<CarrierVisibility>,
    pub operator_declarations: Vec<OperatorDeclaration>,
}

impl ResolvedStage {
    pub fn new(
        index: ResolutionIndex,
        declarations: Vec<DeclarationId>,
        carrier_visibility: Vec<CarrierVisibility>,
        operator_declarations: Vec<OperatorDeclaration>,
    ) -> Self {
        Self {
            index,
            declarations,
            carrier_visibility,
            operator_declarations,
        }
    }

    /// Qualified names of transitive parent traits in the admitted declaration graph.
    pub fn trait_ancestors(
        &self,
        declaration: &DeclarationId,
    ) -> impl Iterator<Item = (&str, &str)> {
        (declaration.kind == NodeKind::Trait)
            .then(|| {
                self.index
                    .trait_parents
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
    pub fn new(
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainCarrier {
    Builtin(String),
    Named(DeclarationId),
    /// A bound decides whether an annotated binder or its visible named alternative supplies the carrier.
    Bound {
        position: usize,
        alternative: DeclarationId,
    },
    Generic,
}
/// Names bound in the defining package retain their declaration identities across admission.
pub struct InterfaceExport {
    pub id: DeclarationId,
    pub module: String,
    pub trait_parents: Vec<DeclarationId>,
    pub domain_carrier: Option<DomainCarrier>,
}

/// The namespaces and the public declarations supplied by compiled source packages.
/// A directory remains importable when none of its declarations are public.
#[derive(Default)]
pub struct CompiledInterface {
    pub modules: BTreeSet<(String, String)>,
    pub declarations: Vec<InterfaceExport>,
}
