use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::name::DiagnosticName;

mod display;
#[cfg(test)]
mod tests;

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Reference(ReferenceTypeReference),
    Constrained(ConstrainedTypeReference),
    FixedArray(FixedArrayTypeReference),
    Slice(SliceTypeReference),
    Generic(GenericTypeReference),
    /// A proof-static indexed-domain argument whose value is computed from
    /// const binders. Unlike ordinary const-generic arithmetic, this survives
    /// pre-resolution so exact operand and operator symbols can participate in
    /// PDI3 semantic identity.
    ConstExpression(crate::expression::ExpressionHandle),
    DynamicTrait {
        symbol: SymbolHandle,
        name: DiagnosticName,
        conformance: Option<SymbolHandle>,
        conformance_carrier: Option<DiagnosticName>,
        conformance_name: Option<DiagnosticName>,
    },
    Named {
        symbol: SymbolHandle,
        name: DiagnosticName,
    },
    SelfType {
        symbol: SymbolHandle,
    },
    Unit,
}

impl Default for TypeReference {
    fn default() -> Self {
        Self::Unit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTypeReference {
    pub storage: ReferenceTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTypeReferenceStorage {
    pub referee: Handle<TypeReference>,
    pub access: psi_language_core::ReferenceAccess,
    /// Explicit lifetime name (`&'buf T`), frozen decision 15 stage 2. A
    /// borrow-region tag only — no symbol, ignored by layout/codegen and by
    /// structural type equality; consulted solely by the borrow checker.
    pub lifetime: Option<DiagnosticName>,
}

impl Default for ReferenceTypeReferenceStorage {
    fn default() -> Self {
        Self {
            referee: Handle::invalid(),
            access: psi_language_core::ReferenceAccess::Shared,
            lifetime: None,
        }
    }
}

impl Deref for ReferenceTypeReference {
    type Target = ReferenceTypeReferenceStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for ReferenceTypeReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstrainedTypeReference {
    pub storage: ConstrainedTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstrainedTypeReferenceStorage {
    pub base_type: Handle<TypeReference>,
    pub constraints: HandleSpan<TypeConstraint>,
}

impl Default for ConstrainedTypeReferenceStorage {
    fn default() -> Self {
        Self {
            base_type: Handle::invalid(),
            constraints: HandleSpan::empty(),
        }
    }
}

impl Deref for ConstrainedTypeReference {
    type Target = ConstrainedTypeReferenceStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for ConstrainedTypeReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedArrayTypeReference {
    pub storage: FixedArrayTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedArrayTypeReferenceStorage {
    pub element_type: Handle<TypeReference>,
    pub length: FixedArrayLength,
}

impl Default for FixedArrayTypeReferenceStorage {
    fn default() -> Self {
        Self {
            element_type: Handle::invalid(),
            length: FixedArrayLength::Literal(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter {
        symbol: SymbolHandle,
        name: DiagnosticName,
    },
    /// A zero-argument machine call in length position (`[u8; table_size()]`),
    /// const-evaluated before layout (comptime stage 1).
    ConstCall {
        name: DiagnosticName,
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
            Self::ConstCall { name } => write!(formatter, "{name}()"),
        }
    }
}

impl Deref for FixedArrayTypeReference {
    type Target = FixedArrayTypeReferenceStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for FixedArrayTypeReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceTypeReference {
    pub storage: SliceTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceTypeReferenceStorage {
    pub element_type: Handle<TypeReference>,
}

impl Default for SliceTypeReferenceStorage {
    fn default() -> Self {
        Self {
            element_type: Handle::invalid(),
        }
    }
}

impl Deref for SliceTypeReference {
    type Target = SliceTypeReferenceStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for SliceTypeReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericTypeReference {
    pub storage: GenericTypeReferenceStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericTypeReferenceStorage {
    pub base_symbol: SymbolHandle,
    pub base_name: DiagnosticName,
    /// Erased borrow-region arguments. These remain diagnostic names rather
    /// than symbols and do not participate in runtime generic identity.
    pub lifetime_arguments: Vec<DiagnosticName>,
    pub arguments: HandleSpan<TypeReference>,
}

impl Deref for GenericTypeReference {
    type Target = GenericTypeReferenceStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for GenericTypeReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReferenceTable {
    nodes: TypeReferenceNodeStorage,
    spans: TypeReferenceSpanStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeReferenceNodeStorage {
    type_references: Arena<TypeReferenceNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeReferenceSpanStorage {
    type_reference_handles: Arena<TypeReferenceHandle>,
    constraints: Arena<TypeConstraintNode>,
}

impl TypeReferenceTable {
    pub fn new() -> Self {
        Self {
            nodes: TypeReferenceNodeStorage {
                type_references: Arena::new(),
            },
            spans: TypeReferenceSpanStorage {
                type_reference_handles: Arena::new(),
                constraints: Arena::new(),
            },
        }
    }

    pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
        self.nodes.type_references.insert(type_reference)
    }

    pub fn insert_type_reference_handles(
        &mut self,
        type_references: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.spans
            .type_reference_handles
            .insert_many(type_references)
    }

    pub fn reserve_type_reference_handles(
        &mut self,
        count: u32,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.spans.type_reference_handles.insert_many(
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
        *self
            .spans
            .type_reference_handles
            .get_mut(Handle::from_parts(
                type_references
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("type reference handle index overflow"),
                type_references.start().generation(),
            )) = type_reference;
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.spans.constraints.insert_many(constraints)
    }

    pub fn reserve_constraints(&mut self, count: u32) -> HandleSpan<TypeConstraintNode> {
        self.spans.constraints.insert_many(
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
        *self.spans.constraints.get_mut(Handle::from_parts(
            constraints
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("type constraint index overflow"),
            constraints.start().generation(),
        )) = constraint;
    }

    fn insert_type_reference_handle_span_from_trees(
        &mut self,
        type_references: &[TypeReference],
        expressions: &mut crate::expression::ExpressionTable,
        source_generic_arguments: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: &crate::expression::ExpressionTable,
    ) -> HandleSpan<TypeReferenceHandle> {
        let span = self.reserve_type_reference_handles(
            type_references
                .len()
                .try_into()
                .expect("type reference handle span count overflow"),
        );

        for (offset, type_reference) in type_references.iter().enumerate() {
            let type_reference = self.insert_tree(
                type_reference,
                expressions,
                source_generic_arguments,
                source_constraints,
                source_expressions,
            );
            self.set_type_reference_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("type reference handle span count overflow"),
                type_reference,
            );
        }

        span
    }

    fn insert_constraint_span_from_tree(
        &mut self,
        constraints: HandleSpan<TypeConstraint>,
        expressions: &mut crate::expression::ExpressionTable,
        source_type_references: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: &crate::expression::ExpressionTable,
    ) -> HandleSpan<TypeConstraintNode> {
        let source = source_constraints.span_or_empty(constraints);
        let span = self.reserve_constraints(
            source
                .len()
                .try_into()
                .expect("type constraint span count overflow"),
        );

        for (offset, constraint) in source.iter().enumerate() {
            let constraint = TypeConstraintNode::from_tree(
                constraint,
                self,
                expressions,
                source_type_references,
                source_constraints,
                source_expressions,
            );
            self.set_constraint_at_offset(
                span,
                offset
                    .try_into()
                    .expect("type constraint span count overflow"),
                constraint,
            );
        }

        span
    }

    pub fn type_reference(&self, handle: TypeReferenceHandle) -> &TypeReferenceNode {
        self.nodes.type_references.get(handle)
    }

    pub fn type_reference_handles(
        &self,
        span: HandleSpan<TypeReferenceHandle>,
    ) -> &[TypeReferenceHandle] {
        self.spans.type_reference_handles.span_or_empty(span)
    }

    pub fn constraints(&self, span: HandleSpan<TypeConstraintNode>) -> &[TypeConstraintNode] {
        self.spans.constraints.span_or_empty(span)
    }

    pub fn type_reference_count(&self) -> usize {
        self.nodes.type_references.len()
    }

    pub fn constraint_count(&self) -> usize {
        self.spans.constraints.len()
    }

    pub fn insert_tree(
        &mut self,
        type_reference: &TypeReference,
        expressions: &mut crate::expression::ExpressionTable,
        source_generic_arguments: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: &crate::expression::ExpressionTable,
    ) -> TypeReferenceHandle {
        match type_reference {
            TypeReference::Reference(reference) => {
                let referee = self.insert_tree(
                    source_generic_arguments.get(reference.referee),
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    access: reference.access,
                    lifetime: reference.lifetime.clone(),
                })
            }
            TypeReference::Constrained(constrained) => {
                let base_type = self.insert_tree(
                    source_generic_arguments.get(constrained.base_type),
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                let constraints = self.insert_constraint_span_from_tree(
                    constrained.constraints,
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                self.insert(TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                })
            }
            TypeReference::FixedArray(fixed_array) => {
                let element_type = self.insert_tree(
                    source_generic_arguments.get(fixed_array.element_type),
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                self.insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: fixed_array.length.clone(),
                })
            }
            TypeReference::Slice(slice) => {
                let element_type = self.insert_tree(
                    source_generic_arguments.get(slice.element_type),
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                self.insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReference::Generic(generic) => {
                let arguments = self.insert_type_reference_handle_span_from_trees(
                    source_generic_arguments.span_or_empty(generic.arguments),
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                self.insert(TypeReferenceNode::Generic {
                    base_symbol: generic.base_symbol,
                    base_name: generic.base_name.clone(),
                    lifetime_arguments: generic.lifetime_arguments.clone(),
                    arguments,
                })
            }
            TypeReference::ConstExpression(expression) => {
                let expression = expressions.copy_from(source_expressions, *expression);
                self.insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReference::DynamicTrait {
                symbol,
                name,
                conformance,
                conformance_carrier,
                conformance_name,
            } => self.insert(TypeReferenceNode::DynamicTrait {
                symbol: *symbol,
                name: name.clone(),
                conformance: *conformance,
                conformance_carrier: conformance_carrier.clone(),
                conformance_name: conformance_name.clone(),
            }),
            TypeReference::Named { symbol, name } => self.insert(TypeReferenceNode::Named {
                symbol: *symbol,
                name: name.clone(),
            }),
            TypeReference::SelfType { symbol } => {
                self.insert(TypeReferenceNode::SelfType { symbol: *symbol })
            }
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
    Reference {
        referee: TypeReferenceHandle,
        access: psi_language_core::ReferenceAccess,
        /// Explicit lifetime name (`&'buf T`), frozen decision 15 stage 2.
        lifetime: Option<DiagnosticName>,
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
        base_name: DiagnosticName,
        lifetime_arguments: Vec<DiagnosticName>,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    /// Table-backed form of [`TypeReference::ConstExpression`].
    ConstExpression(crate::expression::ExpressionHandle),
    DynamicTrait {
        symbol: SymbolHandle,
        name: DiagnosticName,
        conformance: Option<SymbolHandle>,
        conformance_carrier: Option<DiagnosticName>,
        conformance_name: Option<DiagnosticName>,
    },
    Named {
        symbol: SymbolHandle,
        name: DiagnosticName,
    },
    SelfType {
        symbol: SymbolHandle,
    },
    Unit,
}

impl Default for TypeReferenceNode {
    fn default() -> Self {
        Self::Unit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    Named(DiagnosticName),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
    },
    ArithmeticDomain(psi_numerics::arithmetic::ArithmeticDomain),
    /// A declared domain or closed indexed-domain family application.
    Domain(DomainConstraint),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainConstraint {
    pub name: DiagnosticName,
    pub arguments: HandleSpan<TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraintNode {
    Named(DiagnosticName),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
    },
    ArithmeticDomain(psi_numerics::arithmetic::ArithmeticDomain),
    /// A declared domain or closed indexed-domain family application.
    Domain(DomainConstraintNode),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainConstraintNode {
    pub name: DiagnosticName,
    pub arguments: HandleSpan<TypeReferenceHandle>,
}

impl TypeConstraintNode {
    fn from_tree(
        constraint: &TypeConstraint,
        type_references: &mut TypeReferenceTable,
        expressions: &mut crate::expression::ExpressionTable,
        source_type_references: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: &crate::expression::ExpressionTable,
    ) -> Self {
        match constraint {
            TypeConstraint::Named(name) => Self::Named(name.clone()),
            TypeConstraint::Range { minimum, maximum } => Self::Range {
                minimum: expressions.copy_from(source_expressions, *minimum),
                maximum: expressions.copy_from(source_expressions, *maximum),
            },
            TypeConstraint::ArithmeticDomain(domain) => Self::ArithmeticDomain(*domain),
            TypeConstraint::Domain(domain) => Self::Domain(DomainConstraintNode {
                name: domain.name.clone(),
                arguments: type_references.insert_type_reference_handle_span_from_trees(
                    source_type_references.span_or_empty(domain.arguments),
                    expressions,
                    source_type_references,
                    source_constraints,
                    source_expressions,
                ),
            }),
        }
    }
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(DiagnosticName::default())
    }
}

impl Default for TypeConstraint {
    fn default() -> Self {
        Self::Named(DiagnosticName::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Bool,
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    /// A pointer-width ADDRESS, distinct from `usize`/counts (address and count
    /// are separate axes; index_count_and_address_model brief). Naive
    /// pointer-width for now -- rides the 8-byte unsigned path.
    Addr,
}

impl TypeReference {
    pub fn primitive_type(&self) -> Option<PrimitiveType> {
        match self {
            TypeReference::Reference(_) => None,
            TypeReference::Constrained(_) => None,
            TypeReference::Named { name, .. } => PrimitiveType::from_name(name),
            TypeReference::FixedArray(_)
            | TypeReference::Slice(_)
            | TypeReference::Generic(_)
            | TypeReference::ConstExpression(_)
            | TypeReference::DynamicTrait { .. }
            | TypeReference::SelfType { .. }
            | TypeReference::Unit => None,
        }
    }
}

impl PrimitiveType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bool" => Some(Self::Bool),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "addr" => Some(Self::Addr),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Addr => "addr",
        }
    }

    pub fn accepts_integer_literal(self) -> bool {
        matches!(
            self,
            Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::Addr
        )
    }

    pub fn accepts_float_literal(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    pub fn accepts_range_constraint(self) -> bool {
        matches!(
            self,
            Self::F32
                | Self::F64
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::Addr
        )
    }

    pub fn accepts_finite_constraint(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
}
