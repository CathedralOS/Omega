/* Constant evaluation: templates. */
use super::super::{
    DataMember, Diagnostic, ExpressionNode, HashMap, HashSet, Identifier, Item, StatementNode,
    SyntaxTrees, TypeConstraintNode, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};
use crate::resolution::declarations::generic_data::uses::{
    ConstructorFrontier, collect_expression_handles, collect_statement_expression_handles,
    visit_expression_type_inputs, visit_static_argument_type_inputs,
};
use crate::resolution::tables::syntax_expressions;
use crate::resolution::tables::syntax_items;
use crate::resolution::tables::syntax_type_references;

use crate::resolution::declarations::generic_data::EvaluatedConst;

use super::super::uses::concrete_machine_declarations;
use crate::resolution::declarations::generic_data::evaluate_const_argument_expression;
use crate::resolution::declarations::generic_data::generic_const_integer_types;
use arena::HandleSpan;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::item::CapabilityContract;
use syntax_trees::item::Machine;
use syntax_trees::item::ProofFact;

pub(in crate::resolution::declarations::generic_data) fn replace_const_expression_names_from(
    syntax: &mut SyntaxTrees,
    expression_watermark: syntax_trees::expression::ExpressionFrontier,
    const_expressions: &HashMap<String, ExpressionNode>,
) {
    replace_uncaptured_const_names(
        syntax,
        expression_watermark,
        const_expressions,
        &HashSet::new(),
    );
}

/*
    Runtime bindings keep their authored selection. A local enters the frontier
    only after its initializer; machine-header expressions use entry parameters.
*/
#[derive(Default)]
pub(in crate::resolution::declarations::generic_data) struct RuntimeTemplateCaptures {
    expressions: HashSet<ExpressionHandle>,
    type_references: HashSet<TypeReferenceHandle>,
}

impl RuntimeTemplateCaptures {
    pub fn captures_type_reference(&self, reference: TypeReferenceHandle) -> bool {
        self.type_references.contains(&reference)
    }
}

pub(in crate::resolution::declarations::generic_data) fn capture_machine_runtime_template_names(
    syntax: &SyntaxTrees,
    machine: &Machine,
    type_watermark: syntax_trees::types::TypeReferenceFrontier,
    expression_watermark: syntax_trees::expression::ExpressionFrontier,
) -> RuntimeTemplateCaptures {
    let states = syntax.items.state_handles(machine.states);
    let parameters = states
        .first()
        .map(|state| syntax.items.state(*state).parameters)
        .unwrap_or_default();
    let mut captured = RuntimeTemplateCaptures::default();
    let header_frontier = ConstructorFrontier {
        parameters,
        prior_statements: &[],
    };
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if expression_watermark.contains(handle)
            && let ExpressionNode::Name {
                members: path,
                application: path_application,
            } = expression
            && path_application.is_none()
            && header_frontier.captures(syntax, path.to_owned())
        {
            captured.expressions.insert(handle);
        }
    }
    for (handle, _) in syntax_type_references(syntax).named_nodes_from(type_watermark, syntax) {
        if let Some(name) = syntax.type_references.type_reference(handle).as_named()
            && header_frontier.captures_name(syntax, name)
        {
            captured.type_references.insert(handle);
        }
    }
    for state in states {
        let state = syntax.items.state(*state);
        let mut signature = TypeExpressionInputs::default();
        collect_type_expression_handles(syntax, state.return_type, &mut signature);
        for parameter in syntax.items.state_parameters(state.parameters) {
            collect_type_expression_handles(
                syntax,
                syntax.items.state_parameter(*parameter).type_reference,
                &mut signature,
            );
        }
        collect_contract_expression_handles(syntax, state.contracts, &mut signature);
        let statements = syntax.items.statements(state.statements);
        update_captured_names(
            syntax,
            &mut signature,
            &ConstructorFrontier {
                parameters: state.parameters,
                prior_statements: &[],
            },
            &mut captured,
        );
        for (ordinal, statement) in statements.iter().enumerate() {
            let mut inputs = TypeExpressionInputs::default();
            collect_statement_expression_handles(syntax, *statement, &mut inputs.expressions);
            if let StatementNode::LocalData(local) = syntax.statements.statement(*statement) {
                    collect_type_expression_handles(syntax, local.type_reference, &mut inputs);
            }
            update_captured_names(
                syntax,
                &mut inputs,
                &ConstructorFrontier {
                    parameters: state.parameters,
                    prior_statements: &statements[..ordinal],
                },
                &mut captured,
            );
        }
    }
    captured
}

