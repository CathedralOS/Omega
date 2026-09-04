use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;
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

    /// Every fixed-array type reference in the table, as `(handle, length)`
    /// pairs. The orchestration const-eval pass scans these for `ConstCall`
    /// lengths to substitute before checking/layout.
    pub fn fixed_array_lengths(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, &FixedArrayLength)> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::FixedArray { length, .. } => Some((handle, length)),
                _ => None,
            })
    }

    /// Every NAMED type reference in the table, as `(handle, symbol)` pairs.
    /// The machine-monomorphization pass scans these for generic type-parameter
    /// references to substitute before checking/layout (a type parameter's
    /// symbol is unique to its declaring machine, so symbol equality exactly
    /// identifies the occurrences to substitute).
    pub fn named_references(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, SymbolHandle, &str)> + '_ {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Named { symbol, name } => Some((handle, *symbol, name.as_str())),
                _ => None,
            })
    }

    /// Replace a type reference's node wholesale -- the machine-monomorphization
    /// substitution: a `Named` type-parameter reference becomes a copy of the
    /// inferred argument's node. Compound argument nodes share their child
    /// handles, which is sound (children are never mutated by the pass).
    pub fn substitute_node(&mut self, handle: TypeReferenceHandle, node: TypeReferenceNode) {
        *self.type_references.get_mut(handle) = node;
    }

    /// Replace a fixed-array type reference's length with a concrete literal
    /// (the comptime const-eval substitution). Panics if `handle` is not a
    /// fixed-array node -- callers obtain handles from `fixed_array_lengths`.
    pub fn set_fixed_array_length(&mut self, handle: TypeReferenceHandle, value: usize) {
        let TypeReferenceNode::FixedArray { length, .. } = self.type_references.get_mut(handle)
        else {
            panic!("set_fixed_array_length called on a non-fixed-array type reference");
        };
        *length = FixedArrayLength::Literal(value);
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

    /// Every constrained type's carrier and constraint span. Used by the
    /// post-lowering domain-normalization pass once all declarations exist.
    pub fn constrained_type_references(
        &self,
    ) -> Vec<(TypeReferenceHandle, HandleSpan<TypeConstraintNode>)> {
        self.type_references
            .iter()
            .filter_map(|(_, node)| match node {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => Some((*base_type, *constraints)),
                _ => None,
            })
            .collect()
    }

    /// Every constrained node together with its carrier and constraint span.
    /// The node handle permits pre-normalization rewrites such as transparent
    /// alias expansion.
    pub fn constrained_type_reference_sites(
        &self,
    ) -> Vec<(
        TypeReferenceHandle,
        TypeReferenceHandle,
        HandleSpan<TypeConstraintNode>,
    )> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => Some((handle, *base_type, *constraints)),
                _ => None,
            })
            .collect()
    }

    /// Every proof-static open index expression retained in type position.
    /// Callers use the node handle to scope specialization rewrites to newly
    /// cloned type-reference regions without scanning diagnostic renderings.
    pub fn const_expression_sites(
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

    pub fn set_constraint_span(
        &mut self,
        handle: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    ) {
        let TypeReferenceNode::Constrained {
            constraints: current,
            ..
        } = self.type_references.get_mut(handle)
        else {
            panic!("set_constraint_span called on a non-constrained type reference");
        };
        *current = constraints;
    }

    pub fn constraints_mut(
        &mut self,
        span: HandleSpan<TypeConstraintNode>,
    ) -> &mut [TypeConstraintNode] {
        self.constraints.span_mut_or_empty(span)
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

    pub fn find_named_type_reference(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
        self.type_references
            .iter()
            .find_map(|(handle, type_reference)| match type_reference {
                TypeReferenceNode::Named {
                    symbol: candidate, ..
                } if *candidate == symbol => Some(handle),
                _ => None,
            })
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

    /// The terminal nominal name of a reference, looking through reference and
    /// constraint shells. Compound structural types deliberately have no
    /// nominal name: callers that need their meaning must inspect the type
    /// reference rather than fall back to a diagnostic spelling.
    pub fn named_type(&self, handle: TypeReferenceHandle) -> Option<&Identifier> {
        if !handle.is_valid() {
            return None;
        }
        match self.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => self.named_type(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => self.named_type(*base_type),
            TypeReferenceNode::Named { name, .. } => Some(name),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => None,
        }
    }

    /// True when the type bottoms out in a borrowed byte slice `&[u8]` (a
    /// `Slice` whose element is `u8`, looking through a leading reference /
    /// constraints). This is the honest zero-copy RAW-bytes/text view -- a
    /// length-prefixed window into a buffer -- as opposed to an owned `[u8; N]`
    /// (a `FixedArray`, which the wire layer packs as a repeated field). It is
    /// the spelling that replaces the retired `&string` for borrowed wire text.
    pub fn is_borrowed_byte_slice(&self, handle: TypeReferenceHandle) -> bool {
        let mut handle = handle;
        loop {
            if !handle.is_valid() {
                return false;
            }
            match self.type_reference(handle) {
                TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                TypeReferenceNode::Slice { element_type } => {
                    return self.primitive_type(*element_type) == Some(PrimitiveType::U8);
                }
                _ => return false,
            }
        }
    }

    /// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17)
    /// declared on this type, looking through a leading reference / nested
    /// constraints. `Exact` when none is declared.
    pub fn arithmetic_domain(
        &self,
        handle: TypeReferenceHandle,
    ) -> psi_numerics::arithmetic::ArithmeticDomain {
        self.type_reference(handle).arithmetic_domain(self)
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
                access,
                lifetime,
            } => {
                let referee =
                    self.copy_from(source, source_expressions, target_expressions, *referee);
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    access: *access,
                    lifetime: lifetime.clone(),
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
                lifetime_arguments,
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
                    lifetime_arguments: lifetime_arguments.clone(),
                    arguments,
                })
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let expression = target_expressions.copy_from(source_expressions, *expression);
                self.insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReferenceNode::DynamicTrait {
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
            TypeReferenceNode::Named { symbol, name } => self.insert(TypeReferenceNode::Named {
                symbol: *symbol,
                name: name.clone(),
            }),
            TypeReferenceNode::Unit => self.insert(TypeReferenceNode::Unit),
        }
    }

    /// Remap lexical symbols reachable from a type reference copied into a
    /// fresh scope. Constraint expressions remain owned by the expression
    /// table and are remapped there.
    pub fn remap_symbols_in(
        &mut self,
        type_reference: TypeReferenceHandle,
        expressions: &mut crate::expression::ExpressionTable,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        if !type_reference.is_valid() {
            return;
        }
        let node = self.type_reference(type_reference).clone();
        match node {
            TypeReferenceNode::Reference { referee, .. } => {
                self.remap_symbols_in(referee, expressions, symbols);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.remap_symbols_in(base_type, expressions, symbols);
                for constraint in self.constraints(constraints).to_vec() {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            for argument in domain.arguments {
                                self.remap_symbols_in(argument, expressions, symbols);
                            }
                        }
                        TypeConstraintNode::Range { minimum, maximum } => {
                            expressions.remap_symbols_in(minimum, symbols);
                            expressions.remap_symbols_in(maximum, symbols);
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                self.remap_symbols_in(element_type, expressions, symbols);
                if let FixedArrayLength::ConstParameter { symbol, .. } = length
                    && let TypeReferenceNode::FixedArray { length, .. } =
                        self.type_references.get_mut(type_reference)
                    && let FixedArrayLength::ConstParameter {
                        symbol: current, ..
                    } = length
                {
                    *current = remapped(symbol, symbols);
                }
            }
            TypeReferenceNode::Slice { element_type } => {
                self.remap_symbols_in(element_type, expressions, symbols);
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                for argument in self.type_reference_handles(arguments).to_vec() {
                    self.remap_symbols_in(argument, expressions, symbols);
                }
                let TypeReferenceNode::Generic {
                    base_symbol: current,
                    ..
                } = self.type_references.get_mut(type_reference)
                else {
                    unreachable!();
                };
                *current = remapped(base_symbol, symbols);
            }
            TypeReferenceNode::ConstExpression(expression) => {
                expressions.remap_symbols_in(expression, symbols);
            }
            TypeReferenceNode::DynamicTrait { symbol, .. }
            | TypeReferenceNode::Named { symbol, .. } => {
                let current = match self.type_references.get_mut(type_reference) {
                    TypeReferenceNode::DynamicTrait { symbol, .. }
                    | TypeReferenceNode::Named { symbol, .. } => symbol,
                    _ => unreachable!(),
                };
                *current = remapped(symbol, symbols);
            }
            TypeReferenceNode::Unit => {}
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
            let copied_constraint = match constraint {
                TypeConstraintNode::Domain(domain) => {
                    let mut domain = domain.clone();
                    domain.arguments = domain
                        .arguments
                        .iter()
                        .map(|argument| {
                            self.copy_from(
                                source,
                                source_expressions,
                                target_expressions,
                                *argument,
                            )
                        })
                        .collect();
                    TypeConstraintNode::Domain(domain)
                }
                _ => constraint.copy_from(source_expressions, target_expressions),
            };
            self.set_constraint_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("type constraint span count overflow"),
                copied_constraint,
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

fn remapped(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(source, target)| (*source == symbol).then_some(*target))
        .unwrap_or(symbol)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TypeReferenceNode {
    Reference {
        referee: TypeReferenceHandle,
        access: psi_language_core::ReferenceAccess,
        /// Explicit lifetime name (`&'buf T`), frozen decision 15 stage 2. A
        /// borrow-region tag only — no symbol, ignored by layout/codegen and by
        /// structural type equality; consulted solely by the borrow checker
        /// (see `checks/borrows/elision.rs`).
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
        base_symbol: SymbolHandle,
        base_name: Identifier,
        /// Erased borrow-region arguments. They are retained for lifetime
        /// checking but ignored by runtime generic identity and layout.
        lifetime_arguments: Vec<Identifier>,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    /// A PDI3 proof-static expression over const binders. The expression is
    /// erased with its enclosing domain constraint but retains resolved names
    /// and selected-operation evidence for semantic identity and checking.
    ConstExpression(crate::expression::ExpressionHandle),
    DynamicTrait {
        symbol: SymbolHandle,
        name: Identifier,
        conformance: Option<SymbolHandle>,
        conformance_carrier: Option<Identifier>,
        conformance_name: Option<Identifier>,
    },
    Named {
        symbol: SymbolHandle,
        name: Identifier,
    },
    #[default]
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter {
        symbol: SymbolHandle,
        name: Identifier,
    },
    /// A zero-argument machine call in length position (`[u8; table_size()]`).
    /// The orchestration const-eval pass replaces this with `Literal` before
    /// checking/layout; downstream consumers never see it in a well-formed
    /// pipeline (comptime stage 1).
    ConstCall {
        name: Identifier,
        source_span: psi_source::SourceSpan,
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
            Self::ConstCall { name, .. } => write!(formatter, "{name}()"),
        }
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
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => None,
        }
    }

    pub fn arithmetic_domain(
        &self,
        table: &TypeReferenceTable,
    ) -> psi_numerics::arithmetic::ArithmeticDomain {
        match self {
            TypeReferenceNode::Reference { referee, .. } => table.arithmetic_domain(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => table
                .constraints(*constraints)
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::ArithmeticDomain(domain) => Some(*domain),
                    _ => None,
                })
                .unwrap_or_else(|| table.arithmetic_domain(*base_type)),
            _ => psi_numerics::arithmetic::ArithmeticDomain::Exact,
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
            TypeReferenceNode::ConstExpression(_) => SymbolHandle::invalid(),
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
    ArithmeticDomain(psi_numerics::arithmetic::ArithmeticDomain),
    /// A declared domain on a carrier (`[u8] in Utf8`); ch8.
    Domain(DomainConstraint),
}

/// One closed semantic subject for a domain constraint.
///
/// `DomainConstraint::name` is retained only for diagnostics. Compiler-owned
/// meaning must travel through this enum rather than an invalid symbol paired
/// with a significant string. A symbol-backed package declaration always
/// remains `Declared`, regardless of its diagnostic spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DomainConstraintSubject {
    #[default]
    Declared,
    Carry(psi_language_semantics::CarryPermission),
    Value(psi_language_semantics::value_domain::ValueDomain),
    OmegaLayout {
        grammar: OmegaLayoutGrammar,
    },
}

/// The closed grammar selected by a compiler-owned `OmegaLayout` constraint.
///
/// The canonical one-argument surface selects `Derived`. Explicit grammar
/// arguments remain unclassified until the language implements them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmegaLayoutGrammar {
    Derived,
}

impl OmegaLayoutGrammar {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
        }
    }
}

