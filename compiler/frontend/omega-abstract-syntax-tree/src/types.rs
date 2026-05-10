use crate::identifier::Identifier;
use omega_core::arena::{Arena, Handle, HandleSpan};

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Constrained {
        base_type: Box<TypeReference>,
        constraints: Vec<TypeConstraint>,
    },
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Slice {
        element_type: Box<TypeReference>,
    },
    Generic {
        base_name: Identifier,
        arguments: Vec<TypeReference>,
    },
    Named(Identifier),
    Unit,
}

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

    pub fn insert_type_reference_handles(
        &mut self,
        type_references: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(type_references)
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(constraints)
    }

    fn insert_type_reference_handle_span_from_trees<'type_reference>(
        &mut self,
        type_references: impl IntoIterator<Item = &'type_reference TypeReference>,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> HandleSpan<TypeReferenceHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for type_reference in type_references {
            let type_reference = self.insert_tree(type_reference, expressions);
            let handle = self.type_reference_handles.append(type_reference);
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("type reference handle span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn insert_constraint_span_from_tree(
        &mut self,
        constraints: &[TypeConstraint],
        expressions: &mut crate::expression::ExpressionTable,
    ) -> HandleSpan<TypeConstraintNode> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for constraint in constraints {
            let handle = self
                .constraints
                .append(TypeConstraintNode::from_tree(constraint, expressions));
            if count == 0 {
                start = handle;
            }
            count = count
                .checked_add(1)
                .expect("type constraint span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
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

    pub fn insert_tree(
        &mut self,
        type_reference: &TypeReference,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> TypeReferenceHandle {
        match type_reference {
            TypeReference::Constrained {
                base_type,
                constraints,
            } => {
                let base_type = self.insert_tree(base_type, expressions);
                let constraints = self.insert_constraint_span_from_tree(constraints, expressions);
                self.insert(TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                })
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                let element_type = self.insert_tree(element_type, expressions);
                self.insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: *length,
                })
            }
            TypeReference::Slice { element_type } => {
                let element_type = self.insert_tree(element_type, expressions);
                self.insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReference::Generic {
                base_name,
                arguments,
            } => {
                let arguments =
                    self.insert_type_reference_handle_span_from_trees(arguments, expressions);
                self.insert(TypeReferenceNode::Generic {
                    base_name: base_name.clone(),
                    arguments,
                })
            }
            TypeReference::Named(name) => self.insert(TypeReferenceNode::Named(name.clone())),
            TypeReference::Unit => self.insert(TypeReferenceNode::Unit),
        }
    }
}

impl Default for TypeReferenceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReferenceNode {
    Constrained {
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    },
    FixedArray {
        element_type: TypeReferenceHandle,
        length: usize,
    },
    Slice {
        element_type: TypeReferenceHandle,
    },
    Generic {
        base_name: Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    Named(Identifier),
    Unit,
}

impl Default for TypeReferenceNode {
    fn default() -> Self {
        Self::Unit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    Named(Identifier),
    Range {
        minimum: crate::expression::Expression,
        maximum: crate::expression::Expression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraintNode {
    Named(Identifier),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
    },
}

impl TypeConstraintNode {
    fn from_tree(
        constraint: &TypeConstraint,
        expressions: &mut crate::expression::ExpressionTable,
    ) -> Self {
        match constraint {
            TypeConstraint::Named(name) => Self::Named(name.clone()),
            TypeConstraint::Range { minimum, maximum } => Self::Range {
                minimum: expressions.insert_tree(minimum),
                maximum: expressions.insert_tree(maximum),
            },
        }
    }
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(Identifier::generated(""))
    }
}

impl TypeReference {
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(Identifier::generated(name))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TypeConstraint, TypeConstraintNode, TypeReference, TypeReferenceNode, TypeReferenceTable,
    };
    use crate::expression::{Expression, ExpressionTable};
    use crate::identifier::Identifier;

    #[test]
    fn type_reference_table_stores_nested_references_as_handles() {
        let type_reference = TypeReference::Generic {
            base_name: Identifier::generated("Result"),
            arguments: vec![
                TypeReference::named("usize"),
                TypeReference::FixedArray {
                    element_type: Box::new(TypeReference::named("u8")),
                    length: 16,
                },
            ],
        };

        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let root = types.insert_tree(&type_reference, &mut expressions);

        assert_eq!(types.type_reference_count(), 4);
        let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
            panic!("root type reference should be generic");
        };

        assert_eq!(arguments.count(), 2);
    }

    #[test]
    fn type_reference_table_stores_constraint_bounds_as_expression_handles() {
        let type_reference = TypeReference::Constrained {
            base_type: Box::new(TypeReference::named("i32")),
            constraints: vec![TypeConstraint::Range {
                minimum: Expression::Integer(0),
                maximum: Expression::Integer(10),
            }],
        };

        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let root = types.insert_tree(&type_reference, &mut expressions);

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