pub(in crate::resolution::declarations::generic_data) fn replace_machine_const_expression_names_from(
    syntax: &mut SyntaxTrees,
    expression_watermark: syntax_trees::expression::ExpressionFrontier,
    const_expressions: &HashMap<String, ExpressionNode>,
    captured: &RuntimeTemplateCaptures,
) {
    replace_uncaptured_const_names(
        syntax,
        expression_watermark,
        const_expressions,
        &captured.expressions,
    );
}

fn update_captured_names(
    syntax: &SyntaxTrees,
    inputs: &mut TypeExpressionInputs,
    frontier: &ConstructorFrontier<'_>,
    captured: &mut RuntimeTemplateCaptures,
) {
    let mut visited = HashSet::new();
    loop {
        let pending = inputs
            .expressions
            .difference(&visited)
            .copied()
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }
        for handle in pending {
            visited.insert(handle);
            visit_expression_type_inputs(syntax, handle, &mut |reference| {
                collect_type_expression_handles(syntax, reference, inputs);
            });
        }
    }
    for handle in &inputs.expressions {
        if let ExpressionNode::Name {
            members: path,
            application: path_application,
        } = syntax.expressions.expression(*handle)
        {
            if inputs.bound_expressions.contains(handle)
                || (path_application.is_none() && frontier.captures(syntax, path.to_owned()))
            {
                captured.expressions.insert(*handle);
            } else {
                captured.expressions.remove(handle);
            }
        }
    }
    for reference in &inputs.types {
        if let Some(name) = syntax.type_references.type_reference(*reference).as_named() {
            if inputs.bound_types.contains(reference) || frontier.captures_name(syntax, name) {
                captured.type_references.insert(*reference);
            } else {
                captured.type_references.remove(reference);
            }
        }
    }
}

#[derive(Default)]
struct TypeExpressionInputs {
    expressions: HashSet<ExpressionHandle>,
    types: HashSet<TypeReferenceHandle>,
    bound_expressions: HashSet<ExpressionHandle>,
    bound_types: HashSet<TypeReferenceHandle>,
}

impl TypeExpressionInputs {
    fn extend(&mut self, other: Self) {
        self.expressions.extend(other.expressions);
        self.types.extend(other.types);
        self.bound_expressions.extend(other.bound_expressions);
        self.bound_types.extend(other.bound_types);
    }
}

struct TypeInputWalk<'syntax> {
    syntax: &'syntax SyntaxTrees,
    entered: HashSet<TypeReferenceHandle>,
}

impl<'syntax> TypeInputWalk<'syntax> {
    fn new(syntax: &'syntax SyntaxTrees) -> Self {
        Self {
            syntax,
            entered: HashSet::new(),
        }
    }

    fn expression(&mut self, expression: ExpressionHandle, inputs: &mut TypeExpressionInputs) {
        let syntax = self.syntax;
        let mut expressions = HashSet::new();
        collect_expression_handles(syntax, expression, &mut expressions);
        for &expression in &expressions {
            visit_expression_type_inputs(syntax, expression, &mut |reference| {
                self.reference(reference, inputs);
            });
        }
        inputs.expressions.extend(expressions);
    }

