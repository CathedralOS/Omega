//! Resolved package source bindings, indexed by the resolver's package positions.
//! Compiler frontiers and judgments belong to the compiler invocation, not here.

use crate::resolution::graph::ResolvedPackageSourceClosure;

#[derive(Default)]
pub struct PackageSourceBindings {
    pub(crate) collect_timings: bool,
    pub(crate) root_bindings: Vec<Option<package_compilation::PackageSourceBinding>>,
}

impl PackageSourceBindings {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn for_closure(closure: &ResolvedPackageSourceClosure) -> Self {
        Self {
            collect_timings: false,
            root_bindings: vec![None; closure.graph().packages().len()],
        }
    }

    pub(crate) fn size_for(&mut self, closure: &ResolvedPackageSourceClosure) {
        let count = closure.graph().packages().len();
        if self.root_bindings.len() != count {
            self.root_bindings = vec![None; count];
        }
    }
}
