/*
We represent symbol-resolved type references (`type_system::types`) and
resolved symbol diagnostic names (`names::name`) in `SymbolResolvedTrees`.
*/

pub mod type_system {
    pub mod types {
        use arena::{Arena, Handle, HandleSpan};
        use std::fmt;
        use std::ops::{Deref, DerefMut};
        use symbols::SymbolHandle;

        use crate::name::DiagnosticName;

        pub(crate) mod display {
            use crate::types::{ConstrainedTypeReference, TypeConstraint, TypeReference};

            impl TypeReference {
                fn display_name_with(
                    &self,
                    constrained_name: impl Fn(&ConstrainedTypeReference) -> String,
                ) -> String {
                    match self {
                        TypeReference::Reference(reference) => {
                            format!("{}<type>", reference.access.borrow_prefix())
                        }
                        TypeReference::Constrained(constrained) => constrained_name(constrained),
                        TypeReference::FixedArray(fixed_array) => {
                            format!("[<type>; {}]", fixed_array.length)
                        }
                        TypeReference::Slice(slice) => {
                            let _ = slice;
                            "[<type>]".to_owned()
                        }
                        TypeReference::Generic(generic) => {
                            let arguments = match generic.arguments.count() {
                                1 => "1 argument".to_owned(),
                                count => format!("{count} arguments"),
                            };
                            format!("{}<{arguments}>", generic.base_name)
                        }
                        TypeReference::ConstExpression(_) => "const <expression>".to_owned(),
                        TypeReference::DynamicTrait {
                            name, arguments, ..
                        } => match arguments.count() {
                            0 => format!("dyn {name}"),
                            1 => format!("dyn {name}<1 argument>"),
                            count => format!("dyn {name}<{count} arguments>"),
                        },
                        TypeReference::Named { name, .. } => name.to_string(),
                        TypeReference::Unit => "()".to_owned(),
                        TypeReference::AliasUse { expansion, .. } => {
                            expansion.display_name_with(constrained_name)
                        }
                    }
                }

                pub fn display_name(&self) -> String {
                    self.display_name_with(|constrained| {
                        format!(
                            "<type>[{}]",
                            match constrained.constraints.count() {
                                1 => "1 constraint".to_owned(),
                                count => format!("{count} constraints"),
                            }
                        )
                    })
                }
            }

            impl TypeConstraint {
                pub fn display_name(&self) -> String {
                    match self {
                        TypeConstraint::Named(name) => name.to_string(),
                        TypeConstraint::Domain(domain) => {
                            let _ = domain.arguments;
                            format!("in {}", domain.name)
                        }

                        TypeConstraint::ArithmeticDomain(domain) => format!("in {}", domain.name()),
                    }
                }
            }
        }
        #[cfg(test)]
        pub(crate) mod tests {
            use super::{
                FixedArrayLength, FixedArrayTypeReference, FixedArrayTypeReferenceStorage,
                GenericTypeReference, GenericTypeReferenceStorage, TypeConstraint, TypeReference,
                TypeReferenceNode, TypeReferenceTable,
            };
            use crate::expression::ExpressionTable;
            use crate::name::DiagnosticName;
            use arena::Arena;
            use symbols::SymbolHandle;

            #[test]
            fn type_reference_table_stores_nested_typed_references_as_handles() {
                let mut source_arguments = Arena::<TypeReference>::new();
                let fixed_array_element = source_arguments.append(TypeReference::Named {
                    supplied_operand_owner: Default::default(),
                    symbol: SymbolHandle::invalid(),
                    name: DiagnosticName::generated("u8"),
                });
                let arguments = source_arguments.insert_many([
                    TypeReference::Named {
                        supplied_operand_owner: Default::default(),
                        symbol: SymbolHandle::invalid(),
                        name: DiagnosticName::generated("usize"),
                    },
                    TypeReference::FixedArray(FixedArrayTypeReference {
                        storage: FixedArrayTypeReferenceStorage {
                            supplied_operand_owner: Default::default(),
                            element_type: fixed_array_element,
                            length: FixedArrayLength::Literal(16),
                        },
                    }),
                ]);
                let type_reference = TypeReference::Generic(GenericTypeReference {
                    storage: GenericTypeReferenceStorage {
                        supplied_operand_owner: Default::default(),
                        base_symbol: SymbolHandle::invalid(),
                        base_name: DiagnosticName::generated("Result"),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    },
                });

                let source_constraints = Arena::<TypeConstraint>::new();
                let source_expressions = ExpressionTable::new();
                let mut expressions = ExpressionTable::new();
                let mut types = TypeReferenceTable::new();
                let root = types.insert_tree(
                    &type_reference,
                    &mut expressions,
                    &source_arguments,
                    &source_constraints,
                    &source_expressions,
                );

                assert_eq!(types.type_reference_count(), 4);
                let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root)
                else {
                    panic!("root type reference should be generic");
                };

