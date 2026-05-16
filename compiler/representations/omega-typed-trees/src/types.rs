use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

use crate::name::ProgramName;

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Reference {
        referee: Box<TypeReference>,
        is_mutable: bool,
    },
    Constrained {
        base_type: Box<TypeReference>,
        constraints: HandleSpan<TypeConstraint>,
    },
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Slice {
        element_type: Box<TypeReference>,
    },
    Generic {
        base_symbol: SymbolHandle,
        base_name: ProgramName,
        arguments: HandleSpan<TypeReference>,
    },
    Named {
        symbol: SymbolHandle,
        name: ProgramName,
    },
    Unit,
}

impl Default for TypeReference {
    fn default() -> Self {
        Self::Unit
    }
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
        source_constraints: &Arena<TypeConstraint>,
        source_arguments: &Arena<TypeReference>,
    ) -> HandleSpan<TypeReferenceHandle> {
        let mut handles = HandleSpan::empty();

        for type_reference in type_references {
            let type_reference = self.insert_tree(
                type_reference,
                expressions,
                source_constraints,
                source_arguments,
            );
            self.type_reference_handles
                .append_to_span(&mut handles, type_reference);
        }

        handles
    }

    fn insert_constraint_span_from_tree(
        &mut self,
        constraints: HandleSpan<TypeConstraint>,
        expressions: &mut crate::expression::ExpressionTable,
        source_constraints: &Arena<TypeConstraint>,
    ) -> HandleSpan<TypeConstraintNode> {
        let mut nodes = HandleSpan::empty();

        for constraint in source_constraints.span_or_empty(constraints) {
            self.constraints.append_to_span(
                &mut nodes,
                TypeConstraintNode::from_tree(constraint, expressions),
            );
        }

        nodes
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
            } => {
                let referee =
                    self.copy_from(source, source_expressions, target_expressions, *referee);
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    is_mutable: *is_mutable,
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
                    length: *length,
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
        let mut copied = HandleSpan::empty();

        for type_reference in source.type_reference_handles(type_references) {
            let type_reference = self.copy_from(
                source,
                source_expressions,
                target_expressions,
                *type_reference,
            );
            self.type_reference_handles
                .append_to_span(&mut copied, type_reference);
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
        let mut copied = HandleSpan::empty();

        for constraint in source.constraints(constraints) {
            self.constraints.append_to_span(
                &mut copied,
                constraint.copy_from(source_expressions, target_expressions),
            );
        }

        copied
    }

    pub fn insert_tree(
        &mut self,
        type_reference: &TypeReference,
        expressions: &mut crate::expression::ExpressionTable,
        source_constraints: &Arena<TypeConstraint>,
        source_arguments: &Arena<TypeReference>,
    ) -> TypeReferenceHandle {
        match type_reference {
            TypeReference::Reference {
                referee,
                is_mutable,
            } => {
                let referee =
                    self.insert_tree(referee, expressions, source_constraints, source_arguments);
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    is_mutable: *is_mutable,
                })
            }
            TypeReference::Constrained {
                base_type,
                constraints,
            } => {
                let base_type =
                    self.insert_tree(base_type, expressions, source_constraints, source_arguments);
                let constraints = self.insert_constraint_span_from_tree(
                    *constraints,
                    expressions,
                    source_constraints,
                );
                self.insert(TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                })
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                let element_type = self.insert_tree(
                    element_type,
                    expressions,
                    source_constraints,
                    source_arguments,
                );
                self.insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: *length,
                })
            }
            TypeReference::Slice { element_type } => {
                let element_type = self.insert_tree(
                    element_type,
                    expressions,
                    source_constraints,
                    source_arguments,
                );
                self.insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReference::Generic {
                base_symbol,
                base_name,
                arguments,
            } => {
                let arguments = self.insert_type_reference_handle_span_from_trees(
                    source_arguments.span_or_empty(*arguments),
                    expressions,
                    source_constraints,
                    source_arguments,
                );
                self.insert(TypeReferenceNode::Generic {
                    base_symbol: *base_symbol,
                    base_name: base_name.clone(),
                    arguments,
                })
            }
            TypeReference::Named { symbol, name } => self.insert(TypeReferenceNode::Named {
                symbol: *symbol,
                name: name.clone(),
            }),
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
        is_mutable: bool,
    },
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
        base_symbol: SymbolHandle,
        base_name: ProgramName,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    Named {
        symbol: SymbolHandle,
        name: ProgramName,
    },
    Unit,
}

impl Default for TypeReferenceNode {
    fn default() -> Self {
        Self::Unit
    }
}

