//! Types retained by checking for executable expression occurrences.
//!
//! A place's declaration and a computation's result are different judgments:
//! borrowing a place does not give its result the place's by-value type, and
//! arithmetic does not inherit an operand's refinement. Keep both decisions
//! rather than asking lowering to walk source declarations again. Carrier
//! rows separately retain the shell-free storage type of every type reference.
//! The producer publishes these tables after the typed arenas stop changing.

use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedExpressionType {
    pub expression: ExpressionHandle,
    /// Zero means that the occurrence establishes no declared place type.
    pub place: TypeReferenceHandle,
    /// Zero means that no destination-independent result type was resolved.
    pub result: TypeReferenceHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedCarrierType {
    pub reference: TypeReferenceHandle,
    pub carrier: TypeReferenceHandle,
}

/// Dense immutable tables keyed by the corresponding generational handle.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedTypeFacts {
    expressions: Box<[ResolvedExpressionType]>,
    carriers: Box<[ResolvedCarrierType]>,
}

impl ResolvedTypeFacts {
    pub fn with_rows(
        expressions: Vec<ResolvedExpressionType>,
        carriers: Vec<ResolvedCarrierType>,
    ) -> Self {
        Self {
            expressions: expressions.into_boxed_slice(),
            carriers: carriers.into_boxed_slice(),
        }
    }

    /// The declared place type, or zero; never a computed-result substitute.
    pub fn place(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
        self.expression(expression)
            .map_or_else(TypeReferenceHandle::invalid, |row| row.place)
    }

    /// The result type without a destination guess; zero denotes absence.
    pub fn result(&self, expression: ExpressionHandle) -> TypeReferenceHandle {
        self.expression(expression)
            .map_or_else(TypeReferenceHandle::invalid, |row| row.result)
    }

    /// The storage carrier beneath shells, or zero for an unnamed handle.
    pub fn carrier(&self, reference: TypeReferenceHandle) -> TypeReferenceHandle {
        if !reference.is_valid() {
            return TypeReferenceHandle::invalid();
        }
        match self.carriers.get(reference.arena_index() as usize) {
            Some(row) if row.reference == reference => row.carrier,
            _ => TypeReferenceHandle::invalid(),
        }
    }

    fn expression(&self, expression: ExpressionHandle) -> Option<&ResolvedExpressionType> {
        if !expression.is_valid() {
            return None;
        }
        let row = self.expressions.get(expression.arena_index() as usize)?;
        (row.expression == expression).then_some(row)
    }
}
