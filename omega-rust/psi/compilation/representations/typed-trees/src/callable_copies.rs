//! Copy callable types with every table their published contract refers to.
//!
//! The destination is reserved before following a contract's children. Shared
//! and recursive references therefore keep one destination, while staging and
//! later symbol rebinding never retain a source arena coordinate as identity.

use crate::callable_values::{RuntimeCallableContract, RuntimeCallableContractHandle};
use crate::domain::ProofFact;
use crate::expression::{ExpressionCopy, ExpressionHandle, ExpressionNode, StaticMachineArgument};
use crate::signature::{SignatureContractKind, StateSignature};
use crate::types::{TypeExpressionCopy, TypeReferenceHandle};
use crate::{SymbolRebinding, TypedTreeTables, remapped};
use arena::{Handle, HandleSpan};

pub(crate) struct TypeCopy<'source, 'target, 'records> {
    pub(crate) source: &'source TypedTreeTables,
    pub(crate) target: &'target mut TypedTreeTables,
    pub(crate) records: &'records mut Vec<TypeExpressionCopy>,
    pub(crate) types: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    contracts: Vec<(RuntimeCallableContractHandle, RuntimeCallableContractHandle)>,
}

pub(crate) struct TypeRemap<'target, 'scope> {
    pub(crate) target: &'target mut TypedTreeTables,
    pub(crate) symbols: SymbolRebinding<'scope>,
    pub(crate) types: Vec<TypeReferenceHandle>,
    contracts: Vec<RuntimeCallableContractHandle>,
    expressions: Vec<ExpressionHandle>,
    facts: Vec<Handle<ProofFact>>,
}

impl TypedTreeTables {
    pub fn copy_type_reference_from(
        &mut self,
        source: &Self,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        self.copy_type_reference_with_expressions(source, reference, &mut Vec::new())
    }

    pub fn copy_type_reference_with_expressions(
        &mut self,
        source: &Self,
        reference: TypeReferenceHandle,
        records: &mut Vec<TypeExpressionCopy>,
    ) -> TypeReferenceHandle {
        TypeCopy::new(source, self, records).ty(reference)
    }

    pub fn remap_type_reference_symbols_in<'scope>(
        &mut self,
        reference: TypeReferenceHandle,
        symbols: impl Into<SymbolRebinding<'scope>>,
    ) {
        TypeRemap::new(self, symbols.into()).ty(reference);
    }

    pub fn copy_runtime_callable_binding_from(
        &mut self,
        source: &Self,
        binding: &crate::callable_values::RuntimeCallBinding,
        records: &mut Vec<TypeExpressionCopy>,
    ) -> crate::callable_values::RuntimeCallBinding {
        let mut copy = TypeCopy::new(source, self, records);
        crate::callable_values::RuntimeCallBinding {
            callee: copy.contract_expression(binding.callee),
            contract: copy.contract(binding.contract),
        }
    }

    pub fn remap_runtime_callable_binding_symbols_in<'scope>(
        &mut self,
        binding: &crate::callable_values::RuntimeCallBinding,
        symbols: impl Into<SymbolRebinding<'scope>>,
    ) {
        let mut remap = TypeRemap::new(self, symbols.into());
        remap.expression(binding.callee);
        remap.contract(binding.contract);
    }
}

impl<'source, 'target, 'records> TypeCopy<'source, 'target, 'records> {
    fn new(
        source: &'source TypedTreeTables,
        target: &'target mut TypedTreeTables,
        records: &'records mut Vec<TypeExpressionCopy>,
    ) -> Self {
        Self {
            source,
            target,
            records,
            types: Vec::new(),
            contracts: Vec::new(),
        }
    }

    pub(crate) fn contract(
        &mut self,
        source: RuntimeCallableContractHandle,
    ) -> RuntimeCallableContractHandle {
        if let Some((_, copied)) = self
            .contracts
            .iter()
            .find(|(original, _)| *original == source)
        {
            return *copied;
        }
        let copied = self
            .target
            .runtime_callable_contracts
            .insert(RuntimeCallableContract::default());
        self.contracts.push((source, copied));
        let source = self.source.runtime_callable_contracts.get(source).clone();
        let signature = self.signature(source.signature);
        *self.target.runtime_callable_contracts.get_mut(copied) = RuntimeCallableContract {
            signature,
            lexical_owner: source.lexical_owner,
            resolved_source: None,
        };
        copied
    }

