use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
#[path = "preparation.rs"]
mod preparation;
use crate::paged_roster::paged_roster;
use arena::{Handle, HandleSpan, PagedArenaFrontier};
pub use preparation::{
    ConstArgumentNormalization, ConstArgumentOrigin, TypeAliasApplicationOrigin,
};
use preparation::{TypePreparation, TypePreparationAppendCheckpoint};

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;
pub type TypeReferenceFrontier = PagedArenaFrontier<TypeReferenceNode>;

paged_roster! {
    pub struct TypeReferenceTable => pub(crate) TypeReferenceTableAppendCheckpoint {
        arena type_references: TypeReferenceNode,
        arena type_reference_handles: TypeReferenceHandle,
        arena constraints: TypeConstraintNode,
        roster preparation: TypePreparation => TypePreparationAppendCheckpoint,
    }
}

impl TypeReferenceTable {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
        self.type_references.insert(type_reference)
    }

    pub fn node_count(&self) -> TypeReferenceFrontier {
        self.type_references.frontier()
    }

    pub fn named_nodes_from(
        &self,
        watermark: TypeReferenceFrontier,
        context: &impl crate::SyntaxTextContext,
    ) -> Vec<(TypeReferenceHandle, String)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| watermark.contains(*handle))
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Named(name) => Some((handle, name.as_str(context).to_string())),
                _ => None,
            })
            .collect()
    }

    pub fn const_expression_nodes(&self) -> Vec<(TypeReferenceHandle, ExpressionHandle)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| !self.is_alias_header_reference(*handle))
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::ConstExpression(expression) => Some((handle, *expression)),
                _ => None,
            })
            .collect()
    }

    pub fn const_parameter_array_nodes_from(
        &self,
        watermark: TypeReferenceFrontier,
        context: &impl crate::SyntaxTextContext,
    ) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, String)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| watermark.contains(*handle))
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter(name),
                } => Some((handle, *element_type, name.as_str(context).to_string())),
                _ => None,
            })
            .collect()
    }

    pub fn insert_named(&mut self, name: Identifier) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Named(name))
    }

    pub fn insert_unit(&mut self) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Unit)
    }

    pub fn insert_reference(
        &mut self,
        referee: TypeReferenceHandle,
        access: language_core::ReferenceAccess,
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
        access: language_core::ReferenceAccess,
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

    /* Every type reference node of the forest, in table order. */
    pub fn iter_type_references(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> {
        self.type_references.iter()
    }

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

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum TypeReferenceNode {
    Reference {
        referee: TypeReferenceHandle,
        access: language_core::ReferenceAccess,
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
        lifetime_arguments: Vec<Identifier>,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    // The existing selection carrier preserves which path component
    // owns an application, before namespace resolution assigns its kind.
    StaticSelection(crate::expression::StaticMachineArgument),
    Callable {
        signature: crate::item::StateSignatureHandle,
    },
    ConstExpression(ExpressionHandle),
    Named(Identifier),
    #[default]
    Unit,
}

impl TypeReferenceNode {
    pub fn as_named(&self) -> Option<&Identifier> {
        let TypeReferenceNode::Named(named) = self else {
            return None;
        };
        Some(named)
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter(Identifier),
    ConstCall(Identifier),
    Expression(TypeReferenceHandle),
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

impl FixedArrayLength {
    pub fn same_value(&self, other: &Self, context: &impl crate::SyntaxTextContext) -> bool {
        match (self, other) {
            (Self::Literal(left), Self::Literal(right)) => left == right,
            (Self::ConstParameter(left), Self::ConstParameter(right))
            | (Self::ConstCall(left), Self::ConstCall(right)) => {
                left.as_str(context) == right.as_str(context)
            }
            (Self::Expression(left), Self::Expression(right)) => left == right,
            _ => false,
        }
    }

    pub fn display_name(&self, context: &impl crate::SyntaxTextContext) -> String {
        match self {
            Self::Literal(value) => value.to_string(),
            Self::ConstParameter(name) => name.as_str(context).to_owned(),
            Self::ConstCall(name) => format!("{}()", name.as_str(context)),
            Self::Expression(_) => "<constant expression>".to_owned(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum TypeConstraintNode {
    Named(Identifier),

    ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
    Domain(DomainConstraint),
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DomainConstraint {
    pub name: Identifier,
    pub arguments: HandleSpan<TypeReferenceHandle>,
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(Identifier::generated(""))
    }
}

#[cfg(test)]
#[path = "../tests/unit/types_tests.rs"]
mod tests;
