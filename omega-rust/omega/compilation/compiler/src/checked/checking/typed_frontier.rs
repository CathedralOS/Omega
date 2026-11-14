// Developer harnesses can retain the frozen source the lowered-form checker
// receives even when judgment or later product preparation refuses. The shared
// SourcePreparationRequest returns that frontier directly after Psi 04; the
// product path copies it only when a caller supplies an optional capture slot.
// A captured frontier is source data, not a second judgment or an acceptance.

use compilation_products::{CheckedCompilation, CompileFailure};

/// Source requests captured with the frontier, so a developer replay judges
/// the same calling-policy and entry roots as the original compilation.
#[derive(Debug, Clone, Default)]
pub struct CallingInputs {
    pub applications: Vec<typed_trees::trait_definition::TraitApplicationEnvironment>,
    pub entries: Vec<ProgramEntryInput>,
}

#[derive(Debug, Clone)]
pub struct ProgramEntryInput {
    pub target: String,
    pub application: typed_trees::trait_definition::TraitApplicationEnvironment,
    pub physical_source_digest: [u8; 32],
    pub implementation: symbols::SymbolHandle,
}

pub struct CallingSelectionContext<'source> {
    source: &'source typed_trees::FrozenTypedTrees,
    applications: &'source [typed_trees::trait_definition::TraitApplicationEnvironment],
    entries: Vec<structure_elaboration::ProgramEntryContractSelection<'source>>,
}

impl CallingInputs {
    pub fn borrow<'source>(
        &'source self,
        source: &'source typed_trees::FrozenTypedTrees,
    ) -> CallingSelectionContext<'source> {
        CallingSelectionContext {
            source,
            applications: &self.applications,
            entries: self
                .entries
                .iter()
                .map(
                    |entry| structure_elaboration::ProgramEntryContractSelection {
                        target: &entry.target,
                        application: &entry.application,
                        physical_source_digest: entry.physical_source_digest,
                        implementation: entry.implementation,
                    },
                )
                .collect(),
        }
    }
}

impl CallingSelectionContext<'_> {
    pub fn selection(&self) -> Option<structure_elaboration::CallingPolicySelections<'_>> {
        (!self.entries.is_empty() || !self.applications.is_empty()).then_some(
            structure_elaboration::CallingPolicySelections {
                source: self.source,
                applications: self.applications,
                entries: &self.entries,
            },
        )
    }
}

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
    /// The actual frozen typed source returned by Psi 04 preparation.
    pub typed: typed_trees::FrozenTypedTrees,
    /// The Build's bound program roots, in the order the checker received
    /// them. Empty when the Build binds no entry.
    pub entries: Vec<symbols::SymbolHandle>,
    /// The package whose declarations this check claims. `None` for a source
    /// checked outside a package. The structural judgment needs this subject
    /// because the trees also hold declarations of dependency packages.
    pub subject: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// The declarations the Build's selections name: selected providers and
    /// what each provides, layout-scope entries and chosen opaque
    /// representations. The check claims them and their closure even when no
    /// entry reaches them.
    pub selected: Vec<symbols::SymbolHandle>,
    /// The distinct visible-continuation obligations of the executed bindings.
    pub bindings: Vec<typed_trees::callable_signature_shapes::BoundCallableShape>,
    pub calling: CallingInputs,
    /// Exact source choices for developer rejudgment. Product consumers read
    /// only the normalized selections in the accepted module.
    pub providers: Vec<structure_elaboration::provider_selections::ProviderSelectionInput>,
}

impl TypedFrontier {
    pub fn provider_selections(
        &self,
    ) -> Option<structure_elaboration::provider_selections::ProviderSelections<'_>> {
        (!self.providers.is_empty()).then_some(
            structure_elaboration::provider_selections::ProviderSelections {
                source: &self.typed,
                choices: &self.providers,
            },
        )
    }
}

/// What the product path concluded after receiving its typed source.
pub enum CurrentCheckerVerdict {
    /// The check the product path runs returned a checked compilation.
    Accepted(Box<CheckedCompilation>),
    /// Structural judgment or a later product step refused it.
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
    /// compare on.
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