    fn signature(&mut self, mut signature: StateSignature) -> StateSignature {
        use crate::data::{MachineParameterContract, TypeParameterKind};
        let mut parameters = self
            .source
            .data_type_parameters
            .span_or_empty(signature.type_parameters)
            .to_vec();
        assert_eq!(
            parameters.len(),
            signature.type_parameters.len(),
            "copied callable keeps its complete generic parameter span"
        );
        for parameter in &mut parameters {
            match &mut parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    *type_reference = self.ty(*type_reference)
                }
                TypeParameterKind::Machine { contract } => match contract {
                    MachineParameterContract::Structural(required) => {
                        *required = self.signature(required.clone())
                    }
                    MachineParameterContract::Nominal {
                        receiving_arguments,
                        trait_arguments,
                        instantiated,
                        ..
                    } => {
                        for reference in receiving_arguments
                            .iter_mut()
                            .chain(trait_arguments.iter_mut())
                        {
                            *reference = self.ty(*reference);
                        }
                        if let Some(required) = instantiated {
                            **required = self.signature((**required).clone());
                        }
                    }
                    MachineParameterContract::RequirementIdentity => {}
                },
                TypeParameterKind::Proposition { contract } => {
                    contract.parameters = self.parameters(contract.parameters)
                }
                TypeParameterKind::Type | TypeParameterKind::Implementation => {}
            }
        }
        signature.type_parameters = self.target.data_type_parameters.insert_many(parameters);
        for bound in &mut signature.conformance_bounds {
            for argument in &mut bound.arguments {
                *argument = self.ty(*argument);
            }
        }
        signature.parameters = self.parameters(signature.parameters);
        signature.return_type = self.ty(signature.return_type);
        let invokes = self
            .source
            .signature_invokes
            .span_or_empty(signature.invokes);
        assert_eq!(
            invokes.len(),
            signature.invokes.len(),
            "copied callable keeps its complete invocation span"
        );
        signature.invokes = self
            .target
            .signature_invokes
            .insert_many(invokes.iter().cloned());
        let mut contracts = self
            .source
            .signature_contracts
            .span_or_empty(signature.contracts)
            .to_vec();
        assert_eq!(
            contracts.len(),
            signature.contracts.len(),
            "copied callable keeps its complete contract span"
        );
        for contract in &mut contracts {
            let originals = contract.facts.handles().collect::<Vec<_>>();
            let facts = originals
                .iter()
                .map(|fact| self.fact(*fact))
                .collect::<Vec<_>>();
            contract.facts = self.target.proof_facts.insert_many(facts);
            for (source, destination) in originals.into_iter().zip(contract.facts.handles()) {
                let source_index = crate::proof_fact_source_span_index(source);
                if let Some(Some(span)) = self.source.proof_fact_source_spans.get(source_index) {
                    let destination_index = crate::proof_fact_source_span_index(destination);
                    self.target.proof_fact_source_spans.resize(
                        self.target
                            .proof_fact_source_spans
                            .len()
                            .max(destination_index + 1),
                        None,
                    );
                    self.target.proof_fact_source_spans[destination_index] = Some(*span);
                }
            }
        }
        signature.contracts = self.target.signature_contracts.insert_many(contracts);
        signature
    }

    fn parameters(
        &mut self,
        source: HandleSpan<crate::signature::StateParameter>,
    ) -> HandleSpan<crate::signature::StateParameter> {
        let mut parameters = self.source.state_parameters.span_or_empty(source).to_vec();
        assert_eq!(
            parameters.len(),
            source.len(),
            "copied callable keeps its complete argument span"
        );
        for parameter in &mut parameters {
            parameter.type_reference = self.ty(parameter.type_reference);
        }
        self.target.state_parameters.insert_many(parameters)
    }

    fn fact(&mut self, handle: Handle<ProofFact>) -> ProofFact {
        match self.source.proof_facts.get(handle).clone() {
            ProofFact::Expression(expression) => {
                ProofFact::Expression(self.contract_expression(expression))
            }
            ProofFact::Membership(mut membership) => {
                membership.value = self.contract_expression(membership.value);
                membership.domain = self.target.domain_path_members.insert_many(
                    self.source
                        .domain_path_members
                        .span_or_empty(membership.domain)
                        .iter()
                        .cloned(),
                );
                membership.domain_arguments = self.type_span(membership.domain_arguments);
                ProofFact::Membership(membership)
            }
            ProofFact::Proposition(mut application) => {
                let arguments = self
                    .source
                    .expression_table
                    .expression_handles(application.arguments)
                    .to_vec()
                    .into_iter()
                    .map(|argument| self.contract_expression(argument))
                    .collect::<Vec<_>>();
                application.arguments = self
                    .target
                    .expression_table
                    .insert_expression_handles(arguments);
                ProofFact::Proposition(application)
            }
        }
    }

    fn contract_expression(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        let copied = self.expression_graph(expression);
        let root = copied.root;
        self.records.push(TypeExpressionCopy {
            source: TypeReferenceHandle::invalid(),
            destination: TypeReferenceHandle::invalid(),
            expression: copied,
            contract_expression: true,
        });
        root
    }

    pub(crate) fn expression_graph(&mut self, expression: ExpressionHandle) -> ExpressionCopy {
        let copied = self
            .target
            .expression_table
            .copy_graph(Some(&self.source.expression_table), expression);
        for (_, expression) in &copied.correspondence {
            expression_type_payloads(self, *expression);
        }
        copied
    }
}

