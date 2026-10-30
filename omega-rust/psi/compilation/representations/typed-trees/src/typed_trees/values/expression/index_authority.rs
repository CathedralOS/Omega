//! Checked index authority belongs to its expression, not a program-wide join.
//!
//! Source preparation binds the public operation and law slots once. Subtree
//! copying carries that authority with the node; specialization substitutes its
//! type and evidence binders alongside the other expression payloads. Choosing
//! the first observed specialization would narrow the universal template to an
//! arbitrary caller's algebra. Canonical identity only reads retained authority.
//! The sparse arena avoids allocating law records for ordinary expressions.

use super::{ExpressionHandle, ExpressionNode, ExpressionTable, source_span_index};
use crate::types::TypeReferenceHandle;
use arena::Handle;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct IndexExpressionAuthority {
    pub normalization: Option<OpenIndexNormalization>,
    pub operation: Option<OpenIndexOperationSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct OpenIndexNormalization {
    pub index_type: TypeReferenceHandle,
    /// Artifact provenance only, never semantic type identity.
    pub normalizer_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct OpenIndexOperationSelection {
    pub expression: ExpressionHandle,
    pub spelling: language_core::operator_spelling::OperatorSpelling,
    pub operator: SymbolHandle,
    pub operand_type: TypeReferenceHandle,
    pub provider: SymbolHandle,
    /// Exact applied conformance operands; an unapplied provider uses its symbol.
    pub application: Option<super::StaticMachineArgument>,
    pub algebra_trait: SymbolHandle,
    pub algebra_requirement: String,
    pub algebra_alias: Option<String>,
    /// Reordering and reassociation consume different checked law slots.
    pub commutativity_licensed: bool,
    pub associativity_licensed: bool,
}

impl OpenIndexOperationSelection {
    /// Transport an already selected provider without discovering operations
    /// or granting laws. Its symbol, application and diagnostic alias form one
    /// payload; executable and contract copies must move them together.
    pub fn apply_provider(&mut self, selected: &super::StaticMachineArgument, alias: &str) {
        self.provider = selected.symbol;
        self.application = selected.application.as_ref().map(|_| selected.clone());
        self.algebra_alias = Some(alias.to_owned());
    }

    pub fn operation_contract_identity(&self, program: &crate::TypedTrees) -> String {
        let subject = program.normalized_type_identity_text(self.operand_type);
        format!(
            "trait::{}::{}({subject},{subject})->{subject}",
            program.symbols.display_path(self.algebra_trait, "::"),
            self.algebra_requirement,
        )
    }
}

impl ExpressionTable {
    pub fn index_expression_authority(
        &self,
        expression: ExpressionHandle,
    ) -> Option<&IndexExpressionAuthority> {
        if !self.expressions.is_valid(expression) {
            return None;
        }
        let handle = *self
            .index_expression_authority_handles
            .get(source_span_index(expression))?;
        handle
            .is_valid()
            .then(|| self.index_expression_authorities.get(handle))
    }

    pub fn index_expression_authority_mut(
        &mut self,
        expression: ExpressionHandle,
    ) -> &mut IndexExpressionAuthority {
        assert!(
            self.expressions.is_valid(expression),
            "index authority requires a live expression"
        );
        let position = source_span_index(expression);
        if self.index_expression_authority_handles.len() <= position {
            self.index_expression_authority_handles
                .resize(position + 1, Handle::invalid());
        }
        let handle = &mut self.index_expression_authority_handles[position];
        if !handle.is_valid() {
            *handle = self
                .index_expression_authorities
                .insert(IndexExpressionAuthority::default());
        }
        self.index_expression_authorities.get_mut(*handle)
    }

    pub fn open_index_operation(
        &self,
        expression: ExpressionHandle,
    ) -> Option<&OpenIndexOperationSelection> {
        // Folding and operator binding can replace a binary node. Historical
        // authority is not a license on the replacement expression's new kind.
        self.binary_node(expression)?;
        self.index_expression_authority(expression)?
            .operation
            .as_ref()
    }

    pub fn open_index_operations(
        &self,
        root: ExpressionHandle,
    ) -> impl Iterator<Item = &OpenIndexOperationSelection> {
        let mut pending = vec![root];
        std::iter::from_fn(move || {
            while let Some(expression) = pending.pop() {
                match self.expression(expression) {
                    ExpressionNode::Binary(binary) => {
                        pending.push(binary.right);
                        pending.push(binary.left);
                    }
                    ExpressionNode::Unary(unary) => pending.push(unary.operand),
                    _ => {}
                }
                if let Some(operation) = self.open_index_operation(expression) {
                    return Some(operation);
                }
            }
            None
        })
    }

    pub fn selected_index_operations(&self) -> impl Iterator<Item = &OpenIndexOperationSelection> {
        self.index_expression_authorities
            .iter()
            .filter_map(|(_, authority)| {
                let operation = authority.operation.as_ref()?;
                self.open_index_operation(operation.expression)
            })
    }

    pub fn open_index_normalization_count(&self) -> usize {
        self.index_expression_authorities
            .iter()
            .filter(|(_, authority)| authority.normalization.is_some())
            .count()
    }
}
