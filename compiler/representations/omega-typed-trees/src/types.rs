use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::fmt;

use crate::name::Identifier;

mod display;
#[cfg(test)]
mod tests;

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

    pub fn insert_type_reference_handles(
        &mut self,
        type_references: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(type_references)
    }

    pub fn reserve_type_reference_handles(
        &mut self,
        count: u32,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(
            std::iter::repeat_with(TypeReferenceHandle::invalid)
                .take(usize::try_from(count).expect("type reference handle span count overflow")),
        )
    }

    pub fn set_type_reference_handle_at_offset(
        &mut self,
        type_references: HandleSpan<TypeReferenceHandle>,
        offset: u32,
        type_reference: TypeReferenceHandle,
    ) {
        *self.type_reference_handles.get_mut(Handle::from_parts(
            type_references
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("type reference handle index overflow"),
            type_references.start().generation(),
        )) = type_reference;
    }

    pub fn push_type_reference_handle(
        &mut self,
        span: &mut HandleSpan<TypeReferenceHandle>,
        type_reference: TypeReferenceHandle,
    ) {
        self.type_reference_handles
            .append_to_span(span, type_reference);
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(constraints)
    }

    pub fn reserve_constraints(&mut self, count: u32) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(
            std::iter::repeat_with(TypeConstraintNode::default)
                .take(usize::try_from(count).expect("type constraint span count overflow")),
        )
    }

    pub fn set_constraint_at_offset(
        &mut self,
        constraints: HandleSpan<TypeConstraintNode>,
        offset: u32,
        constraint: TypeConstraintNode,
    ) {
        *self.constraints.get_mut(Handle::from_parts(
            constraints
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("type constraint index overflow"),
            constraints.start().generation(),
        )) = constraint;
    }

    pub fn push_constraint(
        &mut self,
        span: &mut HandleSpan<TypeConstraintNode>,
        constraint: TypeConstraintNode,
    ) {
        self.constraints.append_to_span(span, constraint);
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

    pub fn constraint_span(
        &self,
        span: HandleSpan<TypeConstraintNode>,
    ) -> Option<&[TypeConstraintNode]> {
        self.constraints.span(span)
    }

    pub fn type_reference_count(&self) -> usize {
        self.type_references.len()
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    pub fn display_name(&self, handle: TypeReferenceHandle) -> String {
        self.type_reference(handle).display_name(self)
    }

    pub fn display_name_with_constraints(
        &self,
        handle: TypeReferenceHandle,
        expressions: &crate::expression::ExpressionTable,
    ) -> String {
        self.type_reference(handle)
            .display_name_with_constraints(self, expressions)
    }

    pub fn primitive_type(&self, handle: TypeReferenceHandle) -> Option<PrimitiveType> {
        self.type_reference(handle).primitive_type(self)
    }

    pub fn type_symbol(&self, handle: TypeReferenceHandle) -> SymbolHandle {
        self.type_reference(handle).type_symbol(self)
    }

    pub fn copy_from(
        &mut self,
        source: &TypeReferenceTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        type_reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        match source.type_reference(type_reference) {
            TypeReferenceNode::Reference {
                referee,
                is_mutable,
                is_relaxed,
            } => {
                let referee =
                    self.copy_from(source, source_expressions, target_expressions, *referee);
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    is_mutable: *is_mutable,
                    is_relaxed: *is_relaxed,
                })
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_type =
                    self.copy_from(source, source_expressions, target_expressions, *base_type);
                let constraints = self.copy_constraints_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *constraints,
                );
                self.insert(TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                })
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_type = self.copy_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *element_type,
                );
                self.insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: length.clone(),
                })
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = self.copy_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *element_type,
                );
                self.insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
            } => {
                let arguments = self.copy_type_reference_handles_from(
                    source,
                    source_expressions,
                    target_expressions,
                    *arguments,
                );
                self.insert(TypeReferenceNode::Generic {
                    base_symbol: *base_symbol,
                    base_name: base_name.clone(),
                    arguments,
                })
            }
            TypeReferenceNode::DynamicTrait { symbol, name } => {
                self.insert(TypeReferenceNode::DynamicTrait {
                    symbol: *symbol,
                    name: name.clone(),
                })
            }
            TypeReferenceNode::Named { symbol, name } => self.insert(TypeReferenceNode::Named {
                symbol: *symbol,
                name: name.clone(),
            }),
            TypeReferenceNode::Unit => self.insert(TypeReferenceNode::Unit),
        }
    }

    fn copy_type_reference_handles_from(
        &mut self,
        source: &TypeReferenceTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        type_references: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        let source_type_references = source.type_reference_handles(type_references);
        let copied = self.reserve_type_reference_handles(
            source_type_references
                .len()
                .try_into()
                .expect("type reference handle span count overflow"),
        );

        for (offset, type_reference) in source_type_references.iter().enumerate() {
            let type_reference = self.copy_from(
                source,
                source_expressions,
                target_expressions,
                *type_reference,
            );
            self.set_type_reference_handle_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("type reference handle span count overflow"),
                type_reference,
            );
        }

        copied
    }

    fn copy_constraints_from(
        &mut self,
        source: &TypeReferenceTable,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        let source_constraints = source.constraints(constraints);
        let copied = self.reserve_constraints(
            source_constraints
                .len()
                .try_into()
                .expect("type constraint span count overflow"),
        );

        for (offset, constraint) in source_constraints.iter().enumerate() {
            self.set_constraint_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("type constraint span count overflow"),
                constraint.copy_from(source_expressions, target_expressions),
            );
        }

        copied
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
        base_symbol: SymbolHandle,
        base_name: Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    DynamicTrait {
        symbol: SymbolHandle,
        name: Identifier,
    },
    Named {
        symbol: SymbolHandle,
        name: Identifier,
    },
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter {
        symbol: SymbolHandle,
        name: Identifier,
    },
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
            Self::ConstParameter { name, .. } => write!(formatter, "{name}"),
        }
    }
}