impl<'target, 'scope> TypeRemap<'target, 'scope> {
    fn new(target: &'target mut TypedTreeTables, symbols: SymbolRebinding<'scope>) -> Self {
        Self {
            target,
            symbols,
            types: Vec::new(),
            contracts: Vec::new(),
            expressions: Vec::new(),
            facts: Vec::new(),
        }
    }

    pub(crate) fn contract(&mut self, handle: RuntimeCallableContractHandle) {
        if self.contracts.contains(&handle) {
            return;
        }
        self.contracts.push(handle);
        let mut contract = self.target.runtime_callable_contracts.get(handle).clone();
        self.signature(&mut contract.signature);
        contract.lexical_owner = remapped(contract.lexical_owner, self.symbols);
        contract.resolved_source = None;
        *self.target.runtime_callable_contracts.get_mut(handle) = contract;
    }

    fn signature(&mut self, signature: &mut StateSignature) {
        use crate::data::{MachineParameterContract, TypeParameterKind};
        signature.symbol = remapped(signature.symbol, self.symbols);
        let mut parameters = self
            .target
            .data_type_parameters
            .span_or_empty(signature.type_parameters)
            .to_vec();
        for parameter in &mut parameters {
            parameter.symbol = remapped(parameter.symbol, self.symbols);
            match &mut parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => self.ty(*type_reference),
                TypeParameterKind::Machine { contract } => match contract {
                    MachineParameterContract::Structural(required) => self.signature(required),
                    MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        trait_definition,
                        requirement,
                        trait_arguments,
                        instantiated,
                    } => {
                        *receiving_trait = remapped(*receiving_trait, self.symbols);
                        *trait_definition = remapped(*trait_definition, self.symbols);
                        *requirement = remapped(*requirement, self.symbols);
                        for reference in receiving_arguments.iter().chain(trait_arguments.iter()) {
                            self.ty(*reference);
                        }
                        if let Some(required) = instantiated {
                            self.signature(required);
                        }
                    }
                    MachineParameterContract::RequirementIdentity => {}
                },
                TypeParameterKind::Proposition { contract } => self.parameters(contract.parameters),
                TypeParameterKind::Type | TypeParameterKind::Implementation => {}
            }
        }
        self.target
            .data_type_parameters
            .span_mut_or_empty(signature.type_parameters)
            .clone_from_slice(&parameters);
        for bound in &mut signature.conformance_bounds {
            bound.binder = remapped(bound.binder, self.symbols);
            bound.trait_symbol = remapped(bound.trait_symbol, self.symbols);
            for argument in &bound.arguments {
                self.ty(*argument);
            }
        }
        self.parameters(signature.parameters);
        self.ty(signature.return_type);
        for invocation in self
            .target
            .signature_invokes
            .span_mut_or_empty(signature.invokes)
        {
            match &mut invocation.target {
                crate::signature::AuthoredInvocationTarget::Parameter { symbol, .. }
                | crate::signature::AuthoredInvocationTarget::Service(symbol) => {
                    *symbol = remapped(*symbol, self.symbols)
                }
                crate::signature::AuthoredInvocationTarget::Unresolved => {}
            }
        }
        let mut contracts = self
            .target
            .signature_contracts
            .span_or_empty(signature.contracts)
            .to_vec();
        for contract in &mut contracts {
            if let SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } = &mut contract.kind
            {
                *result_data = remapped(*result_data, self.symbols);
                *result_case = remapped(*result_case, self.symbols);
            }
            for fact in contract.facts.handles() {
                self.fact(fact);
            }
        }
        self.target
            .signature_contracts
            .span_mut_or_empty(signature.contracts)
            .clone_from_slice(&contracts);
    }

    fn parameters(&mut self, parameters: HandleSpan<crate::signature::StateParameter>) {
        let mut values = self
            .target
            .state_parameters
            .span_or_empty(parameters)
            .to_vec();
        for parameter in &mut values {
            parameter.symbol = remapped(parameter.symbol, self.symbols);
            self.ty(parameter.type_reference);
        }
        self.target
            .state_parameters
            .span_mut_or_empty(parameters)
            .clone_from_slice(&values);
    }

    fn fact(&mut self, handle: Handle<ProofFact>) {
        if self.facts.contains(&handle) {
            return;
        }
        self.facts.push(handle);
        let mut fact = self.target.proof_facts.get(handle).clone();
        match &mut fact {
            ProofFact::Expression(expression) => self.expression(*expression),
            ProofFact::Membership(membership) => {
                self.expression(membership.value);
                membership.domain_symbol = remapped(membership.domain_symbol, self.symbols);
                for argument in self
                    .target
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .to_vec()
                {
                    self.ty(argument);
                }
            }
            ProofFact::Proposition(application) => {
                application.proposition = remapped(application.proposition, self.symbols);
                for argument in &mut application.binder_arguments {
                    argument.symbol = remapped(argument.symbol, self.symbols.authored());
                }
                for argument in self
                    .target
                    .expression_table
                    .expression_handles(application.arguments)
                    .to_vec()
                {
                    self.expression(argument);
                }
            }
        }
        *self.target.proof_facts.get_mut(handle) = fact;
    }

    pub(crate) fn expression(&mut self, expression: ExpressionHandle) {
        let frontier = self.expressions.len();
        self.target.expression_table.remap_symbols_in_with_visited(
            expression,
            self.symbols,
            &mut self.expressions,
        );
        // Recursive payload remapping may append expressions. Keep the
        // just-visited range and its original order fixed for this pass.
        let end = self.expressions.len();
        for index in frontier..end {
            let expression = self.expressions[index];
            expression_type_payloads(self, expression);
        }
    }
}

