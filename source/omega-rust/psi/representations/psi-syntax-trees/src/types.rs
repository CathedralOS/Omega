use crate::identifier::Identifier;
use psi_arena::{Arena, Handle, HandleSpan};
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

    /// Arena index of the next node inserted into the table. Handles created
    /// at or after this watermark belong to a copy in progress -- generic
    /// specialization rewrites `Named(T)` nodes only inside that fresh
    /// subtree.
    pub fn node_count(&self) -> u32 {
        self.type_references
            .iter()
            .map(|(handle, _)| handle.arena_index())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .expect("type-reference arena index overflow")
    }

    /// The `Named` nodes created at or after `watermark`, as (handle, name).
    pub fn named_nodes_from(&self, watermark: u32) -> Vec<(TypeReferenceHandle, String)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| handle.arena_index() >= watermark)
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Named(name) => Some((handle, name.as_str().to_string())),
                _ => None,
            })
            .collect()
    }

    /// `Self` nodes created at or after a specialization watermark. Trait
    /// synthesis replaces these only in the fresh copy, never in the authored
    /// trait signature.
    pub fn self_type_nodes_from(&self, watermark: u32) -> Vec<TypeReferenceHandle> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| {
                (handle.arena_index() >= watermark && matches!(node, TypeReferenceNode::SelfType))
                    .then_some(handle)
            })
            .collect()
    }

    /// Const-generic expression nodes awaiting the orchestration prepass.
    pub fn const_expression_nodes(
        &self,
    ) -> Vec<(TypeReferenceHandle, crate::expression::ExpressionHandle)> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::ConstExpression(expression) => Some((handle, *expression)),
                _ => None,
            })
            .collect()
    }

    /// Every generic-application node currently stored in the table.
    ///
    /// Pre-resolution whole-program desugars use this snapshot to rewrite a
    /// semantic type application wherever it occurs (fields, parameters,
    /// returns, locals, or nested generic arguments), rather than depending on
    /// two source spellings sharing one arena handle.
    pub fn generic_nodes(&self) -> Vec<TypeReferenceHandle> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| {
                matches!(node, TypeReferenceNode::Generic { .. }).then_some(handle)
            })
            .collect()
    }

    /// Fixed-array nodes in a freshly copied subtree whose length still names
    /// a const parameter. Generic-instance method cloning substitutes these
    /// after the copy, alongside `Named(T)` type nodes.
    pub fn const_parameter_array_nodes_from(
        &self,
        watermark: u32,
    ) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, String)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| handle.arena_index() >= watermark)
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter(name),
                } => Some((handle, *element_type, name.as_str().to_string())),
                _ => None,
            })
            .collect()
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
        access: psi_language_core::ReferenceAccess,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: None,
        })
    }

    pub fn insert_reference_with_lifetime(
        &mut self,
        referee: TypeReferenceHandle,
        access: psi_language_core::ReferenceAccess,
        lifetime: Option<Identifier>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
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
            lifetime_arguments: Vec::new(),
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

    /// Replace a node in place. Every site holding this handle sees the new
    /// node; used by pre-resolution desugars (plan-laid value types rewrite
    /// `Policy<Schema>` spellings to the synthesized instance's plain name).
    pub fn replace_type_reference(
        &mut self,
        handle: TypeReferenceHandle,
        type_reference: TypeReferenceNode,
    ) {
        *self.type_references.get_mut(handle) = type_reference;
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

    /// Snapshot every authored declared-domain application. The returned
    /// argument handles remain stable while pre-resolution canonicalization
    /// rewrites the pointed-to leaves in place.
    pub fn domain_constraints(&self) -> Vec<DomainConstraint> {
        self.constraints
            .iter()
            .filter_map(|(_, constraint)| match constraint {
                TypeConstraintNode::Domain(domain) => Some(domain.clone()),
                _ => None,
            })
            .collect()
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TypeReferenceNode {
    Reference {
        referee: TypeReferenceHandle,
        access: psi_language_core::ReferenceAccess,
        /// Explicit lifetime name (`&'buf T`), frozen decision 15 stage 2. `None`
        /// is the elided case (stage 1). A borrow-region tag only: it carries no
        /// symbol and is ignored by layout, codegen, and structural type
        /// equality; only the borrow checker consults it to link a returned view
        /// to the input it borrows.
        lifetime: Option<Identifier>,
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
        /// Erased borrow-region arguments (`Message<'buf, T>`). Kept separate
        /// from runtime type/const/machine arguments so they never affect
        /// monomorphization arity or layout identity.
        lifetime_arguments: Vec<Identifier>,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    /// A pre-resolution const-generic argument expression. Generic-data
    /// monomorphization evaluates this against scoped integer consts, then
    /// replaces it with the canonical decimal `Named` leaf used by literal
    /// const arguments. It must never survive into symbol-resolved trees.
    ConstExpression(crate::expression::ExpressionHandle),
    DynamicTrait {
        /// Bare `dyn Trait`, or the data carrier in `dyn Data::Conformance`.
        name: Identifier,
        /// Exact named nominal conformance selected for this descriptor.
        conformance: Option<Identifier>,
    },
    Named(Identifier),
    SelfType,
    #[default]
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter(Identifier),
    /// A zero-argument machine call in length position (`[u8; table_size()]`):
    /// the effect-free callee is CONST-EVALUATED by the reference interpreter
    /// before layout (comptime stage 1).
    ConstCall(Identifier),
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
            Self::ConstCall(name) => write!(formatter, "{name}()"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraintNode {
    Named(Identifier),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
    },
    /// An arithmetic overflow domain on a primitive (`u32 in Wrapping`); decision
    /// 17. A behaviour tag, not a value-range predicate.
    ArithmeticDomain(psi_numerics::arithmetic::ArithmeticDomain),
    /// A declared domain on a carrier (`[u8] in Utf8`) or a closed indexed
    /// domain-family application (`f64 in Quantity<Unit::KM>`).
    Domain(DomainConstraint),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainConstraint {
    pub name: Identifier,
    /// Proof-static arguments remain ordinary type-reference leaves until the
    /// pre-resolution const pass replaces closed values with canonical atoms.
    pub arguments: HandleSpan<TypeReferenceHandle>,
}

impl fmt::Display for DomainConstraint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(formatter)
    }
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
    use psi_arena::HandleSpan;

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
    fn substitution_watermark_excludes_existing_type_nodes() {
        let mut types = TypeReferenceTable::new();
        let existing = types.insert_named(Identifier::generated("T"));
        let watermark = types.node_count();
        let copied = types.insert_named(Identifier::generated("T"));

        let nodes = types.named_nodes_from(watermark);
        assert_eq!(nodes, vec![(copied, "T".to_string())]);
        assert_ne!(nodes[0].0, existing);
    }

    #[test]
    fn type_reference_table_stores_constraint_bounds_as_expression_handles() {
        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let base_type = types.insert_named(Identifier::generated("i32"));
        let minimum = expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(0),
        ));
        let maximum = expressions.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::from_value(10),
        ));
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