/// DOM1's normalized binding-site carrier. The authored short name remains
/// diagnostics-only; `subject` retains compiler-owned meaning, while a valid
/// symbol marks a declared, carrier-compatible domain resolved after all typed
/// roots exist.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainConstraint {
    pub name: Identifier,
    /// Closed canonical const atoms, or a direct const-binder leaf while a
    /// generic template remains structural. These handles live in the owning
    /// typed type-reference table and erase with the domain constraint.
    pub arguments: Vec<TypeReferenceHandle>,
    pub subject: DomainConstraintSubject,
    pub symbol: SymbolHandle,
    pub semantic_id: psi_language_semantics::SemanticDomainId,
    pub classification: Option<psi_language_semantics::DomainClassification>,
    pub predicate_body: psi_language_semantics::DomainPredicateBody,
    pub semantic_roles: psi_language_semantics::DomainSemanticRoles,
    pub establishment_routes: Vec<psi_language_semantics::DomainEstablishmentRoute>,
    /// Temporary source custody retained only until carrier-aware domain
    /// normalization can bind the authored spelling to an exact symbol and
    /// move the occurrence into the declaration-selection ledger.
    pub authored_selection: Option<AuthoredDomainConstraintSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDomainConstraintSelection {
    pub source_span: psi_source::SourceSpan,
    pub exposure:
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
}