trait TypePayloadTransform {
    fn tables(&mut self) -> &mut TypedTreeTables;
    fn reference(&mut self, reference: TypeReferenceHandle) -> TypeReferenceHandle;
    fn reference_span(
        &mut self,
        references: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle>;
    fn callable(
        &mut self,
        contract: RuntimeCallableContractHandle,
    ) -> RuntimeCallableContractHandle;
}

impl TypePayloadTransform for TypeCopy<'_, '_, '_> {
    fn tables(&mut self) -> &mut TypedTreeTables {
        self.target
    }
    fn reference(&mut self, reference: TypeReferenceHandle) -> TypeReferenceHandle {
        self.ty(reference)
    }
    fn reference_span(
        &mut self,
        references: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_span(references)
    }
    fn callable(
        &mut self,
        contract: RuntimeCallableContractHandle,
    ) -> RuntimeCallableContractHandle {
        self.contract(contract)
    }
}

impl TypePayloadTransform for TypeRemap<'_, '_> {
    fn tables(&mut self) -> &mut TypedTreeTables {
        self.target
    }
    fn reference(&mut self, reference: TypeReferenceHandle) -> TypeReferenceHandle {
        self.ty(reference);
        reference
    }
    fn reference_span(
        &mut self,
        references: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        for reference in self
            .target
            .type_reference_table
            .type_reference_handles(references)
            .to_vec()
        {
            self.ty(reference);
        }
        references
    }
    fn callable(
        &mut self,
        contract: RuntimeCallableContractHandle,
    ) -> RuntimeCallableContractHandle {
        self.contract(contract);
        contract
    }
}

fn static_argument_types(
    owner: &mut impl TypePayloadTransform,
    argument: &mut StaticMachineArgument,
) {
    argument.type_reference = owner.reference(argument.type_reference);
    if let Some(application) = &mut argument.application {
        for argument in &mut application.arguments {
            static_argument_types(owner, argument);
        }
        if application.member.is_valid() {
            let mut member = owner
                .tables()
                .expression_table
                .static_member(application.member)
                .expect("copied static application keeps its member")
                .clone();
            static_argument_types(owner, &mut member);
            *owner
                .tables()
                .expression_table
                .static_member_mut(application.member)
                .expect("copied static application keeps its member") = member;
        }
    }
}