impl TypeReferenceNode {
    pub fn display_name(&self, table: &TypeReferenceTable) -> String {
        match self {
            TypeReferenceNode::Reference {
                referee,
                is_mutable,
            } => {
                let qualifier = if *is_mutable { "mut " } else { "" };
                format!("&{qualifier}{}", table.display_name(*referee))
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                format!(
                    "{}[{}]",
                    table.display_name(*base_type),
                    match constraints.count() {
                        1 => "1 constraint".to_owned(),
                        count => format!("{count} constraints"),
                    }
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                format!("[{}; {}]", table.display_name(*element_type), length)
            }
            TypeReferenceNode::Slice { element_type } => {
                format!("[{}]", table.display_name(*element_type))
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                format!(
                    "{base_name}<{}>",
                    comma_join_display(table.type_reference_handles(*arguments), |argument| {
                        table.display_name(*argument)
                    })
                )
            }
            TypeReferenceNode::Named { name, .. } => name.to_string(),
            TypeReferenceNode::Unit => "()".to_owned(),
        }
    }

    pub fn display_name_with_constraints(
        &self,
        table: &TypeReferenceTable,
        expressions: &crate::expression::ExpressionTable,
    ) -> String {
        match self {
            TypeReferenceNode::Reference {
                referee,
                is_mutable,
            } => {
                let qualifier = if *is_mutable { "mut " } else { "" };
                format!(
                    "&{qualifier}{}",
                    table.display_name_with_constraints(*referee, expressions)
                )
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                format!(
                    "{}[{}]",
                    table.display_name_with_constraints(*base_type, expressions),
                    comma_join_display(table.constraints(*constraints), |constraint| {
                        constraint.display_name(expressions)
                    })
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                format!(
                    "[{}; {}]",
                    table.display_name_with_constraints(*element_type, expressions),
                    length
                )
            }
            TypeReferenceNode::Slice { element_type } => {
                format!(
                    "[{}]",
                    table.display_name_with_constraints(*element_type, expressions)
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                format!(
                    "{base_name}<{}>",
                    comma_join_display(table.type_reference_handles(*arguments), |argument| {
                        table.display_name_with_constraints(*argument, expressions)
                    })
                )
            }
            TypeReferenceNode::Named { name, .. } => name.to_string(),
            TypeReferenceNode::Unit => "()".to_owned(),
        }
    }

    pub fn primitive_type(&self, table: &TypeReferenceTable) -> Option<PrimitiveType> {
        match self {
            TypeReferenceNode::Reference { .. } => None,
            TypeReferenceNode::Constrained { base_type, .. } => table.primitive_type(*base_type),
            TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
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
pub enum TypeConstraint {
    Named(ProgramName),
    Range {
        minimum: crate::expression::Expression,
        maximum: crate::expression::Expression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraintNode {
    Named(ProgramName),
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

    pub fn display_name(&self, expressions: &crate::expression::ExpressionTable) -> String {
        match self {
            TypeConstraintNode::Named(name) => name.to_string(),
            TypeConstraintNode::Range { minimum, maximum } => {
                format!(
                    "range<{}, {}>",
                    expressions.display_name(*minimum),
                    expressions.display_name(*maximum)
                )
            }
        }
    }

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
        Self::Named(ProgramName::default())
    }
}

impl Default for TypeConstraint {
    fn default() -> Self {
        Self::Named(ProgramName::default())
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

impl TypeReference {
    pub fn display_name(&self) -> String {
        match self {
            TypeReference::Reference {
                referee,
                is_mutable,
            } => {
                let qualifier = if *is_mutable { "mut " } else { "" };
                format!("&{qualifier}{}", referee.display_name())
            }
            TypeReference::Constrained {
                base_type,
                constraints,
            } => {
                format!(
                    "{}[{}]",
                    base_type.display_name(),
                    match constraints.count() {
                        1 => "1 constraint".to_owned(),
                        count => format!("{count} constraints"),
                    }
                )
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                format!("[{}; {}]", element_type.display_name(), length)
            }
            TypeReference::Slice { element_type } => {
                format!("[{}]", element_type.display_name())
            }
            TypeReference::Generic {
                base_name,
                arguments,
                ..
            } => {
                format!("{base_name}<{} arguments>", arguments.count())
            }
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn display_name_with_constraints(
        &self,
        type_constraints: &Arena<TypeConstraint>,
    ) -> String {
        match self {
            TypeReference::Reference {
                referee,
                is_mutable,
            } => {
                let qualifier = if *is_mutable { "mut " } else { "" };
                format!(
                    "&{qualifier}{}",
                    referee.display_name_with_constraints(type_constraints)
                )
            }
            TypeReference::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = type_constraints.span(*constraints).unwrap_or(&[]);
                format!(
                    "{}[{}]",
                    base_type.display_name_with_constraints(type_constraints),
                    comma_join_display(constraints.iter(), TypeConstraint::display_name)
                )
            }
            TypeReference::FixedArray {
                element_type,
                length,
            } => {
                format!(
                    "[{}; {}]",
                    element_type.display_name_with_constraints(type_constraints),
                    length
                )
            }
            TypeReference::Slice { element_type } => {
                format!(
                    "[{}]",
                    element_type.display_name_with_constraints(type_constraints)
                )
            }
            TypeReference::Generic {
                base_name,
                arguments,
                ..
            } => {
                format!("{base_name}<{} arguments>", arguments.count())
            }
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn primitive_type(&self) -> Option<PrimitiveType> {
        match self {
            TypeReference::Reference { .. } => None,
            TypeReference::Constrained { base_type, .. } => base_type.primitive_type(),
            TypeReference::Named { name, .. } => PrimitiveType::from_name(name),
            TypeReference::FixedArray { .. }
            | TypeReference::Slice { .. }
            | TypeReference::Generic { .. }
            | TypeReference::Unit => None,
        }
    }
}

impl TypeConstraint {
    pub fn display_name(&self) -> String {
        match self {
            TypeConstraint::Named(name) => name.to_string(),
            TypeConstraint::Range { minimum, maximum } => {
                format!(
                    "range<{}, {}>",
                    minimum.display_name(),
                    maximum.display_name()
                )
            }
        }
    }
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

fn comma_join_display<'item, I, T>(
    values: I,
    mut display_name: impl FnMut(&'item T) -> String,
) -> String
where
    I: IntoIterator<Item = &'item T>,
    T: 'item,
{
    let mut output = String::new();
    let mut first = true;

    for value in values {
        if first {
            first = false;
        } else {
            output.push_str(", ");
        }

        output.push_str(&display_name(value));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{
        PrimitiveType, TypeConstraint, TypeConstraintNode, TypeReference, TypeReferenceNode,
        TypeReferenceTable,
    };
    use crate::expression::{Expression, ExpressionTable};
    use crate::name::ProgramName;
    use omega_core::arena::Arena;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn type_reference_table_stores_nested_typed_references_as_handles() {
        let mut source_arguments = Arena::<TypeReference>::new();
        let arguments = source_arguments.insert_many([
            TypeReference::Named {
                symbol: SymbolHandle::invalid(),
                name: ProgramName::generated("usize"),
            },
            TypeReference::FixedArray {
                element_type: Box::new(TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: ProgramName::generated("u8"),
                }),
                length: 16,
            },
        ]);
        let type_reference = TypeReference::Generic {
            base_symbol: SymbolHandle::invalid(),
            base_name: ProgramName::generated("Result"),
            arguments,
        };

        let source_constraints = Arena::<TypeConstraint>::new();
        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let root = types.insert_tree(
            &type_reference,
            &mut expressions,
            &source_constraints,
            &source_arguments,
        );

        assert_eq!(types.type_reference_count(), 4);
        let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
            panic!("root type reference should be generic");
        };

        assert_eq!(arguments.count(), 2);
        assert_eq!(types.display_name(root), "Result<usize, [u8; 16]>");
    }

    #[test]
    fn type_reference_table_stores_typed_constraints_as_expression_handles() {
        let mut source_constraints = Arena::<TypeConstraint>::new();
        let constraints = source_constraints.insert_many([TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(10),
        }]);
        let type_reference = TypeReference::Constrained {
            base_type: Box::new(TypeReference::Named {
                symbol: SymbolHandle::invalid(),
                name: ProgramName::generated("i32"),
            }),
            constraints,
        };

        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let source_arguments = Arena::<TypeReference>::new();
        let root = types.insert_tree(
            &type_reference,
            &mut expressions,
            &source_constraints,
            &source_arguments,
        );

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
        assert_eq!(
            types.display_name_with_constraints(root, &expressions),
            "i32[range<0, 10>]"
        );
        assert_eq!(types.primitive_type(root), Some(PrimitiveType::I32));
        assert_eq!(types.type_symbol(root), SymbolHandle::invalid());
    }

    #[test]
    fn type_reference_table_copies_table_payloads_without_tree_roundtrip() {
        let mut source_constraints = Arena::<TypeConstraint>::new();
        let constraints = source_constraints.insert_many([TypeConstraint::Range {
            minimum: Expression::Integer(1),
            maximum: Expression::Integer(8),
        }]);
        let type_reference = TypeReference::Constrained {
            base_type: Box::new(TypeReference::FixedArray {
                element_type: Box::new(TypeReference::Named {
                    symbol: SymbolHandle::from_arena_index(11),
                    name: ProgramName::generated("u8"),
                }),
                length: 8,
            }),
            constraints,
        };

        let mut source_expressions = ExpressionTable::new();
        let mut source_types = TypeReferenceTable::new();
        let source_arguments = Arena::<TypeReference>::new();
        let source_root = source_types.insert_tree(
            &type_reference,
            &mut source_expressions,
            &source_constraints,
            &source_arguments,
        );

        let mut copied_expressions = ExpressionTable::new();
        let mut copied_types = TypeReferenceTable::new();
        let copied_root = copied_types.copy_from(
            &source_types,
            &source_expressions,
            &mut copied_expressions,
            source_root,
        );

        assert_eq!(
            copied_types.display_name_with_constraints(copied_root, &copied_expressions),
            "[u8; 8][range<1, 8>]"
        );
        assert_eq!(
            copied_types.type_reference_count(),
            source_types.type_reference_count()
        );
        assert_eq!(
            copied_expressions.expression_count(),
            source_expressions.expression_count()
        );
    }
}
