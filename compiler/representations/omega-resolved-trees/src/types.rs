use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

use crate::name::ProgramName;

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeReference {
    Reference {
        referee: Box<TypeReference>,
        is_mutable: bool,
    },
    Constrained(ConstrainedTypeReference),
    FixedArray {
        element_type: Box<TypeReference>,
        length: usize,
    },
    Slice {
        element_type: Box<TypeReference>,
    },
    Generic(GenericTypeReference),
    Named {
        symbol: SymbolHandle,
        name: ProgramName,
    },
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstrainedTypeReference {
    pub storage: ConstrainedTypeReferenceStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstrainedTypeReferenceStorage {
    pub base_type: Box<TypeReference>,
    pub constraints: HandleSpan<TypeConstraint>,
}

impl Default for ConstrainedTypeReferenceStorage {
    fn default() -> Self {
        Self {
            base_type: Box::new(TypeReference::Unit),
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
pub struct GenericTypeReference {
    pub storage: GenericTypeReferenceStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenericTypeReferenceStorage {
    pub base_symbol: SymbolHandle,
    pub base_name: ProgramName,
    pub arguments: Vec<TypeReference>,
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
        self.spans.type_reference_handles.insert_many(type_references)
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.spans.constraints.insert_many(constraints)
    }

    fn insert_type_reference_handle_span_from_trees<'type_reference>(
        &mut self,
        type_references: impl IntoIterator<Item = &'type_reference TypeReference>,
        expressions: &mut crate::expression::ExpressionTable,
        source_constraints: &Arena<TypeConstraint>,
    ) -> HandleSpan<TypeReferenceHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for type_reference in type_references {
            let type_reference = self.insert_tree(type_reference, expressions, source_constraints);
            let handle = self.spans.type_reference_handles.append(type_reference);
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
        constraints: HandleSpan<TypeConstraint>,
        expressions: &mut crate::expression::ExpressionTable,
        source_constraints: &Arena<TypeConstraint>,
    ) -> HandleSpan<TypeConstraintNode> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for constraint in source_constraints.span_or_empty(constraints) {
            let handle = self
                .spans
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
        source_constraints: &Arena<TypeConstraint>,
    ) -> TypeReferenceHandle {
        match type_reference {
            TypeReference::Reference {
                referee,
                is_mutable,
            } => {
                let referee = self.insert_tree(referee, expressions, source_constraints);
                self.insert(TypeReferenceNode::Reference {
                    referee,
                    is_mutable: *is_mutable,
                })
            }
            TypeReference::Constrained(constrained) => {
                let base_type =
                    self.insert_tree(&constrained.base_type, expressions, source_constraints);
                let constraints = self.insert_constraint_span_from_tree(
                    constrained.constraints,
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
                let element_type = self.insert_tree(element_type, expressions, source_constraints);
                self.insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: *length,
                })
            }
            TypeReference::Slice { element_type } => {
                let element_type = self.insert_tree(element_type, expressions, source_constraints);
                self.insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReference::Generic(generic) => {
                let arguments = self.insert_type_reference_handle_span_from_trees(
                    &generic.arguments,
                    expressions,
                    source_constraints,
                );
                self.insert(TypeReferenceNode::Generic {
                    base_symbol: generic.base_symbol,
                    base_name: generic.base_name.clone(),
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
            TypeReference::Constrained(constrained) => {
                format!(
                    "{}[{}]",
                    constrained.base_type.display_name(),
                    match constrained.constraints.count() {
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
            TypeReference::Generic(generic) => {
                let arguments =
                    comma_join_display(generic.arguments.iter(), TypeReference::display_name);
                format!("{}<{arguments}>", generic.base_name)
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
            TypeReference::Constrained(constrained) => {
                let constraints =
                    type_constraints.span(constrained.constraints).unwrap_or(&[]);
                format!(
                    "{}[{}]",
                    constrained.base_type.display_name_with_constraints(type_constraints),
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
                format!("[{}]", element_type.display_name_with_constraints(type_constraints))
            }
            TypeReference::Generic(generic) => {
                let arguments = comma_join_display(generic.arguments.iter(), |argument| {
                    argument.display_name_with_constraints(type_constraints)
                });
                format!("{}<{arguments}>", generic.base_name)
            }
            TypeReference::Named { name, .. } => name.to_string(),
            TypeReference::Unit => "()".to_owned(),
        }
    }

    pub fn primitive_type(&self) -> Option<PrimitiveType> {
        match self {
            TypeReference::Reference { .. } => None,
            TypeReference::Constrained(constrained) => constrained.base_type.primitive_type(),
            TypeReference::Named { name, .. } => PrimitiveType::from_name(name),
            TypeReference::FixedArray { .. }
            | TypeReference::Slice { .. }
            | TypeReference::Generic(_)
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
        ConstrainedTypeReference, ConstrainedTypeReferenceStorage, GenericTypeReference,
        GenericTypeReferenceStorage, TypeConstraint, TypeConstraintNode, TypeReference,
        TypeReferenceNode, TypeReferenceTable,
    };
    use crate::expression::{Expression, ExpressionTable};
    use crate::name::ProgramName;
    use omega_core::arena::Arena;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn type_reference_table_stores_nested_typed_references_as_handles() {
        let type_reference = TypeReference::Generic(GenericTypeReference {
            storage: GenericTypeReferenceStorage {
                base_symbol: SymbolHandle::invalid(),
                base_name: ProgramName::generated("Result"),
                arguments: vec![
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
                ],
            },
        });

        let source_constraints = Arena::<TypeConstraint>::new();
        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let root = types.insert_tree(&type_reference, &mut expressions, &source_constraints);

        assert_eq!(types.type_reference_count(), 4);
        let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
            panic!("root type reference should be generic");
        };

        assert_eq!(arguments.count(), 2);
    }

    #[test]
    fn type_reference_table_stores_typed_constraints_as_expression_handles() {
        let mut source_constraints = Arena::<TypeConstraint>::new();
        let constraints = source_constraints.insert_many([TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(10),
        }]);
        let type_reference = TypeReference::Constrained(ConstrainedTypeReference {
            storage: ConstrainedTypeReferenceStorage {
                base_type: Box::new(TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: ProgramName::generated("i32"),
                }),
                constraints,
            },
        });

        let mut expressions = ExpressionTable::new();
        let mut types = TypeReferenceTable::new();
        let root = types.insert_tree(&type_reference, &mut expressions, &source_constraints);

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
