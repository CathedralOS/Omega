//! Where a semantic builder reads its descriptor rows.
//!
//! A builder that several checks of one module run used to rebuild every
//! descriptor closure it needed from the bare module, adapting each reached
//! body again. A builder given a caller's `ModuleAnalyses` now reads each
//! closure from that bundle, so it shares the rows the bundle has already
//! adapted for other checks. A builder given only the module builds one
//! bundle on its first closure and shares it among its own closures.
//!
//! Either way each closure answers exactly as the free graph functions
//! answer (`terminal_graph_closure`, `terminal_graph_rows` and
//! `terminal_graph_for`): the same rows, the same failure, and the same
//! refusal of a module that declares one machine identity twice. A builder
//! that never asks for a closure never builds a bundle.

use std::sync::OnceLock;

use semantic_vocabulary::MachineId;
use terminal_psi::TerminalModule;

use crate::descriptors::descriptor_results::{
    DescriptorGraphError, DescriptorMachine, DescriptorSlotKind, GenericCallInputs,
    terminal_graph_for,
};
use crate::module_analyses::{DescriptorRows, ModuleAnalyses};

#[cfg(test)]
mod tests;

pub(crate) struct DescriptorRowSource<'a, 'm> {
    module: &'m TerminalModule,
    given: Option<&'a ModuleAnalyses<'m>>,
    /// The bundle built on the first closure when no caller supplied one,
    /// or why the module has none.
    built: OnceLock<Result<ModuleAnalyses<'m>, DescriptorGraphError>>,
}

impl<'a, 'm> DescriptorRowSource<'a, 'm> {
    /// Reads rows from the caller's bundle.
    pub(crate) fn of(analyses: &'a ModuleAnalyses<'m>) -> Self {
        Self {
            module: analyses.module(),
            given: Some(analyses),
            built: OnceLock::new(),
        }
    }

    /// Builds a bundle for this module when the first closure is asked for.
    pub(crate) fn new(module: &'m TerminalModule) -> Self {
        Self {
            module,
            given: None,
            built: OnceLock::new(),
        }
    }

    pub(crate) fn module(&self) -> &'m TerminalModule {
        self.module
    }

    fn analyses(&self) -> Result<&ModuleAnalyses<'m>, DescriptorGraphError> {
        match self.given {
            Some(analyses) => Ok(analyses),
            None => self
                .built
                .get_or_init(|| ModuleAnalyses::new(self.module))
                .as_ref()
                .map_err(|error| *error),
        }
    }

    /// The rows of every machine the roots reach, or the closure's first
    /// failure, as `terminal_graph_closure` gives them with default inputs.
    pub(crate) fn closure(
        &self,
        kind: DescriptorSlotKind,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> Result<DescriptorRows<'_>, DescriptorGraphError> {
        self.analyses()?.descriptor_closure(kind, roots)
    }

    /// The successful rows of the closure of `roots`, as
    /// `terminal_graph_rows` gives them with default inputs after dropping
    /// each failed row. A consumer that tolerates an unsupported body reads
    /// only these.
    pub(crate) fn successful_rows(
        &self,
        kind: DescriptorSlotKind,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> DescriptorRows<'_> {
        let Ok(analyses) = self.analyses() else {
            return DescriptorRows::Owned(Vec::new());
        };
        let roots = roots.into_iter().collect::<Vec<_>>();
        // A closure without a failure is the same list with nothing dropped.
        analyses
            .descriptor_closure(kind, roots.iter().copied())
            .unwrap_or_else(|_| {
                DescriptorRows::Owned(
                    analyses
                        .closure_rows(kind, roots, &[], GenericCallInputs::default())
                        .into_iter()
                        .filter_map(Result::ok)
                        .collect(),
                )
            })
    }

    /// Every machine's row, or `None` when any row fails, as
    /// `terminal_graph_for` gives them with default inputs.
    ///
    /// Its consumers read an owned slice and rows cannot be copied, so the
    /// rows are adapted for this list alone. A caller's bundle still lends
    /// its machine lookup and type index; without one the list is built as
    /// `terminal_graph_for` builds it.
    pub(crate) fn module_graph(&self, kind: DescriptorSlotKind) -> Option<Vec<DescriptorMachine>> {
        match self.given {
            Some(analyses) => analyses.module_rows(kind, &[]),
            None => terminal_graph_for(self.module, kind, &[]),
        }
    }
}
