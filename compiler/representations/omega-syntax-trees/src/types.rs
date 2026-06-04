use crate::identifier::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};
use std::fmt;

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReferenceTable {
    type_references: Arena<TypeReferenceNode>,
    type_reference_handles: Arena<TypeReferenceHandle>,
    constraints: Arena<TypeConstraintNode>,
}

impl TypeReferenceTable {
    pub fn new() -> Self {
        Self {
            type_references: Arena::new(),
            type_reference_handles: Arena::new(),
            constraints: Arena::new(),
        }
    }

    pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
        self.type_references.insert(type_reference)
    }

    pub fn insert_named(&mut self, name: Identifier) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Named(name))
    }

    pub fn insert_generated_named(&mut self, name: impl Into<String>) -> TypeReferenceHandle {
        self.insert_named(Identifier::generated(name))
    }

    pub fn insert_self_type(&mut self) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::SelfType)
    }

    pub fn insert_unit(&mut self) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Unit)
    }

    pub fn insert_reference(
        &mut self,
        referee: TypeReferenceHandle,
        is_mutable: bool,
        is_relaxed: bool,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Reference {
            referee,
            is_mutable,
            is_relaxed,
        })
    }

    pub fn insert_constrained(
        &mut self,
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Constrained {
            base_type,
            constraints,
        })
    }

    pub fn insert_fixed_array(
        &mut self,
        element_type: TypeReferenceHandle,
        length: impl Into<FixedArrayLength>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::FixedArray {
            element_type,
            length: length.into(),
        })
    }

    pub fn insert_slice(&mut self, element_type: TypeReferenceHandle) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Slice { element_type })
    }

    pub fn insert_generic(
        &mut self,
        base_name: Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Generic {
            base_name,
            arguments,
        })
    }

    pub fn insert_type_reference_handles(
        &mut self,
        type_references: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(type_references)
    }

    pub fn append_type_reference_handle(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> Handle<TypeReferenceHandle> {
        self.type_reference_handles.append(type_reference)
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(constraints)
    }

    pub fn append_constraint(
        &mut self,
        constraint: TypeConstraintNode,
    ) -> Handle<TypeConstraintNode> {
        self.constraints.append(constraint)
    }

    pub fn type_reference(&self, handle: TypeReferenceHandle) -> &TypeReferenceNode {
        self.type_references.get(handle)
    }

    pub fn type_reference_handles(
        &self,
        span: HandleSpan<TypeReferenceHandle>,
    ) -> &[TypeReferenceHandle] {
        self.type_reference_handles.span_or_empty(span)
    }

    pub fn constraints(&self, span: HandleSpan<TypeConstraintNode>) -> &[TypeConstraintNode] {
        self.constraints.span_or_empty(span)
    }

    pub fn type_reference_count(&self) -> usize {
        self.type_references.len()
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
}

impl Default for TypeReferenceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReferenceNode {
    Reference {
        referee: TypeReferenceHandle,
        is_mutable: bool,
        is_relaxed: bool,
    },
    Constrained {
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    },
    FixedArray {
        element_type: TypeReferenceHandle,
        length: FixedArrayLength,
    },
    Slice {
        element_type: TypeReferenceHandle,
    },
    Generic {
        base_name: Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    Named(Identifier),
    SelfType,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter(Identifier),
}

impl Default for FixedArrayLength {
    fn default() -> Self {
        Self::Literal(0)
    }
}

impl From<usize> for FixedArrayLength {
    fn from(value: usize) -> Self {
        Self::Literal(value)
    }
}

impl fmt::Display for FixedArrayLength {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(value) => write!(formatter, "{value}"),
            Self::ConstParameter(name) => write!(formatter, "{name}"),
        }
    }
}

impl Default for TypeReferenceNode {
    fn default() -> Self {
        Self::Unit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraintNode {
    Named(Identifier),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
    },
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(Identifier::generated(""))
    }
}

#[cfg(test)]
mod tests {
    use super::{TypeConstraintNode, TypeReferenceNode, TypeReferenceTable};
    use crate::expression::{ExpressionNode, ExpressionTable};
    use crate::identifier::Identifier;
    use omega_core::arena::HandleSpan;

    #[test]
    fn type_reference_table_stores_nested_references_as_handles() {
        let mut types = TypeReferenceTable::new();
        let usize_handle = types.insert_named(Identifier::generated("usize"));
        let u8_handle = types.insert_named(Identifier::generated("u8"));
        let fixed_array_handle = types.insert_fixed_array(u8_handle, 16);
        let arguments = types.insert_type_reference_handles([usize_handle, fixed_array_handle]);
        let root = types.insert_generic(Identifier::generated("Result"), arguments);

        assert_eq!(types.type_reference_count(), 4);
        let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
            panic!("root type reference should be generic");
        };

        assert_eq!(arguments.count(), 2);
    }

    #[test]
    fn type_reference_table_stores_constraint_bounds_as_expression_handles() {
        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let base_type = types.insert_named(Identifier::generated("i32"));
        let minimum = expressions.insert(ExpressionNode::Integer(0));
        let maximum = expressions.insert(ExpressionNode::Integer(10));
        let constraint = types.append_constraint(TypeConstraintNode::Range { minimum, maximum });
        let root = types.insert_constrained(base_type, HandleSpan::from_parts(constraint, 1));

        assert_eq!(types.type_reference_count(), 2);
        assert_eq!(expressions.expression_count(), 2);

        let TypeReferenceNode::Constrained { constraints, .. } = types.type_reference(root) else {
            panic!("root type reference should be constrained");
        };
        let [TypeConstraintNode::Range { minimum, maximum }] = types.constraints(*constraints)
        else {
            panic!("expected one range constraint");
        };

        assert!(minimum.is_valid());
        assert!(maximum.is_valid());
    }
}
