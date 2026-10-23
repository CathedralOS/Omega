use arena::{Arena, Handle, HandleSpan};
use std::fmt;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

use crate::name::DiagnosticName;

mod display;
#[cfg(test)]
mod tests;

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

/// A supplied subtree rejoins its declaration after symbol allocation. The
/// row is shared by both resolved carrier forms; zero means ordinary lexical
/// ownership. It never supplies a lifetime binder on behalf of another owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperandLifetimeOwner {
    pub declaration: SymbolHandle,
}
pub type OperandLifetimeOwnerHandle = Handle<OperandLifetimeOwner>;

/// One authored generic application and the exact closed occurrence replacing
/// it. Declaration exposure is supplied by the containing typed source context,
/// never by the visibility of the synthesized data declaration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenericApplicationOrigin {
    pub instance: Handle<TypeReference>,
    pub application: Handle<TypeReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    /// `dyn Trait<Args>`: a runtime selector of one named implementation of
    /// this exact trait application. `symbol` is the trait; the arguments are
    /// ordinary type positions, resolved like a generic application's.
    DynamicTrait {
        symbol: SymbolHandle,
        name: DiagnosticName,
        arguments: HandleSpan<TypeReference>,
        supplied_operand_owner: OperandLifetimeOwnerHandle,
    },
    Named {
        symbol: SymbolHandle,
        name: DiagnosticName,
        supplied_operand_owner: OperandLifetimeOwnerHandle,
    },
    #[default]
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTypeReference {
    pub storage: ReferenceTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReferenceTypeReferenceStorage {
    pub supplied_operand_owner: OperandLifetimeOwnerHandle,
    pub referee: Handle<TypeReference>,
    pub access: language_core::ReferenceAccess,
    /// Explicit lifetime name (`&'buf T`), frozen decision 15 stage 2. A
    /// borrow-region tag only — no symbol, ignored by layout/codegen and by
    /// structural type equality; consulted solely by the borrow checker.
    pub lifetime: Option<DiagnosticName>,
}

deref_to_storage!(ReferenceTypeReference => ReferenceTypeReferenceStorage);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstrainedTypeReference {
    pub storage: ConstrainedTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstrainedTypeReferenceStorage {
    pub supplied_operand_owner: OperandLifetimeOwnerHandle,
    pub base_type: Handle<TypeReference>,
    pub constraints: HandleSpan<TypeConstraint>,
}

deref_to_storage!(ConstrainedTypeReference => ConstrainedTypeReferenceStorage);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedArrayTypeReference {
    pub storage: FixedArrayTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FixedArrayTypeReferenceStorage {
    pub supplied_operand_owner: OperandLifetimeOwnerHandle,
    pub element_type: Handle<TypeReference>,
    pub length: FixedArrayLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    /// Exact symbolic constant-position use, pending owned evaluation.
    Expression(Handle<TypeReference>),
    ConstParameter {
        symbol: SymbolHandle,
        name: DiagnosticName,
    },
    /// A zero-argument machine call in length position (`[u8; table_size()]`),
    /// const-evaluated before layout (comptime stage 1).
    ConstCall {
        machine: SymbolHandle,
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
            Self::Expression(_) => formatter.write_str("<constant expression>"),
            Self::ConstParameter { name, .. } => write!(formatter, "{name}"),
            Self::ConstCall { name, .. } => write!(formatter, "{name}()"),
        }
    }
}

deref_to_storage!(FixedArrayTypeReference => FixedArrayTypeReferenceStorage);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceTypeReference {
    pub storage: SliceTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SliceTypeReferenceStorage {
    pub supplied_operand_owner: OperandLifetimeOwnerHandle,
    pub element_type: Handle<TypeReference>,
}

deref_to_storage!(SliceTypeReference => SliceTypeReferenceStorage);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericTypeReference {
    pub storage: GenericTypeReferenceStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericTypeReferenceStorage {
    pub supplied_operand_owner: OperandLifetimeOwnerHandle,
    pub base_symbol: SymbolHandle,
    pub base_name: DiagnosticName,
    /// Erased borrow-region arguments. These remain diagnostic names rather
    /// than symbols and do not participate in runtime generic identity.
    pub lifetime_arguments: Vec<DiagnosticName>,
    pub arguments: HandleSpan<TypeReference>,
}

deref_to_storage!(GenericTypeReference => GenericTypeReferenceStorage);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReferenceTable {
    nodes: TypeReferenceNodeStorage,
    spans: TypeReferenceSpanStorage,
    supplied_operand_owners: Arena<SuppliedOperandOwner>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SuppliedOperandOwner {
    reference: TypeReferenceHandle,
    owner: OperandLifetimeOwnerHandle,
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
            supplied_operand_owners: Arena::new(),
        }
    }

    pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
        self.nodes.type_references.insert(type_reference)
    }

    pub fn supplied_operand_owner(
        &self,
        reference: TypeReferenceHandle,
    ) -> OperandLifetimeOwnerHandle {
        self.supplied_operand_owners
            .iter()
            .find_map(|(_, row)| (row.reference == reference).then_some(row.owner))
            .unwrap_or_default()
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
            .get_mut(type_references.handle_at(offset)) = type_reference;
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
        *self
            .spans
            .constraints
            .get_mut(constraints.handle_at(offset)) = constraint;
    }

    fn insert_type_reference_handle_span_from_trees(
        &mut self,
        type_references: &[TypeReference],
        expressions: &mut crate::expression::ExpressionTable,
        source_generic_arguments: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: crate::expression::ExpressionSource<'_>,
    ) -> HandleSpan<TypeReferenceHandle> {
        let span = self.reserve_type_reference_handles(
            type_references
                .len()
                .try_into()
                .expect("type reference handle span count overflow"),
        );

        for (offset, type_reference) in type_references.iter().enumerate() {
            let type_reference = self.insert_tree_from_source(
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
        source_expressions: crate::expression::ExpressionSource<'_>,
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

    pub fn insert_tree(
        &mut self,
        type_reference: &TypeReference,
        expressions: &mut crate::expression::ExpressionTable,
        source_generic_arguments: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: &crate::expression::ExpressionTable,
    ) -> TypeReferenceHandle {
        self.insert_tree_from_source(
            type_reference,
            expressions,
            source_generic_arguments,
            source_constraints,
            crate::expression::ExpressionSource::Table(source_expressions),
        )
    }

    pub(crate) fn insert_tree_from_source(
        &mut self,
        type_reference: &TypeReference,
        expressions: &mut crate::expression::ExpressionTable,
        source_generic_arguments: &Arena<TypeReference>,
        source_constraints: &Arena<TypeConstraint>,
        source_expressions: crate::expression::ExpressionSource<'_>,
    ) -> TypeReferenceHandle {
        let reference = match type_reference {
            TypeReference::Reference(reference) => {
                let referee = self.insert_tree_from_source(
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
                let base_type = self.insert_tree_from_source(
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
                let element_type = self.insert_tree_from_source(
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
                let element_type = self.insert_tree_from_source(
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
                let expression = source_expressions.copy_into(expressions, *expression);
                self.insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReference::DynamicTrait {
                symbol,
                name,
                arguments,
                ..
            } => {
                let arguments = self.insert_type_reference_handle_span_from_trees(
                    source_generic_arguments.span_or_empty(*arguments),
                    expressions,
                    source_generic_arguments,
                    source_constraints,
                    source_expressions,
                );
                self.insert(TypeReferenceNode::DynamicTrait {
                    symbol: *symbol,
                    name: name.clone(),
                    arguments,
                })
            }
            TypeReference::Named { symbol, name, .. } => self.insert(TypeReferenceNode::Named {
                symbol: *symbol,
                name: name.clone(),
            }),
            TypeReference::Unit => self.insert(TypeReferenceNode::Unit),
        };
        let owner = type_reference.supplied_operand_owner();
        if owner.is_valid() {
            self.supplied_operand_owners
                .insert(SuppliedOperandOwner { reference, owner });
        }
        reference
    }
}

impl TypeReference {
    pub fn supplied_operand_owner(&self) -> OperandLifetimeOwnerHandle {
        match self {
            Self::Reference(reference) => reference.supplied_operand_owner,
            Self::Constrained(reference) => reference.supplied_operand_owner,
            Self::FixedArray(reference) => reference.supplied_operand_owner,
            Self::Slice(reference) => reference.supplied_operand_owner,
            Self::Generic(reference) => reference.supplied_operand_owner,
            Self::Named {
                supplied_operand_owner,
                ..
            }
            | Self::DynamicTrait {
                supplied_operand_owner,
                ..
            } => *supplied_operand_owner,
            Self::ConstExpression(_) | Self::Unit => OperandLifetimeOwnerHandle::invalid(),
        }
    }

    pub fn retain_supplied_operand_owner(&mut self, owner: OperandLifetimeOwnerHandle) {
        match self {
            Self::Reference(reference) => reference.supplied_operand_owner = owner,
            Self::Constrained(reference) => reference.supplied_operand_owner = owner,
            Self::FixedArray(reference) => reference.supplied_operand_owner = owner,
            Self::Slice(reference) => reference.supplied_operand_owner = owner,
            Self::Generic(reference) => reference.supplied_operand_owner = owner,
            Self::Named {
                supplied_operand_owner,
                ..
            }
            | Self::DynamicTrait {
                supplied_operand_owner,
                ..
            } => *supplied_operand_owner = owner,
            Self::ConstExpression(_) | Self::Unit => {}
        }
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
        access: language_core::ReferenceAccess,
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
    /// Table-backed form of [`TypeReference::DynamicTrait`].
    DynamicTrait {
        symbol: SymbolHandle,
        name: DiagnosticName,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    Named {
        symbol: SymbolHandle,
        name: DiagnosticName,
    },
    #[default]
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    Named(DiagnosticName),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
        end_inclusive: bool,
    },
    ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
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
        end_inclusive: bool,
    },
    ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
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
        source_expressions: crate::expression::ExpressionSource<'_>,
    ) -> Self {
        match constraint {
            TypeConstraint::Named(name) => Self::Named(name.clone()),
            TypeConstraint::Range {
                minimum,
                maximum,
                end_inclusive,
            } => Self::Range {
                minimum: source_expressions.copy_into(expressions, *minimum),
                maximum: source_expressions.copy_into(expressions, *maximum),
                end_inclusive: *end_inclusive,
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
    /// A pointer-width address, distinct from counts (see
    /// wiki/spec/language/counts_and_addresses.md). Naive
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

    /// The fixed-range integer carriers: every range-constrained spelling
    /// except the two finite floats.
    pub fn accepts_integer_literal(self) -> bool {
        self.accepts_range_constraint() && !self.accepts_finite_constraint()
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