    fn contracts(
        &mut self,
        contracts: HandleSpan<CapabilityContract>,
        inputs: &mut TypeExpressionInputs,
    ) {
        let syntax = self.syntax;
        for contract in syntax.items.capability_contracts(contracts) {
            for fact in syntax.items.proof_facts(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => self.expression(*expression, inputs),
                    ProofFact::Membership(membership) => {
                        self.expression(membership.value, inputs);
                        for &argument in syntax
                            .type_references
                            .type_reference_handles(membership.domain_arguments)
                        {
                            self.reference(argument, inputs);
                        }
                    }
                }
            }
        }
    }

    fn protect_signature_scope(
        &self,
        signature: &crate::resolution::declarations::callable_signatures::SignatureInputs,
        scope: usize,
        inputs: &mut TypeExpressionInputs,
    ) {
        for &expression in &inputs.expressions {
            if let ExpressionNode::Name {
                members: path,
                application: _,
            } = self.syntax.expressions.expression(expression)
                && let Some(name) = self
                    .syntax
                    .expressions
                    .identifier_path_members(*path)
                    .first()
                && signature.hides_value(scope, name.as_str(self.syntax))
            {
                inputs.bound_expressions.insert(expression);
            }
        }
        for &reference in &inputs.types {
            if let Some(name) = self
                .syntax
                .type_references
                .type_reference(reference)
                .as_named()
                && signature.hides_type(scope, name.as_str(self.syntax))
            {
                inputs.bound_types.insert(reference);
            }
        }
    }

    fn reference(&mut self, reference: TypeReferenceHandle, inputs: &mut TypeExpressionInputs) {
        if !reference.is_valid() || !self.entered.insert(reference) {
            return;
        }
        inputs.types.insert(reference);
        let syntax = self.syntax;
        match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::ConstExpression(expression) => self.expression(*expression, inputs),
            TypeReferenceNode::Reference { referee, .. } => self.reference(*referee, inputs),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                self.reference(*element_type, inputs);
                if let syntax_trees::types::FixedArrayLength::Expression(value) = length {
                    self.reference(*value, inputs);
                }
            }
            TypeReferenceNode::Slice { element_type } => self.reference(*element_type, inputs),
            TypeReferenceNode::Generic { arguments, .. } => {
                for &argument in syntax.type_references.type_reference_handles(*arguments) {
                    self.reference(argument, inputs);
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                self.reference(*base_type, inputs);
                for constraint in syntax.type_references.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        for &argument in syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                        {
                            self.reference(argument, inputs);
                        }
                    }
                }
            }
            TypeReferenceNode::Callable { signature } => {
                let signature =
                    crate::resolution::declarations::callable_signatures::SignatureInputs::of(
                        syntax,
                        syntax.items.state_signature(*signature),
                    );
                for &(scope, reference) in &signature.types {
                    let mut nested = TypeExpressionInputs::default();
                    self.reference(reference, &mut nested);
                    self.protect_signature_scope(&signature, scope, &mut nested);
                    inputs.extend(nested);
                }
                for &(scope, expression) in &signature.expressions {
                    let mut nested = TypeExpressionInputs::default();
                    self.expression(expression, &mut nested);
                    self.protect_signature_scope(&signature, scope, &mut nested);
                    inputs.extend(nested);
                }
            }
            TypeReferenceNode::StaticSelection(selection) => {
                visit_static_argument_type_inputs(
                    syntax,
                    std::slice::from_ref(selection),
                    &mut |reference| {
                        self.reference(reference, inputs);
                    },
                );
            }
            TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
        }
    }
}

fn collect_contract_expression_handles(
    syntax: &SyntaxTrees,
    contracts: HandleSpan<CapabilityContract>,
    inputs: &mut TypeExpressionInputs,
) {
    TypeInputWalk::new(syntax).contracts(contracts, inputs);
}

fn collect_type_expression_handles(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    inputs: &mut TypeExpressionInputs,
) {
    TypeInputWalk::new(syntax).reference(reference, inputs);
}

fn replace_uncaptured_const_names(
    syntax: &mut SyntaxTrees,
    expression_watermark: syntax_trees::expression::ExpressionFrontier,
    const_expressions: &HashMap<String, ExpressionNode>,
    captured: &HashSet<ExpressionHandle>,
) {
    let replacements = syntax_expressions(syntax)
        .iter_expressions()
        .filter(|(handle, _)| expression_watermark.contains(*handle))
        .filter(|(handle, _)| !captured.contains(handle))
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name {
                members: path,
                application: path_application,
            } = expression
            else {
                return None;
            };
            if path_application.is_some() {
                return None;
            }
            let [name] = syntax.expressions.identifier_path_members(path.to_owned()) else {
                return None;
            };
            const_expressions
                .get(name.as_str(syntax))
                .cloned()
                .map(|value| (handle, value))
        })
        .collect::<Vec<_>>();
    for (handle, value) in replacements {
        syntax.expressions.replace_expression(handle, value);
    }
}

