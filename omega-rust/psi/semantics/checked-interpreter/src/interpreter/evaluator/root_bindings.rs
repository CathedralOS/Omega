//! Executed Build declarations retain coordinates, not target-selection authority.
use super::{Cell, EvalResult, Evaluator, Frame, Halt, State, TypeReferenceNode, trap};
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
        // Read a reference carrier once, preserving its original cell even
        // when an ordinary call produced it. General writable-place lookup
        // remains effect-free; its speculative callers must not replay calls.
        let receiver = self.eval_read_cell(binding.receiver, frame)?;
        let receiver = self.deref_cell(receiver);
        if !self
            .root_build
            .as_ref()
            .is_some_and(|root| Cell::ptr_eq(root, &receiver))
        {
            return trap("root binding requires the current activation's original Build value");
        }
        // A delegated operand binds the description the compiler issued for
        // it; the target machine is looked up, never executed.
        let described = if binding.implementation_operand.is_valid() {
            let operand = self.eval_expression(binding.implementation_operand, frame)?;
            Some(self.described_product_entry(&operand)?)
        } else {
            None
        };
        self.executed_root_bindings
            .try_reserve(1)
            .map_err(|_| Halt::Resource("root-binding result allocation was refused".to_owned()))?;
        self.executed_root_bindings
            .push(crate::ExecutedRootBinding {
                statement,
                described,
            });
        Ok(())
    }
}
