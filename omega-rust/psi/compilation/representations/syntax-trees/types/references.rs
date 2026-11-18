use crate::expression::ExpressionHandle;
use crate::identifier::Identifier;
use crate::item::ItemHandle;
use crate::paged_roster::paged_roster;
use arena::{Handle, HandleSpan, PagedArenaFrontier};

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;
pub type TypeReferenceFrontier = PagedArenaFrontier<TypeReferenceNode>;

paged_roster! {
    pub struct TypeReferenceTable => pub(crate) TypeReferenceTableAppendCheckpoint {
        arena type_references: TypeReferenceNode,
        arena type_reference_handles: TypeReferenceHandle,
        arena constraints: TypeConstraintNode,
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
    compiler_storage::Record,
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
    compiler_storage::Record,
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
    compiler_storage::Record,
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
    compiler_storage::Record,
)]
pub struct ConstArgumentOrigin {
    pub reference: source_text::SourceSpan,
    pub declaration: source_text::SourceSpan,
    pub initializer: source_text::SourceSpan,
    pub evaluated_value: language_core::constant_value::EvaluatedConstant,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Default,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
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
    compiler_storage::Record,
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
    compiler_storage::Record,
)]
struct GenericApplicationOrigin {
    instance: TypeReferenceHandle,
    application: TypeReferenceHandle,
}

