//! Value bounds inside types inherit the surrounding value frontier.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

use super::StateValueScope;

impl StateValueScope<'_, '_> {
    pub(crate) fn type_reference(
        &self,
        reference: TypeReferenceHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let table = &self.program.type_reference_table;
        match table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                self.type_reference(*referee, diagnostics)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                self.type_reference(*element_type, diagnostics)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.type_reference(*base_type, diagnostics);
                for constraint in table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Range { minimum, maximum } => {
                            self.expression(*minimum, diagnostics);
                            self.expression(*maximum, diagnostics);
                        }
                        TypeConstraintNode::Domain(domain) => {
                            for argument in &domain.arguments {
                                self.type_reference(*argument, diagnostics);
                            }
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                for argument in table.type_reference_handles(*arguments) {
                    self.type_reference(*argument, diagnostics);
                }
            }
            // Const expressions/array extents have a separate const telescope;
            // nominal names do not read state storage or expand field scopes.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
