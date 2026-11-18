use crate::item::*;
use std::ops::Deref;

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct ConstSyntax {
    pub scope: Identifier,
    pub name: Identifier,
    pub is_public: bool,
    pub type_reference: TypeReferenceHandle,
    pub initializer: ExpressionHandle,
}

/// A constant occurrence and the optional work attached to it during preparation.
/// Fresh parser output carries only the declaration. Copies preserve preparation
/// on this occurrence and remap the contained expression and type handles.
#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct ConstDefinition {
    pub declaration: ConstSyntax,
    pub(crate) preparation: Option<Box<ConstPreparation>>,
}

impl From<ConstSyntax> for ConstDefinition {
    fn from(declaration: ConstSyntax) -> Self {
        Self {
            declaration,
            preparation: None,
        }
    }
}

impl Deref for ConstDefinition {
    type Target = ConstSyntax;

    fn deref(&self) -> &Self::Target {
        &self.declaration
    }
}

impl ConstDefinition {
    /// The expression consumed by resolution. Preparation retains the materialized
    /// expression separately so inspection can still traverse the original source.
    pub fn value(&self) -> ExpressionHandle {
        self.preparation()
            .materialized_initializer
            .unwrap_or(self.declaration.initializer)
    }

    pub fn preparation(&self) -> &ConstPreparation {
        self.preparation
            .as_deref()
            .unwrap_or(&ConstPreparation::EMPTY)
    }

    pub fn preparation_mut(&mut self) -> &mut ConstPreparation {
        self.preparation.get_or_insert_with(Box::default)
    }
}
