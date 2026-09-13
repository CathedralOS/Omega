//! Executed Build declarations retain coordinates, not target-selection authority.

use super::*;
use typed_trees::statement::{RootBinding, StatementHandle};

impl<'program> Evaluator<'program> {
    /// Only the outer argument-taking build entry installs the activation cell.
    /// Ordinary calls/reborrows share it; newly constructed records do not.
    pub(super) fn retain_root_build(
        &mut self,
        state: &State,
        arguments: &[Cell],
    ) -> EvalResult<()> {
        for (parameter, argument) in self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(arguments)
        {
            let TypeReferenceNode::Reference {
                access, referee, ..
            } = self
                .program
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                continue;
            };
            let symbol = self.program.type_reference_table.type_symbol(*referee);
            if !access.is_exclusive()
                || !self.symbol_has_build_prelude_source(symbol)
                || !self.program.data_definitions().iter().any(|definition| {
                    definition.symbol == symbol && definition.name.as_str() == "Build"
                })
            {
                continue;
            }
            if self.root_build.is_some() {
                return trap("a build activation cannot install multiple root Build arguments");
            }
            self.root_build = Some(argument.clone());
        }
        Ok(())
    }

    pub(super) fn execute_root_binding(
        &mut self,
        statement: StatementHandle,
        binding: &RootBinding,
        frame: &Frame,
    ) -> EvalResult<()> {
        self.tick()?;
        let receiver = self.resolve_place(binding.receiver, frame)?;
        let receiver = self.deref_cell(receiver);
        if !self
            .root_build
            .as_ref()
            .is_some_and(|root| Cell::ptr_eq(root, &receiver))
        {
            return trap("root binding requires the current activation's original Build value");
        }
        self.executed_root_bindings
            .try_reserve(1)
            .map_err(|_| Halt::Resource("root-binding result allocation was refused".to_owned()))?;
        self.executed_root_bindings.push(statement);
        Ok(())
    }
}