/*
    Generic definitions remain in the tree after their concrete records are
    synthesized so the normal frontend can validate the template. A symbolic
    const expression cannot cross that boundary yet, so reduce each template
    expression to either its concrete value or one declared const-parameter
    dependency. The concrete clones already carry the fully evaluated value;
    this placeholder exists only to preserve the established generic type/kind
    checks on the source template.
*/
pub(in crate::resolution::declarations::generic_data) fn normalize_generic_template_const_expressions(
    syntax: &mut SyntaxTrees,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Result<(), Diagnostic> {
    let templates: Vec<(String, HashSet<String>, Vec<TypeReferenceHandle>)> = declarations
        .iter()
        .map(|declaration| syntax.root_item(*declaration))
        .filter_map(|item| {
            let definition = item.as_data()?;
            if definition.type_parameters.is_empty() {
                return None;
            }
            let symbolic_parameters = syntax_items(syntax)
                .type_parameters(definition.type_parameters)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
                .map(|parameter| parameter.name.as_str(syntax).to_string())
                .collect();
            let fields = syntax_items(syntax)
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field.type_reference),
                    DataMember::Variant(_) => None,
                    DataMember::Retired(_) => None,
                })
                .collect();
            Some((
                definition.name.as_str(syntax).to_string(),
                symbolic_parameters,
                fields,
            ))
        })
        .collect();

    for (template, symbolic_parameters, fields) in templates {
        for field in fields {
            normalize_template_type_reference(
                syntax,
                field,
                selection,
                &symbolic_parameters,
                warnings,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "const argument expression in generic data `{template}` is invalid: {reason}"
                ))
            })?;
        }
    }
    Ok(())
}

pub(in crate::resolution::declarations::generic_data) fn normalize_template_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    selection: Option<&crate::resolution::declarations::declaration_selection::ConstantSelection>,
    symbolic_parameters: &HashSet<String>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let node = syntax_type_references(syntax)
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Callable { signature } => {
            let inputs = crate::resolution::declarations::callable_signatures::SignatureInputs::of(
                syntax,
                syntax.items.state_signature(signature),
            );
            let scopes = (0..inputs.scope_count())
                .map(|scope| {
                    let mut symbolic = symbolic_parameters
                        .iter()
                        .filter(|name| !inputs.hides_value(scope, name))
                        .cloned()
                        .collect::<HashSet<_>>();
                    symbolic.extend(inputs.bound_values(scope));
                    symbolic
                })
                .collect::<Vec<_>>();
            for &(scope, reference) in &inputs.types {
                normalize_template_type_reference(
                    syntax,
                    reference,
                    selection,
                    &scopes[scope],
                    warnings,
                )?;
            }
            for &(scope, expression) in &inputs.expressions {
                let mut expressions = HashSet::new();
                collect_expression_handles(syntax, expression, &mut expressions);
                let mut types = Vec::new();
                for expression in expressions {
                    visit_expression_type_inputs(syntax, expression, &mut |reference| {
                        types.push(reference)
                    });
                }
                for reference in types {
                    normalize_template_type_reference(
                        syntax,
                        reference,
                        selection,
                        &scopes[scope],
                        warnings,
                    )?;
                }
            }
            Ok(())
        }
        TypeReferenceNode::Reference { referee, .. } => normalize_template_type_reference(
            syntax,
            referee,
            selection,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::Constrained { base_type, .. } => normalize_template_type_reference(
            syntax,
            base_type,
            selection,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => normalize_template_type_reference(
            syntax,
            element_type,
            selection,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax_type_references(syntax)
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, &base_name, selection);
            for (index, argument) in arguments.into_iter().enumerate() {
                let node = syntax_type_references(syntax)
                    .type_reference(argument)
                    .clone();
                if let TypeReferenceNode::ConstExpression(expression) = node {
                    let placeholder = evaluate_const_argument_expression(
                        syntax,
                        expression,
                        selection,
                        &HashMap::new(),
                        symbolic_parameters,
                        integer_types.get(index).copied().flatten(),
                        warnings,
                    )?;
                    let name = match placeholder {
                        EvaluatedConst::Concrete(value) => value.to_string(),
                        EvaluatedConst::Symbolic(name) => name,
                    };
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(name)),
                    );
                } else {
                    normalize_template_type_reference(
                        syntax,
                        argument,
                        selection,
                        symbolic_parameters,
                        warnings,
                    )?;
                }
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            let placeholder = evaluate_const_argument_expression(
                syntax,
                expression,
                selection,
                &HashMap::new(),
                symbolic_parameters,
                None,
                warnings,
            )?;
            let name = match placeholder {
                EvaluatedConst::Concrete(value) => value.to_string(),
                EvaluatedConst::Symbolic(name) => name,
            };
            syntax.tables.type_references.replace_type_reference(
                type_reference,
                TypeReferenceNode::Named(Identifier::generated(name)),
            );
            Ok(())
        }
        TypeReferenceNode::StaticSelection(_) => Err(
            "a scoped type selection must resolve before scalar template normalization".to_owned(),
        ),
        TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => Ok(()),
    }
}

