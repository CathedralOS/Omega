use crate::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableStructLiteral,
    TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::item::{
    CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
    CapabilityMember, CapabilityState, DataDefinition, DataField, DataMember, DataVariant, Item,
    ItemHandle, ItemTable, LibraryDefinition, LibraryFunction, Machine, Platform, State,
    StateHandle, StateParameterHandle, StateParameterNode, StateSignature, StateSignatureHandle,
    TargetDefinition, TargetHost, TargetHostSetting, TargetHostSettingValue, TrustDefinition,
    TrustLevel, TrustMode, TrustPolicy, TypeParameter, UseItem,
};
use crate::statement::{
    StatementHandle, StatementNode, StatementTable, TableAssignment, TableCall, TableLocalData,
    TableTransition, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};
use crate::types::{
    TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::source::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub root_item_handles: Arena<ItemHandle>,
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

impl SyntaxTrees {
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            root_item_handles: Arena::new(),
            items: ItemTable::new(),
            expressions: ExpressionTable::new(),
            statements: StatementTable::new(),
            type_references: TypeReferenceTable::new(),
        }
    }

    pub fn from_root_items(source_id: SourceId, items: impl IntoIterator<Item = Item>) -> Self {
        let mut syntax_trees = Self::new(source_id);

        for item in items {
            syntax_trees.push_root_item(item);
        }

        syntax_trees
    }

    pub fn push_root_item(&mut self, item: Item) -> ItemHandle {
        let handle = self.insert_item(item);
        self.root_item_handles.append(handle);
        handle
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        self.root_item_handles.storage_slice()
    }

    pub fn root_item(&self, handle: ItemHandle) -> &Item {
        self.items.item(handle)
    }

    pub fn root_items(&self) -> impl Iterator<Item = &Item> {
        self.root_item_handles()
            .iter()
            .map(|handle| self.root_item(*handle))
    }

    pub fn root_item_count(&self) -> usize {
        self.root_item_handles.len()
    }

    pub fn extend_from(&mut self, other: &SyntaxTrees) {
        for handle in other.root_item_handles() {
            self.push_copied_root_item(other, *handle);
        }
    }

    fn insert_item(&mut self, item: Item) -> ItemHandle {
        match &item {
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Platform(platform) => self.insert_platform(platform),
            Item::Capability(_)
            | Item::Data(_)
            | Item::Invariant(_)
            | Item::Library(_)
            | Item::Target(_)
            | Item::TrustDefinition(_)
            | Item::Use(_) => {}
        }

        self.items.append_item(item)
    }

    fn insert_machine(&mut self, machine: &Machine) {
        self.items.insert_machine(machine);
    }

    fn insert_platform(&mut self, platform: &Platform) {
        self.items.insert_platform(platform);
    }

    fn push_copied_root_item(&mut self, other: &SyntaxTrees, handle: ItemHandle) -> ItemHandle {
        let item = self.copy_item(other, other.root_item(handle));
        self.push_root_item(item)
    }

    fn copy_item(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
        match item {
            Item::Capability(capability) => {
                Item::Capability(self.copy_capability_definition(other, capability))
            }
            Item::Data(data) => Item::Data(self.copy_data_definition(other, data)),
            Item::Invariant(invariant) => Item::Invariant(crate::item::InvariantDefinition {
                name: invariant.name.clone(),
                constraints: self.copy_constraint_span(other, invariant.constraints),
            }),
            Item::Library(library) => Item::Library(self.copy_library_definition(other, library)),
            Item::TrustDefinition(trust) => Item::TrustDefinition(TrustDefinition {
                name: trust.name.clone(),
                token_count: trust.token_count,
            }),
            Item::Use(use_item) => Item::Use(UseItem {
                path: self.copy_item_identifier_span(other, use_item.path),
            }),
            Item::Machine(machine) => Item::Machine(self.copy_machine(other, machine)),
            Item::Platform(platform) => Item::Platform(self.copy_platform(other, platform)),
            Item::Target(target) => Item::Target(self.copy_target_definition(other, target)),
        }
    }

    fn copy_capability_definition(
        &mut self,
        other: &SyntaxTrees,
        capability: &CapabilityDefinition,
    ) -> CapabilityDefinition {
        CapabilityDefinition {
            name: capability.name.clone(),
            members: self.copy_capability_member_span(other, capability.members),
        }
    }

    fn copy_data_definition(
        &mut self,
        other: &SyntaxTrees,
        data: &DataDefinition,
    ) -> DataDefinition {
        DataDefinition {
            name: data.name.clone(),
            type_parameters: self.copy_type_parameter_span(other, data.type_parameters),
            members: self.copy_data_member_span(other, data.members),
        }
    }

    fn copy_library_definition(
        &mut self,
        other: &SyntaxTrees,
        library: &LibraryDefinition,
    ) -> LibraryDefinition {
        LibraryDefinition {
            name: library.name.clone(),
            path: library.path.clone(),
            calling_convention: library.calling_convention.clone(),
            functions: self.copy_library_function_span(other, library.functions),
        }
    }

    fn copy_machine(&mut self, other: &SyntaxTrees, machine: &Machine) -> Machine {
        Machine {
            name: machine.name.clone(),
            states: self.copy_state_handle_span(other, machine.states),
        }
    }

    fn copy_platform(&mut self, other: &SyntaxTrees, platform: &Platform) -> Platform {
        Platform {
            name: platform.name.clone(),
            states: self.copy_state_signature_handle_span(other, platform.states),
        }
    }

    fn copy_target_definition(
        &mut self,
        other: &SyntaxTrees,
        target: &TargetDefinition,
    ) -> TargetDefinition {
        TargetDefinition {
            name: target.name.clone(),
            host: target.host.as_ref().map(|host| TargetHost {
                provider: self.copy_item_identifier_span(other, host.provider),
                settings: self.copy_target_host_setting_span(other, host.settings),
            }),
            trust_policies: self.copy_trust_policy_span(other, target.trust_policies),
        }
    }

    fn copy_type_parameter_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TypeParameter>,
    ) -> HandleSpan<TypeParameter> {
        self.copy_span(
            other.items.type_parameters(span).iter().cloned(),
            |this, parameter| this.items.append_type_parameter(parameter),
        )
    }

    fn copy_trust_level_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TrustLevel>,
    ) -> HandleSpan<TrustLevel> {
        self.copy_span(
            other
                .items
                .trust_levels(span)
                .iter()
                .map(|level| match level {
                    TrustLevel::Host => TrustLevel::Host,
                    TrustLevel::Named(name) => TrustLevel::Named(name.clone()),
                }),
            |this, trust_level| this.items.append_trust_level(trust_level),
        )
    }

    fn copy_library_function_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<LibraryFunction>,
    ) -> HandleSpan<LibraryFunction> {
        self.copy_mapped_span(
            other.items.library_functions(span),
            |this, function| LibraryFunction {
                signature: this.copy_state_signature_value(other, &function.signature),
                symbol: function.symbol.clone(),
                calling_convention: function.calling_convention.clone(),
                trusts: this.copy_trust_level_span(other, function.trusts),
            },
            |this, function| this.items.append_library_function(function),
        )
    }

    fn copy_capability_member_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<CapabilityMember>,
    ) -> HandleSpan<CapabilityMember> {
        self.copy_mapped_span(
            other.items.capability_members(span),
            |this, member| match member {
                CapabilityMember::Field(field) => CapabilityMember::Field(CapabilityField {
                    name: field.name.clone(),
                    type_reference: this.copy_type_reference_handle(other, field.type_reference),
                }),
                CapabilityMember::State(state) => CapabilityMember::State(CapabilityState {
                    signature: this.copy_state_signature_value(other, &state.signature),
                    contracts: this.copy_capability_contract_span(other, state.contracts),
                }),
            },
            |this, member| this.items.append_capability_member(member),
        )
    }

    fn copy_capability_contract_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<CapabilityContract>,
    ) -> HandleSpan<CapabilityContract> {
        let contracts = other
            .items
            .capability_contracts(span)
            .iter()
            .map(|contract| CapabilityContract {
                kind: match &contract.kind {
                    CapabilityContractKind::Ensures => CapabilityContractKind::Ensures,
                    CapabilityContractKind::Requires => CapabilityContractKind::Requires,
                    CapabilityContractKind::Trusted(TrustLevel::Host) => {
                        CapabilityContractKind::Trusted(TrustLevel::Host)
                    }
                    CapabilityContractKind::Trusted(TrustLevel::Named(name)) => {
                        CapabilityContractKind::Trusted(TrustLevel::Named(name.clone()))
                    }
                },
                token_count: contract.token_count,
            });
        self.copy_span(contracts, |this, contract| {
            this.items.append_capability_contract(contract)
        })
    }

    fn copy_data_member_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<DataMember>,
    ) -> HandleSpan<DataMember> {
        self.copy_mapped_span(
            other.items.data_members(span),
            |this, member| match member {
                DataMember::Field(field) => DataMember::Field(DataField {
                    name: field.name.clone(),
                    type_reference: this.copy_type_reference_handle(other, field.type_reference),
                    initial_value: this.copy_expression_handle(other, field.initial_value),
                }),
                DataMember::Variant(variant) => DataMember::Variant(DataVariant {
                    name: variant.name.clone(),
                }),
            },
            |this, member| this.items.append_data_member(member),
        )
    }

    fn copy_target_host_setting_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TargetHostSetting>,
    ) -> HandleSpan<TargetHostSetting> {
        let settings = other
            .items
            .target_host_settings(span)
            .iter()
            .map(|setting| TargetHostSetting {
                name: setting.name.clone(),
                value: match &setting.value {
                    TargetHostSettingValue::Call {
                        name,
                        argument_tokens,
                    } => TargetHostSettingValue::Call {
                        name: name.clone(),
                        argument_tokens: *argument_tokens,
                    },
                    TargetHostSettingValue::Named(name) => {
                        TargetHostSettingValue::Named(name.clone())
                    }
                },
            });
        self.copy_span(settings, |this, setting| {
            this.items.append_target_host_setting(setting)
        })
    }

    fn copy_trust_policy_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TrustPolicy>,
    ) -> HandleSpan<TrustPolicy> {
        self.copy_mapped_span(
            other.items.trust_policies(span),
            |this, policy| TrustPolicy {
                mode: match policy.mode {
                    TrustMode::Checked => TrustMode::Checked,
                    TrustMode::Unchecked => TrustMode::Unchecked,
                },
                path: this.copy_item_identifier_span(other, policy.path),
            },
            |this, policy| this.items.append_trust_policy(policy),
        )
    }

    fn copy_state_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateHandle>,
    ) -> HandleSpan<StateHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_handles(span).iter().copied() {
            let state = other.items.state(handle);
            let parameters = self.copy_state_parameter_handle_span(other, state.parameters);
            let return_type = self.copy_type_reference_handle(other, state.return_type);
            let statements = self.copy_statement_handle_span(other, state.statements);
            let copied = self.items.insert_state(&State {
                name: state.name.clone(),
                parameters,
                return_type,
                statements,
            });
            let copied = self.items.append_state_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_state_signature_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateSignatureHandle>,
    ) -> HandleSpan<StateSignatureHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_signatures(span).iter().copied() {
            let signature = other.items.state_signature(handle);
            let copied_signature = self.copy_state_signature_node(other, signature);
            let copied = self.items.insert_state_signature(&copied_signature);
            let copied = self.items.append_state_signature_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_state_signature_value(
        &mut self,
        other: &SyntaxTrees,
        signature: &StateSignature,
    ) -> StateSignature {
        StateSignature {
            name: signature.name.clone(),
            parameters: self.copy_state_parameter_handle_span(other, signature.parameters),
            return_type: self.copy_type_reference_handle(other, signature.return_type),
        }
    }

    fn copy_state_signature_node(
        &mut self,
        other: &SyntaxTrees,
        signature: &crate::item::StateSignatureNode,
    ) -> StateSignature {
        StateSignature {
            name: signature.name.clone(),
            parameters: self.copy_state_parameter_handle_span(other, signature.parameters),
            return_type: self.copy_type_reference_handle(other, signature.return_type),
        }
    }

    fn copy_state_parameter_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateParameterHandle>,
    ) -> HandleSpan<StateParameterHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_parameters(span).iter().copied() {
            let parameter = other.items.state_parameter(handle);
            let type_reference = self.copy_type_reference_handle(other, parameter.type_reference);
            let copied = self.items.insert_state_parameter_node(StateParameterNode {
                name: parameter.name.clone(),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            });
            let copied = self.items.append_state_parameter_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_statement_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StatementHandle>,
    ) -> HandleSpan<StatementHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.statements(span).iter().copied() {
            let statement = self.copy_statement_node(other, other.statements.statement(handle));
            let copied = self.statements.insert(statement);
            let copied = self.items.append_statement_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_statement_node(
        &mut self,
        other: &SyntaxTrees,
        statement: &StatementNode,
    ) -> StatementNode {
        match statement {
            StatementNode::Assignment(assignment) => StatementNode::Assignment(TableAssignment {
                target: self.copy_expression_handle(other, assignment.target),
                value: self.copy_expression_handle(other, assignment.value),
            }),
            StatementNode::Call(call) => StatementNode::Call(TableCall {
                receiver: self.copy_statement_identifier_span(other, call.receiver),
                receiver_starts_at_self: call.receiver_starts_at_self,
                target: call.target.clone(),
                arguments: self.copy_statement_expression_span(other, call.arguments),
            }),
            StatementNode::Expression(value) => {
                StatementNode::Expression(self.copy_expression_handle(other, *value))
            }
            StatementNode::LocalData(local_data) => StatementNode::LocalData(TableLocalData {
                name: local_data.name.clone(),
                type_reference: self.copy_type_reference_handle(other, local_data.type_reference),
                initial_value: self.copy_expression_handle(other, local_data.initial_value),
            }),
            StatementNode::Transition(transition) => StatementNode::Transition(TableTransition {
                target: self.copy_transition_target(other, transition.target),
                continuation: self.copy_transition_target(other, transition.continuation),
                guard: match transition.guard {
                    TransitionGuardNode::Always => TransitionGuardNode::Always,
                    TransitionGuardNode::When(expression) => {
                        TransitionGuardNode::When(self.copy_expression_handle(other, expression))
                    }
                },
            }),
        }
    }

    fn copy_transition_target(
        &mut self,
        other: &SyntaxTrees,
        handle: TransitionTargetHandle,
    ) -> TransitionTargetHandle {
        if !handle.is_valid() {
            return TransitionTargetHandle::invalid();
        }

        let target = match other.statements.transition_target(handle) {
            TransitionTargetNode::Named {
                path,
                path_starts_at_self,
                arguments,
            } => TransitionTargetNode::Named {
                path: self.copy_statement_identifier_span(other, *path),
                path_starts_at_self: *path_starts_at_self,
                arguments: self.copy_statement_expression_span(other, *arguments),
            },
            TransitionTargetNode::Value(value) => {
                TransitionTargetNode::Value(self.copy_expression_handle(other, *value))
            }
            TransitionTargetNode::SelfTarget => TransitionTargetNode::SelfTarget,
            TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
        };

        self.statements.insert_transition_target(target)
    }

    fn copy_type_reference_handle(
        &mut self,
        other: &SyntaxTrees,
        handle: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        if !handle.is_valid() {
            return TypeReferenceHandle::invalid();
        }

        match other.type_references.type_reference(handle) {
            TypeReferenceNode::Reference {
                referee,
                is_mutable,
            } => {
                let referee = self.copy_type_reference_handle(other, *referee);
                self.type_references.insert_reference(referee, *is_mutable)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_type = self.copy_type_reference_handle(other, *base_type);
                let constraints = self.copy_constraint_span(other, *constraints);
                self.type_references
                    .insert_constrained(base_type, constraints)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_type = self.copy_type_reference_handle(other, *element_type);
                self.type_references
                    .insert_fixed_array(element_type, *length)
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = self.copy_type_reference_handle(other, *element_type);
                self.type_references.insert_slice(element_type)
            }
            TypeReferenceNode::Generic {
                base_name,
                arguments,
            } => {
                let arguments = self.copy_type_reference_handle_span(other, *arguments);
                self.type_references
                    .insert_generic(base_name.clone(), arguments)
            }
            TypeReferenceNode::Named(name) => self.type_references.insert_named(name.clone()),
            TypeReferenceNode::SelfType => self.type_references.insert_self_type(),
            TypeReferenceNode::Unit => self.type_references.insert_unit(),
        }
    }

    fn copy_type_reference_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.copy_mapped_span(
            other
                .type_references
                .type_reference_handles(span)
                .iter()
                .copied(),
            |this, handle| this.copy_type_reference_handle(other, handle),
            |this, handle| this.type_references.append_type_reference_handle(handle),
        )
    }

    fn copy_constraint_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.copy_mapped_span(
            other.type_references.constraints(span),
            |this, constraint| match constraint {
                TypeConstraintNode::Named(name) => TypeConstraintNode::Named(name.clone()),
                TypeConstraintNode::Range { minimum, maximum } => TypeConstraintNode::Range {
                    minimum: this.copy_expression_handle(other, *minimum),
                    maximum: this.copy_expression_handle(other, *maximum),
                },
            },
            |this, constraint| this.type_references.append_constraint(constraint),
        )
    }

    fn copy_expression_handle(
        &mut self,
        other: &SyntaxTrees,
        handle: ExpressionHandle,
    ) -> ExpressionHandle {
        if !handle.is_valid() {
            return ExpressionHandle::invalid();
        }

        let expression = match other.expressions.expression(handle) {
            ExpressionNode::ArrayLiteral(values) => {
                ExpressionNode::ArrayLiteral(self.copy_expression_handle_list(other, *values))
            }
            ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
                left: self.copy_expression_handle(other, binary.left),
                operator: binary.operator,
                right: self.copy_expression_handle(other, binary.right),
            }),
            ExpressionNode::Boolean(value) => ExpressionNode::Boolean(*value),
            ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
                value: self.copy_expression_handle(other, cast.value),
                target_type: self.copy_expression_identifier_span(other, cast.target_type),
            }),
            ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
                receiver: self.copy_expression_handle(other, call.receiver),
                target: call.target.clone(),
                arguments: self.copy_expression_handle_list(other, call.arguments),
            }),
            ExpressionNode::Float(value) => ExpressionNode::Float(value.clone()),
            ExpressionNode::Indexed(indexed) => ExpressionNode::Indexed(TableIndexedExpression {
                collection: self.copy_expression_handle(other, indexed.collection),
                index: self.copy_expression_handle(other, indexed.index),
            }),
            ExpressionNode::Integer(value) => ExpressionNode::Integer(*value),
            ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                receiver: self.copy_expression_handle(other, member.receiver),
                member: member.member.clone(),
            }),
            ExpressionNode::Mutable(expression) => {
                ExpressionNode::Mutable(self.copy_expression_handle(other, *expression))
            }
            ExpressionNode::Name(path) => {
                ExpressionNode::Name(self.copy_expression_identifier_span(other, *path))
            }
            ExpressionNode::SelfValue => ExpressionNode::SelfValue,
            ExpressionNode::StructLiteral(struct_literal) => {
                ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    fields: self.copy_struct_field_span(other, struct_literal.fields),
                })
            }
            ExpressionNode::String(value) => ExpressionNode::String(value.clone()),
        };

        self.expressions.insert(expression)
    }

    fn copy_expression_handle_list(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.copy_mapped_span(
            other.expressions.expression_handles(span).iter().copied(),
            |this, handle| this.copy_expression_handle(other, handle),
            |this, handle| this.expressions.append_expression_handle(handle),
        )
    }

    fn copy_struct_field_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.copy_mapped_span(
            other.expressions.struct_fields(span),
            |this, field| TableStructLiteralField {
                name: field.name.clone(),
                value: this.copy_expression_handle(other, field.value),
            },
            |this, field| this.expressions.append_struct_field(field),
        )
    }

    fn copy_item_identifier_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        self.copy_span(
            other.items.identifier_path_members(span).iter().cloned(),
            |this, member| this.items.append_identifier_path_member(member),
        )
    }

    fn copy_statement_identifier_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        self.copy_span(
            other
                .statements
                .identifier_path_members(span)
                .iter()
                .cloned(),
            |this, member| this.statements.append_identifier_path_member(member),
        )
    }

    fn copy_expression_identifier_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        self.copy_span(
            other
                .expressions
                .identifier_path_members(span)
                .iter()
                .cloned(),
            |this, member| this.expressions.append_identifier_path_member(member),
        )
    }

    fn copy_statement_expression_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.copy_mapped_span(
            other.statements.expression_handles(span).iter().copied(),
            |this, handle| this.copy_expression_handle(other, handle),
            |this, handle| this.statements.append_expression_handle(handle),
        )
    }

    fn copy_mapped_span<S, T>(
        &mut self,
        values: impl IntoIterator<Item = S>,
        mut map: impl FnMut(&mut Self, S) -> T,
        mut append: impl FnMut(&mut Self, T) -> Handle<T>,
    ) -> HandleSpan<T> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for value in values {
            let value = map(self, value);
            let handle = append(self, value);
            if count == 0 {
                start = handle;
            }
            count = count.checked_add(1).expect("copied span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_span<T>(
        &mut self,
        values: impl IntoIterator<Item = T>,
        mut append: impl FnMut(&mut Self, T) -> Handle<T>,
    ) -> HandleSpan<T> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for value in values {
            let handle = append(self, value);
            if count == 0 {
                start = handle;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }
}

impl Default for SyntaxTrees {
    fn default() -> Self {
        Self::new(SourceId::default())
    }
}

#[cfg(test)]
mod tests {
    use super::SyntaxTrees;
    use crate::identifier::Identifier;
    use crate::item::{Item, Machine, State};
    use crate::statement::{
        StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use omega_core::arena::HandleSpan;

    #[test]
    fn syntax_trees_collect_state_expression_and_type_payloads() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let guard = syntax_trees
            .expressions
            .insert(crate::expression::ExpressionNode::Integer(1));
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Terminal);
        let statement =
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: crate::statement::TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::When(guard),
                }));
        let statement_handle = syntax_trees.items.append_statement_handle(statement);
        let statements = HandleSpan::from_parts(statement_handle, 1);
        let return_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
        let state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type,
            statements,
        });
        let state_handle = syntax_trees.items.append_state_handle(state);

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("Main"),
            states: HandleSpan::from_parts(state_handle, 1),
        }));

        assert_eq!(syntax_trees.root_item_count(), 1);
        assert_eq!(syntax_trees.type_references.type_reference_count(), 1);
        assert_eq!(syntax_trees.expressions.expression_count(), 1);
        assert_eq!(syntax_trees.statements.statement_count(), 1);
        assert_eq!(syntax_trees.items.machine_count(), 1);
        assert_eq!(syntax_trees.items.state_count(), 1);
    }

    #[test]
    fn syntax_trees_extend_from_preserves_root_payload_handles() {
        let mut file = SyntaxTrees::new(Default::default());
        let return_type = file
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));
        let state = file.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type,
            statements: HandleSpan::empty(),
        });
        let state = file.items.append_state_handle(state);
        file.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            states: HandleSpan::from_parts(state, 1),
        }));

        let mut assembled = SyntaxTrees::new(Default::default());
        assembled.extend_from(&file);

        let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
            panic!("expected machine root item");
        };
        let state_handle = assembled
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state handle");
        let state = assembled.items.state(state_handle);
        assert_eq!(state.name.as_str(), "entry");
        assert!(state.return_type.is_valid());
    }

    #[test]
    fn syntax_trees_extend_from_preserves_statement_call_arguments() {
        let mut file = SyntaxTrees::new(Default::default());
        let receiver = file
            .statements
            .append_identifier_path_member(Identifier::generated("self"));
        let receiver = HandleSpan::from_parts(receiver, 1);
        let argument = file
            .expressions
            .insert(crate::expression::ExpressionNode::Integer(0));
        let argument = file.statements.append_expression_handle(argument);
        let call = file.statements.insert(StatementNode::Call(TableCall {
            receiver,
            receiver_starts_at_self: true,
            target: Identifier::generated("take_non_negative"),
            arguments: HandleSpan::from_parts(argument, 1),
        }));
        let call = file.items.append_statement_handle(call);
        let state = file.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            statements: HandleSpan::from_parts(call, 1),
        });
        let state = file.items.append_state_handle(state);
        file.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            states: HandleSpan::from_parts(state, 1),
        }));

        let mut assembled = SyntaxTrees::new(Default::default());
        assembled.extend_from(&file);

        let Item::Machine(machine) = assembled.root_items().next().expect("machine root") else {
            panic!("expected machine root item");
        };
        let state_handle = assembled
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state handle");
        let state = assembled.items.state(state_handle);
        let statement_handle = assembled
            .items
            .statements(state.statements)
            .first()
            .copied()
            .expect("call statement");
        let StatementNode::Call(call) = assembled.statements.statement(statement_handle) else {
            panic!("expected call statement");
        };
        assert_eq!(
            assembled
                .statements
                .expression_handles(call.arguments)
                .len(),
            1
        );
    }
}
