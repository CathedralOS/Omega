// A diagnostic request can retain the exact prepared source even when its
// lowered-form judgment rejects. The ordinary product path shares that frozen
// source with its accepted publication; only an explicit frontier request
// copies the source into this inspection result.

use compilation_products::{CheckedCompilation, CompileFailure};

/// What to check: either form the session already checks.
pub enum TypedFrontierRequest {
    /// A package Product, as `CompilationSession::check_package_build` takes it.
    Package(crate::PackageBuild),
    /// A standalone source and its returned Build, as
    /// `CompilationSession::complete_source_check` takes them.
    Source {
        source: super::SourceCompilation,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
    },
}

/// The Product program as Psi 04 left it, with the entries this Build bound.
#[derive(Debug, Clone)]
pub struct TypedFrontier {
    /// The exact frozen typed source supplied to the lowered-form checker.
    pub typed: typed_trees::FrozenTypedTrees,
    /// The Build's bound program roots, in the order the checker received
    /// them. Empty when the Build binds no entry.
    pub entries: Vec<symbols::SymbolHandle>,
    /// The package this check was asked to validate, as the checker's own
    /// library slice takes it. `None` for a source checked outside a package.
    /// A second checker needs it to know which declarations the check claims
    /// (`wiki/spec/build/validation_subjects.md`); the trees also hold
    /// declarations of other packages and do not say.
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// The declarations the Build's selections name: selected providers and
    /// what each provides, layout-scope entries and chosen opaque
    /// representations. The check claims them and their closure even when no
    /// entry reaches them.
    pub selected: Vec<symbols::SymbolHandle>,
    /// The distinct visible-continuation obligations of the executed bindings.
    pub bindings: Vec<typed_trees::callable_signature_shapes::BoundCallableShape>,
}

/// What the current checker concluded about a program that reached it.
pub enum CurrentCheckerVerdict {
    /// The check the product path runs returned a checked compilation.
    Accepted(Box<CheckedCompilation>),
    /// Psi 05, validation, or a later step of the same check rejected it.
    Rejected(CompileFailure),
}

/// The outcome of one check that kept its typed frontier.
pub enum TypedFrontierCheck {
    /// Psi 00 through 04 completed for the Product program.
    Reached {
        frontier: Box<TypedFrontier>,
        verdict: CurrentCheckerVerdict,
    },
    /// The compilation stopped before Psi 04 produced the Product's frozen
    /// typed trees: a parse error or another refusal in Psi 00 through 03, a
    /// Build that failed, or a refusal in Psi 04 itself. There are no trees to
    /// compare on. Most unknown-type and type-mismatch errors are not here:
    /// Psi 05 and validation report them, after Psi 04.
    StoppedBeforeTypedFrontier(CompileFailure),
}

impl TypedFrontierCheck {
    /// Pair what the check returned with what it left in the slot. An
    /// accepted check that left no trees is reported as a refusal rather than
    /// as an acceptance with nothing to compare.
    pub(crate) fn join(
        frontier: Option<TypedFrontier>,
        result: Result<CheckedCompilation, CompileFailure>,
    ) -> Self {
        match (frontier, result) {
            (Some(frontier), Ok(checked)) => Self::Reached {
                frontier: Box::new(frontier),
                verdict: CurrentCheckerVerdict::Accepted(Box::new(checked)),
            },
            (Some(frontier), Err(failure)) => Self::Reached {
                frontier: Box::new(frontier),
                verdict: CurrentCheckerVerdict::Rejected(failure),
            },
            (None, Err(failure)) => Self::StoppedBeforeTypedFrontier(failure),
            (None, Ok(_)) => Self::StoppedBeforeTypedFrontier(
                vec![diagnostics::Diagnostic::error(
                    "the check accepted a program without retaining its typed frontier",
                )]
                .into(),
            ),
        }
    }
}
