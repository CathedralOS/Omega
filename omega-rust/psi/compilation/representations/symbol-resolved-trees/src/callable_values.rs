//! A callable type owns its argument namespace and the contract it publishes.
//!
//! The surrounding declaration supplies lexical binders; it is separate from
//! the callable's own formals and from provenance retained while expanding an
//! alias. A machine selection never changes this declared contract.

use crate::signature::StateSignature;
use arena::Handle;
use symbols::SymbolHandle;

pub type RuntimeCallableContractHandle = Handle<RuntimeCallableContract>;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct RuntimeCallableContract {
    pub signature: StateSignature,
    pub lexical_owner: SymbolHandle,
    pub definition_references: Vec<source::SourceSpan>,
}

/// A signature's dependencies retain the scope that binds each operand.
/// Consumers can inspect free inputs without mistaking the callable's own
/// arguments, nested binders or reserved result for outside dependencies.
#[derive(Default)]
pub struct ResolvedSignatureInputs<'program> {
    scopes: Vec<SignatureInputScope>,
    pub types: Vec<(usize, &'program crate::types::TypeReference)>,
    pub expressions: Vec<(usize, crate::expression::ExpressionHandle)>,
    pub static_arguments: Vec<(usize, &'program crate::expression::StaticMachineArgument)>,
    pub symbols: Vec<(usize, SymbolHandle)>,
    pub invocations: Vec<(usize, &'program crate::name::DiagnosticName)>,
    unresolved: bool,
}

#[derive(Default)]
struct SignatureInputScope {
    parent: Option<usize>,
    declaration: SymbolHandle,
    bound: Vec<SymbolHandle>,
}

impl<'program> ResolvedSignatureInputs<'program> {
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    pub fn of(
        program: &'program crate::SymbolResolvedTrees,
        signature: &'program StateSignature,
    ) -> Self {
        let mut inputs = Self::default();
        inputs.signature(program, signature, None);
        inputs
    }

    pub fn of_expression(
        program: &'program crate::SymbolResolvedTrees,
        expression: crate::expression::ExpressionHandle,
    ) -> Self {
        let mut inputs = Self::default();
        inputs.scopes.push(SignatureInputScope::default());
        inputs.expression(program, expression, 0);
        inputs
    }

    pub fn is_bound(&self, mut scope: usize, symbol: SymbolHandle) -> bool {
        if !symbol.is_valid() {
            return false;
        }
        loop {
            let current = &self.scopes[scope];
            if current.bound.contains(&symbol) {
                return true;
            }
            let Some(parent) = current.parent else {
                return false;
            };
            scope = parent;
        }
    }

    pub fn bound_symbols(&self, mut scope: usize) -> Vec<SymbolHandle> {
        let mut bound = Vec::new();
        loop {
            let current = &self.scopes[scope];
            bound.extend(current.bound.iter().copied());
            let Some(parent) = current.parent else {
                return bound;
            };
            scope = parent;
        }
    }

    pub fn owns_result(&self, mut scope: usize, owner: SymbolHandle) -> bool {
        if !owner.is_valid() {
            return false;
        }
        loop {
            let current = &self.scopes[scope];
            if current.declaration == owner {
                return true;
            }
            let Some(parent) = current.parent else {
                return false;
            };
            scope = parent;
        }
    }

    pub fn has_unresolved_inputs(&self) -> bool {
        self.unresolved
    }

    fn symbol(&mut self, scope: usize, symbol: SymbolHandle) {
        if symbol.is_valid() {
            self.symbols.push((scope, symbol));
        } else {
            self.unresolved = true;
        }
    }

    fn signature(
        &mut self,
        program: &'program crate::SymbolResolvedTrees,
        signature: &'program StateSignature,
        parent: Option<usize>,
    ) {
        use crate::data::{MachineParameterContract, TypeParameterKind};
        let parameters = program.data_type_parameters(signature.type_parameters);
        let arguments = program.state_parameters(signature.parameters);
        let scope = self.scopes.len();
        self.scopes.push(SignatureInputScope {
            parent,
            declaration: signature.symbol,
            bound: parameters
                .iter()
                .map(|parameter| parameter.symbol)
                .chain(arguments.iter().map(|argument| argument.symbol))
                .chain(
                    signature
                        .conformance_bounds
                        .iter()
                        .map(|bound| bound.binder),
                )
                .collect(),
        });
        if !signature.symbol.is_valid()
            || self.scopes[scope]
                .bound
                .iter()
                .any(|symbol| !symbol.is_valid())
        {
            self.unresolved = true;
        }
        // Invocation names do not yet carry selected targets in resolved trees.
        // Keep those inputs visible and defer closure instead of guessing a
        // declaration from its spelling.
        let invocations = program.signature_invokes(signature.invokes);
        self.invocations
            .extend(invocations.iter().map(|invocation| (scope, invocation)));
        self.unresolved |= !invocations.is_empty();
        self.types.extend(
            arguments
                .iter()
                .map(|argument| (scope, &argument.type_reference)),
        );
        self.types
            .extend(signature.return_type.iter().map(|result| (scope, result)));
        for bound in &signature.conformance_bounds {
            self.symbol(scope, bound.trait_symbol);
            self.types.extend(
                program
                    .child_type_references(bound.arguments)
                    .iter()
                    .map(|reference| (scope, reference)),
            );
        }
        for parameter in parameters {
            match &parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    self.types.push((scope, type_reference))
                }
                TypeParameterKind::Machine { contract } => match contract {
                    MachineParameterContract::Structural(required) => {
                        self.signature(program, required, Some(scope))
                    }
                    MachineParameterContract::Nominal {
                        trait_definition,
                        requirement,
                        trait_arguments,
                        ..
                    } => {
                        self.symbol(scope, *trait_definition);
                        self.symbol(scope, *requirement);
                        self.types.extend(
                            program
                                .child_type_references(*trait_arguments)
                                .iter()
                                .map(|reference| (scope, reference)),
                        );
                    }
                    MachineParameterContract::AuthoredNominal {
                        trait_arguments, ..
                    } => {
                        self.unresolved = true;
                        self.types.extend(
                            program
                                .child_type_references(*trait_arguments)
                                .iter()
                                .map(|reference| (scope, reference)),
                        );
                    }
                    MachineParameterContract::Inferred => self.unresolved = true,
                    MachineParameterContract::RequirementIdentity => {}
                },
                TypeParameterKind::Proposition { contract } => {
                    let arguments = program.state_parameters(contract.parameters);
                    let nested = self.scopes.len();
                    self.scopes.push(SignatureInputScope {
                        parent: Some(scope),
                        bound: arguments.iter().map(|argument| argument.symbol).collect(),
                        ..Default::default()
                    });
                    self.types.extend(
                        arguments
                            .iter()
                            .map(|argument| (nested, &argument.type_reference)),
                    );
                }
                TypeParameterKind::Type | TypeParameterKind::Implementation => {}
            }
        }
        for contract in program.signature_contracts(signature.contracts) {
            if let crate::signature::SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } = contract.kind
            {
                self.symbol(scope, result_data);
                self.symbol(scope, result_case);
            }
            for fact in program.proof_facts(contract.facts) {
                match fact {
                    crate::domain::ProofFact::Expression(expression) => {
                        self.expression(program, *expression, scope)
                    }
                    crate::domain::ProofFact::Membership(membership) => {
                        self.expression(program, membership.value, scope);
                        self.symbol(scope, membership.domain_symbol);
                        self.types.extend(
                            program
                                .child_type_references(membership.domain_arguments)
                                .iter()
                                .map(|reference| (scope, reference)),
                        );
                    }
                }
            }
        }
        for service in program
            .service_reach_rows
            .services(signature.service_reach_row)
        {
            match program.service_reaches.definition(*service) {
                Some(definition) => self.symbol(scope, definition.symbol),
                None => self.unresolved = true,
            }
        }
    }

    fn static_argument(
        &mut self,
        program: &'program crate::SymbolResolvedTrees,
        argument: &'program crate::expression::StaticMachineArgument,
        scope: usize,
    ) {
        self.static_arguments.push((scope, argument));
        if argument.type_reference.is_valid() {
            self.types
                .push((scope, program.child_type_reference(argument.type_reference)));
        }
        if argument.symbol.is_valid() {
            self.symbol(scope, argument.symbol);
        } else if !argument.type_reference.is_valid() && argument.const_literal.is_none() {
            self.unresolved = true;
        }
        if let Some(application) = &argument.application {
            for argument in &application.arguments {
                self.static_argument(program, argument, scope);
            }
            if application.member.is_valid() {
                match program
                    .tables
                    .bodies
                    .expressions
                    .static_member(application.member)
                {
                    Some(member) => self.static_argument(program, member, scope),
                    None => self.unresolved = true,
                }
            }
        }
    }

    fn expression(
        &mut self,
        program: &'program crate::SymbolResolvedTrees,
        expression: crate::expression::ExpressionHandle,
        scope: usize,
    ) {
        use crate::expression::{ExpressionNode, MatchFieldAction, MatchPattern};
        if !expression.is_valid() || self.expressions.contains(&(scope, expression)) {
            return;
        }
        self.expressions.push((scope, expression));
        let table = &program.tables.bodies.expressions;
        match table.expression(expression) {
            ExpressionNode::Name(path) => {
                if path.reserved_result_of.is_valid() {
                    if !self.owns_result(scope, path.reserved_result_of) {
                        self.symbol(scope, path.reserved_result_of);
                    }
                } else {
                    self.symbol(scope, path.symbol);
                }
                if path.head_symbol.is_valid() {
                    self.symbol(scope, path.head_symbol);
                }
                self.symbols.extend(
                    table
                        .name_path_member_symbols(path.member_symbols)
                        .iter()
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                        .map(|symbol| (scope, symbol)),
                );
            }
            ExpressionNode::Call(call) => {
                // Erased evidence operands have not acquired exact selections in
                // resolved trees. Their spelling cannot prove a closed input set.
                self.unresolved |= !call.evidence_arguments.is_empty();
                self.expression(program, call.callee, scope);
                self.expression(program, call.receiver, scope);
                if call.target_symbol.is_valid() {
                    self.symbol(scope, call.target_symbol);
                } else if !call.callee.is_valid() {
                    self.unresolved = true;
                }
                if call.requirement_application.is_valid() {
                    self.types.push((
                        scope,
                        program.child_type_reference(call.requirement_application),
                    ));
                }
                for argument in &call.machine_arguments {
                    self.static_argument(program, argument, scope);
                }
                for argument in table.expression_handles(call.arguments) {
                    self.expression(program, *argument, scope);
                }
            }
            ExpressionNode::Match(matched) => {
                self.expression(program, matched.subject, scope);
                for arm in table.match_arms(matched.arms) {
                    match arm.pattern {
                        MatchPattern::Value(value) => self.expression(program, value, scope),
                        MatchPattern::Destructure(pattern) => {
                            self.expression(program, pattern.head, scope);
                            self.expression(program, pattern.guard, scope);
                            self.expression(program, pattern.condition, scope);
                            if pattern.application_type.is_valid() {
                                self.types.push((
                                    scope,
                                    program.child_type_reference(pattern.application_type),
                                ));
                            }
                            for field in table.match_fields(pattern.fields) {
                                self.symbol(scope, field.field_symbol);
                                match &field.action {
                                    MatchFieldAction::Equal(value) => {
                                        self.expression(program, *value, scope)
                                    }
                                    MatchFieldAction::Bind(_) | MatchFieldAction::Waive => {}
                                }
                            }
                        }
                        MatchPattern::Wildcard => {}
                    }
                    self.expression(program, arm.value, scope);
                }
            }
            ExpressionNode::SavedSubject(saved) => {
                self.expression(program, saved.pattern_head, scope);
                if saved.capture.is_valid() {
                    self.expression(program, table.match_subject(saved.capture).subject, scope);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in table.expression_handles(*values) {
                    self.expression(program, *value, scope);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.expression(program, atomic.value, scope);
                self.expression(program, atomic.result, scope);
            }
            ExpressionNode::Binary(binary) => {
                self.expression(program, binary.left, scope);
                self.expression(program, binary.right, scope);
            }
            ExpressionNode::Unary(unary) => self.expression(program, unary.operand, scope),
            ExpressionNode::Borrow(borrow) => self.expression(program, borrow.target, scope),
            ExpressionNode::Cast(cast) => {
                self.expression(program, cast.value, scope);
                if cast.target_type.is_valid() {
                    self.types
                        .push((scope, program.child_type_reference(cast.target_type)));
                }
                if cast.semantic_domain_symbol.is_valid() {
                    self.symbol(scope, cast.semantic_domain_symbol);
                }
                self.types.extend(
                    program
                        .child_type_references(cast.semantic_domain_arguments)
                        .iter()
                        .map(|reference| (scope, reference)),
                );
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression(program, indexed.collection, scope);
                self.expression(program, indexed.index, scope);
            }
            ExpressionNode::Membership(membership) => {
                self.expression(program, membership.value, scope);
                for symbol in [
                    membership.domain_symbol,
                    membership.case_type_symbol,
                    membership.case_symbol,
                ] {
                    if symbol.is_valid() {
                        self.symbol(scope, symbol);
                    }
                }
                if !membership.domain_symbol.is_valid() && !membership.case_type_symbol.is_valid() {
                    self.unresolved = true;
                }
            }
            ExpressionNode::Member(member) => {
                self.expression(program, member.receiver, scope);
                self.symbol(scope, member.member_symbol);
            }
            ExpressionNode::Range(range) => {
                self.expression(program, range.start, scope);
                self.expression(program, range.end, scope);
            }
            ExpressionNode::StructLiteral(literal) => {
                self.symbol(scope, literal.type_symbol);
                if let Some(case) = literal.case_symbol {
                    self.symbol(scope, case);
                }
                if literal.application_type.is_valid() {
                    self.types.push((
                        scope,
                        program.child_type_reference(literal.application_type),
                    ));
                }
                for field in table.struct_fields(literal.fields) {
                    self.symbol(scope, field.field_symbol);
                    self.expression(program, field.value, scope);
                }
            }
            ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
                self.types
                    .push((scope, program.child_type_reference(*reference)));
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
        }
    }
}
