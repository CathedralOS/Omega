/*
We represent parsed type references (`TypeReferenceNode`, `TypeConstraintNode`)
and surface value expressions (`ExpressionNode`, `FloatSpelling`) with their
diagnostic display formatting in `SyntaxTrees`.
*/

pub mod type_system {
    pub mod types {
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
                arena const_argument_builtin_operators: source::SourceSpan,
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
            compiler_storage::Record,
        )]
        pub struct TypeAliasApplicationOrigin {
            pub instance: TypeReferenceHandle,
            pub application: TypeReferenceHandle,
            pub declaration: source::SourceSpan,
            pub definition_references: Vec<source::SourceSpan>,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Default,
            syntax_trees_derive::PlainSyntaxEquivalent,
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
            compiler_storage::Record,
        )]
        pub struct ConstArgumentOrigin {
            pub reference: source::SourceSpan,
            pub declaration: source::SourceSpan,
            pub initializer: source::SourceSpan,
            pub evaluated_value: language_core::constant_value::EvaluatedConstant,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Default,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        struct RetainedConstArgumentNormalization {
            argument: TypeReferenceHandle,
            normalization: ConstArgumentNormalization,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub struct ConstArgumentNormalization {
            pub authored_expression: ExpressionHandle,
            pub materialized_expression: ExpressionHandle,
            pub reference: source::SourceSpan,
            pub canonical_result_encoding: String,
            pub selections: HandleSpan<ConstArgumentOrigin>,
            pub builtin_operators: HandleSpan<source::SourceSpan>,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Default,
            syntax_trees_derive::PlainSyntaxEquivalent,
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
                    self.supplied_operand_owners.get_mut(handle).declaration =
                        ItemHandle::invalid();
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

            pub fn retain_pending_const_argument_uses(&mut self, argument: TypeReferenceHandle) {
                let positions = self.type_references.iter().filter_map(|(handle, node)| {
            let owns = match node {
                TypeReferenceNode::Generic { arguments, .. } => self.type_reference_handles(*arguments).contains(&argument),
                TypeReferenceNode::Constrained { constraints, .. } => self.constraints(*constraints).iter().any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(domain) if self.type_reference_handles(domain.arguments).contains(&argument))
                }),
                _ => false,
            };
            owns.then_some(handle)
        }).collect::<Vec<_>>();
                if positions.is_empty() {
                    self.retain_pending_const_argument(argument, argument);
                } else {
                    for position in positions {
                        self.retain_pending_const_argument(position, argument);
                    }
                }
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
                reference: source::SourceSpan,
                canonical_result_encoding: String,
                origins: impl IntoIterator<Item = ConstArgumentOrigin>,
                builtin_operators: impl IntoIterator<Item = source::SourceSpan>,
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
                    .find_map(|(handle, retained)| {
                        (retained.argument == argument).then_some(handle)
                    })
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
                    .find_map(|(handle, retained)| {
                        (retained.argument == argument).then_some(handle)
                    })
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
                operators: HandleSpan<source::SourceSpan>,
            ) -> &[source::SourceSpan] {
                self.const_argument_builtin_operators
                    .span_or_empty(operators)
            }

            pub fn generic_application_origin(
                &self,
                instance: TypeReferenceHandle,
            ) -> TypeReferenceHandle {
                self.generic_application_origins
                    .iter()
                    .find_map(|(_, origin)| {
                        (origin.instance == instance).then_some(origin.application)
                    })
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
                        TypeReferenceNode::Named(name) => {
                            Some((handle, name.as_str(context).to_string()))
                        }
                        _ => None,
                    })
                    .collect()
            }

            pub fn generic_nodes_from(
                &self,
                watermark: TypeReferenceFrontier,
                context: &impl crate::SyntaxTextContext,
            ) -> Vec<(TypeReferenceHandle, String)> {
                self.type_references
                    .iter()
                    .filter(|(handle, _)| watermark.contains(*handle))
                    .filter_map(|(handle, node)| match node {
                        TypeReferenceNode::Generic { base_name, .. } => {
                            Some((handle, base_name.as_str(context).to_string()))
                        }
                        _ => None,
                    })
                    .collect()
            }

            pub fn const_expression_nodes(&self) -> Vec<(TypeReferenceHandle, ExpressionHandle)> {
                self.type_references
                    .iter()
                    .filter(|(handle, _)| !self.is_alias_header_reference(*handle))
                    .filter_map(|(handle, node)| match node {
                        TypeReferenceNode::ConstExpression(expression) => {
                            Some((handle, *expression))
                        }
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

            pub fn insert_slice(
                &mut self,
                element_type: TypeReferenceHandle,
            ) -> TypeReferenceHandle {
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

            pub fn fresh_type_occurrence(
                &mut self,
                source: TypeReferenceHandle,
            ) -> TypeReferenceHandle {
                let fresh = self.insert(self.type_reference(source).clone());
                self.replace_type_occurrence(fresh, source);
                fresh
            }

            pub fn alias_use_at(
                &mut self,
                source: TypeReferenceHandle,
                declaration: source::SourceSpan,
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
                            .filter_map(|(handle, row)| {
                                (row.$field == destination).then_some(handle)
                            })
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

            pub fn constraints(
                &self,
                span: HandleSpan<TypeConstraintNode>,
            ) -> &[TypeConstraintNode] {
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
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
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
            ConstExpression(ExpressionHandle),
            DynamicTrait {
                trait_name: Identifier,
                trait_arguments: HandleSpan<TypeReferenceHandle>,
            },
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

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
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
            pub fn same_value(
                &self,
                other: &Self,
                context: &impl crate::SyntaxTextContext,
            ) -> bool {
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

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum TypeConstraintNode {
            Named(Identifier),

            ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
            Domain(DomainConstraint),
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
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
        mod tests {
            use super::TypeReferenceTable;
            use crate::identifier::Identifier;

            #[test]
            fn substitution_watermark_excludes_existing_type_nodes() {
                let mut types = TypeReferenceTable::new();
                let existing = types.insert_named(Identifier::generated("T"));
                let watermark = types.node_count();
                let copied = types.insert_named(Identifier::generated("T"));

                let nodes = types.named_nodes_from(watermark, &crate::SyntaxTextOwner::default());
                assert_eq!(nodes, vec![(copied, "T".to_string())]);
                assert_ne!(nodes[0].0, existing);
            }
        }
    }
}

pub mod values {
    pub mod expression {

        use crate::FloatSpelling;
        use crate::identifier::Identifier;
        use crate::paged_roster::paged_roster;
        use arena::{Handle, HandleSpan, PagedArenaFrontier};
        use numerics::literals::IntegerLiteral;
        use source::SourceSpan;

        pub(crate) mod display {
            use diagnostics::bracketed_display_names;

            use crate::expression::{
                BinaryOperator, ExpressionNode, MatchPattern, StaticMachineArgument,
                TableBinaryExpression, TableCallExpression, TableCastExpression,
                TableUnaryExpression, UnaryOperator,
            };
            use crate::identifier::JoinedSyntaxPath;

            impl ExpressionNode {
                pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
                    match self {
                        Self::Match(dispatch) => {
                            let arms = table
                                .expressions
                                .match_arms(dispatch.arms)
                                .iter()
                                .map(|arm| {
                                    let pattern = match arm.pattern {
                                        MatchPattern::Value(value) => {
                                            table.display_expression_name(value)
                                        }
                                        MatchPattern::Destructure(pattern) => {
                                            display_destructure_pattern(table, pattern)
                                        }
                                        MatchPattern::Wildcard => "_".to_owned(),
                                    };
                                    format!(
                                        "{pattern} => {}",
                                        table.display_expression_name(arm.value)
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!(
                                "match {} {{ {arms} }}",
                                table.display_expression_name(dispatch.subject)
                            )
                        }
                        Self::SavedSubject(saved) => {
                            format!("<saved subject #{}>", saved.capture.arena_index())
                        }
                        Self::ArrayLiteral(values) => bracketed_display_names(
                            table.expressions.expression_handles(*values).iter(),
                            |value| table.display_expression_name(*value),
                        ),
                        Self::Atomic(atomic) => format!(
                            "atomic[{:?}]({})",
                            atomic.ordering,
                            table.display_expression_name(atomic.value)
                        ),
                        Self::Binary(binary) => binary.display_name(table),
                        Self::Boolean(value) => value.to_string(),
                        Self::Cast(cast) => cast.display_name(table),
                        Self::Call(call) => call.display_name(table),
                        Self::Float(value) => value.as_str(table).to_owned(),
                        Self::UndeterminedNaN(format) => {
                            format!("<undetermined NaN {}>", format.name())
                        }
                        Self::Indexed(indexed) => {
                            format!(
                                "{}[{}]",
                                table.display_expression_name(indexed.collection),
                                table.display_expression_name(indexed.index)
                            )
                        }
                        Self::Integer(value) => value.to_string(),
                        Self::Membership(membership) => {
                            format!(
                                "{} in {}",
                                table.display_expression_name(membership.value),
                                table
                                    .expressions
                                    .identifier_path_members(membership.domain)
                                    .joined_path_with_in(table, "::")
                            )
                        }
                        Self::Member(member) => {
                            format!(
                                "{}.{}",
                                table.display_expression_name(member.receiver),
                                member.member.as_str(table)
                            )
                        }
                        Self::Borrow(expression) => format!(
                            "{}{}",
                            expression.access.borrow_prefix(),
                            table.display_expression_name(expression.target)
                        ),
                        Self::Name(path) => StaticMachineArgument {
                            path: table
                                .expressions
                                .identifier_path_members(path.members)
                                .to_vec()
                                .into_boxed_slice(),
                            application: path.application.clone(),
                            ..Default::default()
                        }
                        .display_name(table),
                        Self::Range(range) => {
                            let separator = if range.end_inclusive { "..=" } else { ".." };
                            match (range.start.is_valid(), range.end.is_valid()) {
                                (true, true) => format!(
                                    "{}{}{}",
                                    table.display_expression_name(range.start),
                                    separator,
                                    table.display_expression_name(range.end)
                                ),
                                (true, false) => format!(
                                    "{}{}",
                                    table.display_expression_name(range.start),
                                    separator
                                ),
                                (false, true) => {
                                    format!(
                                        "{}{}",
                                        separator,
                                        table.display_expression_name(range.end)
                                    )
                                }
                                (false, false) => separator.to_owned(),
                            }
                        }
                        Self::StructLiteral(struct_literal) => {
                            struct_literal.constructor_name.as_str(table).to_owned()
                        }
                        Self::String(value) => source::display_literal_bytes(value),
                        Self::Unary(unary) => unary.display_name(table),
                        Self::ZeroValue(_) => "zero_value<type>()".to_owned(),
                        Self::TypeExpression(_) => "<type expression>".to_owned(),
                    }
                }
            }

            fn display_destructure_pattern(
                table: &crate::SyntaxTrees,
                pattern: crate::expression::TableDestructurePattern,
            ) -> String {
                use crate::expression::MatchFieldAction;
                let mut fields = table
                    .expressions
                    .match_fields(pattern.fields)
                    .iter()
                    .map(|field| {
                        let member = field.member.as_str(table);
                        match &field.action {
                            MatchFieldAction::Bind(binding) if binding.as_str(table) == member => {
                                member.to_owned()
                            }
                            MatchFieldAction::Bind(binding) => {
                                format!("{member} as {}", binding.as_str(table))
                            }
                            MatchFieldAction::Equal(value) => {
                                format!("{member}: {}", table.display_expression_name(*value))
                            }
                            MatchFieldAction::Waive => format!("{member} as _"),
                        }
                    })
                    .collect::<Vec<_>>();
                if pattern.has_rest {
                    fields.push("..".to_owned());
                }
                let selectors = table
                    .statements
                    .outcome_proof_selectors(pattern.proof_selectors);
                let proofs = if selectors.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; {}",
                        selectors
                            .iter()
                            .map(|selector| format!(
                                "{}: {}",
                                selector.output_field.as_str(table),
                                selector.binding.as_str(table)
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let guard = if pattern.guard.is_valid() {
                    format!(" if {}", table.display_expression_name(pattern.guard))
                } else {
                    String::new()
                };
                format!(
                    "{} {{ {}{proofs} }}{guard}",
                    table.display_expression_name(pattern.head),
                    fields.join(", ")
                )
            }

            impl TableBinaryExpression {
                pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
                    format!(
                        "{} {}{} {}",
                        table.display_expression_name(self.left),
                        self.operator.display_name(),
                        self.policy.operator_suffix(),
                        table.display_expression_name(self.right)
                    )
                }
            }

            impl TableUnaryExpression {
                pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
                    format!(
                        "{}{}",
                        self.operator.display_name(),
                        table.display_expression_name(self.operand)
                    )
                }
            }

            impl TableCastExpression {
                pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
                    let target_type = table
                        .expressions
                        .identifier_path_members(self.target_label)
                        .joined_path_with_in(table, "::");
                    format!(
                        "{} as {}",
                        table.display_expression_name(self.value),
                        target_type
                    )
                }
            }

            impl TableCallExpression {
                pub fn display_name(&self, table: &crate::SyntaxTrees) -> String {
                    let machine_arguments = if self.machine_arguments.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "<{}>",
                            self.machine_arguments
                                .iter()
                                .map(|argument| argument.display_name(table))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    let arguments = table
                        .expressions
                        .expression_handles(self.arguments)
                        .iter()
                        .map(|argument| table.display_expression_name(*argument))
                        .collect::<Vec<_>>()
                        .join(", ");

                    if self.receiver.is_valid() {
                        format!(
                            "{}.{}{machine_arguments}({arguments})",
                            table.display_expression_name(self.receiver),
                            self.target.as_str(table)
                        )
                    } else {
                        format!(
                            "{}{machine_arguments}({arguments})",
                            self.target.as_str(table)
                        )
                    }
                }
            }

            impl StaticMachineArgument {
                fn display_name(&self, table: &crate::SyntaxTrees) -> String {
                    if self.type_reference.is_valid() {
                        return "<structural type>".to_owned();
                    }
                    if let Some(literal) = &self.const_literal {
                        return literal.to_string();
                    }
                    if let Some(projection) = &self.evidence_projection {
                        return format!(
                            "{}.{}",
                            projection.term.as_str(table),
                            projection.member.as_str(table)
                        );
                    }
                    let mut rendered = self.path.joined_path_with_in(table, "::");
                    if let Some(application) = &self.application {
                        let mut arguments = application
                            .lifetime_arguments
                            .iter()
                            .map(|lifetime| format!("'{}", lifetime.as_str(table)))
                            .collect::<Vec<_>>();
                        arguments.extend(
                            application
                                .arguments
                                .iter()
                                .map(|argument| argument.display_name(table)),
                        );
                        rendered.push('<');
                        rendered.push_str(&arguments.join(", "));
                        rendered.push('>');
                        if let Some(member) = table.expressions.static_member(application.member) {
                            rendered.push_str("::");
                            rendered.push_str(&member.display_name(table));
                        }
                    }
                    rendered
                }
            }

            impl BinaryOperator {
                pub fn display_name(self) -> &'static str {
                    match self {
                        Self::Add => "+",
                        Self::And => "&&",
                        Self::BitwiseAnd => "&",
                        Self::BitwiseOr => "|",
                        Self::BitwiseXor => "^",
                        Self::Divide => "/",
                        Self::Equal => "==",
                        Self::Greater => ">",
                        Self::GreaterOrEqual => ">=",
                        Self::Less => "<",
                        Self::LessOrEqual => "<=",
                        Self::Modulo => "%",
                        Self::Multiply => "*",
                        Self::NotEqual => "!=",
                        Self::Or => "||",
                        Self::ShiftLeft => "<<",
                        Self::ShiftRight => ">>",
                        Self::Subtract => "-",
                    }
                }
            }

            impl UnaryOperator {
                pub fn display_name(self) -> &'static str {
                    match self {
                        Self::BitwiseNot => "~",
                        Self::LogicalNot => "!",
                    }
                }
            }
        }
        #[cfg(test)]
        pub(crate) mod tests {

            use super::ExpressionNode;

            #[test]
            fn subtree_copy_rejoins_saved_subject_aliases_without_merging_dispatches() {
                use super::{
                    MatchPattern, TableDestructurePattern, TableMatchArm, TableMatchExpression,
                    TableSavedSubject,
                };
                let mut source = crate::SyntaxTrees::default();
                let subject = source.expressions.insert(ExpressionNode::Boolean(true));
                let head = source.expressions.insert(ExpressionNode::Boolean(false));
                let capture = source.expressions.insert_match_subject(subject);
                let alias =
                    source
                        .expressions
                        .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                            component: Default::default(),
                            capture,
                            pattern_head: head,
                        }));
                let arms = source.expressions.insert_match_arms([
                    TableMatchArm {
                        pattern: MatchPattern::Destructure(TableDestructurePattern {
                            head,
                            ..Default::default()
                        }),
                        value: alias,
                        ..Default::default()
                    },
                    TableMatchArm {
                        pattern: MatchPattern::Wildcard,
                        value: alias,
                        ..Default::default()
                    },
                ]);
                let root = source
                    .expressions
                    .insert(ExpressionNode::Match(TableMatchExpression {
                        subject,
                        capture,
                        arms,
                    }));
                let mut destination = crate::SyntaxTrees::default();
                let first = destination.copy_expression_from(&source, root);
                let second = destination.copy_expression_from(&source, root);
                let mut captures = Vec::new();
                for copied in [first, second] {
                    let ExpressionNode::Match(dispatch) =
                        destination.expressions.expression(copied)
                    else {
                        panic!("copied dispatch");
                    };
                    assert_eq!(
                        destination
                            .expressions
                            .match_subject(dispatch.capture)
                            .subject,
                        dispatch.subject
                    );
                    let arms = destination.expressions.match_arms(dispatch.arms);
                    assert_eq!(
                        arms[0].value, arms[1].value,
                        "one source occurrence stays one copied occurrence"
                    );
                    let MatchPattern::Destructure(pattern) = arms[0].pattern else {
                        panic!("copied pattern");
                    };
                    let ExpressionNode::SavedSubject(saved) =
                        destination.expressions.expression(arms[0].value)
                    else {
                        panic!("copied saved read");
                    };
                    assert_eq!(saved.capture, dispatch.capture);
                    assert_eq!(saved.pattern_head, pattern.head);
                    captures.push(dispatch.capture);
                }
                assert_ne!(
                    captures[0], captures[1],
                    "independent copy invocations define independent captures"
                );
            }

            #[test]
            fn match_arms_preserve_order_source_and_nonnumeric_values() {
                use super::{MatchPattern, TableMatchArm, TableMatchExpression};
                let mut syntax = crate::SyntaxTrees::default();
                let table = &mut syntax.tables.expressions;
                let subject = table.insert(ExpressionNode::Boolean(true));
                let pattern = table.insert(ExpressionNode::Boolean(false));
                let first = table.insert(ExpressionNode::String("first".as_bytes().into()));
                let fallback = table.insert(ExpressionNode::String("fallback".as_bytes().into()));
                let source_span =
                    source::SourceSpan::new(source::SourceId(7), source::Span::new(11, 29));
                let arms = table.insert_match_arms([
                    TableMatchArm {
                        pattern: MatchPattern::Value(pattern),
                        value: first,
                        source_span,
                    },
                    TableMatchArm {
                        pattern: MatchPattern::Wildcard,
                        value: fallback,
                        source_span,
                    },
                ]);
                let expression = table.insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    arms,
                    ..Default::default()
                }));
                table.set_source_span(expression, source_span);
                assert_eq!(
                    table.match_arms(arms)[0].pattern,
                    MatchPattern::Value(pattern)
                );
                assert_eq!(table.match_arms(arms)[1].pattern, MatchPattern::Wildcard);
                assert_eq!(table.match_arms(arms)[0].source_span, source_span);
                assert_eq!(
                    syntax.display_expression_name(expression),
                    "match true { false => \"first\", _ => \"fallback\" }"
                );
            }

            #[test]
            fn call_acknowledgement_preserves_shared_forest_and_call_payload() {
                use super::{ExpressionHandle, ExpressionTable, TableCallExpression};
                use crate::identifier::Identifier;
                let mut table = ExpressionTable::new();
                let argument = table.insert(ExpressionNode::Boolean(true));
                let arguments = table.insert_expression_handles([argument]);
                let call = table.insert(ExpressionNode::Call(TableCallExpression {
                    receiver: ExpressionHandle::invalid(),
                    target: Identifier::generated("operation"),
                    target_is_static: true,
                    machine_arguments: Box::default(),
                    arguments,
                    evidence_arguments: vec![Identifier::generated("evidence")].into_boxed_slice(),
                    operational_acknowledgement: Default::default(),
                }));
                let span = source::SourceSpan::new(source::SourceId(7), source::Span::new(11, 29));
                table.set_source_span(call, span);
                table.share_pages();
                let snapshot = table.clone();
                let revision = table.expressions.revision();
                let acknowledgement = language_core::CallOperationalAcknowledgement {
                    acknowledges_suspend: true,
                    acknowledges_block: true,
                    ..Default::default()
                };
                assert!(table.set_call_acknowledgement(call, acknowledgement));
                assert!(table.expressions.revision() > revision);
                assert_eq!(snapshot.expressions.revision(), revision);
                let ExpressionNode::Call(current) = table.expression(call) else {
                    panic!("call lost")
                };
                let ExpressionNode::Call(retained) = snapshot.expression(call) else {
                    panic!("snapshot call lost")
                };
                assert_eq!(current.operational_acknowledgement, acknowledgement);
                assert_eq!(retained.operational_acknowledgement, Default::default());
                let mut restored = current.clone();
                restored.operational_acknowledgement = retained.operational_acknowledgement;
                assert_eq!(format!("{restored:?}"), format!("{retained:?}"));
                assert_eq!(table.expression_handles(current.arguments), &[argument]);
                assert_eq!(table.source_span(call), span);
                assert_eq!(snapshot.source_span(call), span);
            }

            #[test]
            fn rejected_call_acknowledgement_leaves_shared_page_untouched() {
                let mut table = super::ExpressionTable::new();
                let expression = table.insert(ExpressionNode::Boolean(true));
                table.share_pages();
                let snapshot = table.clone();
                let revision = table.expressions.revision();
                assert!(!table.set_call_acknowledgement(expression, Default::default()));
                assert_eq!(table.expressions.revision(), revision);
                assert!(std::ptr::eq(
                    table.expression(expression),
                    snapshot.expression(expression)
                ));
                assert!(matches!(
                    table.expression(expression),
                    ExpressionNode::Boolean(true)
                ));
            }
        }

        pub type ExpressionHandle = Handle<ExpressionNode>;
        pub type ExpressionFrontier = PagedArenaFrontier<ExpressionNode>;

        paged_roster! {
            pub struct ExpressionTable => pub(crate) ExpressionTableAppendCheckpoint {
                arena expressions: ExpressionNode,
                arena source_spans: SourceSpan,
                arena expression_handles: ExpressionHandle,
                arena identifier_path_members: Identifier,
                arena struct_fields: TableStructLiteralField,
                arena match_arms: TableMatchArm,
                arena match_fields: TableMatchField,
                arena match_subjects: MatchSubjectCapture,
                arena static_members: StaticMachineArgument,
            }
        }

        impl ExpressionTable {
            pub fn insert_static_member(
                &mut self,
                member: StaticMachineArgument,
            ) -> Handle<StaticMachineArgument> {
                self.static_members.insert(member)
            }

            pub fn static_member(
                &self,
                member: Handle<StaticMachineArgument>,
            ) -> Option<&StaticMachineArgument> {
                self.static_members
                    .is_valid(member)
                    .then(|| self.static_members.get(member))
            }

            pub fn static_member_mut(
                &mut self,
                member: Handle<StaticMachineArgument>,
            ) -> Option<&mut StaticMachineArgument> {
                if self.static_members.is_valid(member) {
                    Some(self.static_members.get_mut(member))
                } else {
                    None
                }
            }

            pub fn insert_match_arms(
                &mut self,
                arms: impl IntoIterator<Item = TableMatchArm>,
            ) -> HandleSpan<TableMatchArm> {
                self.match_arms.insert_many(arms)
            }

            pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
                self.match_arms.span_or_empty(arms)
            }

            pub fn insert_match_fields(
                &mut self,
                fields: impl IntoIterator<Item = TableMatchField>,
            ) -> HandleSpan<TableMatchField> {
                self.match_fields.insert_many(fields)
            }

            pub fn match_fields(&self, fields: HandleSpan<TableMatchField>) -> &[TableMatchField] {
                self.match_fields.span_or_empty(fields)
            }

            pub fn match_pattern_dependencies(
                &self,
                pattern: MatchPattern,
            ) -> impl Iterator<Item = ExpressionHandle> + '_ {
                let (head, fields, guard) = match pattern {
                    MatchPattern::Value(value) => {
                        (value, HandleSpan::empty(), ExpressionHandle::invalid())
                    }
                    MatchPattern::Destructure(pattern) => {
                        (pattern.head, pattern.fields, pattern.guard)
                    }
                    MatchPattern::Wildcard => (
                        ExpressionHandle::invalid(),
                        HandleSpan::empty(),
                        ExpressionHandle::invalid(),
                    ),
                };
                head.is_valid()
                    .then_some(head)
                    .into_iter()
                    .chain(self.match_fields(fields).iter().filter_map(
                        |field| match &field.action {
                            MatchFieldAction::Equal(value) => Some(*value),
                            _ => None,
                        },
                    ))
                    .chain(guard.is_valid().then_some(guard))
            }

            pub fn insert_match_subject(
                &mut self,
                subject: ExpressionHandle,
            ) -> Handle<MatchSubjectCapture> {
                self.match_subjects.insert(MatchSubjectCapture {
                    purpose: SubjectCapturePurpose::Match,
                    subject,
                })
            }

            pub fn insert_transition_subject(
                &mut self,
                subject: ExpressionHandle,
            ) -> Handle<MatchSubjectCapture> {
                self.match_subjects.insert(MatchSubjectCapture {
                    purpose: SubjectCapturePurpose::Transition,
                    subject,
                })
            }

            pub fn match_subject(
                &self,
                capture: Handle<MatchSubjectCapture>,
            ) -> &MatchSubjectCapture {
                self.match_subjects.get(capture)
            }

            pub fn new() -> Self {
                Self::new_with_generation(1)
            }

            pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
                let handle = self.expressions.insert(expression);
                let source_span = self.source_spans.append(SourceSpan::default());
                debug_assert_eq!(source_span.arena_index(), handle.arena_index());
                debug_assert_eq!(source_span.generation(), handle.generation());
                handle
            }

            pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
                *self.source_spans.get(source_span_handle(handle))
            }

            pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
                *self.source_spans.get_mut(source_span_handle(handle)) = source_span;
            }

            pub fn append_expression_handle(
                &mut self,
                expression: ExpressionHandle,
            ) -> Handle<ExpressionHandle> {
                self.expression_handles.append(expression)
            }

            pub fn insert_expression_handles(
                &mut self,
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> HandleSpan<ExpressionHandle> {
                self.expression_handles.insert_many(expressions)
            }

            pub fn insert_struct_fields(
                &mut self,
                fields: impl IntoIterator<Item = TableStructLiteralField>,
            ) -> HandleSpan<TableStructLiteralField> {
                self.struct_fields.insert_many(fields)
            }

            pub fn append_identifier_path_member(
                &mut self,
                member: Identifier,
            ) -> Handle<Identifier> {
                self.identifier_path_members.append(member)
            }

            pub fn append_identifier_path_member_to_span(
                &mut self,
                span: &mut HandleSpan<Identifier>,
                member: Identifier,
            ) {
                self.identifier_path_members.append_to_span(span, member);
            }

            pub fn copy_identifier_path_prefix(
                &mut self,
                span: HandleSpan<Identifier>,
                count: usize,
            ) -> HandleSpan<Identifier> {
                let count = count.min(span.len());
                if count == 0 {
                    return HandleSpan::empty();
                }

                self.identifier_path_members.copy_span_pair(
                    HandleSpan::from_parts(span.start(), count as u32),
                    HandleSpan::empty(),
                )
            }

            pub fn contains_expression(&self, handle: ExpressionHandle) -> bool {
                self.expressions.is_valid(handle)
            }

            #[inline]
            pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
                self.expressions.get(handle)
            }

            pub fn expression_handles(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.expression_handles.span_or_empty(span)
            }

            pub fn struct_fields(
                &self,
                span: HandleSpan<TableStructLiteralField>,
            ) -> &[TableStructLiteralField] {
                self.struct_fields.span_or_empty(span)
            }

            pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
                self.identifier_path_members.span_or_empty(span)
            }

            pub fn iter_expressions(
                &self,
            ) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
                self.expressions.iter()
            }

            pub fn replace_expression(&mut self, handle: ExpressionHandle, node: ExpressionNode) {
                *self.expressions.get_mut(handle) = node;
            }

            pub fn set_call_acknowledgement(
                &mut self,
                expression: ExpressionHandle,
                acknowledgement: language_core::CallOperationalAcknowledgement,
            ) -> bool {
                if !matches!(self.expressions.get(expression), ExpressionNode::Call(_)) {
                    return false;
                }
                let ExpressionNode::Call(call) = self.expressions.get_mut(expression) else {
                    unreachable!("call kind was checked before acquiring the mutable page")
                };
                call.operational_acknowledgement = acknowledgement;
                true
            }

            pub fn expression_count(&self) -> usize {
                self.expressions.len()
            }

            pub fn frontier(&self) -> ExpressionFrontier {
                self.expressions.frontier()
            }
        }

        fn source_span_handle(handle: ExpressionHandle) -> Handle<SourceSpan> {
            Handle::from_parts(handle.arena_index(), handle.generation())
        }

        impl Default for ExpressionTable {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub enum ExpressionNode {
            Match(TableMatchExpression),
            SavedSubject(TableSavedSubject),
            ArrayLiteral(HandleSpan<ExpressionHandle>),
            Atomic(TableAtomicExpression),
            Binary(TableBinaryExpression),
            Boolean(bool),
            Cast(TableCastExpression),
            Call(TableCallExpression),
            Float(FloatSpelling),
            UndeterminedNaN(numerics::literals::FloatFormat),
            Indexed(TableIndexedExpression),
            Integer(IntegerLiteral),
            Membership(TableMembershipExpression),
            Member(TableMemberExpression),
            Borrow(TableBorrowExpression),
            Name(TableNamePath),
            Range(TableRangeExpression),
            StructLiteral(TableStructLiteral),
            String(source::SourceBytes),
            Unary(TableUnaryExpression),
            TypeExpression(crate::types::TypeReferenceHandle),
            ZeroValue(crate::types::TypeReferenceHandle),
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableBorrowExpression {
            pub target: ExpressionHandle,
            pub access: language_core::ReferenceAccess,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableAtomicExpression {
            pub value: ExpressionHandle,
            pub result: ExpressionHandle,
            pub ordering: language_core::atomic::AtomicOrderingPlan,
            pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
        }

        impl Default for ExpressionNode {
            fn default() -> Self {
                Self::Integer(IntegerLiteral::zero())
            }
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableBinaryExpression {
            pub left: ExpressionHandle,
            pub operator: BinaryOperator,
            pub right: ExpressionHandle,
            /// The policy an operator suffix chose for this one operation
            /// (`+%`, `+|`, `+!`); `Exact` for the bare operator.
            pub policy: numerics::arithmetic::ArithmeticDomain,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableUnaryExpression {
            pub operator: UnaryOperator,
            pub operand: ExpressionHandle,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableCastExpression {
            pub value: ExpressionHandle,
            pub target_type: crate::types::TypeReferenceHandle,
            pub target_label: HandleSpan<Identifier>,
            pub domain: numerics::arithmetic::ArithmeticDomain,
            pub semantic_domain: HandleSpan<Identifier>,
            pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
            pub form: language_core::cast_form::CastForm,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableIndexedExpression {
            pub collection: ExpressionHandle,
            pub index: ExpressionHandle,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableRangeExpression {
            pub start: ExpressionHandle,
            pub end: ExpressionHandle,
            pub end_inclusive: bool,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableMembershipExpression {
            pub value: ExpressionHandle,
            pub domain: HandleSpan<Identifier>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableMemberExpression {
            pub receiver: ExpressionHandle,
            pub access_source_span: SourceSpan,
            pub member: Identifier,
            pub case_variant: Option<Identifier>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableCallExpression {
            pub receiver: ExpressionHandle,
            pub target: Identifier,
            pub target_is_static: bool,
            pub machine_arguments: Box<[StaticMachineArgument]>,
            pub arguments: HandleSpan<ExpressionHandle>,
            pub evidence_arguments: Box<[Identifier]>,
            pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct StaticMachineArgument {
            pub type_reference: crate::types::TypeReferenceHandle,
            pub path: Box<[Identifier]>,
            pub application: Option<Box<StaticSymbolApplication>>,
            pub const_literal: Option<numerics::literals::IntegerLiteral>,
            pub evidence_projection: Option<EvidenceProjection>,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct StaticSymbolApplication {
            pub member: Handle<StaticMachineArgument>,
            pub lifetime_arguments: Box<[Identifier]>,
            pub arguments: Box<[StaticMachineArgument]>,
        }

        // A path keeps each static telescope on the component it applies to.
        // Ordinary value names have no application. Describing such a path is
        // distinct from invoking it and uses the same application representation
        // as a static argument.
        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableNamePath {
            pub members: HandleSpan<Identifier>,
            pub application: Option<Box<StaticSymbolApplication>>,
        }

        impl From<HandleSpan<Identifier>> for TableNamePath {
            fn from(members: HandleSpan<Identifier>) -> Self {
                Self {
                    members,
                    application: None,
                }
            }
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct EvidenceProjection {
            pub term: Identifier,
            pub member: Identifier,
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableStructLiteral {
            pub application_type: crate::types::TypeReferenceHandle,
            pub constructor_name: Identifier,
            pub fields: HandleSpan<TableStructLiteralField>,
        }

        impl Default for TableStructLiteral {
            fn default() -> Self {
                Self {
                    application_type: Default::default(),
                    constructor_name: Identifier::generated(""),
                    fields: HandleSpan::empty(),
                }
            }
        }

        #[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record)]
        pub struct TableStructLiteralField {
            pub name: Identifier,
            pub value: ExpressionHandle,
        }

        impl Default for TableStructLiteralField {
            fn default() -> Self {
                Self {
                    name: Identifier::generated(""),
                    value: ExpressionHandle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum BinaryOperator {
            Add,
            And,
            BitwiseAnd,
            BitwiseOr,
            BitwiseXor,
            Divide,
            Equal,
            Greater,
            GreaterOrEqual,
            Less,
            LessOrEqual,
            Modulo,
            Multiply,
            NotEqual,
            Or,
            ShiftLeft,
            ShiftRight,
            Subtract,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum UnaryOperator {
            BitwiseNot,
            LogicalNot,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableMatchExpression {
            pub subject: ExpressionHandle,
            pub capture: Handle<MatchSubjectCapture>,
            pub arms: HandleSpan<TableMatchArm>,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub enum SubjectCapturePurpose {
            #[default]
            Match,
            Transition,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct MatchSubjectCapture {
            pub purpose: SubjectCapturePurpose,
            pub subject: ExpressionHandle,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableSavedSubject {
            pub component: language_core::PatternComponent,
            pub capture: Handle<MatchSubjectCapture>,
            pub pattern_head: ExpressionHandle,
        }

        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            syntax_trees_derive::PlainSyntaxEquivalent,
            compiler_storage::Record,
        )]
        pub struct TableMatchArm {
            pub pattern: MatchPattern,
            pub value: ExpressionHandle,
            pub source_span: SourceSpan,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum MatchPattern {
            Value(ExpressionHandle),
            Destructure(TableDestructurePattern),
            #[default]
            Wildcard,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableDestructurePattern {
            pub application_type: crate::types::TypeReferenceHandle,
            pub head: ExpressionHandle,
            pub fields: HandleSpan<TableMatchField>,
            pub has_rest: bool,
            pub guard: ExpressionHandle,
            pub proof_selectors: HandleSpan<crate::statement::TableOutcomeProofSelector>,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub struct TableMatchField {
            pub component: language_core::PatternComponent,
            pub member: Identifier,
            pub action: MatchFieldAction,
        }

        #[derive(
            Debug, Clone, Default, syntax_trees_derive::SyntaxEquivalent, compiler_storage::Record,
        )]
        pub enum MatchFieldAction {
            Bind(Identifier),
            Equal(ExpressionHandle),
            #[default]
            Waive,
        }
    }
    pub mod float_spelling {
        use crate::{RetainedSource, SyntaxTextContext, SyntaxTextOwner};
        use arena::Handle;
        use source::SourceSpan;

        #[derive(Clone, compiler_storage::Record)]
        pub struct FloatSpelling {
            text: FloatSpellingText,
            source_span: SourceSpan,
        }

        #[derive(Clone, compiler_storage::Record)]
        enum FloatSpellingText {
            Owned(Box<str>),
            SourceBacked(Handle<RetainedSource>),
            Materialized(numerics::literals::FloatLiteral),
        }

        impl FloatSpelling {
            pub fn new(text: impl Into<String>, source_span: SourceSpan) -> Self {
                Self {
                    text: FloatSpellingText::Owned(text.into().into_boxed_str()),
                    source_span,
                }
            }

            pub fn generated(text: impl Into<String>) -> Self {
                Self::new(text, SourceSpan::default())
            }

            pub fn materialized(
                value: numerics::literals::FloatLiteral,
                source_span: SourceSpan,
            ) -> Self {
                Self {
                    text: FloatSpellingText::Materialized(value),
                    source_span,
                }
            }

            pub fn literal(
                &self,
                context: &impl SyntaxTextContext,
            ) -> Option<numerics::literals::FloatLiteral> {
                match &self.text {
                    FloatSpellingText::Materialized(value) => Some(value.clone()),
                    _ => numerics::literals::FloatLiteral::parse(self.as_str(context)),
                }
            }

            pub fn source_backed(source: Handle<RetainedSource>, source_span: SourceSpan) -> Self {
                Self {
                    text: FloatSpellingText::SourceBacked(source),
                    source_span,
                }
            }

            pub fn as_str<'text>(
                &'text self,
                context: &'text impl SyntaxTextContext,
            ) -> &'text str {
                match &self.text {
                    FloatSpellingText::Owned(text) => text,
                    FloatSpellingText::Materialized(value) => value.text(),
                    FloatSpellingText::SourceBacked(source) => {
                        let span = self.source_span.span;
                        &context.source_text(*source)[span.start..span.end]
                    }
                }
            }
        }

        impl crate::SyntaxEquivalent for FloatSpelling {
            fn same_syntax(
                &self,
                other: &Self,
                left: &SyntaxTextOwner,
                right: &SyntaxTextOwner,
            ) -> bool {
                self.as_str(left) == other.as_str(right)
                    && match (&self.text, &other.text) {
                        (
                            FloatSpellingText::Materialized(left),
                            FloatSpellingText::Materialized(right),
                        ) => left == right,
                        (FloatSpellingText::Materialized(_), _)
                        | (_, FloatSpellingText::Materialized(_)) => false,
                        _ => true,
                    }
            }
        }

        impl From<&str> for FloatSpelling {
            fn from(text: &str) -> Self {
                Self::generated(text)
            }
        }

        impl From<String> for FloatSpelling {
            fn from(text: String) -> Self {
                Self::generated(text)
            }
        }

        impl std::fmt::Debug for FloatSpelling {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut value = formatter.debug_struct("FloatSpelling");
                match &self.text {
                    FloatSpellingText::Owned(text) => {
                        value.field("text", text);
                    }
                    FloatSpellingText::SourceBacked(source) => {
                        value.field("source", source);
                    }
                    FloatSpellingText::Materialized(literal) => {
                        value.field("materialized", literal);
                    }
                }
                value.field("source_span", &self.source_span).finish()
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::SyntaxEquivalent;
            use source::{SourceId, Span};
            use std::sync::Arc;

            #[test]
            fn equality_uses_exact_spelling_across_owners_and_generated_provenance() {
                let mut left = SyntaxTextOwner::default();
                let mut right = SyntaxTextOwner::default();
                let source = left.retain(Arc::from("1.25f32"));
                let other = right.retain(Arc::from("xx1.25f32"));
                let span = SourceSpan {
                    source_id: SourceId(7),
                    span: Span { start: 0, end: 7 },
                };
                let authored = FloatSpelling::source_backed(source, span);
                let shifted = FloatSpelling::source_backed(
                    other,
                    SourceSpan {
                        source_id: SourceId(8),
                        span: Span { start: 2, end: 9 },
                    },
                );
                assert!(authored.same_syntax(&shifted, &left, &right));
                let generated = FloatSpelling::new("2.5f32", span);
                assert_eq!(
                    generated.as_str(&left),
                    "2.5f32",
                    "provenance does not replace generated spelling"
                );
                assert!(!authored.same_syntax(&generated, &left, &right));
                assert!(authored.same_syntax(&FloatSpelling::generated("1.25f32"), &left, &right));
            }

            #[test]
            #[should_panic(expected = "does not belong to this forest")]
            fn another_owner_cannot_resolve_a_float_source_handle() {
                let mut owner = SyntaxTextOwner::default();
                let source = owner.retain(Arc::from("1.0"));
                let spelling = FloatSpelling::source_backed(
                    source,
                    SourceSpan {
                        source_id: SourceId(1),
                        span: Span { start: 0, end: 3 },
                    },
                );
                spelling.as_str(&SyntaxTextOwner::default());
            }
        }
    }
}