/*
    Declared type inputs for closed data specialization: fields, constants and
    concrete callable signatures/bodies, including bodyless requirements.
    Revisit newly synthesized declarations on each completion round.
*/
pub(in crate::resolution::declarations::generic_data) fn collect_selected_type_reference_positions(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Vec<TypeReferenceHandle> {
    collect_owned_type_reference_positions(
        syntax,
        declarations,
        true,
        false,
        TypePositionTraversal::Identity,
    )
}

/*
    A substituted declaration adds inline storage, not demands to instantiate
    every referent it describes. The synthesis owner adds declaration-obligation
    demands independently; identity traversal alone does not discharge them.
*/
pub(in crate::resolution::declarations::generic_data) fn collect_stored_type_reference_positions(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Vec<TypeReferenceHandle> {
    collect_owned_type_reference_positions(
        syntax,
        declarations,
        true,
        false,
        TypePositionTraversal::Storage,
    )
}

pub(in crate::resolution::declarations::generic_data) fn collect_data_type_reference_positions(
    syntax: &SyntaxTrees,
    public_only: bool,
) -> Vec<TypeReferenceHandle> {
    collect_owned_type_reference_positions(
        syntax,
        syntax.root_item_handles(),
        false,
        public_only,
        TypePositionTraversal::Identity,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypePositionTraversal {
    Identity,
    Storage,
}

pub(in crate::resolution::declarations) fn collect_type_positions(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    positions: &mut Vec<TypeReferenceHandle>,
    include_domain_arguments: bool,
) {
    visit_type_positions(
        syntax,
        type_reference,
        positions,
        include_domain_arguments,
        TypePositionTraversal::Identity,
    );
}

fn visit_type_positions(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    positions: &mut Vec<TypeReferenceHandle>,
    include_domain_arguments: bool,
    traversal: TypePositionTraversal,
) {
    let node = syntax.tables.type_references.type_reference(type_reference);
    // A containing structural equation reads the canonical const arguments of
    // its nested applications. Visit each application after its arguments.
    if !matches!(node, TypeReferenceNode::Generic { .. }) {
        positions.push(type_reference);
    }
    macro_rules! visit_child {
        ($child:expr $(,)?) => {
            visit_type_positions(
                syntax,
                $child,
                positions,
                include_domain_arguments,
                traversal,
            )
        };
    }
    match node {
        TypeReferenceNode::Callable { signature } => {
            if traversal == TypePositionTraversal::Storage {
                return;
            }
            let inputs = crate::resolution::declarations::callable_signatures::SignatureInputs::of(
                syntax,
                syntax.items.state_signature(*signature),
            );
            for &(_, reference) in &inputs.types {
                visit_child!(reference);
            }
            for &(_, expression) in &inputs.expressions {
                let mut expressions = HashSet::new();
                collect_expression_handles(syntax, expression, &mut expressions);
                for expression in expressions {
                    visit_expression_type_inputs(syntax, expression, &mut |reference| {
                        visit_child!(reference)
                    });
                }
            }
        }
        TypeReferenceNode::Reference { referee, .. } => {
            if traversal == TypePositionTraversal::Identity {
                visit_child!(*referee);
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            visit_child!(*base_type);
            if !include_domain_arguments || traversal == TypePositionTraversal::Storage {
                return;
            }
            for constraint in syntax.type_references.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for argument in
                        syntax_type_references(syntax).type_reference_handles(domain.arguments)
                    {
                        visit_child!(*argument);
                    }
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            visit_child!(*element_type);
            if let syntax_trees::types::FixedArrayLength::Expression(value) = length {
                visit_child!(*value);
            }
        }
        TypeReferenceNode::Slice { element_type } => visit_child!(*element_type),
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in syntax_type_references(syntax).type_reference_handles(*arguments) {
                visit_child!(*argument);
            }
            positions.push(type_reference);
        }
        TypeReferenceNode::ConstExpression(expression) => {
            let mut expressions = HashSet::new();
            super::super::uses::collect_expression_handles(syntax, *expression, &mut expressions);
            for expression in expressions {
                visit_expression_type_inputs(syntax, expression, &mut |reference| {
                    visit_child!(reference);
                });
            }
        }
        TypeReferenceNode::StaticSelection(selection) => {
            super::super::uses::visit_static_argument_type_inputs(
                syntax,
                std::slice::from_ref(selection),
                &mut |reference| {
                    visit_child!(reference);
                },
            );
        }
        TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
    }
}

fn collect_owned_type_reference_positions(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
    include_callables: bool,
    public_only: bool,
    traversal: TypePositionTraversal,
) -> Vec<TypeReferenceHandle> {
    let mut positions: Vec<TypeReferenceHandle> = Vec::new();
    for &declaration in declarations {
        let item = syntax.root_item(declaration);
        match item {
            Item::Data(definition)
                if definition.type_parameters.is_empty()
                    && (!public_only || definition.is_public) =>
            {
                for member in syntax.tables.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => visit_type_positions(
                            syntax,
                            field.type_reference,
                            &mut positions,
                            true,
                            traversal,
                        ),
                        DataMember::Variant(variant) => {
                            for field in syntax.tables.items.data_payload_fields(variant.payload) {
                                visit_type_positions(
                                    syntax,
                                    field.type_reference,
                                    &mut positions,
                                    true,
                                    traversal,
                                );
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Const(definition) if !public_only || definition.is_public => {
                visit_type_positions(
                    syntax,
                    definition.type_reference,
                    &mut positions,
                    true,
                    traversal,
                )
            }
            _ => {}
        }
    }
    if !include_callables {
        return positions;
    }
    visit_callable_type_inputs(syntax, declarations, |reference, _| {
        visit_type_positions(syntax, reference, &mut positions, false, traversal);
    });
    positions
}

/*
    Every concrete callable publishes its declared type inputs here. Constant
    evaluation and closed data specialization consume this same list, including
    bodyless requirements. Open declarations remain templates in both routes.
*/
fn visit_callable_type_inputs(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
    mut visit: impl FnMut(TypeReferenceHandle, bool),
) {
    for machine in concrete_machine_declarations(syntax, declarations) {
        for conformance in syntax.items.satisfies_clauses(machine.satisfies) {
            for &argument in
                syntax_type_references(syntax).type_reference_handles(conformance.arguments)
            {
                visit(argument, machine.is_public);
            }
        }
        for (ordinal, state_handle) in syntax_items(syntax)
            .state_handles(machine.states)
            .iter()
            .enumerate()
        {
            let state = syntax.items.state(*state_handle);
            let is_public_signature = ordinal == 0 && machine.is_public;
            visit(state.return_type, is_public_signature);
            for parameter in syntax.items.state_parameters(state.parameters) {
                visit(
                    syntax.items.state_parameter(*parameter).type_reference,
                    is_public_signature,
                );
            }
            for statement in syntax.items.statements(state.statements) {
                for reference in super::super::uses::statement_type_inputs(syntax, *statement) {
                    visit(reference, false);
                }
            }
        }
    }
    for &declaration in declarations {
        let Item::Trait(definition) = syntax.root_item(declaration) else {
            continue;
        };
        if !definition.type_parameters.is_empty() {
            continue;
        }
        for handle in syntax.items.state_signatures(definition.machines) {
            let signature = syntax.items.state_signature(*handle);
            if !signature.type_parameters.is_empty() {
                continue;
            }
            visit(signature.return_type, definition.is_public);
            for parameter in syntax.items.state_parameters(signature.parameters) {
                visit(
                    syntax.items.state_parameter(*parameter).type_reference,
                    definition.is_public,
                );
            }
        }
    }
}
