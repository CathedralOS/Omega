/*
    Declaration-owned type normalization before nominal synthesis.

    Alias expansion and const callable classification share the existing
    lexical header walk. Both need the selected namespace before constant
    discovery; a requirement path must not enter scalar const evaluation.

    Source application rows retain the alias telescope independently from the
    ordinary type replacing it. This stage forms no checked admission receipt.
*/

use super::callable_signatures::SignatureInputs;
use super::declaration_selection::{ConstantSelection, GenericParameterRole, RootHeaders};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use source_text::SourceSpan;
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, TypeAliasDefinition, TypeParameterKind};
use syntax_trees::types::{TypeAliasApplicationOrigin, TypeReferenceHandle, TypeReferenceNode};

mod expression_bindings {
    /*
        Alias-formal occurrences substitute supplying expressions before resolution.
        No operator runs here. The original alias application still owns argument
        carrier/formation admission, and ordinary checking admits the copied RHS.
    */

    use super::{
        Diagnostic, SyntaxTrees, TypeAliasDefinition, TypeParameterKind, TypeReferenceHandle,
    };
    use crate::resolution::declarations::declaration_selection::ConstantSelection;
    use crate::resolution::declarations::generic_data::{
        collect_expression_handles, visit_expression_type_inputs,
    };
    use arena::HandleSpan;
    use source_text::SourceSpan;
    use std::collections::{HashMap, HashSet};
    use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
    use syntax_trees::identifier::Identifier;
    use syntax_trees::types::TypeReferenceNode;

    #[derive(Default)]
    pub(super) struct ExpressionBindings {
        pub(super) uses: Vec<(ExpressionHandle, TypeReferenceHandle)>,
        pub(super) declaration_references: Vec<SourceSpan>,
        pub(super) unresolved: Vec<ExpressionHandle>,
    }

    pub(super) fn bind_const_expression(
        templates: &SyntaxTrees,
        selection: &ConstantSelection<'_>,
        root: ExpressionHandle,
        alias: &TypeAliasDefinition,
        arguments: &[TypeReferenceHandle],
        bindings: &mut ExpressionBindings,
        callable_scope: Option<(&super::SignatureInputs, usize)>,
    ) -> Vec<TypeReferenceHandle> {
        let mut expressions = HashSet::new();
        collect_expression_handles(templates, root, &mut expressions);
        let proofs = super::static_operands::proof_bindings(templates, &expressions);
        let mut expressions = expressions.into_iter().collect::<Vec<_>>();
        expressions.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
        let parameters = templates.items.type_parameters(alias.type_parameters);
        let mut children = Vec::new();
        for expression in expressions {
            visit_expression_type_inputs(templates, expression, &mut |reference| {
                children.push(reference)
            });
            match templates.expressions.expression(expression) {
                ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } if path_application.is_none() => {
                    let [name] = templates
                        .expressions
                        .identifier_path_members(path.to_owned())
                    else {
                        bindings.unresolved.push(expression);
                        continue;
                    };
                    let shadowed = proofs.get(&expression).is_some_and(|bindings| {
                        bindings
                            .iter()
                            .any(|binding| binding.as_str() == name.as_str(templates))
                    });
                    if callable_scope.is_some_and(|(inputs, scope)| {
                        shadowed
                            || name.as_str(templates) == "result"
                            || inputs.hides_value(scope, name.as_str(templates))
                    }) {
                        continue;
                    }
                    let bound_parameter = parameters.iter().any(|parameter| {
                        parameter.name.as_str(templates) == name.as_str(templates)
                    });
                    let parameter = (!shadowed)
                        .then(|| {
                            parameters.iter().position(|parameter| {
                                matches!(&parameter.kind, TypeParameterKind::Const { .. })
                                    && parameter.name.as_str(templates) == name.as_str(templates)
                            })
                        })
                        .flatten();
                    if let Some(position) = parameter {
                        bindings.uses.push((expression, arguments[position]));
                    } else if !shadowed
                        && !bound_parameter
                        && declaration_constant(templates, selection, expression)
                    {
                        bindings
                            .declaration_references
                            .push(templates.expressions.source_span(expression));
                    } else {
                        bindings.unresolved.push(expression);
                    }
                }
                ExpressionNode::Name { .. } | ExpressionNode::Call(_) => {
                    bindings.unresolved.push(expression)
                }
                _ => {}
            }
        }
        children
    }

    fn declaration_constant(
        syntax: &SyntaxTrees,
        selection: &ConstantSelection<'_>,
        expression: ExpressionHandle,
    ) -> bool {
        let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = syntax.expressions.expression(expression)
        else {
            return false;
        };
        if path_application.is_some() {
            return false;
        }
        let name = syntax
            .expressions
            .identifier_path_members(path.to_owned())
            .iter()
            .map(|member| member.as_str(syntax))
            .collect::<Vec<_>>()
            .join("::");
        selection.selects_const(
            syntax,
            &Identifier::new(name, syntax.expressions.source_span(expression)),
        )
    }

    pub(super) fn supplied_expressions(
        syntax: &mut SyntaxTrees,
        uses: &[(ExpressionHandle, TypeReferenceHandle)],
    ) -> Result<Vec<(ExpressionHandle, ExpressionHandle)>, Diagnostic> {
        let mut actuals: HashMap<TypeReferenceHandle, ExpressionHandle> = HashMap::new();
        let mut substitutions = Vec::with_capacity(uses.len());
        for &(source, argument) in uses {
            let actual = if let Some(actual) = actuals.get(&argument) {
                *actual
            } else {
                let owner = syntax
                    .type_references
                    .supplied_operand_owner(argument)
                    .map_err(Diagnostic::error)?;
                if owner.is_valid() {
                    return Err(Diagnostic::error(
                        "transparent alias RHS expression cannot yet preserve explicitly supplied declaration custody",
                    ));
                }
                let expression = match syntax.type_references.type_reference(argument).clone() {
                    TypeReferenceNode::ConstExpression(expression) => expression,
                    TypeReferenceNode::Named(name) => {
                        let span = name.source_span();
                        let members = name
                            .as_str(syntax)
                            .split("::")
                            .map(|member| Identifier::new(member, span))
                            .collect::<Vec<_>>();
                        let mut path = HandleSpan::empty();
                        for member in members {
                            syntax
                                .expressions
                                .append_identifier_path_member_to_span(&mut path, member);
                        }
                        let expression = syntax.expressions.insert(ExpressionNode::Name {
                            members: path,
                            application: None,
                        });
                        syntax.expressions.set_source_span(expression, span);
                        expression
                    }
                    _ => {
                        return Err(Diagnostic::error(
                            "transparent alias Const expression requires a value argument",
                        ));
                    }
                };
                actuals.insert(argument, expression);
                expression
            };
            substitutions.push((source, actual));
        }
        Ok(substitutions)
    }
}
mod static_operands {
    /*
        Static alias selections enter the same type-use expansion as annotations.
        Projection retains the authored tuple without evaluating values or selecting
        code. The alias declaration and its receiving slots still own admission.
    */

