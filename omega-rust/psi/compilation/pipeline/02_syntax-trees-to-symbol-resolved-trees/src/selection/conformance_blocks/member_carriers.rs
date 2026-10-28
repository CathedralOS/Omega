//! Conservative carrier eligibility before full applied conformance checking.
//! Unknown static carriers retain ambiguity; this never proves conformance.

use symbol_resolved_trees::{
    SymbolResolvedTrees,
    signature::StateSignature,
    state::State,
    types::{FixedArrayLength, TypeReference},
};

pub(super) fn compatible(
    program: &SymbolResolvedTrees,
    requirement: &StateSignature,
    realization: &State,
) -> bool {
    let expected = program.state_parameters(requirement.parameters);
    let actual = program.state_parameters(realization.parameters);
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(left, right)| {
            left.is_const == right.is_const
                && left.is_mutable == right.is_mutable
                && left.relevance == right.relevance
                && carriers(program, &left.type_reference, &right.type_reference)
        })
        && carriers(
            program,
            requirement
                .return_type
                .as_ref()
                .unwrap_or(&TypeReference::Unit),
            realization
                .return_type
                .as_ref()
                .unwrap_or(&TypeReference::Unit),
        )
}

fn carriers(program: &SymbolResolvedTrees, left: &TypeReference, right: &TypeReference) -> bool {
    if let TypeReference::Constrained(value) = left {
        return carriers(
            program,
            program.child_type_reference(value.base_type),
            right,
        );
    }
    if let TypeReference::Constrained(value) = right {
        return carriers(program, left, program.child_type_reference(value.base_type));
    }
    if let (Some(left), Some(right)) = (left.primitive_type(), right.primitive_type()) {
        return left == right;
    }
    match (left, right) {
        (TypeReference::Unit, TypeReference::Unit) => true,
        (TypeReference::Reference(left), TypeReference::Reference(right)) => {
            left.access == right.access
                && carriers(
                    program,
                    program.child_type_reference(left.referee),
                    program.child_type_reference(right.referee),
                )
        }
        (TypeReference::Slice(left), TypeReference::Slice(right)) => carriers(
            program,
            program.child_type_reference(left.element_type),
            program.child_type_reference(right.element_type),
        ),
        (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
            !matches!((&left.length, &right.length), (FixedArrayLength::Literal(a), FixedArrayLength::Literal(b)) if a != b)
                && carriers(
                    program,
                    program.child_type_reference(left.element_type),
                    program.child_type_reference(right.element_type),
                )
        }
        (TypeReference::Named { symbol: left, .. }, TypeReference::Named { symbol: right, .. }) => {
            nominal(program, *left, *right)
        }
        (TypeReference::Generic(left), TypeReference::Generic(right)) => {
            nominal(program, left.base_symbol, right.base_symbol)
                && arguments(program, left.arguments, right.arguments)
        }
        (
            TypeReference::DynamicTrait {
                symbol: left,
                arguments: a,
                ..
            },
            TypeReference::DynamicTrait {
                symbol: right,
                arguments: b,
                ..
            },
        ) => nominal(program, *left, *right) && arguments(program, *a, *b),
        // A pre-typed value application or generic binder cannot settle an
        // overload. Keeping it eligible preserves refusal on ambiguity; the
        // full applied checker admits the uniquely identified row later.
        (TypeReference::ConstExpression(_), _) | (_, TypeReference::ConstExpression(_)) => true,
        (TypeReference::Named { symbol, .. }, _) | (_, TypeReference::Named { symbol, .. })
            if unresolved(program, *symbol) =>
        {
            true
        }
        _ => false,
    }
}

fn unresolved(program: &SymbolResolvedTrees, symbol: symbols::SymbolHandle) -> bool {
    !symbol.is_valid() || program.symbols.get(symbol).kind == symbols::SymbolKind::TypeParameter
}

fn nominal(
    program: &SymbolResolvedTrees,
    left: symbols::SymbolHandle,
    right: symbols::SymbolHandle,
) -> bool {
    unresolved(program, left) || unresolved(program, right) || left == right
}

fn arguments(
    program: &SymbolResolvedTrees,
    left: arena::HandleSpan<TypeReference>,
    right: arena::HandleSpan<TypeReference>,
) -> bool {
    let left = program.child_type_references(left);
    let right = program.child_type_references(right);
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| carriers(program, a, b))
}
