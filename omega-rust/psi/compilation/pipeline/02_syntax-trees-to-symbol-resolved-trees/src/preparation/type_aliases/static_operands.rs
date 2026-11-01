//! Static alias selections enter the same type-use expansion as annotations.
//! Projection retains the authored tuple without evaluating values or selecting
//! code. The alias declaration and its receiving slots still own admission.

use super::{Expansion, Identifier, SyntaxTrees, TypeReferenceHandle, TypeReferenceNode};
use crate::preparation::generic_data::{
    collect_expression_handles, collect_statement_expression_handles, visit_expression_type_inputs,
};
use diagnostics::Diagnostic;
use std::collections::{HashMap, HashSet};
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, StaticMachineArgument,
};
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::statement::{StatementHandle, StatementNode};

impl Expansion<'_, '_> {
    pub(super) fn statement_static_operands(
        &mut self,
        syntax: &mut SyntaxTrees,
        statement: StatementHandle,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        if let StatementNode::Call(mut call) = syntax.statements.statement(statement).clone() {
            self.static_arguments(syntax, &mut call.machine_arguments, bound)?;
            syntax
                .statements
                .replace_statement(statement, StatementNode::Call(call));
        }
        let mut expressions = HashSet::new();
        collect_statement_expression_handles(syntax, statement, &mut expressions);
        if let StatementNode::Transition(transition) = syntax.statements.statement(statement) {
            let selectors = syntax
                .statements
                .outcome_proof_selectors(transition.proof_selectors);
            if !selectors.is_empty() {
                let mut target_expressions = HashSet::new();
                let mut target_bound = bound.to_vec();
                target_bound.extend(
                    selectors
                        .iter()
                        .map(|selector| selector.binding.as_str(syntax).to_owned()),
                );
                for target in transition.targets() {
                    match syntax.statements.transition_target(target) {
                        syntax_trees::statement::TransitionTargetNode::Value(value) => {
                            collect_expression_handles(syntax, *value, &mut target_expressions)
                        }
                        syntax_trees::statement::TransitionTargetNode::Named {
                            arguments, ..
                        } => {
                            for value in syntax.statements.expression_handles(*arguments) {
                                collect_expression_handles(syntax, *value, &mut target_expressions);
                            }
                        }
                        _ => {}
                    }
                }
                expressions.retain(|expression| !target_expressions.contains(expression));
                self.static_expression_inputs(syntax, target_expressions, &target_bound)?;
            }
        }
        self.static_expression_inputs(syntax, expressions, bound)
    }

    pub(super) fn expression_static_operands(
        &mut self,
        syntax: &mut SyntaxTrees,
        expression: ExpressionHandle,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        let mut expressions = HashSet::new();
        collect_expression_handles(syntax, expression, &mut expressions);
        self.static_expression_inputs(syntax, expressions, bound)
    }