impl DomainConstraint {
    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

impl fmt::Display for DomainConstraint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(formatter)
    }
}

impl TypeConstraintNode {
    fn copy_from(
        &self,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
    ) -> Self {
        match self {
            TypeConstraintNode::Named(name) => Self::Named(name.clone()),
            TypeConstraintNode::Domain(domain) => Self::Domain(domain.clone()),
            TypeConstraintNode::Range { minimum, maximum } => Self::Range {
                minimum: target_expressions.copy_from(source_expressions, *minimum),
                maximum: target_expressions.copy_from(source_expressions, *maximum),
            },
            TypeConstraintNode::ArithmeticDomain(domain) => Self::ArithmeticDomain(*domain),
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
            // Atomic types currently share the size/alignment of their
            // underlying primitives. Ordering legality and target instruction
            // strength are separate semantic/lowering obligations.
            "AtomicBool" => Some(Self::Bool),
            "AtomicU32" => Some(Self::U32),
            "AtomicU64" => Some(Self::U64),
            name if name.starts_with("AtomicBool#PlacedField<") => Some(Self::Bool),
            name if name.starts_with("AtomicU32#PlacedField<") => Some(Self::U32),
            name if name.starts_with("AtomicU64#PlacedField<") => Some(Self::U64),
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

    /// Whether an integer primitive is signed. `usize` is treated as unsigned.
    /// Non-integer primitives report `true` (the signed/default form), which is
    /// the safe choice for codegen that only distinguishes signed vs unsigned
    /// integer machine operations.
    pub fn is_signed_integer(self) -> bool {
        !matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::Addr
        )
    }

    /// Byte size of a scalar primitive. Single source of truth for the backend's
    /// scalar-width decisions (conversions, guard operands, and storage layout).
    pub fn scalar_byte_size(self) -> Option<usize> {
        match self {
            Self::Bool | Self::I8 | Self::U8 => Some(1),
            Self::I16 | Self::U16 => Some(2),
            Self::F32 | Self::I32 | Self::U32 => Some(4),
            Self::F64 | Self::I64 | Self::U64 | Self::Addr => Some(8),
        }
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
