//! The exact data member roster shared by cleanup projection and traversal.
//!
//! Custody paths name the fields of the retained application, which may differ
//! from its template's fields. Read that produced owner before opening a
//! declaration telescope. A scoped template walk still carries its original
//! bindings; it cannot adopt a retained row for a different applied carrier.

use super::Substitutions;
use symbols::SymbolHandle;
use typed_trees::data::DataDefinition;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::{TypedReferences, visit_type_reference};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) struct DataMemberOwner<'program> {
    pub declaration: &'program DataDefinition,
    pub substitutions: Substitutions,
}

pub(super) fn owner<'program>(
    program: &'program dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<DataMemberOwner<'program>> {
    if !matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
    ) {
        return None;
    }
    let exact_application = unchanged_application_scope(program, reference, substitutions);
    if exact_application
        && let Some(declaration) = program.data_application_definition(reference).or_else(|| {
            // A Named carrier names this declaration directly. Generic
            // closed_data_definition joins erased layout origins, which
            // cannot supply occurrence-specific member regions here.
            matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { .. }
            )
            .then(|| program.closed_data_definition(reference))
            .flatten()
        })
    {
        return Some(DataMemberOwner {
            declaration,
            substitutions: substitutions.to_vec(),
        });
    }
    let (symbol, arguments) = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            let symbol = program
                .machine_by_symbol(*symbol)
                .map_or(*symbol, |machine| machine.attached_data_symbol);
            return Some(DataMemberOwner {
                declaration: program.unique_data_definition_by_symbol(symbol)?,
                substitutions: substitutions.to_vec(),
            });
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => (*base_symbol, *arguments),
        _ => return None,
    };
    let declaration = program.unique_data_definition_by_symbol(symbol)?;
    let parameters = program.data_type_parameters(declaration);
    let arguments = program
        .type_reference_table
        .type_reference_handles(arguments);
    if parameters.len() != arguments.len()
        || arguments.iter().any(|argument| {
            !program
                .type_reference_table
                .contains_type_reference(*argument)
        })
    {
        return None;
    }
    let mut nested = substitutions.to_vec();
    nested.extend(
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument)),
    );
    Some(DataMemberOwner {
        declaration,
        substitutions: nested,
    })
}

/// An occurrence-owned view can be reused only when these outer bindings leave
/// its original operands unchanged. Layout identity erases regions and cannot
/// establish that premise. Reuse the canonical reference walk to inspect the
/// real type/const operands, without treating folded constant provenance as a
/// still-rebindable expression or manufacturing an applied type row.
fn unchanged_application_scope(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    if substitutions.is_empty() {
        return true;
    }
    let mut operands = ScopedOperands {
        program,
        substitutions,
        type_references: Vec::new(),
        expressions: Vec::new(),
        rebound: false,
    };
    visit_type_reference(program, &mut operands, reference);
    !operands.rebound
}

struct ScopedOperands<'program> {
    program: &'program dyn validation::CheckingInput,
    substitutions: &'program [(SymbolHandle, TypeReferenceHandle)],
    type_references: Vec<TypeReferenceHandle>,
    expressions: Vec<ExpressionHandle>,
    rebound: bool,
}

impl ScopedOperands<'_> {
    fn observe(&mut self, symbol: SymbolHandle) {
        self.rebound |= self.substitutions.iter().any(|(formal, actual)| {
            *formal == symbol
                && !matches!(
                    self.program.type_reference_table.type_reference(*actual),
                    TypeReferenceNode::Named { symbol: actual, .. } if *actual == symbol
                )
        });
    }
}

impl Extend<SymbolHandle> for ScopedOperands<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {
        // Dependency origins on folded constants remain provenance, not
        // operands to substitute. The callbacks below read actual syntax.
    }
}

impl TypedReferences for ScopedOperands<'_> {
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.rebound || self.type_references.contains(&reference) {
            return false;
        }
        self.type_references.push(reference);
        let program = self.program;
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => self.observe(*symbol),
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::ConstParameter { symbol, .. },
                ..
            } => self.observe(*symbol),
            _ => {}
        }
        true
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if self.rebound || self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        let program = self.program;
        if let ExpressionNode::Name(path) = program.expression_table.expression(expression) {
            self.observe(path.head_symbol);
            self.observe(path.symbol);
        }
        true
    }
}