    fn static_expression_inputs(
        &mut self,
        syntax: &mut SyntaxTrees,
        expressions: HashSet<ExpressionHandle>,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        // Runtime pattern aliases have already become saved-subject member
        // reads in the parser. Erased arm proofs remain lexical names: retain
        // their exact guard/body scope instead of selecting a global alias.
        let proof_scopes = proof_bindings(syntax, &expressions);
        let mut expressions = expressions.into_iter().collect::<Vec<_>>();
        expressions
            .sort_unstable_by_key(|expression| (expression.arena_index(), expression.generation()));
        for expression in expressions {
            let scoped_bound = proof_scopes.get(&expression).map(|proofs| {
                let mut scoped = bound.to_vec();
                scoped.extend(proofs.iter().cloned());
                scoped
            });
            let bound = scoped_bound.as_deref().unwrap_or(bound);
            if let ExpressionNode::Binary(binary) =
                syntax.expressions.expression(expression).clone()
                && matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                )
            {
                self.predicate_type_operand(syntax, binary.left, bound)?;
                self.predicate_type_operand(syntax, binary.right, bound)?;
            }
            if let ExpressionNode::Call(mut call) =
                syntax.expressions.expression(expression).clone()
            {
                self.static_arguments(syntax, &mut call.machine_arguments, bound)?;
                syntax
                    .expressions
                    .replace_expression(expression, ExpressionNode::Call(call));
            }
            let mut references = Vec::new();
            visit_expression_type_inputs(syntax, expression, &mut |reference| {
                references.push(reference)
            });
            for reference in references {
                self.reference(syntax, reference, bound)?;
            }
        }
        Ok(())
    }

    /// A selected alias in a type comparison takes the ordinary source-use
    /// expansion, retaining its original application and formation obligations.
    /// Lexical binders remain values/types in their own scope, never aliases
    /// selected merely because a declaration has the same spelling.
    fn predicate_type_operand(
        &mut self,
        syntax: &mut SyntaxTrees,
        expression: ExpressionHandle,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        match syntax.expressions.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let path = syntax.expressions.identifier_path_members(path).to_vec();
                let Some(first) = path.first() else {
                    return Ok(());
                };
                let argument = StaticMachineArgument {
                    path: path.clone().into_boxed_slice(),
                    ..Default::default()
                };
                let name = argument_name(syntax, &argument, first)?;
                if bound
                    .iter()
                    .any(|parameter| parameter == name.as_str(syntax))
                    || self.selection.type_alias_source(syntax, &name).is_none()
                {
                    return Ok(());
                }
                let reference = argument_reference(syntax, &argument)?;
                self.reference(syntax, reference, bound)?;
                syntax
                    .expressions
                    .replace_expression(expression, ExpressionNode::TypeExpression(reference));
            }
            ExpressionNode::Borrow(borrow) => {
                self.predicate_type_operand(syntax, borrow.target, bound)?
            }
            ExpressionNode::ArrayLiteral(elements) => {
                if let [element] = syntax.expressions.expression_handles(elements) {
                    let element = *element;
                    self.predicate_type_operand(syntax, element, bound)?;
                }
            }
            ExpressionNode::Indexed(indexed)
                if matches!(
                    syntax.expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) =>
            {
                self.predicate_type_operand(syntax, indexed.collection, bound)?
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn retain_contract_bindings(
        syntax: &SyntaxTrees,
        contracts: arena::HandleSpan<syntax_trees::item::CapabilityContract>,
        bound: &mut Vec<String>,
    ) {
        bound.extend(
            syntax
                .items
                .capability_contracts(contracts)
                .iter()
                .filter_map(|contract| contract.binding.as_ref())
                .map(|binding| binding.as_str(syntax).to_owned()),
        );
    }

    pub(super) fn retain_statement_bindings(
        syntax: &SyntaxTrees,
        statement: StatementHandle,
        bound: &mut Vec<String>,
    ) {
        match syntax.statements.statement(statement) {
            StatementNode::LocalData(local) => bound.push(local.name.as_str(syntax).to_owned()),
            StatementNode::ProofOutputBindingStatement(output) => bound.extend(
                output
                    .bindings
                    .iter()
                    .map(|selector| selector.binding.as_str(syntax).to_owned()),
            ),
            _ => {}
        }
    }

    pub(super) fn contract_static_operands(
        &mut self,
        syntax: &mut SyntaxTrees,
        contracts: arena::HandleSpan<syntax_trees::item::CapabilityContract>,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        let contracts = syntax.items.capability_contracts(contracts).to_vec();
        for contract in contracts {
            let facts = syntax.items.proof_facts(contract.facts).to_vec();
            for fact in facts {
                match fact {
                    syntax_trees::item::ProofFact::Expression(expression) => {
                        self.expression_static_operands(syntax, expression, bound)?
                    }
                    syntax_trees::item::ProofFact::Membership(membership) => {
                        self.expression_static_operands(syntax, membership.value, bound)?;
                        let arguments = syntax
                            .type_references
                            .type_reference_handles(membership.domain_arguments)
                            .to_vec();
                        for reference in arguments {
                            self.reference(syntax, reference, bound)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn static_arguments(
        &mut self,
        syntax: &mut SyntaxTrees,
        arguments: &mut [StaticMachineArgument],
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        for argument in arguments {
            if argument.type_reference.is_valid() {
                self.reference(syntax, argument.type_reference, bound)?;
                continue;
            }
            if let Some(application) = &mut argument.application {
                self.static_arguments(syntax, &mut application.arguments, bound)?;
                if application.member.is_valid() {
                    let mut member = syntax
                        .expressions
                        .static_member(application.member)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "static alias operand has no retained qualified member",
                            )
                        })?
                        .clone();
                    self.static_arguments(syntax, std::slice::from_mut(&mut member), bound)?;
                    let target = syntax
                        .expressions
                        .static_member_mut(application.member)
                        .ok_or_else(|| {
                            Diagnostic::error("static alias operand lost its qualified member")
                        })?;
                    *target = member;
                    continue;
                }
            }
            let Some(first) = argument.path.first() else {
                continue;
            };
            let name = argument_name(syntax, argument, first)?;
            if bound
                .iter()
                .any(|parameter| parameter == name.as_str(syntax))
                || self.selection.type_alias_source(syntax, &name).is_none()
            {
                continue;
            }
            let reference = argument_reference(syntax, argument)?;
            self.reference(syntax, reference, bound)?;
            // The ordinary AliasUse owns the original head and complete tuple;
            // do not leave a parallel unexpanded static-declaration selection.
            *argument = StaticMachineArgument {
                type_reference: reference,
                ..Default::default()
            };
        }
        Ok(())
    }
}

/// Transport a supplied operand into ordinary type-application syntax. The
/// receiving telescope decides Type/Const/Value/Machine/Implementation roles;
/// this projection neither normalizes its value nor expands a code selection.
fn argument_reference(
    syntax: &mut SyntaxTrees,
    argument: &StaticMachineArgument,
) -> Result<TypeReferenceHandle, Diagnostic> {
    if argument.type_reference.is_valid() {
        return Ok(argument.type_reference);
    }
    if let Some(literal) = &argument.const_literal {
        let expression = syntax
            .expressions
            .insert(ExpressionNode::Integer(literal.clone()));
        return Ok(syntax
            .type_references
            .insert(TypeReferenceNode::ConstExpression(expression)));
    }
    if let Some(projection) = &argument.evidence_projection {
        let term_span = projection.term.source_span();
        let member_span = projection.member.source_span();
        if term_span.source_id != member_span.source_id {
            return Err(
                Diagnostic::error("static alias projection mixes supplying sources")
                    .with_source_span(term_span),
            );
        }
        let head = syntax
            .expressions
            .append_identifier_path_member(projection.term.clone());
        let receiver = syntax
            .expressions
            .insert(ExpressionNode::Name(arena::HandleSpan::from_parts(head, 1)));
        syntax.expressions.set_source_span(receiver, term_span);
        let expression = syntax.expressions.insert(ExpressionNode::Member(
            syntax_trees::expression::TableMemberExpression {
                receiver,
                access_source_span: source::SourceSpan::new(
                    term_span.source_id,
                    source::Span::new(term_span.span.end, member_span.span.end),
                ),
                member: projection.member.clone(),
                case_variant: None,
            },
        ));
        syntax.expressions.set_source_span(
            expression,
            source::SourceSpan::new(
                term_span.source_id,
                source::Span::new(term_span.span.start, member_span.span.end),
            ),
        );
        return Ok(syntax
            .type_references
            .insert(TypeReferenceNode::ConstExpression(expression)));
    }
    let first = argument
        .path
        .first()
        .ok_or_else(|| Diagnostic::error("static alias argument has no retained operand"))?;
    let name = argument_name(syntax, argument, first)?;
    let node = if let Some(application) = &argument.application {
        if application.member.is_valid() {
            return Err(Diagnostic::error("qualified static member requires its exact applied owner before type materialization").with_source_span(first.source_span()));
        }
        let arguments = application
            .arguments
            .iter()
            .map(|argument| argument_reference(syntax, argument))
            .collect::<Result<Vec<_>, _>>()?;
        TypeReferenceNode::Generic {
            base_name: name,
            lifetime_arguments: application.lifetime_arguments.to_vec(),
            arguments: syntax
                .type_references
                .insert_type_reference_handles(arguments),
        }
    } else {
        TypeReferenceNode::Named(name)
    };
    Ok(syntax.type_references.insert(node))
}

fn argument_name(
    syntax: &SyntaxTrees,
    argument: &StaticMachineArgument,
    first: &Identifier,
) -> Result<Identifier, Diagnostic> {
    let mut span = first.source_span();
    for member in &argument.path {
        let member_span = member.source_span();
        if member_span.source_id != span.source_id {
            return Err(
                Diagnostic::error("static alias path mixes supplying sources")
                    .with_source_span(span),
            );
        }
        span.span.end = member_span.span.end;
    }
    Ok(Identifier::new(argument.path.joined_path_in(syntax), span))
}

/// Proof selectors own only their guard and result expression subtrees. Both
/// alias selection and alias-formal substitution must respect that lexical scope.
pub(super) fn proof_bindings(
    syntax: &SyntaxTrees,
    expressions: &HashSet<ExpressionHandle>,
) -> HashMap<ExpressionHandle, Vec<String>> {
    let mut proof_scopes: HashMap<ExpressionHandle, Vec<String>> = HashMap::new();
    for &expression in expressions {
        if let ExpressionNode::Match(dispatch) = syntax.expressions.expression(expression) {
            for arm in syntax.expressions.match_arms(dispatch.arms) {
                let syntax_trees::expression::MatchPattern::Destructure(pattern) = arm.pattern
                else {
                    continue;
                };
                let selectors = syntax
                    .statements
                    .outcome_proof_selectors(pattern.proof_selectors);
                if selectors.is_empty() {
                    continue;
                }
                let mut scoped = HashSet::new();
                collect_expression_handles(syntax, arm.value, &mut scoped);
                collect_expression_handles(syntax, pattern.guard, &mut scoped);
                for expression in scoped {
                    proof_scopes.entry(expression).or_default().extend(
                        selectors
                            .iter()
                            .map(|selector| selector.binding.as_str(syntax).to_owned()),
                    );
                }
            }
        }
    }
    proof_scopes
}
