//! The roots and selections one compilation asks stage 05 to lower.

use crate::provider_selections::ProviderSelections;
use crate::source_state::TypedTrees;
use crate::source_state::trait_definition::{Supply, TraitApplicationEnvironment};
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

/// What one compilation asks to have judged. This is request information: the
/// typed trees also hold declarations of other packages and do not say which
/// package the request checks (`wiki/spec/build/validation_subjects.md`: "The subject
/// set is decided by the request, not by where source lives").
#[derive(Debug, Clone, Copy)]
pub struct Compilation<'a> {
    /// Exact source declarations whose canonical metadata this product needs.
    /// This nominates descriptors, not executable roots or outbound service
    /// demand, and grants no compatibility or installation authority.
    pub declaration_metadata: &'a [SymbolHandle],
    /// The Build's bound program entries. The first is the module's entry.
    /// An empty slice checks the declared subjects and selections in a
    /// declaration module with no executable entry.
    pub entries: &'a [SymbolHandle],
    /// Full checks explicitly requested in addition to the entry/selection
    /// closure. `Some(package)` selects that package; `None` selects standalone
    /// user source. An empty slice requests only the concrete root closure.
    pub subjects: &'a [Option<PackageKeyIdentity>],
    /// Which of a subject's two checked contexts this compilation judges
    /// (`wiki/spec/build/scoped_execution.md`, "Two checked contexts"). A
    /// package's Build sources and Product sources are separate programs: a
    /// Build activation's subject declarations are the ones read under the
    /// Build dependency scope, and its entry must come from that context.
    pub purpose: source_provenance::DependencyScope,
    /// The declarations the Build's selections name (selected providers and
    /// what each provides, layout-scope entries, chosen representations),
    /// whichever package declares them. The check claims each with its
    /// closure: "the obligations of the Build's selections".
    pub selected: &'a [SymbolHandle],
    /// One visible-continuation shape obligation per executed Build binding.
    /// Deduplicating machine roots must not discard distinct slot contracts.
    pub bindings: &'a [crate::source_state::callable_signature_shapes::BoundCallableShape],
    /// Exact source applications selected for calling policies and entry ABI
    /// publication. They are consumed before body closure, never by a later
    /// target stage reopening typed source.
    pub calling: Option<CallingPolicySelections<'a>>,
    /// Build/default precedence has selected these exact source providers.
    /// Their schemas and choice coordinates are published in the module.
    pub providers: Option<ProviderSelections<'a>>,
    /// The implementations chosen for nominal requirements of this compilation:
    /// the Build's own selections, and the supply settlement chose for each
    /// requirement the compilation reaches. The check claims each provider's
    /// machines for the slot as roots and owes, for each, the proof that the
    /// machine refines the requirement it supplies (`Supply`).
    pub supplies: &'a [Supply],
}

/// This is source selection context, not executable or package authority.
#[derive(Debug, Clone, Copy)]
pub struct CallingPolicySelections<'source> {
    pub source: &'source TypedTrees,
    pub applications: &'source [TraitApplicationEnvironment],
    pub entries: &'source [ProgramEntryContractSelection<'source>],
}

#[derive(Debug, Clone, Copy)]
pub struct ProgramEntryContractSelection<'source> {
    pub target: &'source str,
    pub application: &'source TraitApplicationEnvironment,
    pub physical_source_digest: [u8; 32],
    pub implementation: SymbolHandle,
}
