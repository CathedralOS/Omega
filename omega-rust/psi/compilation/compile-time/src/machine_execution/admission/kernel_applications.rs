//! Build-time kernels have two genuine suppliers: settled provider semantics
//! and automatic intrinsic meaning from this execution's checked operator facts.
//! Both joins retain the authored occurrence. An automatic operation never
//! manufactures a native provider commitment or an ordinary body guarantee.

use super::BuildTimeCallEdge;
use checked_trees::{CheckedTrees, CheckedValueOrigin};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionHandle};

#[derive(Clone, Copy)]
pub(super) struct KernelApplications<'program> {
    program: &'program TypedTrees,
    selected: &'program [crate::SelectedBuildTimeBinaryOperator],
    checked: Option<&'program CheckedTrees>,
}

impl<'program> KernelApplications<'program> {
    pub(super) fn new(
        program: &'program TypedTrees,
        selected: &'program [crate::SelectedBuildTimeBinaryOperator],
        checked: Option<&'program CheckedTrees>,
    ) -> Self {
        Self {
            program,
            selected,
            checked: checked.filter(|checked| std::ptr::eq(&*checked.typed, program)),
        }
    }

    pub(super) fn contains_call(self, call: &BuildTimeCallEdge) -> bool {
        if !call.authored_expression.is_valid() {
            return false;
        }
        let origin_matches = |origin| {
            matches!(origin, CheckedValueOrigin::StateStatement {
            machine_symbol, state_symbol, statement_index, ..
        } if machine_symbol == call.source_machine_symbol
            && state_symbol == call.source_state_symbol
            && statement_index == call.statement_index)
        };
        let mut matching = self.selected.iter().filter(|row| {
            row.expression == call.authored_expression
                && row.requirement == call.target_machine_symbol
                && origin_matches(row.origin)
        });
        if matching.next().is_some() {
            return matching.next().is_none();
        }
        self.automatic_requirement(call.authored_expression, origin_matches)
            == Some(call.target_machine_symbol)
    }

    pub(super) fn contains_expression(
        self,
        expression: ExpressionHandle,
        machine: SymbolHandle,
    ) -> bool {
        self.selected
            .iter()
            .any(|row| row.expression == expression && row.origin.machine_symbol() == Some(machine))
            || self
                .automatic_requirement(expression, |origin| {
                    origin.machine_symbol() == Some(machine)
                })
                .is_some()
    }

    fn automatic_requirement(
        self,
        expression: ExpressionHandle,
        origin_matches: impl Fn(CheckedValueOrigin) -> bool,
    ) -> Option<SymbolHandle> {
        let checked = self.checked?;
        let facts = &checked.facts.operators;
        let mut selections = facts
            .resolved_expression_selections()
            .filter(|(selected_expression, _, _, _)| *selected_expression == expression);
        let (_, origin, requirement, _) = selections.next()?;
        if selections.next().is_some() || !origin_matches(origin) {
            return None;
        }
        facts
            .requirement_realization(self.program, requirement, expression)
            .ok()
            .flatten()
            .filter(|meaning| meaning.is_compiler_intrinsic())
            .map(|_| requirement)
    }
}
