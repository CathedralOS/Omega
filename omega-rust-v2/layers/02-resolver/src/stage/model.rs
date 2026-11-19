use crate::ParsedSource;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;

/// The package identity and logical path are supplied by stage admission.
pub struct StageSource<'a> {
    pub package: &'a str,
    pub source: ParsedSource<'a>,
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
    pub package: String,
    pub path: String,
    pub kind: NodeKind,
    /// Authored operand types separate candidate occurrences before type normalization.
    pub operands: Vec<String>,
    pub result_domains: Vec<String>,
    /// Nominal owners of signature names in the declaring file's dependency scope.
    pub signature_owners: Vec<(String, String)>,
    /// Each enclosing machine contributes its operand spelling and resolved owners.
    pub enclosing_machines: Vec<EnclosingMachine>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EnclosingMachine {
    pub path: String,
    pub operands: Vec<String>,
    pub result_domains: Vec<String>,
    pub signature_owners: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Symbol {
    Declaration(DeclarationId),
    Candidates(Vec<DeclarationId>),
    ByOverload(Vec<ArgumentAlternative>),
    Binding {
        span: SourceSpan,
        kind: NodeKind,
    },
    Builtin(String),
    /// L3 selects the member using the resolved receiver's type.
    Member(String),
    StaticGuarantee {
        binder: SourceSpan,
    },
    EvidenceMember {
        binder: SourceSpan,
        requirements: Vec<DeclarationId>,
    },
    TargetInstruction(String),
    TargetRegister(String),
    Module {
        package: String,
        path: String,
    },
}

/// L3 validates the interpretation for the chosen overload. A missing name in one
/// role cannot steer overload selection toward the other role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentAlternative {
    pub candidate: DeclarationId,
    pub binder: Option<NodeKind>,
    pub target: Option<Box<Symbol>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameUse {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub target: Symbol,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedStage {
    pub declarations: Vec<DeclarationId>,
    /// Original source of each declaration, in the same order as the declaration list.
    pub definition_sites: Vec<DefinitionSite>,
    pub uses: Vec<NameUse>,
    /// Each argument span refers to the original parsed tree, including unevaluated expressions.
    pub applications: Vec<Application>,
    pub operator_declarations: Vec<OperatorDeclaration>,
    pub operators: Vec<OperatorUse>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct DefinitionSite {
    pub file: String,
    pub span: SourceSpan,
}
#[derive(Debug, Eq, PartialEq)]
pub struct OperatorDeclaration {
    pub token: String,
    pub machine: DeclarationId,
    pub home: Option<DeclarationId>,
}
#[derive(Debug, Eq, PartialEq)]
pub struct OperatorUse {
    pub package: String,
    pub file: String,
    pub span: SourceSpan,
    pub token: String,
}
#[derive(Debug, Eq, PartialEq)]
pub struct Application {
    pub site: NameUse,
    pub arguments: Vec<SourceSpan>,
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
