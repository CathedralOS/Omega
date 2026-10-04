//! Synthesis reads the full declaration context but mutates only owned work.
//!
//! Equation completion introduces exact type roots after earlier declarations
//! have been published. Those declarations remain lookup context, not another
//! normalization worklist. New instances and their attached machines extend the
//! same selected domain so nested applications and constructor uses settle by
//! the ordinary fixed point without changing an unrelated retained declaration.

use syntax_trees::{SyntaxTrees, item::ItemHandle, types::TypeReferenceHandle};

#[derive(Clone, Copy)]
pub(crate) enum GenericDataWorkDomain<'input> {
    All {
        additional_type_roots: &'input [TypeReferenceHandle],
    },
    Selected {
        type_roots: &'input [TypeReferenceHandle],
        declarations: &'input [ItemHandle],
    },
}

pub(super) struct SynthesisWork<'input> {
    domain: GenericDataWorkDomain<'input>,
    declaration_frontier: usize,
    expression_frontier: usize,
}

impl<'input> SynthesisWork<'input> {
    pub(super) fn new(syntax: &SyntaxTrees, domain: GenericDataWorkDomain<'input>) -> Self {
        Self {
            domain,
            declaration_frontier: syntax.root_item_handles().len(),
            expression_frontier: syntax.expressions.expression_count(),
        }
    }

    pub(super) fn is_all(&self) -> bool {
        matches!(self.domain, GenericDataWorkDomain::All { .. })
    }

    /// Synthesis also copies data facts and other declaration-side expressions,
    /// not just machine bodies. Their new arena suffix belongs to this work
    /// even when no executable machine inventory reaches them.
    pub(super) fn owns_new_expression(
        &self,
        expression: syntax_trees::expression::ExpressionHandle,
    ) -> bool {
        expression.is_valid() && expression.arena_index() as usize > self.expression_frontier
    }

    pub(super) fn declarations(&self, syntax: &SyntaxTrees) -> Vec<ItemHandle> {
        match self.domain {
            GenericDataWorkDomain::All { .. } => syntax.root_item_handles().to_vec(),
            GenericDataWorkDomain::Selected { declarations, .. } => {
                let mut selected = declarations.to_vec();
                for &declaration in &syntax.root_item_handles()[self.declaration_frontier..] {
                    if !selected.contains(&declaration) {
                        selected.push(declaration);
                    }
                }
                selected
            }
        }
    }

    pub(super) fn type_positions(
        &self,
        syntax: &SyntaxTrees,
        include_domain_arguments: bool,
    ) -> Vec<TypeReferenceHandle> {
        let declarations = self.declarations(syntax);
        let mut positions = super::const_evaluation::collect_selected_type_reference_positions(
            syntax,
            &declarations,
        );
        let roots = match self.domain {
            GenericDataWorkDomain::All {
                additional_type_roots,
            } => additional_type_roots,
            GenericDataWorkDomain::Selected { type_roots, .. } => type_roots,
        };
        for &root in roots {
            super::collect_type_positions(syntax, root, &mut positions, include_domain_arguments);
        }
        positions
    }
}
