//! Cast result vocabulary retained by typing, available before checking.
//! Reading the original target and policy does not admit unchanged denotation,
//! domain membership or executable cast behavior; checking owns those duties.

use crate::{
    TypedTrees,
    expression::TableCastExpression,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use numerics::arithmetic::ArithmeticDomain;

impl TypedTrees {
    /// Read the retained result qualification without erasing its cast policy.
    /// An unresolved semantic-domain result remains unavailable.
    pub fn cast_result_type_reference(
        &self,
        cast: &TableCastExpression,
    ) -> Option<TypeReferenceHandle> {
        if self
            .type_reference_table
            .contains_type_reference(cast.result_type)
        {
            Some(cast.result_type)
        } else if !cast.semantic_domain.is_empty() {
            None
        } else if cast.domain == ArithmeticDomain::Exact {
            Some(cast.target_type)
        } else if let TypeReferenceNode::Named { symbol, .. } =
            self.type_reference_table.type_reference(cast.target_type)
        {
            self.symbols.builtin_type_atom(*symbol).and_then(|_| {
                self.type_reference_table
                    .find_arithmetic_result_type_reference(*symbol, cast.domain)
            })
        } else {
            self.type_reference_table
                .find_policy_qualified_type_reference(cast.target_type, cast.domain)
        }
    }
}
