//! Synthesis reads the full declaration context but mutates only owned work.
//!
//! Equation completion introduces exact type roots after earlier declarations
//! have been published. Those declarations remain lookup context, not another
//! normalization worklist. New instances and their attached machines extend the
//! same selected domain so nested applications and constructor uses settle by
//! the ordinary fixed point without changing an unrelated retained declaration.
//! Authored applications are a finite root roster. A synthesized field adds
//! inline storage edges; a reference target remains identity. Argument admission
//! is independent of copying a declaration. Data and case predicates describe
//! default domains, not a requirement that every named application be inhabited.
//! Typed elaboration owns the later field views of identity-only applications.

use syntax_trees::expression::ExpressionFrontier;
use syntax_trees::expression::ExpressionHandle;
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TypePositionPurpose {
    Identity,
    Materialization,
}

pub(super) struct SynthesisWork<'input> {
    domain: GenericDataWorkDomain<'input>,
    declaration_frontier: usize,
    expression_frontier: ExpressionFrontier,
}

impl<'input> SynthesisWork<'input> {
    pub(super) fn new(syntax: &SyntaxTrees, domain: GenericDataWorkDomain<'input>) -> Self {
        Self {
            domain,
            declaration_frontier: syntax.root_item_handles().len(),
            expression_frontier: syntax.expressions.frontier(),
        }
    }

    pub(super) fn is_all(&self) -> bool {
        matches!(self.domain, GenericDataWorkDomain::All { .. })
    }

    /// Synthesis also copies data facts and other declaration-side expressions,
    /// not just machine bodies. Their new arena suffix belongs to this work
    /// even when no executable machine inventory reaches them.
    pub(super) fn owns_new_expression(&self, expression: ExpressionHandle) -> bool {
        expression.is_valid() && self.expression_frontier.contains(expression)
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
        purpose: TypePositionPurpose,
        include_domain_arguments: bool,
    ) -> Vec<TypeReferenceHandle> {
        let declarations = if purpose == TypePositionPurpose::Identity {
            self.declarations(syntax)
        } else {
            match self.domain {
                GenericDataWorkDomain::All { .. } => {
                    syntax.root_item_handles()[..self.declaration_frontier].to_vec()
                }
                GenericDataWorkDomain::Selected { declarations, .. } => declarations.to_vec(),
            }
        };
        let mut positions = super::const_evaluation::collect_selected_type_reference_positions(
            syntax,
            &declarations,
        );
        if purpose == TypePositionPurpose::Materialization {
            positions.extend(
                super::const_evaluation::collect_stored_type_reference_positions(
                    syntax,
                    &syntax.root_item_handles()[self.declaration_frontier..],
                ),
            );
        }
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
