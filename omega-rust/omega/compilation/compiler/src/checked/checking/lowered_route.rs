// The lowered-form checker alone decides the product compilation. The shared
// factory captures its exact frozen source owner before judgment and retains
// the first closure only after every part accepts. Publication failure cannot
// select another stage's module.

use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;

/// What the route judges: the declarations the request claims, which are
/// the same `structure_elaboration::elaborate` roots.
pub(super) struct RouteScope<'a> {
    pub(super) entries: &'a [SymbolHandle],
    pub(super) subject: Option<PackageKeyIdentity>,
    pub(super) selected: &'a [SymbolHandle],
    pub(super) bindings: &'a [typed_trees::callable_signature_shapes::BoundCallableShape],
}

impl RouteScope<'_> {
    /// The route's judgment of `typed`: the closure of the compilation's
    /// roots, and the check form of each generic machine the subject
    /// declares. generics.md, "Static type equality": "An unspecialized body
    /// checks every admitted alternative; one consumer cannot authorize an
    /// otherwise invalid generic body." So a generic body is judged as
    /// written whether or not the program applies it, and the machines stage
    /// 04 generated for the program's own applications are judged as closed
    /// machines of the closure. The route accepts only when it accepts every
    /// one of them.
    ///
    /// Keep the exact source owner before judging. Publication is performed by
    /// the shared factory; a missing publication never selects another module.
    pub(super) fn judge(
        &self,
        typed: std::sync::Arc<typed_trees::FrozenTypedTrees>,
    ) -> Result<std::sync::Arc<psi_compiler::AcceptedTerminalSource>, Vec<Diagnostic>> {
        psi_compiler::AcceptedTerminalSource::judge(
            typed,
            structure_elaboration::Compilation {
                entries: self.entries,
                subjects: &[self.subject],
                selected: self.selected,
                bindings: self.bindings,
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .map(std::sync::Arc::new)
    }
}