fn application_types(
    owner: &mut impl TypePayloadTransform,
    application: crate::static_applications::StaticMachineApplicationHandle,
) {
    use crate::static_applications::StaticMachineBindingValue;
    if !application.is_valid() {
        return;
    }
    let mut arguments = owner
        .tables()
        .expression_table
        .static_machine_application_arguments(application)
        .to_vec();
    for argument in &mut arguments {
        static_argument_types(owner, argument);
    }
    owner
        .tables()
        .expression_table
        .static_machine_application_arguments_mut(application)
        .clone_from_slice(&arguments);
    let mut selected = owner
        .tables()
        .expression_table
        .static_machine_application_selected_machines(application)
        .to_vec();
    for argument in &mut selected {
        static_argument_types(owner, argument);
    }
    owner
        .tables()
        .expression_table
        .static_machine_application_selected_machines_mut(application)
        .clone_from_slice(&selected);
    let mut bindings = owner
        .tables()
        .expression_table
        .static_machine_application_bindings(application)
        .to_vec();
    for binding in &mut bindings {
        match &mut binding.value {
            StaticMachineBindingValue::Type(reference)
            | StaticMachineBindingValue::Const(reference)
            | StaticMachineBindingValue::RuntimeValue {
                type_reference: reference,
                ..
            } => *reference = owner.reference(*reference),
            StaticMachineBindingValue::Conformance(argument)
            | StaticMachineBindingValue::Machine(argument) => {
                static_argument_types(owner, argument)
            }
        }
    }
    owner
        .tables()
        .expression_table
        .static_machine_application_bindings_mut(application)
        .clone_from_slice(&bindings);
}

fn expression_type_payloads(owner: &mut impl TypePayloadTransform, expression: ExpressionHandle) {
    let mut node = owner
        .tables()
        .expression_table
        .expression(expression)
        .clone();
    match &mut node {
        ExpressionNode::MachineValue(value) => {
            value.type_reference = owner.reference(value.type_reference);
            static_argument_types(owner, &mut value.selected);
            application_types(owner, value.application);
        }
        ExpressionNode::Call(call) => {
            call.requirement_application = owner.reference(call.requirement_application);
            call.exact_coercion_target = owner.reference(call.exact_coercion_target);
            if let Some(binding) = &mut call.runtime_call {
                binding.contract = owner.callable(binding.contract);
            }
            call.visit_static_arguments_mut(|argument| static_argument_types(owner, argument));
            application_types(owner, call.authored_machine_application);
            if let Some(dispatch) = &call.static_requirement_dispatch
                && dispatch.selection_application != call.authored_machine_application
            {
                application_types(owner, dispatch.selection_application);
            }
        }
        ExpressionNode::Cast(cast) => {
            cast.target_type = owner.reference(cast.target_type);
            cast.result_type = owner.reference(cast.result_type);
            cast.semantic_domain_arguments = owner.reference_span(cast.semantic_domain_arguments);
        }
        ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
            *reference = owner.reference(*reference)
        }
        ExpressionNode::StructLiteral(literal) => {
            literal.application_type = owner.reference(literal.application_type)
        }
        ExpressionNode::Match(matched) => {
            let mut arms = owner
                .tables()
                .expression_table
                .match_arms(matched.arms)
                .to_vec();
            for arm in &mut arms {
                if let crate::expression::MatchPattern::Destructure(pattern) = &mut arm.pattern {
                    pattern.application_type = owner.reference(pattern.application_type);
                }
            }
            matched.arms = owner.tables().expression_table.insert_match_arms(arms);
        }
        ExpressionNode::SavedSubject(_)
        | ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_) => {}
    }
    *owner.tables().expression_table.expression_mut(expression) = node;
    let constructed = owner
        .tables()
        .expression_table
        .constructed_value_type_reference(expression);
    if constructed.is_valid() {
        let copied = owner.reference(constructed);
        owner
            .tables()
            .expression_table
            .set_constructed_value_type_reference(expression, copied);
    }
    let required = owner
        .tables()
        .expression_table
        .construction_required_type_reference(expression);
    if required.is_valid() {
        let copied = owner.reference(required);
        owner
            .tables()
            .expression_table
            .set_construction_required_type_reference(expression, copied);
    }
    if let Some(mut authority) = owner
        .tables()
        .expression_table
        .index_expression_authority(expression)
        .cloned()
    {
        if let Some(normalization) = &mut authority.normalization {
            normalization.index_type = owner.reference(normalization.index_type);
        }
        if let Some(operation) = &mut authority.operation {
            operation.operand_type = owner.reference(operation.operand_type);
            for evidence in &mut operation.law_evidence {
                for actual in &mut evidence.bound.arguments {
                    *actual = owner.reference(*actual);
                }
            }
        }
        if let Some(application) = authority
            .operation
            .as_mut()
            .and_then(|operation| operation.application.as_mut())
        {
            static_argument_types(owner, application);
        }
        *owner
            .tables()
            .expression_table
            .index_expression_authority_mut(expression) = authority;
    }
}