    use super::{Expansion, Identifier, SyntaxTrees, TypeReferenceHandle, TypeReferenceNode};
    use crate::resolution::declarations::generic_data::{
        collect_expression_handles, collect_statement_expression_handles,
        visit_expression_type_inputs,
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
                        if let Some(value) =
                            syntax.statements.transition_target(target).expression()
                        {
                            collect_expression_handles(syntax, value, &mut target_expressions);
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
            let proof_scopes = proof_bindings(syntax, &expressions);
            let mut expressions = expressions.into_iter().collect::<Vec<_>>();
            expressions.sort_unstable_by_key(|expression| {
                (expression.arena_index(), expression.generation())
            });
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

        /*
            A selected alias in a type comparison takes the ordinary source-use
            expansion, retaining its original application and formation obligations.
            Lexical binders remain values/types in their own scope, never aliases
            selected merely because a declaration has the same spelling.
        */
        fn predicate_type_operand(
            &mut self,
            syntax: &mut SyntaxTrees,
            expression: ExpressionHandle,
            bound: &[String],
        ) -> Result<(), Diagnostic> {
            match syntax.expressions.expression(expression).clone() {
                ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } => {
                    let application = path_application;
                    let path = syntax
                        .expressions
                        .identifier_path_members(path.to_owned())
                        .to_vec();
                    let Some(first) = path.first() else {
                        return Ok(());
                    };
                    let argument = StaticMachineArgument {
                        path: path.clone().into_boxed_slice(),
                        application,
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

        pub(super) fn static_arguments(
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
                *argument = StaticMachineArgument {
                    type_reference: reference,
                    ..Default::default()
                };
            }
            Ok(())
        }
    }

    /*
        Transport a supplied operand into ordinary type-application syntax. The
        receiving telescope decides Type/Const/Value/Machine/Implementation roles;
        this projection neither normalizes its value nor expands a code selection.
    */
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
            let receiver = syntax.expressions.insert(ExpressionNode::Name {
                members: arena::HandleSpan::from_parts(head, 1),
                application: None,
            });
            syntax.expressions.set_source_span(receiver, term_span);
            let expression = syntax.expressions.insert(ExpressionNode::Member(
                syntax_trees::expression::TableMemberExpression {
                    receiver,
                    access_source_span: source_text::SourceSpan::new(
                        term_span.source_id,
                        source_text::Span::new(term_span.span.end, member_span.span.end),
                    ),
                    member: projection.member.clone(),
                    position: None,
                    case_variant: None,
                },
            ));
            syntax.expressions.set_source_span(
                expression,
                source_text::SourceSpan::new(
                    term_span.source_id,
                    source_text::Span::new(term_span.span.start, member_span.span.end),
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

    /*
        Proof selectors own only their guard and result expression subtrees. Both
        alias selection and alias-formal substitution must respect that lexical scope.
    */
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
}

struct Expansion<'a, 'base> {
    selection: &'a ConstantSelection<'base>,
    /* The forest's root headers, indexed once for every name this pass reads. */
    headers: RootHeaders,
    active: Vec<SourceSpan>,
    supplied: Vec<TypeReferenceHandle>,
}

impl Expansion<'_, '_> {
    fn reference(
        &mut self,
        syntax: &mut SyntaxTrees,
        reference: TypeReferenceHandle,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        if !reference.is_valid() || self.supplied.contains(&reference) {
            return Ok(());
        }
        let node = syntax.type_references.type_reference(reference).clone();
        let (head, lifetimes, arguments) = match &node {
            TypeReferenceNode::Named(name) => (Some(name.clone()), Vec::new(), Vec::new()),
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => (
                Some(base_name.clone()),
                lifetime_arguments.clone(),
                syntax
                    .type_references
                    .type_reference_handles(*arguments)
                    .to_vec(),
            ),
            _ => (None, Vec::new(), Vec::new()),
        };
        if let Some(head) =
            head.filter(|head| !bound.iter().any(|name| name == head.as_str(syntax)))
        {
            let declaration = self.selection.type_alias_source(syntax, &head);
            let parameters = self.selection.generic_parameter_roles_in(
                syntax,
                &self.headers,
                &head,
                &[
                    SymbolKind::BuiltinType,
                    SymbolKind::Data,
                    SymbolKind::TypeAlias,
                    SymbolKind::Trait,
                    SymbolKind::Machine,
                ],
            );
            for (position, argument) in arguments.iter().enumerate() {
                if parameters
                    .as_ref()
                    .and_then(|parameters| parameters.get(position))
                    .is_some_and(|parameter| *parameter == GenericParameterRole::Type)
                {
                    self.reference(syntax, *argument, bound)?;
                }
            }
            if let Some(declaration) = declaration {
                return self.application(
                    syntax,
                    reference,
                    &head,
                    declaration,
                    &lifetimes,
                    &arguments,
                    bound,
                );
            }
        }
        match node {
            TypeReferenceNode::StaticSelection(mut selection) => {
                self.static_arguments(syntax, std::slice::from_mut(&mut selection), bound)?;
                syntax.type_references.replace_type_reference(
                    reference,
                    TypeReferenceNode::StaticSelection(selection),
                );
            }
            TypeReferenceNode::Reference { referee, .. } => {
                self.reference(syntax, referee, bound)?
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.reference(syntax, base_type, bound)?;
                let constraints = syntax.type_references.constraints(constraints).to_vec();
                for constraint in constraints {
                    if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                        let arguments = syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                            .to_vec();
                        self.domain_arguments(syntax, base_type, &domain.name, &arguments, bound)?;
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                self.reference(syntax, element_type, bound)?
            }
            TypeReferenceNode::Callable { signature } => {
                let signature = syntax.items.state_signature(signature).clone();
                self.signature(syntax, &signature, bound)?;
                only_proof_binders(syntax, signature.type_parameters)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn domain_arguments(
        &mut self,
        syntax: &mut SyntaxTrees,
        carrier: TypeReferenceHandle,
        name: &Identifier,
        arguments: &[TypeReferenceHandle],
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        let type_indices =
            if let Some(family) = self.selection.domain_family_on_type(syntax, name, carrier) {
                super::generic_data::domain_index_telescope(syntax, family)
                    .0
                    .into_iter()
                    .map(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                    .collect::<Vec<_>>()
            } else {
                self.selection
                    .generic_parameter_roles_in(syntax, &self.headers, name, &[SymbolKind::Domain])
                    .unwrap_or_default()
                    .into_iter()
                    .map(|role| role == GenericParameterRole::Type)
                    .collect()
            };
        for (argument, is_type) in arguments.iter().zip(type_indices) {
            if is_type {
                self.reference(syntax, *argument, bound)?;
            }
        }
        Ok(())
    }

    fn application(
        &mut self,
        syntax: &mut SyntaxTrees,
        reference: TypeReferenceHandle,
        name: &Identifier,
        declaration: SourceSpan,
        lifetimes: &[Identifier],
        arguments: &[TypeReferenceHandle],
        _bound: &[String],
    ) -> Result<(), Diagnostic> {
        let (forest, alias) = self
            .selection
            .alias_body(declaration)
            .and_then(|body| body.declaration(declaration))
            .ok_or_else(|| {
                Diagnostic::error("selected type alias has no authored body")
                    .with_source_span(name.source_span())
            })?;
        if alias.lifetime_parameters.len() != lifetimes.len()
            || alias.type_parameters.count() as usize != arguments.len()
        {
            return Err(Diagnostic::error(
                "type alias application does not supply its complete declared telescope",
            )
            .with_source_span(name.source_span()));
        }
        if self.active.contains(&declaration) {
            return Err(Diagnostic::error(
                "transparent type alias expansion contains a recursive declaration cycle",
            )
            .with_source_span(name.source_span()));
        }
        let mut types = Vec::new();
        let mut lifetime_uses = Vec::new();
        let mut undeclared_lifetimes = Vec::new();
        let mut array_length_uses = Vec::new();
        let mut expressions = expression_bindings::ExpressionBindings::default();
        bind_type_occurrences(
            forest,
            self.selection,
            alias.authored_expansion,
            alias,
            arguments,
            lifetimes,
            &mut types,
            &mut lifetime_uses,
            &mut undeclared_lifetimes,
            &mut array_length_uses,
            &mut expressions,
            None,
        );
        // lifetimes.md: named applications "use binders declared in the
        // lexical owner. Duplicate binders and undeclared tags reject."
        // The alias is the lexical owner of every lifetime its expansion
        // writes. A tag it does not declare is rejected here, before
        // substitution copies the spelling into the use site, where the
        // receiving declaration's own binders would otherwise answer it.
        if let Some(tag) = undeclared_lifetimes.first() {
            return Err(Diagnostic::error(format!(
                "type alias `{}` names the lifetime `'{}`, which it does not declare: \
                     undeclared lifetime tags reject",
                alias.name.as_str(forest),
                tag.as_str(forest).trim_start_matches('\''),
            ))
            .with_source_span(tag.source_span()));
        }
        if let Some(operand) = expressions.unresolved.first() {
            return Err(Diagnostic::error("transparent alias expansion cannot yet retain the declaration and supplying scopes of this expression operand").with_source_span(forest.expressions.source_span(*operand)));
        }
        let supplied_expressions =
            expression_bindings::supplied_expressions(syntax, &expressions.uses)
                .map_err(|diagnostic| diagnostic.with_source_span(name.source_span()))?;
        let authored = syntax.type_references.type_reference(reference).clone();
        let raw = syntax.type_references.insert(authored);
        syntax
            .type_references
            .retain_copied_operand_owner(reference, raw);
        let (expanded, supplied_copies) = syntax.instantiate_type_reference_from(
            forest,
            alias.authored_expansion,
            &types,
            &supplied_expressions,
            &lifetime_uses,
            &array_length_uses,
        );
        self.active.push(declaration);
        let prior_supplied = self.supplied.len();
        self.supplied.extend_from_slice(arguments);
        self.supplied.extend(supplied_copies);
        let bound = forest
            .items
            .type_parameters(alias.type_parameters)
            .iter()
            .map(|parameter| parameter.name.as_str(forest).to_owned())
            .collect::<Vec<_>>();
        let result = self.reference(syntax, expanded, &bound);
        self.supplied.truncate(prior_supplied);
        self.active.pop();
        result?;
        syntax
            .type_references
            .replace_type_occurrence(reference, expanded);
        let mut definition_references = definition_reference_spans(forest, alias);
        definition_references.extend(expressions.declaration_references);
        syntax
            .type_references
            .retain_type_alias_application(TypeAliasApplicationOrigin {
                instance: reference,
                application: raw,
                declaration,
                definition_references,
            });
        Ok(())
    }
}

/*
    These are lexical references in the original declaration, not rendered
    identities. Supplied actuals carry their own distinct source occurrences.
*/
fn definition_reference_spans(
    forest: &SyntaxTrees,
    alias: &TypeAliasDefinition,
) -> Vec<SourceSpan> {
    let parameters = forest.items.type_parameters(alias.type_parameters);
    let mut positions = Vec::new();
    super::generic_data::collect_type_positions(
        forest,
        alias.authored_expansion,
        &mut positions,
        true,
    );
    let mut references = Vec::new();
    for position in positions {
        let name = match forest.type_references.type_reference(position) {
            TypeReferenceNode::Named(name) => Some(name),
            TypeReferenceNode::Generic { base_name, .. } => Some(base_name),
            TypeReferenceNode::FixedArray {
                length: syntax_trees::types::FixedArrayLength::ConstParameter(name),
                ..
            } => Some(name),
            _ => None,
        };
        if let Some(name) = name
            && !parameters
                .iter()
                .any(|parameter| parameter.name.as_str(forest) == name.as_str(forest))
            && !references.contains(&name.source_span())
        {
            references.push(name.source_span());
        }
    }
    references
}

fn bind_type_occurrences(
    templates: &SyntaxTrees,
    selection: &ConstantSelection<'_>,
    reference: TypeReferenceHandle,
    alias: &TypeAliasDefinition,
    arguments: &[TypeReferenceHandle],
    lifetimes: &[Identifier],
    types: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    lifetime_uses: &mut Vec<(SourceSpan, String, Identifier)>,
    undeclared_lifetimes: &mut Vec<Identifier>,
    array_length_uses: &mut Vec<(SourceSpan, String, TypeReferenceHandle)>,
    expressions: &mut expression_bindings::ExpressionBindings,
    callable_scope: Option<(&SignatureInputs, usize)>,
) {
    for application in templates.type_references.type_alias_applications(reference) {
        bind_type_occurrences(
            templates,
            selection,
            application.application,
            alias,
            arguments,
            lifetimes,
            types,
            lifetime_uses,
            undeclared_lifetimes,
            array_length_uses,
            expressions,
            callable_scope,
        );
    }
    let mut lifetime = |name: &Identifier| {
        if callable_scope
            .is_some_and(|(inputs, scope)| inputs.hides_lifetime(scope, name.as_str(templates)))
        {
            return;
        }
        if let Some(position) = alias
            .lifetime_parameters
            .iter()
            .position(|parameter| parameter.as_str(templates) == name.as_str(templates))
        {
            lifetime_uses.push((
                name.source_span(),
                name.as_str(templates).to_owned(),
                lifetimes[position].clone(),
            ));
        } else {
            undeclared_lifetimes.push(name.clone());
        }
    };
    let children = match templates.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) => {
            if !callable_scope
                .is_some_and(|(inputs, scope)| inputs.hides_type(scope, name.as_str(templates)))
                && let Some(position) = templates
                    .items
                    .type_parameters(alias.type_parameters)
                    .iter()
                    .position(|parameter| {
                        parameter.name.as_str(templates) == name.as_str(templates)
                    })
            {
                types.push((reference, arguments[position]));
            }
            Vec::new()
        }
        TypeReferenceNode::Reference {
            referee,
            lifetime: region,
            ..
        } => {
            if let Some(region) = region {
                lifetime(region);
            }
            vec![*referee]
        }
        TypeReferenceNode::Generic {
            lifetime_arguments,
            arguments,
            ..
        } => {
            for region in lifetime_arguments {
                lifetime(region);
            }
            templates
                .type_references
                .type_reference_handles(*arguments)
                .to_vec()
        }
        TypeReferenceNode::ConstExpression(expression) => {
            expression_bindings::bind_const_expression(
                templates,
                selection,
                *expression,
                alias,
                arguments,
                expressions,
                callable_scope,
            )
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut children = vec![*base_type];
            for constraint in templates.type_references.constraints(*constraints) {
                if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint {
                    children.extend_from_slice(
                        templates
                            .type_references
                            .type_reference_handles(domain.arguments),
                    );
                }
            }
            children
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let mut children = vec![*element_type];
            match length {
                syntax_trees::types::FixedArrayLength::Expression(reference) => {
                    children.push(*reference)
                }
                syntax_trees::types::FixedArrayLength::ConstParameter(name) => {
                    if !callable_scope.is_some_and(|(inputs, scope)| {
                        inputs.hides_value(scope, name.as_str(templates))
                    }) && let Some(position) = templates
                        .items
                        .type_parameters(alias.type_parameters)
                        .iter()
                        .position(|parameter| {
                            parameter.name.as_str(templates) == name.as_str(templates)
                        })
                    {
                        array_length_uses.push((
                            name.source_span(),
                            name.as_str(templates).to_owned(),
                            arguments[position],
                        ));
                    }
                }
                _ => {}
            }
            children
        }
        TypeReferenceNode::Callable { signature } => {
            let inputs =
                SignatureInputs::of(templates, templates.items.state_signature(*signature));
            let mut children = inputs.types.clone();
            for &(scope, expression) in &inputs.expressions {
                let prior_unresolved = expressions.unresolved.len();
                let references = expression_bindings::bind_const_expression(
                    templates,
                    selection,
                    expression,
                    alias,
                    arguments,
                    expressions,
                    Some((&inputs, scope)),
                );
                // A fact may call a predicate or select a qualified declaration.
                // Retain its authored scope for ordinary signature resolution.
                for reference in expressions.unresolved.drain(prior_unresolved..) {
                    expressions
                        .declaration_references
                        .push(templates.expressions.source_span(reference));
                }
                children.extend(references.into_iter().map(|reference| (scope, reference)));
            }
            for (scope, child) in children {
                bind_type_occurrences(
                    templates,
                    selection,
                    child,
                    alias,
                    arguments,
                    lifetimes,
                    types,
                    lifetime_uses,
                    undeclared_lifetimes,
                    array_length_uses,
                    expressions,
                    Some((&inputs, scope)),
                );
            }
            Vec::new()
        }
        TypeReferenceNode::Slice { element_type } => vec![*element_type],
        _ => Vec::new(),
    };
    for child in children {
        bind_type_occurrences(
            templates,
            selection,
            child,
            alias,
            arguments,
            lifetimes,
            types,
            lifetime_uses,
            undeclared_lifetimes,
            array_length_uses,
            expressions,
            callable_scope,
        );
    }
}

/*
    Run before generic-data applications are demanded. Re-entry is idempotent:
    only authored alias heads expand, while retained application rows are not
    themselves traversal roots.

*/
pub(super) fn normalize(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    super::type_equations::normalize_applied_operands(syntax).map_err(|error| vec![error])?;
    if !needs_normalization(syntax, selection) {
        return Ok(());
    }
    let templates = syntax.clone();
    let mut expansion = Expansion {
        selection,
        headers: RootHeaders::of(syntax),
        active: Vec::new(),
        supplied: Vec::new(),
    };
    for handle in templates.root_item_handles() {
        let mut item = templates.root_item(*handle).clone();
        if let Item::TypeAlias(alias) = &mut item {
            if alias.expansion == alias.authored_expansion {
                let (working, _) = syntax.instantiate_type_reference_from(
                    &templates,
                    alias.authored_expansion,
                    &[],
                    &[],
                    &[],
                    &[],
                );
                alias.expansion = working;
                syntax.items.replace_item(*handle, item.clone());
            }
            if let Item::TypeAlias(alias) = &item {
                mark_header_references(syntax, alias.authored_expansion);
            }
        }
        if let Err(error) = expansion.item(syntax, &item, &[]) {
            return Err(vec![error]);
        }
        if let Item::TypeAlias(alias) = item {
            mark_header_references(syntax, alias.expansion);
        }
    }
    Ok(())
}

fn needs_normalization(syntax: &SyntaxTrees, selection: &ConstantSelection<'_>) -> bool {
    selection.has_aliases()
        || syntax.items.type_parameter_entries().any(|parameter| {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                return false;
            };
            match syntax.type_references.type_reference(type_reference) {
                TypeReferenceNode::StaticSelection(_) => true,
                TypeReferenceNode::Named(name)
                | TypeReferenceNode::Generic {
                    base_name: name, ..
                } => name.as_str(syntax).contains("::"),
                _ => false,
            }
        })
}

fn mark_header_references(syntax: &mut SyntaxTrees, root: TypeReferenceHandle) {
    let mut pending = vec![root];
    let mut seen = Vec::new();
    while let Some(root) = pending.pop() {
        if seen.contains(&root) {
            continue;
        }
        seen.push(root);
        let mut references = Vec::new();
        super::generic_data::collect_type_positions(syntax, root, &mut references, true);
        for reference in references {
            pending.extend(
                syntax
                    .type_references
                    .type_alias_applications(reference)
                    .map(|origin| origin.application),
            );
            syntax
                .type_references
                .retain_alias_header_reference(reference);
        }
    }
}

/// A structural callable signature's own `Name: Type` binder is a proof of a
/// logical contract alias, as in `machine<Inner: NonzeroIsPositive>(n: u8)`.
/// Any other type leaves a binder with no meaning there.
fn only_proof_binders(
    syntax: &SyntaxTrees,
    parameters: HandleSpan<syntax_trees::item::TypeParameter>,
) -> Result<(), Diagnostic> {
    for parameter in syntax.items.type_parameters(parameters) {
        if matches!(parameter.kind, TypeParameterKind::Value { .. }) {
            return Err(Diagnostic::error(format!(
                "a callable signature's own binder `{}` must name a logical contract alias; write `const {}: Type` for a static value",
                parameter.name.as_str(syntax),
                parameter.name.as_str(syntax),
            )));
        }
    }
    Ok(())
}

impl Expansion<'_, '_> {
    fn parameters(
        &mut self,
        syntax: &mut SyntaxTrees,
        parameters: HandleSpan<syntax_trees::item::TypeParameter>,
        inherited: &[String],
        callable_header: bool,
    ) -> Result<Vec<String>, Diagnostic> {
        let parameter_span = parameters;
        let parameters = syntax.items.type_parameters(parameters).to_vec();
        let mut bound = inherited.to_vec();
        bound.extend(
            parameters
                .iter()
                .map(|parameter| parameter.name.as_str(syntax).to_owned()),
        );
        for (index, parameter) in parameters.into_iter().enumerate() {
            match parameter.kind {
                TypeParameterKind::Const { type_reference } => {
                    self.reference(syntax, type_reference, &bound)?;
                    let contract = match syntax.type_references.type_reference(type_reference) {
                        TypeReferenceNode::Callable { signature } => {
                            let mut signature = syntax.items.state_signature(*signature).clone();
                            signature.name = parameter.name;
                            Some(syntax_trees::item::MachineParameterContract::Structural(
                                signature,
                            ))
                        }
                        _ => super::callable_types::nominal_contract(
                            syntax,
                            self.selection,
                            type_reference,
                            &bound,
                        )?,
                    };
                    if let Some(contract) = contract {
                        syntax.items.type_parameters_mut(parameter_span)[index].kind =
                            TypeParameterKind::Machine {
                                contract: Some(contract),
                            };
                    }
                }
                TypeParameterKind::Value { type_reference } => {
                    self.reference(syntax, type_reference, &bound)?;
                    if callable_header
                        && let TypeReferenceNode::Callable { signature } =
                            syntax.type_references.type_reference(type_reference)
                    {
                        // A theorem asks for the alias's evidence, while `const`
                        // above asks for an executable selection. Retain the
                        // whole contract; its logical interpretation still owes
                        // checking and grants no operational authority.
                        let mut contract = syntax.items.state_signature(*signature).clone();
                        contract.name = parameter.name;
                        syntax.items.type_parameters_mut(parameter_span)[index].kind =
                            TypeParameterKind::Proof { contract };
                    }
                }
                TypeParameterKind::Machine {
                    contract:
                        Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
                }
                | TypeParameterKind::Proof {
                    contract: signature,
                } => {
                    self.signature(syntax, &signature, &bound)?;
                    only_proof_binders(syntax, signature.type_parameters)?;
                }
                _ => {}
            }
        }
        Ok(bound)
    }

    fn signature(
        &mut self,
        syntax: &mut SyntaxTrees,
        signature: &syntax_trees::item::StateSignature,
        inherited: &[String],
    ) -> Result<(), Diagnostic> {
        let bound = self.parameters(syntax, signature.type_parameters, inherited, true)?;
        self.reference(syntax, signature.return_type, &bound)?;
        let parameters = syntax.items.state_parameters(signature.parameters).to_vec();
        for parameter in parameters {
            self.reference(
                syntax,
                syntax.items.state_parameter(parameter).type_reference,
                &bound,
            )?;
        }
        let mut expression_bound = bound.clone();
        expression_bound.extend(
            syntax
                .items
                .state_parameters(signature.parameters)
                .iter()
                .map(|handle| {
                    syntax
                        .items
                        .state_parameter(*handle)
                        .name
                        .as_str(syntax)
                        .to_owned()
                }),
        );
        Self::retain_contract_bindings(syntax, signature.contracts, &mut expression_bound);
        self.contract_static_operands(syntax, signature.contracts, &expression_bound)?;
        if let Some(body) = signature.default_body {
            let statements = syntax.items.statements(body.statements).to_vec();
            self.statements(syntax, &statements, &expression_bound)?;
            self.nested_machines(syntax, body.nested_machines, &bound)?;
        }
        Ok(())
    }

    fn statements(
        &mut self,
        syntax: &mut SyntaxTrees,
        statements: &[syntax_trees::statement::StatementHandle],
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        let mut bound = bound.to_vec();
        for statement in statements {
            if let syntax_trees::statement::StatementNode::Assembly(handle) =
                syntax.statements.statement(*statement)
            {
                let blocks = syntax
                    .statements
                    .assembly_blocks(syntax.statements.assembly(*handle).blocks)
                    .to_vec();
                for block in blocks {
                    let requires = syntax
                        .statements
                        .assembly_expressions(block.requires)
                        .to_vec();
                    for expression in requires {
                        self.expression_static_operands(syntax, expression, &bound)?;
                    }
                    let instructions = syntax
                        .statements
                        .assembly_instructions(block.instructions)
                        .to_vec();
                    self.statements(syntax, &instructions, &bound)?;
                    let mut completed_bound = bound.clone();
                    for instruction in instructions {
                        Self::retain_statement_bindings(syntax, instruction, &mut completed_bound);
                    }
                    let ensures = syntax
                        .statements
                        .assembly_expressions(block.ensures)
                        .to_vec();
                    for expression in ensures {
                        self.expression_static_operands(syntax, expression, &completed_bound)?;
                    }
                }
                continue;
            }
            self.statement_static_operands(syntax, *statement, &bound)?;
            for reference in super::generic_data::statement_type_inputs(syntax, *statement) {
                self.reference(syntax, reference, &bound)?;
            }
            Self::retain_statement_bindings(syntax, *statement, &mut bound);
        }
        Ok(())
    }

    fn machine(
        &mut self,
        syntax: &mut SyntaxTrees,
        machine: &syntax_trees::item::Machine,
        inherited: &[String],
    ) -> Result<(), Diagnostic> {
        let mut bound = self.parameters(syntax, machine.type_parameters, inherited, true)?;
        Self::retain_contract_bindings(syntax, machine.contracts, &mut bound);
        let states = syntax.items.state_handles(machine.states).to_vec();
        let mut entry_bound = bound.clone();
        if let Some(entry) = states.first() {
            entry_bound.extend(
                syntax
                    .items
                    .state_parameters(syntax.items.state(*entry).parameters)
                    .iter()
                    .map(|handle| {
                        syntax
                            .items
                            .state_parameter(*handle)
                            .name
                            .as_str(syntax)
                            .to_owned()
                    }),
            );
        }
        self.contract_static_operands(syntax, machine.contracts, &entry_bound)?;
        for handle in states {
            let state = syntax.items.state(handle).clone();
            self.reference(syntax, state.return_type, &bound)?;
            let parameters = syntax.items.state_parameters(state.parameters).to_vec();
            for parameter in parameters {
                self.reference(
                    syntax,
                    syntax.items.state_parameter(parameter).type_reference,
                    &bound,
                )?;
            }
            let mut expression_bound = bound.clone();
            expression_bound.extend(syntax.items.state_parameters(state.parameters).iter().map(
                |handle| {
                    syntax
                        .items
                        .state_parameter(*handle)
                        .name
                        .as_str(syntax)
                        .to_owned()
                },
            ));
            Self::retain_contract_bindings(syntax, state.contracts, &mut expression_bound);
            self.contract_static_operands(syntax, state.contracts, &expression_bound)?;
            let statements = syntax.items.statements(state.statements).to_vec();
            self.statements(syntax, &statements, &expression_bound)?;
        }
        self.nested_machines(syntax, machine.nested_machines, &bound)
    }

    fn nested_machines(
        &mut self,
        syntax: &mut SyntaxTrees,
        children: arena::HandleSpan<Item>,
        bound: &[String],
    ) -> Result<(), Diagnostic> {
        for child in children.handles() {
            let item = syntax.items.item(child).clone();
            self.item(syntax, &item, bound)?;
        }
        Ok(())
    }

    fn item(
        &mut self,
        syntax: &mut SyntaxTrees,
        item: &Item,
        inherited: &[String],
    ) -> Result<(), Diagnostic> {
        use syntax_trees::item::{ConformanceMember, DataMember};
        match item {
            Item::TypeAlias(alias) => {
                let bound = self.parameters(syntax, alias.type_parameters, inherited, false)?;
                self.active.push(alias.name.source_span());
                let result = self.reference(syntax, alias.expansion, &bound);
                self.active.pop();
                result?;
            }
            Item::Domain(domain) => {
                let bound = self.parameters(syntax, domain.type_parameters, inherited, false)?;
                // The carrier is a type use. The indices in the family header
                // name its own binders, whose declared types were expanded above.
                self.reference(syntax, domain.target_type, &bound)?;
            }
            Item::Data(data) => {
                let bound = self.parameters(syntax, data.type_parameters, inherited, false)?;
                let members = syntax.items.data_members(data.members).to_vec();
                for member in members {
                    match member {
                        DataMember::Field(field) => {
                            self.reference(syntax, field.type_reference, &bound)?
                        }
                        DataMember::Variant(variant) => {
                            let fields = syntax.items.data_payload_fields(variant.payload).to_vec();
                            for field in fields {
                                self.reference(syntax, field.type_reference, &bound)?;
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Machine(machine) => self.machine(syntax, machine, inherited)?,
            Item::Trait(definition) => {
                let bound =
                    self.parameters(syntax, definition.type_parameters, inherited, false)?;
                let parents = syntax
                    .type_references
                    .type_reference_handles(definition.parents)
                    .to_vec();
                for parent in parents {
                    self.reference(syntax, parent, &bound)?;
                }
                if let Some(reference) = definition.refines {
                    self.reference(syntax, reference, &bound)?;
                }
                let signatures = syntax.items.state_signatures(definition.machines).to_vec();
                for signature in signatures {
                    let signature = syntax.items.state_signature(signature).clone();
                    self.signature(syntax, &signature, &bound)?;
                }
            }
            Item::Conformance(definition) => {
                let bound =
                    self.parameters(syntax, definition.type_parameters, inherited, false)?;
                let arguments = syntax
                    .type_references
                    .type_reference_handles(definition.trait_arguments)
                    .to_vec();
                for argument in arguments {
                    self.reference(syntax, argument, &bound)?;
                }
                let members = syntax
                    .items
                    .conformance_members(definition.members)
                    .to_vec();
                for member in members {
                    let (ConformanceMember::Machine(machine)
                    | ConformanceMember::Synthesized(machine)
                    | ConformanceMember::TraitDefault { machine, .. }) = member
                    else {
                        // A selection row names an existing machine and has no body.
                        continue;
                    };
                    self.machine(syntax, &machine, &bound)?;
                }
            }
            Item::Const(definition) => {
                if definition.type_reference.is_valid() {
                    self.reference(syntax, definition.type_reference, inherited)?;
                }
                self.expression_static_operands(syntax, definition.value(), inherited)?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod type_alias_tests {
    /* Real parser trees exercise caller-first expansion and declaration cycles. */

    use super::*;

    fn normalized(text: &str) -> Result<SyntaxTrees, Vec<Diagnostic>> {
        let tokens = crate::resolution::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources
            .add("alias/main.omg".into(), text.to_owned())
            .source_id;
        let mut syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let selection =
            ConstantSelection::new(&syntax, Some(std::sync::Arc::new(sources)), Vec::new())?;
        normalize(&mut syntax, &selection)?;
        Ok(syntax)
    }

    fn parsed(text: &str) -> (SyntaxTrees, crate::source_inputs::SourceMap) {
        let tokens = crate::resolution::frontend_fixture::Lexer::new(text)
            .tokenize()
            .unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources
            .add("headers/main.omg".into(), text.to_owned())
            .source_id;
        let syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        (syntax, sources)
    }

    /* Normalization reads each name's roles through the header index; the
    index must answer exactly what scanning every root item answers. */
    #[test]
    fn indexed_header_roles_match_the_full_root_scan() {
        let text = "data Pair<A, B> { first: A; second: B; }
             machine choose<T, const N: u32>(value: T) -> T { value }
             trait Shape<T> {}
             data Count = u64;
             data Twice<T> = Pair<T, T>;";
        let (syntax, sources) = parsed(text);
        let selection =
            ConstantSelection::new(&syntax, Some(std::sync::Arc::new(sources)), Vec::new())
                .unwrap();
        let headers = RootHeaders::of(&syntax);
        assert!(headers.serves(&syntax));
        let kinds = [
            SymbolKind::Data,
            SymbolKind::TypeAlias,
            SymbolKind::Machine,
            SymbolKind::Trait,
        ];
        let names = syntax
            .root_items()
            .filter_map(|item| match item {
                Item::Data(data) => Some(data.name.clone()),
                Item::Machine(machine) => Some(machine.name.clone()),
                Item::Trait(definition) => Some(definition.name.clone()),
                Item::TypeAlias(alias) => Some(alias.name.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 5);
        for name in &names {
            assert_eq!(
                selection.generic_parameter_roles_in(&syntax, &headers, name, &kinds),
                selection.generic_parameter_roles(&syntax, name, &kinds),
                "roles of `{}`",
                name.as_str(&syntax)
            );
        }
        let roles = |spelling: &str| {
            let name = names
                .iter()
                .find(|name| name.as_str(&syntax) == spelling)
                .unwrap();
            selection.generic_parameter_roles_in(&syntax, &headers, name, &kinds)
        };
        use GenericParameterRole::{Const, Type};
        assert_eq!(roles("Pair"), Some(vec![Type, Type]));
        assert_eq!(roles("choose"), Some(vec![Type, Const]));
        assert_eq!(roles("Count"), Some(Vec::new()));
    }

    /* A forest that gained a root item after indexing is not served by the
    old rows, so a lookup falls back to scanning every root item. */
    #[test]
    fn a_header_index_refuses_a_forest_with_more_root_items() {
        let (small, _) = parsed("data One<T> { value: T; }");
        let (larger, _) = parsed("data One<T> { value: T; } data Two<T> { value: T; }");
        let headers = RootHeaders::of(&small);
        assert!(headers.serves(&small));
        assert!(!headers.serves(&larger));
    }

    #[test]
    fn a_domain_declared_on_an_alias_retains_its_qualified_carrier_and_index_binders() {
        let syntax = normalized(
            "data Word {} data Count = u64; data P = bool;
             data Segment<T> { value: T; } domain<T> Segment<T>::Stored<T>;
             data Kept = Segment<Word> in Stored<Word>;
             domain<P, const N: Count> Kept::Side<P, N>;",
        )
        .unwrap();
        let domain = syntax
            .root_items()
            .filter_map(Item::as_domain)
            .last()
            .unwrap();
        assert_eq!(domain.name.as_str(&syntax), "Kept::Side");
        let refs = &syntax.type_references;
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = refs.type_reference(domain.target_type)
        else {
            panic!("the alias's Stored qualification must remain on the carrier")
        };
        let TypeReferenceNode::Generic { base_name, .. } = refs.type_reference(*base_type) else {
            panic!("the alias's carrier is Segment<Word>")
        };
        assert_eq!(base_name.as_str(&syntax), "Segment");
        let [syntax_trees::types::TypeConstraintNode::Domain(stored)] =
            refs.constraints(*constraints)
        else {
            panic!("the carrier has one indexed domain")
        };
        assert_eq!(stored.name.as_str(&syntax), "Stored");
        assert_eq!(refs.type_alias_applications(domain.target_type).count(), 1);
        let parameters = syntax.items.type_parameters(domain.type_parameters);
        let TypeParameterKind::Const { type_reference } = parameters[1].kind else {
            panic!("N has a declared index type")
        };
        assert_eq!(
            refs.type_reference(type_reference)
                .as_named()
                .unwrap()
                .as_str(&syntax),
            "u64"
        );
        let indices = refs.type_reference_handles(domain.index_arguments);
        assert_eq!(
            indices
                .iter()
                .map(|index| refs
                    .type_reference(*index)
                    .as_named()
                    .unwrap()
                    .as_str(&syntax))
                .collect::<Vec<_>>(),
            ["P", "N"]
        );
    }

    #[test]
    fn a_generic_alias_supplies_an_open_static_machine_contract() {
        let syntax = normalized(
            "machine admissible<T>(value: T) -> bool { true }
             data Predicate<T> = machine(value: T) -> bool requires admissible(value);
             machine accepts<const Test: Predicate<u32>>(value: u32) -> bool { Test(value) }",
        )
        .unwrap();
        let machine = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) if machine.name.as_str(&syntax) == "accepts" => {
                    Some(machine)
                }
                _ => None,
            })
            .unwrap();
        let parameters = syntax.items.type_parameters(machine.type_parameters);
        let TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(signature)),
        } = &parameters[0].kind
        else {
            panic!("Test must be a structural callable binder");
        };
        assert_eq!(signature.name.as_str(&syntax), "Test");
        assert!(signature.service_reach_keyword_source_spans.is_empty());
        assert!(!signature.declares_no_crash_route);
        let argument = syntax.items.state_parameters(signature.parameters)[0];
        let reference = syntax.items.state_parameter(argument).type_reference;
        let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(reference)
        else {
            panic!("the applied predicate has a concrete input");
        };
        assert_eq!(name.as_str(&syntax), "u32");
    }

    #[test]
    fn alias_typed_proofs_keep_the_contract_without_selecting_code() {
        let syntax = normalized(
            "data NonzeroIsPositive = machine(n: u8) requires n != 0u8 ensures n > 0u8;
             machine accepts<Step: NonzeroIsPositive, const Run: NonzeroIsPositive>(n: u8) {}",
        )
        .unwrap();
        let machine = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .unwrap();
        let parameters = syntax.items.type_parameters(machine.type_parameters);
        let TypeParameterKind::Proof { contract } = &parameters[0].kind else {
            panic!("an alias-typed header parameter requests proof");
        };
        assert_eq!(contract.name.as_str(&syntax), "Step");
        assert_eq!(
            syntax.items.capability_contracts(contract.contracts).len(),
            2
        );
        assert_eq!(syntax.items.state_parameters(contract.parameters).len(), 1);
        assert!(matches!(
            parameters[1].kind,
            TypeParameterKind::Machine { .. }
        ));
    }

    #[test]
    fn proof_header_resolution_retains_distinct_formal_namespaces() {
        use crate::resolution::resolve;
        use crate::source_resolved::data::TypeParameterKind as ResolvedKind;
        use crate::source_resolved::domain::ProofFact;
        use crate::source_resolved::expression::ExpressionNode as ResolvedExpression;
        let source = "data Positive = machine(n: u8) requires n != 0u8 ensures n > 0u8;
            machine positive(n: u8) requires n != 0u8 ensures n > 0u8 {}
            machine accepts<First: Positive, Second: Positive>(n: u8) { First(n); Second(n); }
            machine entry() -> u8 { accepts::<positive, positive>(7u8); 0u8 }";
        let program = resolve(crate::resolution::tests::source_request(source)).unwrap();
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "accepts")
            .unwrap();
        let parameters = program.data_type_parameters(machine.type_parameters);
        let formals = parameters
            .iter()
            .map(|parameter| {
                let ResolvedKind::Proof { contract } = &parameter.kind else {
                    panic!("the declared proof must keep its kind");
                };
                assert_eq!(contract.symbol, parameter.symbol);
                let formal = program.state_parameters(contract.parameters)[0].symbol;
                assert_eq!(program.symbols.get(formal).parent, parameter.symbol);
                let expressions = &program.tables.bodies.expressions;
                let declarations = &program.tables.declarations;
                for clause in declarations
                    .signature_contracts
                    .span_or_empty(contract.contracts)
                {
                    for fact in declarations.proof_facts.span_or_empty(clause.facts) {
                        let ProofFact::Expression(expression) = fact else {
                            panic!("the fixture uses scalar clauses");
                        };
                        let ResolvedExpression::Binary(binary) =
                            expressions.expression(*expression)
                        else {
                            panic!("each clause compares its formal with zero");
                        };
                        let ResolvedExpression::Name(name) = expressions.expression(binary.left)
                        else {
                            panic!("the left operand names the alias formal");
                        };
                        assert_eq!(
                            name.symbol, formal,
                            "each alias use owns its value references"
                        );
                    }
                }
                formal
            })
            .collect::<Vec<_>>();
        assert_ne!(formals[0], formals[1]);
        let calls = program
            .tables
            .declarations
            .state_statements
            .iter()
            .filter_map(|(_, statement)| match statement {
                crate::source_resolved::statement::Statement::Call(call) => {
                    matches!(call.target.as_str(), "First" | "Second").then_some(call.target_symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls,
            parameters
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>(),
            "logical applications retain their exact proof binders, not executable targets"
        );
        let positive = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "positive")
            .unwrap();
        let call = program
            .tables
            .declarations
            .state_statements
            .iter()
            .find_map(|(_, statement)| match statement {
                crate::source_resolved::statement::Statement::Call(call)
                    if call.target.as_str() == "accepts" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(call.machine_arguments.len(), 2);
        for argument in &call.machine_arguments {
            assert!(program.symbols.symbols().nodes().is_valid(argument.symbol));
            let selected = program.symbols.get(argument.symbol);
            assert_eq!(selected.kind, symbols::SymbolKind::State);
            assert_eq!(selected.parent, positive.symbol);
        }
    }

    /// lifetimes.md: "Duplicate binders and undeclared tags reject." An
    /// alias owns the lifetimes its expansion writes, so `'capture`
    /// rejects at the alias whether or not the receiving machine happens
    /// to declare a binder with that tag. The alias that declares and
    /// substitutes `'capture` expands to the use site's own argument.
    #[test]
    fn an_alias_lifetime_is_bound_by_the_alias_not_by_its_use_site() {
        let rule = "type alias `View` names the lifetime `'capture`, which it does not \
                        declare: undeclared lifetime tags reject";
        for source in [
            "data View = @'capture [u8]; machine build_entry(view: View) { }",
            "data View = @'capture [u8]; machine build_entry<'capture>(view: View) { }",
        ] {
            let errors = normalized(source).unwrap_err();
            assert_eq!(
                errors
                    .iter()
                    .map(|error| error.message.as_str())
                    .collect::<Vec<_>>(),
                [rule],
                "{source}"
            );
            let span = errors[0].source_span.expect("the tag is authored").span;
            assert_eq!(&source[span.start..span.end], "capture", "{source}");
        }
        normalized(
            "data View<'capture> = @'capture [u8]; \
                 machine build_entry<'storage>(view: View<'storage>) { }",
        )
        .unwrap();
    }

    #[test]
    fn aliases_expand_supplied_arguments_before_rhs_cycles_and_keep_obligations() {
        let syntax =
            normalized("data Identity<T> = T; data Cell { value: Identity<Identity<u8>>; }")
                .unwrap();
        let data = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Data(data) => Some(data),
                _ => None,
            })
            .unwrap();
        let syntax_trees::item::DataMember::Field(field) =
            &syntax.items.data_members(data.members)[0]
        else {
            panic!("field");
        };
        let TypeReferenceNode::Named(name) =
            syntax.type_references.type_reference(field.type_reference)
        else {
            panic!("transparent expansion");
        };
        assert_eq!(name.as_str(&syntax), "u8");
        assert_eq!(
            syntax
                .type_references
                .type_alias_applications(field.type_reference)
                .count(),
            2
        );
        let mut repeated = normalized(
            "data T = bool; data Id<X> = X; data Wrap<U> = Id<U>; data Box<T> { value: Wrap<T>; }",
        )
        .unwrap();
        let new_source = "data Other<T> { value: Wrap<T>; }";
        let tokens = crate::resolution::frontend_fixture::Lexer::new(new_source)
            .tokenize()
            .unwrap();
        let later = crate::resolution::frontend_fixture::parse_syntax_trees_with_id(
            source_text::SourceId(77),
            &tokens,
        )
        .unwrap();
        repeated.extend_from(&later);
        let selection = ConstantSelection::new(&repeated, None, Vec::new()).unwrap();
        normalize(&mut repeated, &selection).unwrap();
        let data = repeated
            .root_items()
            .find_map(|item| match item {
                Item::Data(data) if data.name.as_str(&repeated) == "Other" => Some(data),
                _ => None,
            })
            .unwrap();
        let syntax_trees::item::DataMember::Field(field) =
            &repeated.items.data_members(data.members)[0]
        else {
            panic!("field");
        };
        let TypeReferenceNode::Named(name) = repeated
            .type_references
            .type_reference(field.type_reference)
        else {
            panic!("caller type parameter");
        };
        assert_eq!(name.as_str(&repeated), "T");
        let settled = repeated.clone();
        normalize(&mut repeated, &selection).unwrap();
        assert_eq!(
            repeated, settled,
            "a settled source phase must not allocate more header work"
        );

        let mut global = normalized("data Real { value: u8; } data Global<X> = Real; data Outer<Real> = Global<u8>; data View { value: Outer<bool>; }").unwrap();
        let tokens =
            crate::resolution::frontend_fixture::Lexer::new("data Later { value: Outer<bool>; }")
                .tokenize()
                .unwrap();
        let later = crate::resolution::frontend_fixture::parse_syntax_trees_with_id(
            source_text::SourceId(78),
            &tokens,
        )
        .unwrap();
        global.extend_from(&later);
        let selection = ConstantSelection::new(&global, None, Vec::new()).unwrap();
        normalize(&mut global, &selection).unwrap();
        let later = global
            .root_items()
            .find_map(|item| match item {
                Item::Data(data) if data.name.as_str(&global) == "Later" => Some(data),
                _ => None,
            })
            .unwrap();
        let syntax_trees::item::DataMember::Field(field) =
            &global.items.data_members(later.members)[0]
        else {
            panic!("field");
        };
        let TypeReferenceNode::Named(name) =
            global.type_references.type_reference(field.type_reference)
        else {
            panic!("global nominal");
        };
        assert_eq!(
            name.as_str(&global),
            "Real",
            "a normalized global must not become Outer’s same-spelled formal"
        );
        for source in [
            "data Recursive<T> = Recursive<[T; 2]>;",
            "data First<T> = Second<T>; data Second<T> = First<[T; 2]>;",
        ] {
            let errors = normalized(source).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.to_string().contains("recursive declaration cycle"))
            );
        }
    }
}