impl TypeReferenceTable {
    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
        self.type_references.insert(type_reference)
    }

    pub fn retain_alias_header_reference(&mut self, reference: TypeReferenceHandle) {
        if !self.is_alias_header_reference(reference) {
            self.alias_header_references.insert(reference);
        }
    }

    pub fn is_alias_header_reference(&self, reference: TypeReferenceHandle) -> bool {
        self.alias_header_references
            .iter()
            .any(|(_, candidate)| *candidate == reference)
    }

    pub fn type_alias_applications(
        &self,
        reference: TypeReferenceHandle,
    ) -> impl Iterator<Item = &TypeAliasApplicationOrigin> {
        self.type_alias_applications
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
            .type_alias_applications
            .iter()
            .any(|(_, row)| row == &origin)
        {
            self.type_alias_applications.insert(origin);
        }
    }

    pub fn supplied_operand_owner(
        &self,
        operand: TypeReferenceHandle,
    ) -> Result<ItemHandle, &'static str> {
        self.supplied_operand_owners
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
                .supplied_operand_owners
                .iter()
                .find_map(|(handle, row)| (row.operand == operand).then_some(handle));
            if let Some(handle) = existing {
                let row = self.supplied_operand_owners.get_mut(handle);
                if row.declaration != declaration {
                    row.declaration = ItemHandle::invalid();
                }
                return;
            }
            self.supplied_operand_owners.insert(SuppliedOperandOwner {
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
            .supplied_operand_owners
            .iter()
            .find_map(|(handle, row)| (row.operand == operand).then_some(handle));
        if let Some(handle) = existing {
            self.supplied_operand_owners.get_mut(handle).declaration = ItemHandle::invalid();
            return;
        }
        self.supplied_operand_owners.insert(SuppliedOperandOwner {
            operand,
            declaration: ItemHandle::invalid(),
        });
    }

    pub(crate) fn supplied_operand_owner_frontier(&self) -> usize {
        self.supplied_operand_owners.iter().count()
    }

    pub(crate) fn remap_supplied_operand_owners(
        &mut self,
        frontier: usize,
        declarations: &[(ItemHandle, ItemHandle)],
    ) {
        let rows = self
            .supplied_operand_owners
            .iter()
            .skip(frontier)
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in rows {
            let row = self.supplied_operand_owners.get_mut(handle);
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
            self.pending_const_arguments
                .insert(PendingConstArgument { position, original });
        }
    }

    pub fn pending_const_arguments(
        &self,
        position: TypeReferenceHandle,
    ) -> impl Iterator<Item = TypeReferenceHandle> + '_ {
        self.pending_const_arguments
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
        self.const_argument_normalizations
            .iter()
            .find_map(|(_, retained)| {
                (retained.argument == argument).then_some(&retained.normalization)
            })
    }

    pub fn const_argument_origins(
        &self,
        selections: HandleSpan<ConstArgumentOrigin>,
    ) -> &[ConstArgumentOrigin] {
        self.const_argument_origins.span_or_empty(selections)
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
        let selections = self.const_argument_origins.insert_many(origins);
        let builtin_operators = self
            .const_argument_builtin_operators
            .insert_many(builtin_operators);
        self.const_argument_normalizations
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
            .const_argument_normalizations
            .iter()
            .find_map(|(handle, retained)| (retained.argument == argument).then_some(handle))
            .expect("structural expression belongs to a retained normalization");
        let normalization = &mut self
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
            .const_argument_normalizations
            .iter()
            .find_map(|(handle, retained)| (retained.argument == argument).then_some(handle))
            .expect("materialized value belongs to a retained normalization");
        let normalization = &mut self
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
        self.const_argument_builtin_operators
            .span_or_empty(operators)
    }

    pub fn generic_application_origin(&self, instance: TypeReferenceHandle) -> TypeReferenceHandle {
        self.generic_application_origins
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
        self.generic_application_origins
            .insert(GenericApplicationOrigin {
                instance,
                application,
            });
    }

    pub fn node_count(&self) -> TypeReferenceFrontier {
        self.type_references.frontier()
    }

    pub fn named_nodes_from(
        &self,
        watermark: TypeReferenceFrontier,
        context: &impl crate::SyntaxTextContext,
    ) -> Vec<(TypeReferenceHandle, String)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| watermark.contains(*handle))
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Named(name) => Some((handle, name.as_str(context).to_string())),
                _ => None,
            })
            .collect()
    }

    pub fn const_expression_nodes(&self) -> Vec<(TypeReferenceHandle, ExpressionHandle)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| !self.is_alias_header_reference(*handle))
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::ConstExpression(expression) => Some((handle, *expression)),
                _ => None,
            })
            .collect()
    }

    pub fn generic_nodes(&self) -> Vec<TypeReferenceHandle> {
        self.type_references
            .iter()
            .filter(|(handle, _)| !self.is_alias_header_reference(*handle))
            .filter(|(handle, _)| {
                !self
                    .generic_application_origins
                    .iter()
                    .any(|(_, origin)| origin.application == *handle)
            })
            .filter(|(handle, _)| {
                !self
                    .type_alias_applications
                    .iter()
                    .any(|(_, origin)| origin.application == *handle)
            })
            .filter_map(|(handle, node)| {
                matches!(node, TypeReferenceNode::Generic { .. }).then_some(handle)
            })
            .collect()
    }

    pub fn const_parameter_array_nodes_from(
        &self,
        watermark: TypeReferenceFrontier,
        context: &impl crate::SyntaxTextContext,
    ) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, String)> {
        self.type_references
            .iter()
            .filter(|(handle, _)| watermark.contains(*handle))
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter(name),
                } => Some((handle, *element_type, name.as_str(context).to_string())),
                _ => None,
            })
            .collect()
    }

    pub fn insert_named(&mut self, name: Identifier) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Named(name))
    }

    pub fn insert_unit(&mut self) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Unit)
    }

    pub fn insert_reference(
        &mut self,
        referee: TypeReferenceHandle,
        access: language_core::ReferenceAccess,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: None,
        })
    }

    pub fn insert_reference_with_lifetime(
        &mut self,
        referee: TypeReferenceHandle,
        access: language_core::ReferenceAccess,
        lifetime: Option<Identifier>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        })
    }

    pub fn insert_constrained(
        &mut self,
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Constrained {
            base_type,
            constraints,
        })
    }

    pub fn insert_fixed_array(
        &mut self,
        element_type: TypeReferenceHandle,
        length: impl Into<FixedArrayLength>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::FixedArray {
            element_type,
            length: length.into(),
        })
    }

    pub fn insert_slice(&mut self, element_type: TypeReferenceHandle) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Slice { element_type })
    }

    pub fn insert_generic(
        &mut self,
        base_name: Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        self.insert(TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments: Vec::new(),
            arguments,
        })
    }

    pub fn insert_type_reference_handles(
        &mut self,
        type_references: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(type_references)
    }

    pub fn append_type_reference_handle(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> Handle<TypeReferenceHandle> {
        self.type_reference_handles.append(type_reference)
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(constraints)
    }

    pub fn append_constraint(
        &mut self,
        constraint: TypeConstraintNode,
    ) -> Handle<TypeConstraintNode> {
        self.constraints.append(constraint)
    }

    pub fn type_reference(&self, handle: TypeReferenceHandle) -> &TypeReferenceNode {
        self.type_references.get(handle)
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
            .type_alias_applications
            .iter()
            .find_map(|(handle, row)| {
                (row.instance == instance
                    && row.declaration == declaration
                    && row.application == original.application)
                    .then_some(handle)
            })?;
        self.type_alias_applications.get_mut(row).application = application;
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
                    .$table
                    .iter()
                    .filter_map(|(_, row)| (row.$field == source).then_some(row.clone()))
                    .collect::<Vec<_>>();
                let previous = self
                    .$table
                    .iter()
                    .filter_map(|(handle, row)| (row.$field == destination).then_some(handle))
                    .collect::<Vec<_>>();
                for handle in previous {
                    self.$table.free(handle);
                }
                for mut row in rows {
                    row.$field = destination;
                    self.$table.insert(row);
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

    /* Every type reference node of the forest, in table order. */
    pub fn iter_type_references(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> {
        self.type_references.iter()
    }

    pub fn replace_type_reference(
        &mut self,
        handle: TypeReferenceHandle,
        type_reference: TypeReferenceNode,
    ) {
        *self.type_references.get_mut(handle) = type_reference;
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

    pub fn domain_constraints(&self) -> Vec<DomainConstraint> {
        self.constraints
            .iter()
            .filter_map(|(_, constraint)| match constraint {
                TypeConstraintNode::Domain(domain) => Some(domain.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn type_reference_count(&self) -> usize {
        self.type_references.len()
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
}

impl Default for TypeReferenceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum TypeReferenceNode {
    Reference {
        referee: TypeReferenceHandle,
        access: language_core::ReferenceAccess,
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
        base_name: Identifier,
        lifetime_arguments: Vec<Identifier>,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    // The existing selection carrier preserves which path component
    // owns an application, before namespace resolution assigns its kind.
    StaticSelection(crate::expression::StaticMachineArgument),
    Callable {
        signature: crate::item::StateSignatureHandle,
    },
    ConstExpression(ExpressionHandle),
    Named(Identifier),
    #[default]
    Unit,
}

impl TypeReferenceNode {
    pub fn as_named(&self) -> Option<&Identifier> {
        let TypeReferenceNode::Named(named) = self else {
            return None;
        };
        Some(named)
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum FixedArrayLength {
    Literal(usize),
    ConstParameter(Identifier),
    ConstCall(Identifier),
    Expression(TypeReferenceHandle),
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

impl FixedArrayLength {
    pub fn same_value(&self, other: &Self, context: &impl crate::SyntaxTextContext) -> bool {
        match (self, other) {
            (Self::Literal(left), Self::Literal(right)) => left == right,
            (Self::ConstParameter(left), Self::ConstParameter(right))
            | (Self::ConstCall(left), Self::ConstCall(right)) => {
                left.as_str(context) == right.as_str(context)
            }
            (Self::Expression(left), Self::Expression(right)) => left == right,
            _ => false,
        }
    }

    pub fn display_name(&self, context: &impl crate::SyntaxTextContext) -> String {
        match self {
            Self::Literal(value) => value.to_string(),
            Self::ConstParameter(name) => name.as_str(context).to_owned(),
            Self::ConstCall(name) => format!("{}()", name.as_str(context)),
            Self::Expression(_) => "<constant expression>".to_owned(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub enum TypeConstraintNode {
    Named(Identifier),

    ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
    Domain(DomainConstraint),
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    compiler_storage::Record,
)]
pub struct DomainConstraint {
    pub name: Identifier,
    pub arguments: HandleSpan<TypeReferenceHandle>,
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(Identifier::generated(""))
    }
}

#[cfg(test)]
#[path = "../tests/unit/types_tests.rs"]
mod tests;