impl Default for TypeReferenceNode {
    fn default() -> Self {
        Self::Unit
    }
}

impl TypeReferenceNode {
    pub fn primitive_type(&self, table: &TypeReferenceTable) -> Option<PrimitiveType> {
        match self {
            TypeReferenceNode::Reference { .. } => None,
            TypeReferenceNode::Constrained { base_type, .. } => table.primitive_type(*base_type),
            TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => None,
        }
    }

    pub fn type_symbol(&self, table: &TypeReferenceTable) -> SymbolHandle {
        match self {
            TypeReferenceNode::Reference { referee, .. } => table.type_symbol(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => table.type_symbol(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. } => table.type_symbol(*element_type),
            TypeReferenceNode::Slice { element_type } => table.type_symbol(*element_type),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => {
                if PrimitiveType::from_name(base_name.as_str()).is_some() {
                    SymbolHandle::invalid()
                } else {
                    *base_symbol
                }
            }
            TypeReferenceNode::DynamicTrait { symbol, .. } => *symbol,
            TypeReferenceNode::Named { symbol, name } => {
                if PrimitiveType::from_name(name.as_str()).is_some() {
                    SymbolHandle::invalid()
                } else {
                    *symbol
                }
            }
            TypeReferenceNode::Unit => SymbolHandle::invalid(),
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
}

impl TypeConstraintNode {
    fn copy_from(
        &self,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
    ) -> Self {
        match self {
            TypeConstraintNode::Named(name) => Self::Named(name.clone()),
            TypeConstraintNode::Range { minimum, maximum } => Self::Range {
                minimum: target_expressions.copy_from(source_expressions, *minimum),
                maximum: target_expressions.copy_from(source_expressions, *maximum),
            },
        }
    }
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(Identifier::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Bool,
    F32,
    F64,
    I32,
    String,
    U32,
    U64,
    Usize,
}

impl PrimitiveType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "i32" => Some(Self::I32),
            "String" => Some(Self::String),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "usize" => Some(Self::Usize),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I32 => "i32",
            Self::String => "String",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
        }
    }

    pub fn accepts_integer_literal(self) -> bool {
        matches!(self, Self::I32 | Self::U32 | Self::U64 | Self::Usize)
    }

    pub fn accepts_float_literal(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    pub fn accepts_range_constraint(self) -> bool {
        matches!(
            self,
            Self::F32 | Self::F64 | Self::I32 | Self::U32 | Self::U64 | Self::Usize
        )
    }

    pub fn accepts_finite_constraint(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}
