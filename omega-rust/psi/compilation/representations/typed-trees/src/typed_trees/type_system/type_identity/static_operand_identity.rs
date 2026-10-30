//! Exact value/code operands under the same replacement scopes as their types.

use super::{
    IndexIdentityArgument, NormalizedTypeIdentity, SubstitutionSemantics, TypeIdentityContext,
    program_intern_type_identity, substitution,
};
use crate::{TypedTrees, types::TypeReferenceHandle};
use std::cell::Cell;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub struct StaticOperandIdentityRequest<'a> {
    pub operand: IndexIdentityArgument<'a>,
    pub lexical_owner: SymbolHandle,
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
}

impl TypedTrees {
    /// Normalize a supplied operand through a chain of receiving applications.
    /// Unknown callable, index or region custody cannot compare equal to itself.
    pub fn static_operand_identity(
        &self,
        request: StaticOperandIdentityRequest<'_>,
    ) -> Option<NormalizedTypeIdentity> {
        self.static_operand_identity_with_semantics(request, SubstitutionSemantics::Transitive)
    }

    /// Receiving operands belong to their supplying scope. Replacing a sibling
    /// from this same telescope again would change recursive permutations.
    pub fn static_operand_identity_with_simultaneous_substitutions(
        &self,
        request: StaticOperandIdentityRequest<'_>,
    ) -> Option<NormalizedTypeIdentity> {
        self.static_operand_identity_with_semantics(request, SubstitutionSemantics::Simultaneous)
    }

    fn static_operand_identity_with_semantics(
        &self,
        request: StaticOperandIdentityRequest<'_>,
        substitution_semantics: SubstitutionSemantics,
    ) -> Option<NormalizedTypeIdentity> {
        let invalid = Cell::new(false);
        let identity = substitution::index_argument(
            self,
            request.operand,
            &TypeIdentityContext {
                application_owner: request.lexical_owner,
                substitutions: request.substitutions,
                substitution_semantics,
                invalid_application: Some(&invalid),
                invalid_index_argument: Some(&invalid),
                ..TypeIdentityContext::default()
            },
        );
        (!invalid.get()).then(|| program_intern_type_identity(self, identity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{expression::ExpressionNode, name::Identifier, types::TypeReferenceNode};
    use numerics::literals::{IntegerLiteral, IntegerRadix};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn receiving_static_indices_do_not_resubstitute_sibling_bindings() {
        let mut symbols = SymbolTableBuilder::default();
        let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Owner"));
        let parameters = symbols
            .insert_children(
                owner,
                [
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("N")),
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("M")),
                ],
            )
            .handles()
            .collect::<Vec<_>>();
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let references = parameters
            .iter()
            .map(|symbol| {
                let name = Identifier::from(program.symbols.name(*symbol));
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: *symbol,
                        name,
                    })
            })
            .collect::<Vec<_>>();
        let integer = program.expression_table.insert(ExpressionNode::Integer(
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "7").unwrap(),
        ));
        let seven = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(integer));
        let bindings = [(parameters[0], references[1]), (parameters[1], seven)];
        let request = StaticOperandIdentityRequest {
            operand: IndexIdentityArgument::TypeReference(references[0]),
            lexical_owner: owner,
            substitutions: &bindings,
        };
        let direct = |reference| {
            program.static_operand_identity(StaticOperandIdentityRequest {
                operand: IndexIdentityArgument::TypeReference(reference),
                substitutions: &[],
                ..request
            })
        };
        let receiving = program.static_operand_identity_with_simultaneous_substitutions(request);
        assert!(receiving.is_some());
        assert_eq!(receiving, direct(references[1]));
        assert_eq!(program.static_operand_identity(request), direct(seven));
        assert_ne!(receiving, direct(seven));
        let permutation = [
            (parameters[0], references[1]),
            (parameters[1], references[0]),
        ];
        let request = StaticOperandIdentityRequest {
            substitutions: &permutation,
            ..request
        };
        assert_eq!(
            program.static_operand_identity_with_simultaneous_substitutions(request),
            direct(references[1]),
        );
        assert!(program.static_operand_identity(request).is_none());
    }
}
