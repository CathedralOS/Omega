//! Occurrences expanded or evaluated during declaration preparation. The keys
//! name the original type occurrences, so roots, page adoption and copying can
//! preserve the author’s argument and the compiler’s selected meaning together.

use super::{TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable};
use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::item::ItemHandle;
use crate::paged_roster::paged_roster;
use arena::HandleSpan;

paged_roster! {
    pub(super) struct TypePreparation => pub(super) TypePreparationAppendCheckpoint {
        arena generic_application_origins: GenericApplicationOrigin,
        arena const_argument_normalizations: RetainedConstArgumentNormalization,
        arena const_argument_origins: ConstArgumentOrigin,
        arena const_argument_builtin_operators: source_text::SourceSpan,
        arena pending_const_arguments: PendingConstArgument,
        arena supplied_operand_owners: SuppliedOperandOwner,
        arena type_alias_applications: TypeAliasApplicationOrigin,
        arena alias_header_references: TypeReferenceHandle,
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TypeAliasApplicationOrigin {
    pub instance: TypeReferenceHandle,
    pub application: TypeReferenceHandle,
    pub declaration: source_text::SourceSpan,
    pub definition_references: Vec<source_text::SourceSpan>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
struct SuppliedOperandOwner {
    operand: TypeReferenceHandle,
    declaration: ItemHandle,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
struct PendingConstArgument {
    position: TypeReferenceHandle,
    original: TypeReferenceHandle,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct ConstArgumentOrigin {
    pub reference: source_text::SourceSpan,
    pub declaration: source_text::SourceSpan,
    pub initializer: source_text::SourceSpan,
    pub evaluated_value: numeric_literals::EvaluatedConstant,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
struct RetainedConstArgumentNormalization {
    argument: TypeReferenceHandle,
    normalization: ConstArgumentNormalization,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    record_storage::Record,
    syntax_trees_derive::SyntaxRelocate,
)]
pub struct ConstArgumentNormalization {
    pub authored_expression: ExpressionHandle,
    pub materialized_expression: ExpressionHandle,
    pub reference: source_text::SourceSpan,
    pub canonical_result_encoding: String,
    pub selections: HandleSpan<ConstArgumentOrigin>,
    pub builtin_operators: HandleSpan<source_text::SourceSpan>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
struct GenericApplicationOrigin {
    instance: TypeReferenceHandle,
    application: TypeReferenceHandle,
}

impl TypeReferenceTable {
    pub fn retain_alias_header_reference(&mut self, reference: TypeReferenceHandle) {
        if !self.is_alias_header_reference(reference) {
            self.preparation.alias_header_references.insert(reference);
        }
    }

    pub fn is_alias_header_reference(&self, reference: TypeReferenceHandle) -> bool {
        self.preparation
            .alias_header_references
            .iter()
            .any(|(_, candidate)| *candidate == reference)
    }

    pub fn type_alias_applications(
        &self,
        reference: TypeReferenceHandle,
    ) -> impl Iterator<Item = &TypeAliasApplicationOrigin> {
        self.preparation
            .type_alias_applications
            .iter()
            .filter_map(move |(_, origin)| (origin.instance == reference).then_some(origin))
    }

    pub fn retain_type_alias_application(&mut self, origin: TypeAliasApplicationOrigin) {
        assert!(
            origin.instance.is_valid()
                && origin.application.is_valid()
                && origin.instance != origin.application
        );
        if !self
            .preparation
            .type_alias_applications
            .iter()
            .any(|(_, row)| row == &origin)
        {
            self.preparation.type_alias_applications.insert(origin);
        }
    }

    pub fn supplied_operand_owner(
        &self,
        operand: TypeReferenceHandle,
    ) -> Result<ItemHandle, &'static str> {
        self.preparation
            .supplied_operand_owners
            .iter()
            .find_map(|(_, row)| {
                (row.operand == operand).then(|| {
                    if row.declaration.is_valid() {
                        Ok(row.declaration)
                    } else {
                        Err("supplied operand lost its declaration during syntax copying")
                    }
                })
            })
            .unwrap_or(Ok(ItemHandle::invalid()))
    }

    pub fn retain_supplied_operand_owner(
        &mut self,
        operand: TypeReferenceHandle,
        declaration: ItemHandle,
    ) {
        if operand.is_valid() && declaration.is_valid() {
            let existing = self
                .preparation
                .supplied_operand_owners
                .iter()
                .find_map(|(handle, row)| (row.operand == operand).then_some(handle));
            if let Some(handle) = existing {
                let row = self.preparation.supplied_operand_owners.get_mut(handle);
                if row.declaration != declaration {
                    row.declaration = ItemHandle::invalid();
                }
                return;
            }
            self.preparation
                .supplied_operand_owners
                .insert(SuppliedOperandOwner {
                    operand,
                    declaration,
                });
        }
    }

    pub fn retain_copied_operand_owner(
        &mut self,
        source: TypeReferenceHandle,
        destination: TypeReferenceHandle,
    ) {
        match self.supplied_operand_owner(source) {
            Ok(owner) => self.retain_supplied_operand_owner(destination, owner),
            Err(_) => self.retain_unresolved_supplied_operand_owner(destination),
        }
    }

    pub(crate) fn retain_unresolved_supplied_operand_owner(
        &mut self,
        operand: TypeReferenceHandle,
    ) {
        let existing = self
            .preparation
            .supplied_operand_owners
            .iter()
            .find_map(|(handle, row)| (row.operand == operand).then_some(handle));
        if let Some(handle) = existing {
            self.preparation
                .supplied_operand_owners
                .get_mut(handle)
                .declaration = ItemHandle::invalid();
            return;
        }
        self.preparation
            .supplied_operand_owners
            .insert(SuppliedOperandOwner {
                operand,
                declaration: ItemHandle::invalid(),
            });
    }

    pub(crate) fn supplied_operand_owner_frontier(&self) -> usize {
        self.preparation.supplied_operand_owners.iter().count()
    }

    pub(crate) fn remap_supplied_operand_owners(
        &mut self,
        frontier: usize,
        declarations: &[(ItemHandle, ItemHandle)],
    ) {
        let rows = self
            .preparation
            .supplied_operand_owners
            .iter()
            .skip(frontier)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in rows {
            let row = self.preparation.supplied_operand_owners.get_mut(handle);
            row.declaration = declarations
                .iter()
                .find(|(source, _)| *source == row.declaration)
                .map(|(_, destination)| *destination)
                .unwrap_or_default();
        }
    }

    pub fn retain_pending_const_argument(
        &mut self,
        position: TypeReferenceHandle,
        original: TypeReferenceHandle,
    ) {
        if !self
            .pending_const_arguments(position)
            .any(|pending| pending == original)
        {
            self.preparation
                .pending_const_arguments
                .insert(PendingConstArgument { position, original });
        }
    }

    pub fn pending_const_arguments(
        &self,
        position: TypeReferenceHandle,
    ) -> impl Iterator<Item = TypeReferenceHandle> + '_ {
        self.preparation
            .pending_const_arguments
            .iter()
            .filter_map(move |(_, pending)| {
                (pending.position == position && pending.original.is_valid())
                    .then_some(pending.original)
            })
    }

    pub fn const_argument_normalization(
        &self,
        argument: TypeReferenceHandle,
    ) -> Option<&ConstArgumentNormalization> {
        self.preparation
            .const_argument_normalizations
            .iter()
            .find_map(|(_, retained)| {
                (retained.argument == argument).then_some(&retained.normalization)
            })
    }

    pub fn const_argument_origins(
        &self,
        selections: HandleSpan<ConstArgumentOrigin>,
    ) -> &[ConstArgumentOrigin] {
        self.preparation
            .const_argument_origins
            .span_or_empty(selections)
    }

    pub fn retain_const_argument_normalization(
        &mut self,
        argument: TypeReferenceHandle,
        reference: source_text::SourceSpan,
        canonical_result_encoding: String,
        origins: impl IntoIterator<Item = ConstArgumentOrigin>,
        builtin_operators: impl IntoIterator<Item = source_text::SourceSpan>,
    ) {
        assert!(argument.is_valid());
        assert!(self.const_argument_normalization(argument).is_none());
        let selections = self.preparation.const_argument_origins.insert_many(origins);
        let builtin_operators = self
            .preparation
            .const_argument_builtin_operators
            .insert_many(builtin_operators);
        self.preparation
            .const_argument_normalizations
            .insert(RetainedConstArgumentNormalization {
                argument,
                normalization: ConstArgumentNormalization {
                    authored_expression: ExpressionHandle::invalid(),
                    materialized_expression: ExpressionHandle::invalid(),
                    reference,
                    canonical_result_encoding,
                    selections,
                    builtin_operators,
                },
            });
    }

    pub fn retain_const_argument_expression(
        &mut self,
        argument: TypeReferenceHandle,
        expression: ExpressionHandle,
    ) {
        let handle = self
            .preparation
            .const_argument_normalizations
            .iter()
            .find_map(|(handle, retained)| (retained.argument == argument).then_some(handle))
            .expect("structural expression belongs to a retained normalization");
        let normalization = &mut self
            .preparation
            .const_argument_normalizations
            .get_mut(handle)
            .normalization;
        assert!(!normalization.authored_expression.is_valid());
        normalization.authored_expression = expression;
    }

    pub fn retain_const_argument_materialized_expression(
        &mut self,
        argument: TypeReferenceHandle,
        expression: ExpressionHandle,
    ) {
        let handle = self
            .preparation
            .const_argument_normalizations
            .iter()
            .find_map(|(handle, retained)| (retained.argument == argument).then_some(handle))
            .expect("materialized value belongs to a retained normalization");
        let normalization = &mut self
            .preparation
            .const_argument_normalizations
            .get_mut(handle)
            .normalization;
        assert!(!normalization.materialized_expression.is_valid());
        normalization.materialized_expression = expression;
    }

    pub fn const_argument_builtin_operators(
        &self,
        operators: HandleSpan<source_text::SourceSpan>,
    ) -> &[source_text::SourceSpan] {
        self.preparation
            .const_argument_builtin_operators
            .span_or_empty(operators)
    }

    pub fn generic_application_origin(&self, instance: TypeReferenceHandle) -> TypeReferenceHandle {
        self.preparation
            .generic_application_origins
            .iter()
            .find_map(|(_, origin)| (origin.instance == instance).then_some(origin.application))
            .unwrap_or_default()
    }

    pub fn retain_generic_application_origin(
        &mut self,
        instance: TypeReferenceHandle,
        application: TypeReferenceHandle,
    ) {
        assert!(instance.is_valid() && application.is_valid() && instance != application);
        assert!(!self.generic_application_origin(instance).is_valid());
        self.preparation
            .generic_application_origins
            .insert(GenericApplicationOrigin {
                instance,
                application,
            });
    }

    pub fn generic_nodes(&self) -> Vec<TypeReferenceHandle> {
        self.type_references
            .iter()
            .filter(|(handle, _)| !self.is_alias_header_reference(*handle))
            .filter(|(handle, _)| {
                !self
                    .preparation
                    .generic_application_origins
                    .iter()
                    .any(|(_, origin)| origin.application == *handle)
            })
            .filter(|(handle, _)| {
                !self
                    .preparation
                    .type_alias_applications
                    .iter()
                    .any(|(_, origin)| origin.application == *handle)
            })
            .filter_map(|(handle, node)| {
                matches!(node, TypeReferenceNode::Generic { .. }).then_some(handle)
            })
            .collect()
    }

    pub fn fresh_type_occurrence(&mut self, source: TypeReferenceHandle) -> TypeReferenceHandle {
        let fresh = self.insert(self.type_reference(source).clone());
        self.replace_type_occurrence(fresh, source);
        fresh
    }

    pub fn alias_use_at(
        &mut self,
        source: TypeReferenceHandle,
        declaration: source_text::SourceSpan,
        head: Identifier,
    ) -> Option<TypeReferenceHandle> {
        let mut applications = self
            .type_alias_applications(source)
            .filter(|row| row.declaration == declaration)
            .cloned();
        let original = applications.next()?;
        if applications.next().is_some() {
            return None;
        }
        drop(applications);
        let application = self.fresh_type_occurrence(original.application);
        let mut node = self.type_reference(application).clone();
        match &mut node {
            TypeReferenceNode::Named(name) => *name = head,
            TypeReferenceNode::Generic { base_name, .. } => *base_name = head,
            _ => return None,
        }
        self.replace_type_reference(application, node);
        let instance = self.fresh_type_occurrence(source);
        let row = self
            .preparation
            .type_alias_applications
            .iter()
            .find_map(|(handle, row)| {
                (row.instance == instance
                    && row.declaration == declaration
                    && row.application == original.application)
                    .then_some(handle)
            })?;
        self.preparation
            .type_alias_applications
            .get_mut(row)
            .application = application;
        Some(instance)
    }

    pub fn replace_type_occurrence(
        &mut self,
        destination: TypeReferenceHandle,
        source: TypeReferenceHandle,
    ) {
        assert!(destination != source);
        macro_rules! transfer {
            ($table:ident, $field:ident) => {{
                let rows = self
                    .preparation
                    .$table
                    .iter()
                    .filter_map(|(_, row)| (row.$field == source).then_some(row.clone()))
                    .collect::<Vec<_>>();
                let previous = self
                    .preparation
                    .$table
                    .iter()
                    .filter_map(|(handle, row)| (row.$field == destination).then_some(handle))
                    .collect::<Vec<_>>();
                for handle in previous {
                    self.preparation.$table.free(handle);
                }
                for mut row in rows {
                    row.$field = destination;
                    self.preparation.$table.insert(row);
                }
            }};
        }
        let node = self.type_reference(source).clone();
        self.replace_type_reference(destination, node);
        transfer!(supplied_operand_owners, operand);
        transfer!(generic_application_origins, instance);
        transfer!(const_argument_normalizations, argument);
        transfer!(pending_const_arguments, position);
        transfer!(type_alias_applications, instance);
    }
}
