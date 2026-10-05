//! Invocation-owned source frontiers and completed Pure Builds.

use crate::resolution::graph::ResolvedPackageSourceClosure;

/// Target-independent prepared package sources retained across candidate
/// compilations or reviews of one resolved source closure.
///
/// Slots follow the resolver's package positions, each keyed by the entry
/// root it was prepared for. A populated slot supplies only that package's
/// binding-independent parse frontier to a later pass or candidate: custody
/// verification, Build admission, and product checking always run again, and
/// each child's root-path and source-input validation rejects a checkpoint
/// that no longer names its prepared sources. The separate compilation-owned
/// Pure Build store retains only completed output-free values after exact
/// checked-program comparison and a fresh current-pass logical charge. It
/// carries no checked product, review row, entry token, or host grant.
///
/// One store belongs to one resolved closure. Callers reviewing several exact
/// targets of the same closure share it; a differently shaped closure drops
/// stale slots rather than risk a misplaced checkpoint, and a slot whose
/// recorded entry root differs from the request's is prepared fresh.
#[derive(Default)]
pub struct PackageSourcePreparation {
    pub(crate) slots: Vec<Option<PreparedPackageSource>>,
    /// Only this compilation owns these values. Package/purpose/custody remain
    /// exact input keys; each consumer admits and pays its current-pass charge.
    pub(crate) pure_build_compilation: build_evaluation::PureBuildCompilation,
    /// Collect the internal stage ladder on every fresh preparation this
    /// store performs. Off by default: the ladder measures nothing until a
    /// caller asks for it, and the rows stay on the checked record until a
    /// report-facing leg prints them.
    pub(crate) collect_timings: bool,
    /// Each package position's root binding with its canonical build-source
    /// index, captured by the first pass that compiles that package as a
    /// root and read by every later pass and purpose.
    pub(crate) root_bindings: Vec<Option<package_compilation::PackageSourceBinding>>,
}

/// One package's retained parse frontier and the entry root it belongs to.
/// The root is the exact identity the child request validates, so a same
/// count but differently resolved closure cannot consume a misplaced slot.
pub(crate) struct PreparedPackageSource {
    pub(crate) entry_root: std::path::PathBuf,
    pub(crate) prepared: compilation_products::PreparedCheckedSource,
}

impl PackageSourcePreparation {
    /// An empty store; the first candidate sizes it to that closure's graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Slots pre-sized for this closure's package graph.
    pub fn for_closure(closure: &ResolvedPackageSourceClosure) -> Self {
        Self {
            slots: empty_slots(closure.graph().packages().len()),
            pure_build_compilation: Default::default(),
            collect_timings: false,
            root_bindings: vec![None; closure.graph().packages().len()],
        }
    }

    /// Prepare fresh sources with the stage ladder enabled.
    pub fn collecting_timings(mut self, collect_timings: bool) -> Self {
        self.collect_timings = collect_timings;
        self
    }

    /// Size slots to this closure's package positions. A differently shaped
    /// closure starts over empty rather than risk a misplaced checkpoint.
    pub(crate) fn size_for(&mut self, closure: &ResolvedPackageSourceClosure) {
        let package_count = closure.graph().packages().len();
        if self.slots.len() != package_count {
            self.slots = empty_slots(package_count);
            self.pure_build_compilation = Default::default();
            self.root_bindings = vec![None; package_count];
        }
    }
}

fn empty_slots(count: usize) -> Vec<Option<PreparedPackageSource>> {
    std::iter::repeat_with(|| None).take(count).collect()
}