                assert_eq!(arguments.count(), 2);
            }
        }

        pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OperandLifetimeOwner {
            pub declaration: SymbolHandle,
        }
        pub type OperandLifetimeOwnerHandle = Handle<OperandLifetimeOwner>;

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct GenericApplicationOrigin {
            pub instance: Handle<TypeReference>,
            pub application: Handle<TypeReference>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub enum TypeReference {
            Reference(ReferenceTypeReference),
            Constrained(ConstrainedTypeReference),
            FixedArray(FixedArrayTypeReference),
            Slice(SliceTypeReference),
            Generic(GenericTypeReference),
            ConstExpression(ConstExpressionTypeReference),
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
            AliasUse {
                expansion: Box<TypeReference>,
                application: Box<TypeReference>,
                declaration_source: source::SourceSpan,
                definition_references: Vec<source::SourceSpan>,
                supplied_operand_owner: OperandLifetimeOwnerHandle,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ConstExpressionTypeReference {
            pub expression: crate::expression::ExpressionHandle,
            pub supplied_operand_owner: OperandLifetimeOwnerHandle,
            pub lexical_owner: SymbolHandle,
        }

        impl ConstExpressionTypeReference {
            pub fn new(expression: crate::expression::ExpressionHandle) -> Self {
                Self {
                    expression,
                    ..Self::default()
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ReferenceTypeReference {
            pub storage: ReferenceTypeReferenceStorage,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct ReferenceTypeReferenceStorage {
            pub supplied_operand_owner: OperandLifetimeOwnerHandle,
            pub referee: Handle<TypeReference>,
            pub access: language_core::ReferenceAccess,
            pub lifetime: Option<DiagnosticName>,
        }

        deref_to_storage!(ReferenceTypeReference => ReferenceTypeReferenceStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ConstrainedTypeReference {
            pub storage: ConstrainedTypeReferenceStorage,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct ConstrainedTypeReferenceStorage {
            pub supplied_operand_owner: OperandLifetimeOwnerHandle,
            pub base_type: Handle<TypeReference>,
            pub constraints: HandleSpan<TypeConstraint>,
        }

        deref_to_storage!(ConstrainedTypeReference => ConstrainedTypeReferenceStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct FixedArrayTypeReference {
            pub storage: FixedArrayTypeReferenceStorage,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct FixedArrayTypeReferenceStorage {
            pub supplied_operand_owner: OperandLifetimeOwnerHandle,
            pub element_type: Handle<TypeReference>,
            pub length: FixedArrayLength,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum FixedArrayLength {
            Literal(usize),
            Expression(Handle<TypeReference>),
            ConstParameter {
                symbol: SymbolHandle,
                name: DiagnosticName,
            },
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

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct SliceTypeReference {
            pub storage: SliceTypeReferenceStorage,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct SliceTypeReferenceStorage {
            pub supplied_operand_owner: OperandLifetimeOwnerHandle,
            pub element_type: Handle<TypeReference>,
        }

        deref_to_storage!(SliceTypeReference => SliceTypeReferenceStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct GenericTypeReference {
            pub storage: GenericTypeReferenceStorage,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct GenericTypeReferenceStorage {
            pub supplied_operand_owner: OperandLifetimeOwnerHandle,
            pub base_symbol: SymbolHandle,
            pub base_name: DiagnosticName,
            pub lifetime_arguments: Vec<DiagnosticName>,
            pub arguments: HandleSpan<TypeReference>,
        }

        deref_to_storage!(GenericTypeReference => GenericTypeReferenceStorage);

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct TypeReferenceTable {
            nodes: TypeReferenceNodeStorage,
            spans: TypeReferenceSpanStorage,
            supplied_operand_owners: Arena<SuppliedOperandOwner>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        struct SuppliedOperandOwner {
            reference: TypeReferenceHandle,
            owner: OperandLifetimeOwnerHandle,
            lexical_owner: SymbolHandle,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        struct TypeReferenceNodeStorage {
            type_references: Arena<TypeReferenceNode>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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

            pub fn operand_lexical_owner(&self, reference: TypeReferenceHandle) -> SymbolHandle {
                self.supplied_operand_owners
                    .iter()
                    .find_map(|(_, row)| (row.reference == reference).then_some(row.lexical_owner))
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
                    std::iter::repeat_with(TypeReferenceHandle::invalid).take(
                        usize::try_from(count).expect("type reference handle span count overflow"),
                    ),
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

            pub fn constraints(
                &self,
                span: HandleSpan<TypeConstraintNode>,
            ) -> &[TypeConstraintNode] {
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
                        let expression =
                            source_expressions.copy_into(expressions, expression.expression);
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
                    TypeReference::Named { symbol, name, .. } => {
                        self.insert(TypeReferenceNode::Named {
                            symbol: *symbol,
                            name: name.clone(),
                        })
                    }
                    TypeReference::Unit => self.insert(TypeReferenceNode::Unit),
                    TypeReference::AliasUse {
                        expansion,
                        application,
                        declaration_source,
                        definition_references,
                        ..
                    } => {
                        let expansion = self.insert_tree_from_source(
                            expansion,
                            expressions,
                            source_generic_arguments,
                            source_constraints,
                            source_expressions,
                        );
                        let application = self.insert_tree_from_source(
                            application,
                            expressions,
                            source_generic_arguments,
                            source_constraints,
                            source_expressions,
                        );
                        self.insert(TypeReferenceNode::AliasUse {
                            expansion,
                            application,
                            declaration_source: *declaration_source,
                            definition_references: definition_references.clone(),
                        })
                    }
                };
                let owner = type_reference.supplied_operand_owner();
                let lexical_owner = type_reference.operand_lexical_owner();
                if owner.is_valid() || lexical_owner.is_valid() {
                    self.supplied_operand_owners.insert(SuppliedOperandOwner {
                        reference,
                        owner,
                        lexical_owner,
                    });
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
                    }
                    | Self::AliasUse {
                        supplied_operand_owner,
                        ..
                    } => *supplied_operand_owner,
                    Self::ConstExpression(expression) => expression.supplied_operand_owner,
                    Self::Unit => OperandLifetimeOwnerHandle::invalid(),
                }
            }

            pub fn operand_lexical_owner(&self) -> SymbolHandle {
                match self {
                    Self::ConstExpression(expression) => expression.lexical_owner,
                    Self::AliasUse { expansion, .. } => expansion.operand_lexical_owner(),
                    _ => SymbolHandle::invalid(),
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
                    }
                    | Self::AliasUse {
                        supplied_operand_owner,
                        ..
                    } => *supplied_operand_owner = owner,
                    Self::ConstExpression(expression) => expression.supplied_operand_owner = owner,
                    Self::Unit => {}
                }
            }
        }

        impl Default for TypeReferenceTable {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub enum TypeReferenceNode {
            Reference {
                referee: TypeReferenceHandle,
                access: language_core::ReferenceAccess,
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
            ConstExpression(crate::expression::ExpressionHandle),
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
            AliasUse {
                expansion: TypeReferenceHandle,
                application: TypeReferenceHandle,
                declaration_source: source::SourceSpan,
                definition_references: Vec<source::SourceSpan>,
            },
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum TypeConstraint {
            Named(DiagnosticName),

            ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
            Domain(DomainConstraint),
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct DomainConstraint {
            pub name: DiagnosticName,
            pub arguments: HandleSpan<TypeReference>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum TypeConstraintNode {
            Named(DiagnosticName),

            ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
            Domain(DomainConstraintNode),
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
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

        /* A fixed-width integer is its carrier; built-in names only spell it. */
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PrimitiveType {
            Bool,
            F32,
            F64,
            Integer(numerics::integer_carrier::FixedInteger),
            Addr,
        }

        impl TypeReference {
            pub fn primitive_type(&self) -> Option<PrimitiveType> {
                match self {
                    TypeReference::Reference(_) => None,
                    TypeReference::Constrained(_) => None,
                    TypeReference::Named { name, .. } => PrimitiveType::from_name(name),
                    TypeReference::AliasUse { expansion, .. } => expansion.primitive_type(),
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
                    "addr" => Some(Self::Addr),
                    name => numerics::integer_carrier::FixedInteger::from_decimal_spelling(name)
                        .map(Self::Integer),
                }
            }

            pub fn name(self) -> String {
                match self {
                    Self::Bool => "bool".to_owned(),
                    Self::F32 => "f32".to_owned(),
                    Self::F64 => "f64".to_owned(),
                    Self::Integer(carrier) => carrier.to_string(),
                    Self::Addr => "addr".to_owned(),
                }
            }

            pub fn accepts_integer_literal(self) -> bool {
                self.accepts_range_constraint() && !self.accepts_finite_constraint()
            }

            pub fn accepts_range_constraint(self) -> bool {
                matches!(self, Self::F32 | Self::F64 | Self::Integer(_) | Self::Addr)
            }

            pub fn accepts_finite_constraint(self) -> bool {
                matches!(self, Self::F32 | Self::F64)
            }
        }
    }
}

pub mod names {
    pub mod name {
        use std::borrow::Cow;
        use std::fmt;
        use std::ops::Deref;
        use std::sync::Arc;

        use source::SourceSpan;

        #[derive(Clone, Default, Eq, compiler_storage::Record)]
        pub struct DiagnosticName {
            text: DiagnosticNameText,
            source_span: SourceSpan,
        }

        #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        enum DiagnosticNameText {
            #[default]
            Missing,
            Static(Cow<'static, str>),
            Generated(Arc<str>),
        }

        impl DiagnosticName {
            pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
                Self::from_str(&text.into(), source_span)
            }

            pub fn from_str(text: &str, source_span: SourceSpan) -> Self {
                Self {
                    text: DiagnosticNameText::Generated(Arc::from(text)),
                    source_span,
                }
            }

            pub fn generated(text: impl Into<String>) -> Self {
                Self::new(text, SourceSpan::default())
            }

            pub fn generated_static(text: &'static str) -> Self {
                Self {
                    text: DiagnosticNameText::Static(Cow::Borrowed(text)),
                    source_span: SourceSpan::default(),
                }
            }

            pub fn as_str(&self) -> &str {
                match &self.text {
                    DiagnosticNameText::Missing => "",
                    DiagnosticNameText::Static(text) => text,
                    DiagnosticNameText::Generated(text) => text.as_ref(),
                }
            }

            pub fn source_span(&self) -> SourceSpan {
                self.source_span
            }

            pub fn is_self_receiver(&self) -> bool {
                language_core::is_self_receiver(self.as_str())
            }

            pub fn is_source_backed(&self) -> bool {
                self.source_span.span.start != self.source_span.span.end
            }
        }

        impl From<&str> for DiagnosticName {
            fn from(text: &str) -> Self {
                Self::from_str(text, SourceSpan::default())
            }
        }

        impl Deref for DiagnosticName {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for DiagnosticName {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl fmt::Debug for DiagnosticName {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.as_str(), formatter)
            }
        }

        impl PartialEq for DiagnosticName {
            fn eq(&self, other: &Self) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl PartialEq<&str> for DiagnosticName {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        impl PartialEq<str> for DiagnosticName {
            fn eq(&self, other: &str) -> bool {
                self.as_str() == other
            }
        }

        impl PartialEq<DiagnosticName> for &str {
            fn eq(&self, other: &DiagnosticName) -> bool {
                *self == other.as_str()
            }
        }

        #[cfg(test)]
        mod tests {
            use super::DiagnosticName;
            use source::{SourceSpan, Span};

            #[test]
            fn copied_names_own_spelling_and_preserve_provenance() {
                let mut spelling = String::from("naïve::name");
                let source_span = SourceSpan::new(Default::default(), Span::new(7, 19));
                let name = DiagnosticName::from_str(&spelling, source_span);
                let cloned = name.clone();
                assert_ne!(name.as_str().as_ptr(), spelling.as_ptr());
                assert_eq!(name.as_str().as_ptr(), cloned.as_str().as_ptr());
                spelling.clear();
                drop(spelling);

                assert_eq!(name.as_str(), "naïve::name");
                assert_eq!(name.source_span(), source_span);
                assert!(name.is_source_backed());
                assert_eq!(name, DiagnosticName::generated_static("naïve::name"));
                assert_eq!(name.to_string(), "naïve::name");
                assert_eq!(format!("{name:?}"), "\"naïve::name\"");
                assert_eq!(DiagnosticName::from(""), DiagnosticName::default());
            }
        }
    }
}
