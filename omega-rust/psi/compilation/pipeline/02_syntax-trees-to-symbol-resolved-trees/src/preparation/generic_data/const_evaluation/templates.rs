//! Constant evaluation: templates.
use super::super::{
    DataMember, Diagnostic, ExpressionNode, HashMap, HashSet, Item, StatementNode, SyntaxTrees,
    TypeConstraintNode, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};
use crate::preparation::generic_data::uses::{
    ConstructorFrontier, collect_expression_handles, collect_statement_expression_handles,
    concrete_machine_expression_handles, visit_expression_type_inputs,
    visit_static_argument_type_inputs,
};

use crate::preparation::generic_data::EvaluatedConst;

use crate::preparation::generic_data::generic_const_integer_types;
use arena::HandleSpan;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::StaticMachineArgument;
use syntax_trees::item::CapabilityContract;
use syntax_trees::item::Machine;
use syntax_trees::item::ProofFact;

pub(in crate::preparation::generic_data) fn replace_const_expression_names_from(
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

/// Runtime bindings keep their authored selection. A local enters the frontier
/// only after its initializer; machine-header expressions use entry parameters.
#[derive(Default)]
pub(in crate::preparation::generic_data) struct RuntimeTemplateCaptures {
    expressions: HashSet<ExpressionHandle>,
    type_references: HashSet<TypeReferenceHandle>,
}

impl RuntimeTemplateCaptures {
    pub fn captures_type_reference(&self, reference: TypeReferenceHandle) -> bool {
        self.type_references.contains(&reference)
    }
}

pub(in crate::preparation::generic_data) fn capture_machine_runtime_template_names(
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
            && let ExpressionNode::Name(path) = expression
            && header_frontier.captures(syntax, *path)
        {
            captured.expressions.insert(handle);
        }
    }
    for (handle, _) in syntax.type_references.named_nodes_from(type_watermark) {
        if let Some(name) = syntax.type_references.type_reference(handle).as_named()
            && header_frontier.captures_name(syntax, name)
        {
            captured.type_references.insert(handle);
        }
    }
    for state in states {
        let state = syntax.items.state(*state);
        let mut signature = HashSet::new();
        let mut signature_types = HashSet::new();
        collect_type_expression_handles(
            syntax,
            state.return_type,
            &mut signature,
            &mut signature_types,
        );
        for parameter in syntax.items.state_parameters(state.parameters) {
            collect_type_expression_handles(
                syntax,
                syntax.items.state_parameter(*parameter).type_reference,
                &mut signature,
                &mut signature_types,
            );
        }
        collect_contract_expression_handles(
            syntax,
            state.contracts,
            &mut signature,
            &mut signature_types,
        );
        let statements = syntax.items.statements(state.statements);
        update_captured_names(
            syntax,
            &mut signature,
            &mut signature_types,
            &ConstructorFrontier {
                parameters: state.parameters,
                prior_statements: &[],
            },
            &mut captured,
        );
        for (ordinal, statement) in statements.iter().enumerate() {
            let mut expressions = HashSet::new();
            let mut type_references = HashSet::new();
            collect_statement_expression_handles(syntax, *statement, &mut expressions);
            match syntax.statements.statement(*statement) {
                StatementNode::LocalData(local) => {
                    collect_type_expression_handles(
                        syntax,
                        local.type_reference,
                        &mut expressions,
                        &mut type_references,
                    );
                }
                StatementNode::Call(call) => {
                    collect_static_argument_expression_handles(
                        syntax,
                        &call.machine_arguments,
                        &mut expressions,
                        &mut type_references,
                    );
                }
                _ => {}
            }
            update_captured_names(
                syntax,
                &mut expressions,
                &mut type_references,
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

pub(in crate::preparation::generic_data) fn replace_machine_const_expression_names_from(
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
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
    frontier: &ConstructorFrontier<'_>,
    captured: &mut RuntimeTemplateCaptures,
) {
    // The executable child walker deliberately excludes type positions. Those
    // positions still belong to the same lexical frontier during substitution.
    let mut visited = HashSet::new();
    loop {
        let pending = expressions
            .difference(&visited)
            .copied()
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }
        for handle in pending {
            visited.insert(handle);
            visit_expression_type_inputs(syntax, handle, &mut |reference| {
                collect_type_expression_handles(syntax, reference, expressions, type_references);
            });
        }
    }
    for handle in expressions.iter() {
        if let ExpressionNode::Name(path) = syntax.expressions.expression(*handle) {
            if frontier.captures(syntax, *path) {
                captured.expressions.insert(*handle);
            } else {
                captured.expressions.remove(handle);
            }
        }
    }
    for reference in type_references.iter() {
        if let Some(name) = syntax.type_references.type_reference(*reference).as_named() {
            if frontier.captures_name(syntax, name) {
                captured.type_references.insert(*reference);
            } else {
                captured.type_references.remove(reference);
            }
        }
    }
}

fn collect_static_argument_expression_handles(
    syntax: &SyntaxTrees,
    arguments: &[StaticMachineArgument],
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
) {
    visit_static_argument_type_inputs(arguments, &mut |reference| {
        collect_type_expression_handles(syntax, reference, expressions, type_references);
    });
}

fn collect_contract_expression_handles(
    syntax: &SyntaxTrees,
    contracts: HandleSpan<CapabilityContract>,
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
) {
    for contract in syntax.items.capability_contracts(contracts) {
        collect_proof_fact_expression_handles(syntax, contract.facts, expressions, type_references);
    }
}

fn collect_proof_fact_expression_handles(
    syntax: &SyntaxTrees,
    facts: HandleSpan<ProofFact>,
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
) {
    for fact in syntax.items.proof_facts(facts) {
        match fact {
            ProofFact::Expression(expression) => {
                collect_expression_handles(syntax, *expression, expressions)
            }
            ProofFact::Membership(membership) => {
                collect_expression_handles(syntax, membership.value, expressions);
                for argument in syntax
                    .type_references
                    .type_reference_handles(membership.domain_arguments)
                {
                    collect_type_expression_handles(
                        syntax,
                        *argument,
                        expressions,
                        type_references,
                    );
                }
            }
        }
    }
}

fn collect_type_expression_handles(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
) {
    if !reference.is_valid() || !type_references.insert(reference) {
        return;
    }
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::ConstExpression(expression) => {
            collect_expression_handles(syntax, *expression, expressions)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_expression_handles(syntax, *referee, expressions, type_references)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_expression_handles(syntax, *element_type, expressions, type_references)
        }
        TypeReferenceNode::Generic { arguments, .. }
        | TypeReferenceNode::DynamicTrait {
            trait_arguments: arguments,
            ..
        } => {
            for argument in syntax.type_references.type_reference_handles(*arguments) {
                collect_type_expression_handles(syntax, *argument, expressions, type_references);
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_expression_handles(syntax, *base_type, expressions, type_references);
            for constraint in syntax.type_references.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        collect_expression_handles(syntax, *minimum, expressions);
                        collect_expression_handles(syntax, *maximum, expressions);
                    }
                    TypeConstraintNode::Domain(domain) => {
                        for argument in syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                        {
                            collect_type_expression_handles(
                                syntax,
                                *argument,
                                expressions,
                                type_references,
                            );
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
    }
}

fn replace_uncaptured_const_names(
    syntax: &mut SyntaxTrees,
    expression_watermark: syntax_trees::expression::ExpressionFrontier,
    const_expressions: &HashMap<String, ExpressionNode>,
    captured: &HashSet<ExpressionHandle>,
) {
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| expression_watermark.contains(*handle))
        .filter(|(handle, _)| !captured.contains(handle))
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            let [name] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            const_expressions
                .get(name.as_str())
                .cloned()
                .map(|value| (handle, value))
        })
        .collect::<Vec<_>>();
    for (handle, value) in replacements {
        syntax.expressions.replace_expression(handle, value);
    }
}

/// Generic definitions remain after their concrete records are synthesized.
/// Preserve each template's original const graph while reducing closed values
/// for syntax synthesis. A symbolic graph remains symbolic: a diagnostic
/// dependency name cannot replace its issuing expression or selected policy.
pub(in crate::preparation::generic_data) fn normalize_generic_template_const_expressions(
    syntax: &mut SyntaxTrees,
    const_values: &HashMap<String, i128>,
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
            let symbolic_parameters = syntax
                .tables
                .items
                .type_parameters(definition.type_parameters)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
                .map(|parameter| parameter.name.as_str().to_string())
                .collect();
            let fields = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field.type_reference),
                    DataMember::Variant(_) => None,
                    DataMember::Retired(_) => None,
                })
                .collect();
            Some((
                definition.name.as_str().to_string(),
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
                const_values,
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

pub(in crate::preparation::generic_data) fn normalize_template_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    const_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Reference { referee, .. } => normalize_template_type_reference(
            syntax,
            referee,
            const_values,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::Constrained { base_type, .. } => normalize_template_type_reference(
            syntax,
            base_type,
            const_values,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => normalize_template_type_reference(
            syntax,
            element_type,
            const_values,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            for (index, argument) in arguments.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                if let TypeReferenceNode::ConstExpression(expression) = node {
                    let mut builtin_operators = Vec::new();
                    let placeholder =
                        super::arguments::evaluate_const_argument_expression_with_operators(
                            syntax,
                            expression,
                            const_values,
                            &HashMap::new(),
                            symbolic_parameters,
                            integer_types.get(index).copied().flatten(),
                            warnings,
                            &mut builtin_operators,
                        )?;
                    if let EvaluatedConst::Concrete(value) = placeholder {
                        super::replace_integer_argument_with_operators(
                            syntax,
                            argument,
                            value,
                            builtin_operators,
                        );
                    }
                    // A symbolic result is still the original expression;
                    // diagnostic text cannot become its semantic operand.
                } else {
                    normalize_template_type_reference(
                        syntax,
                        argument,
                        const_values,
                        symbolic_parameters,
                        warnings,
                    )?;
                }
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            let mut builtin_operators = Vec::new();
            let placeholder = super::arguments::evaluate_const_argument_expression_with_operators(
                syntax,
                expression,
                const_values,
                &HashMap::new(),
                symbolic_parameters,
                None,
                warnings,
                &mut builtin_operators,
            )?;
            if let EvaluatedConst::Concrete(value) = placeholder {
                super::replace_integer_argument_with_operators(
                    syntax,
                    type_reference,
                    value,
                    builtin_operators,
                );
            }
            Ok(())
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::Unit => Ok(()),
    }
}

/// Every TYPE-REFERENCE position a generic-data spelling can appear in: data
/// FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN types. Run
/// afresh each fixpoint round so newly-synthesized records' fields are seen.
pub(in crate::preparation::generic_data) fn collect_selected_type_reference_positions(
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

/// A substituted declaration adds inline storage, not demands to instantiate
/// every referent it describes. The synthesis owner adds declaration-obligation
/// demands independently; identity traversal alone does not discharge them.
pub(in crate::preparation::generic_data) fn collect_stored_type_reference_positions(
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

pub(in crate::preparation::generic_data) fn collect_data_type_reference_positions(
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
    /// Source input ownership includes each live applied tuple and nested
    /// static type input, rather than demanding new runtime instantiation.
    Source,
}

pub(in crate::preparation) fn collect_type_positions(
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
    if !type_reference.is_valid() || positions.contains(&type_reference) {
        return;
    }
    positions.push(type_reference);
    if traversal == TypePositionTraversal::Source {
        let original = syntax
            .type_references
            .generic_application_origin(type_reference);
        if original.is_valid() {
            visit_type_positions(
                syntax,
                original,
                positions,
                include_domain_arguments,
                traversal,
            );
        }
        for pending in syntax
            .type_references
            .pending_const_arguments(type_reference)
        {
            visit_type_positions(
                syntax,
                pending,
                positions,
                include_domain_arguments,
                traversal,
            );
        }
    }
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            if traversal != TypePositionTraversal::Storage {
                visit_type_positions(
                    syntax,
                    *referee,
                    positions,
                    include_domain_arguments,
                    traversal,
                );
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            visit_type_positions(
                syntax,
                *base_type,
                positions,
                include_domain_arguments,
                traversal,
            );
            // The legacy synthesis walk must not capture machine-domain
            // locals. Original-scope discovery may visit these children,
            // but its caller must resolve their lexical bindings first.
            if !include_domain_arguments || traversal == TypePositionTraversal::Storage {
                return;
            }
            for constraint in syntax.type_references.constraints(*constraints) {
                if traversal == TypePositionTraversal::Source
                    && let TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } = constraint
                {
                    visit_expression_type_positions(
                        syntax,
                        *minimum,
                        positions,
                        include_domain_arguments,
                        traversal,
                    );
                    visit_expression_type_positions(
                        syntax,
                        *maximum,
                        positions,
                        include_domain_arguments,
                        traversal,
                    );
                }
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for argument in syntax
                        .type_references
                        .type_reference_handles(domain.arguments)
                    {
                        visit_type_positions(
                            syntax,
                            *argument,
                            positions,
                            include_domain_arguments,
                            traversal,
                        );
                    }
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => visit_type_positions(
            syntax,
            *element_type,
            positions,
            include_domain_arguments,
            traversal,
        ),
        TypeReferenceNode::Generic { arguments, .. }
        | TypeReferenceNode::DynamicTrait {
            trait_arguments: arguments,
            ..
        } => {
            for argument in syntax
                .tables
                .type_references
                .type_reference_handles(*arguments)
            {
                visit_type_positions(
                    syntax,
                    *argument,
                    positions,
                    include_domain_arguments,
                    traversal,
                );
            }
        }
        TypeReferenceNode::ConstExpression(expression)
            if traversal == TypePositionTraversal::Source =>
        {
            visit_expression_type_positions(
                syntax,
                *expression,
                positions,
                include_domain_arguments,
                traversal,
            );
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::Unit => {}
    }
}

fn visit_expression_type_positions(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    positions: &mut Vec<TypeReferenceHandle>,
    include_domain_arguments: bool,
    traversal: TypePositionTraversal,
) {
    if !expression.is_valid() {
        return;
    }
    let mut expressions = HashSet::new();
    collect_expression_handles(syntax, expression, &mut expressions);
    let mut expressions = expressions.into_iter().collect::<Vec<_>>();
    expressions.sort_unstable();
    for expression in expressions {
        let mut references = Vec::new();
        visit_expression_type_inputs(syntax, expression, &mut |reference| {
            references.push(reference)
        });
        for reference in references {
            visit_type_positions(
                syntax,
                reference,
                positions,
                include_domain_arguments,
                traversal,
            );
        }
    }
}

fn collect_owned_type_reference_positions(
    syntax: &SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
    include_machines: bool,
    public_only: bool,
    traversal: TypePositionTraversal,
) -> Vec<TypeReferenceHandle> {
    let mut positions: Vec<TypeReferenceHandle> = Vec::new();
    for &declaration in declarations {
        let item = syntax.root_item(declaration);
        match item {
            // SKIP the bodies of GENERIC TEMPLATES (defs/machines with type
            // parameters): their `Box<T>` fields carry the type PARAMETER as an
            // argument, not a concrete instantiation -- monomorphizing them would
            // synthesize a bogus `Box<T>` record and corrupt the template. Only
            // concrete records (incl. synthesized instances) and non-generic
            // machine bodies hold real `Box<i32>` spellings.
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
            // A const declaration's own declared type is a real carrier
            // position: `const B: Box<u64> = Box { value: 1 }` must discover
            // the closed `Box<u64>` instance exactly like a data field, or the
            // const retains a raw generic type downstream.
            Item::Const(definition) if !public_only || definition.is_public => {
                visit_type_positions(
                    syntax,
                    definition.type_reference,
                    &mut positions,
                    true,
                    traversal,
                )
            }
            Item::Machine(machine) if include_machines && machine.type_parameters.is_empty() => {
                // Conformance arguments participate in the same concrete
                // generic-data identity as the machine signature. Rewriting
                // `-> Algebra<Unit>` while leaving
                // `satisfies Trait<Algebra<Unit>>` generic makes an otherwise
                // exact requirement mismatch after instance synthesis.
                for conformance in syntax.tables.items.satisfies_clauses(machine.satisfies) {
                    for argument in syntax
                        .tables
                        .type_references
                        .type_reference_handles(conformance.arguments)
                    {
                        visit_type_positions(syntax, *argument, &mut positions, false, traversal);
                    }
                }
                for state_handle in syntax.tables.items.state_handles(machine.states) {
                    let state = syntax.tables.items.state(*state_handle);
                    visit_type_positions(
                        syntax,
                        state.return_type,
                        &mut positions,
                        false,
                        traversal,
                    );
                    for parameter_handle in syntax.tables.items.state_parameters(state.parameters) {
                        visit_type_positions(
                            syntax,
                            syntax
                                .tables
                                .items
                                .state_parameter(*parameter_handle)
                                .type_reference,
                            &mut positions,
                            false,
                            traversal,
                        );
                    }
                    for statement_handle in syntax.tables.items.statements(state.statements) {
                        if let StatementNode::LocalData(local) =
                            syntax.tables.statements.statement(*statement_handle)
                        {
                            visit_type_positions(
                                syntax,
                                local.type_reference,
                                &mut positions,
                                false,
                                traversal,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Cast targets are type-reference owners in concrete machine bodies too.
    // Rewriting the stated local while leaving `as &Pair<u32>` as a raw
    // Generic node gives downstream representation validation two identities
    // for the same synthesized instance. Walk only expressions reachable from
    // non-generic machines; generic template bodies remain deliberately open.
    if !include_machines {
        return positions;
    }
    let concrete_expressions =
        super::super::uses::selected_machine_expression_handles(syntax, declarations);
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if concrete_expressions.contains(handle)
            && let ExpressionNode::Cast(cast) = expression
        {
            visit_type_positions(syntax, cast.target_type, &mut positions, false, traversal);
        }
    }
    positions
}

/// Original declaration type positions, including open generic owners. This
/// inventory does not specialize templates: value selection must resolve each
/// operand under its original owner, and free const binders stay unevaluated.
pub(in crate::preparation::generic_data) struct SourceTraitApplication {
    pub name: syntax_trees::identifier::Identifier,
    pub arguments: HandleSpan<TypeReferenceHandle>,
}

pub(in crate::preparation::generic_data) struct LexicalTypeReferenceInventory {
    pub positions: Vec<TypeReferenceHandle>,
    pub public_positions: Vec<TypeReferenceHandle>,
    pub trait_applications: Vec<SourceTraitApplication>,
    pub invocation_arguments: Vec<TypeReferenceHandle>,
    pub container_arguments: Vec<TypeReferenceHandle>,
}

struct LexicalTypePositions<'syntax> {
    syntax: &'syntax SyntaxTrees,
    inventory: LexicalTypeReferenceInventory,
}

impl LexicalTypePositions<'_> {
    fn retain(&mut self, reference: TypeReferenceHandle, public: bool) {
        if !reference.is_valid() {
            return;
        }
        let mut positions = Vec::new();
        visit_type_positions(
            self.syntax,
            reference,
            &mut positions,
            true,
            TypePositionTraversal::Source,
        );
        for position in &positions {
            if let TypeReferenceNode::ConstExpression(expression) =
                self.syntax.type_references.type_reference(*position)
            {
                self.expression_invocation_arguments(*expression);
            }
            match self.syntax.type_references.type_reference(*position) {
                TypeReferenceNode::Generic { arguments, .. }
                | TypeReferenceNode::DynamicTrait {
                    trait_arguments: arguments,
                    ..
                } => {
                    self.inventory.container_arguments.extend_from_slice(
                        self.syntax
                            .type_references
                            .type_reference_handles(*arguments),
                    );
                }
                TypeReferenceNode::Constrained { constraints, .. } => {
                    for constraint in self.syntax.type_references.constraints(*constraints) {
                        if let TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } = constraint
                        {
                            self.expression_invocation_arguments(*minimum);
                            self.expression_invocation_arguments(*maximum);
                        }
                        if let TypeConstraintNode::Domain(domain) = constraint {
                            self.inventory.container_arguments.extend_from_slice(
                                self.syntax
                                    .type_references
                                    .type_reference_handles(domain.arguments),
                            );
                        }
                    }
                }
                _ => {}
            }
            if let TypeReferenceNode::DynamicTrait {
                trait_name,
                trait_arguments,
                ..
            } = self.syntax.type_references.type_reference(*position)
            {
                self.inventory
                    .trait_applications
                    .push(SourceTraitApplication {
                        name: trait_name.clone(),
                        arguments: *trait_arguments,
                    });
            }
        }
        if public {
            self.inventory
                .public_positions
                .extend_from_slice(&positions);
        }
        self.inventory.positions.extend(positions);
    }

    fn expressions(&mut self, expressions: HashSet<ExpressionHandle>, public: bool) {
        let mut expressions = expressions.into_iter().collect::<Vec<_>>();
        expressions.sort_unstable();
        for expression in expressions {
            if let ExpressionNode::Call(call) = self.syntax.expressions.expression(expression) {
                self.invocation_arguments(&call.machine_arguments);
            }
            let mut references = Vec::new();
            visit_expression_type_inputs(self.syntax, expression, &mut |reference| {
                references.push(reference)
            });
            for reference in references {
                self.retain(reference, public);
            }
        }
    }

    fn expression(&mut self, expression: ExpressionHandle, public: bool) {
        if !expression.is_valid() {
            return;
        }
        let mut expressions = HashSet::new();
        collect_expression_handles(self.syntax, expression, &mut expressions);
        self.expressions(expressions, public);
    }

    fn facts(&mut self, facts: HandleSpan<ProofFact>, public: bool) {
        let mut expressions = HashSet::new();
        let mut references = HashSet::new();
        collect_proof_fact_expression_handles(
            self.syntax,
            facts,
            &mut expressions,
            &mut references,
        );
        let mut references = references.into_iter().collect::<Vec<_>>();
        references.sort_unstable();
        for reference in references {
            self.retain(reference, public);
        }
        self.expressions(expressions, public);
    }

    fn contracts(&mut self, contracts: HandleSpan<CapabilityContract>, public: bool) {
        for contract in self.syntax.items.capability_contracts(contracts) {
            self.facts(contract.facts, public);
        }
    }

    fn trait_application(
        &mut self,
        name: &syntax_trees::identifier::Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
        public: bool,
    ) {
        self.inventory
            .trait_applications
            .push(SourceTraitApplication {
                name: name.clone(),
                arguments,
            });
        self.inventory.container_arguments.extend_from_slice(
            self.syntax
                .type_references
                .type_reference_handles(arguments),
        );
        for argument in self
            .syntax
            .type_references
            .type_reference_handles(arguments)
        {
            self.retain(*argument, public);
        }
    }

    fn trait_reference(&mut self, reference: TypeReferenceHandle, public: bool) {
        if !reference.is_valid() {
            return;
        }
        self.retain(reference, public);
        let original = self
            .syntax
            .type_references
            .generic_application_origin(reference);
        let application = if original.is_valid() {
            original
        } else {
            reference
        };
        if let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = self.syntax.type_references.type_reference(application)
        {
            self.trait_application(base_name, *arguments, public);
        }
    }

    fn parameters(
        &mut self,
        parameters: HandleSpan<syntax_trees::item::TypeParameter>,
        public: bool,
    ) {
        use syntax_trees::item::MachineParameterContract;
        for parameter in self.syntax.items.type_parameters(parameters) {
            match &parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    self.retain(*type_reference, public)
                }
                TypeParameterKind::Machine {
                    contract: Some(MachineParameterContract::Structural(signature)),
                } => self.signature(signature, public),
                TypeParameterKind::Machine {
                    contract:
                        Some(MachineParameterContract::Nominal {
                            requirement,
                            trait_arguments,
                        }),
                } => {
                    let members = self.syntax.items.identifier_path_members(*requirement);
                    if members.len() >= 2 {
                        let first = &members[0];
                        let last = &members[members.len() - 2];
                        let mut span = first.source_span();
                        span.span.end = last.source_span().span.end;
                        let name = members[..members.len() - 1]
                            .iter()
                            .map(|member| member.as_str())
                            .collect::<Vec<_>>()
                            .join("::");
                        self.trait_application(
                            &syntax_trees::identifier::Identifier::new(name, span),
                            *trait_arguments,
                            public,
                        );
                    } else {
                        for argument in self
                            .syntax
                            .type_references
                            .type_reference_handles(*trait_arguments)
                        {
                            self.retain(*argument, public);
                        }
                    }
                }
                TypeParameterKind::Proposition {
                    contract: Some(signature),
                } => {
                    for parameter in self.syntax.items.state_parameters(signature.parameters) {
                        self.retain(
                            self.syntax.items.state_parameter(*parameter).type_reference,
                            public,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn expression_invocation_arguments(&mut self, expression: ExpressionHandle) {
        if !expression.is_valid() {
            return;
        }
        let mut expressions = HashSet::new();
        collect_expression_handles(self.syntax, expression, &mut expressions);
        for expression in expressions {
            if let ExpressionNode::Call(call) = self.syntax.expressions.expression(expression) {
                self.invocation_arguments(&call.machine_arguments);
            }
        }
    }

    fn invocation_arguments(&mut self, arguments: &[StaticMachineArgument]) {
        for argument in arguments {
            if argument.type_reference.is_valid() {
                self.inventory
                    .invocation_arguments
                    .push(argument.type_reference);
            }
            if let Some(application) = &argument.application {
                self.invocation_arguments(&application.arguments);
            }
        }
    }

    fn statements(&mut self, statements: HandleSpan<syntax_trees::statement::StatementHandle>) {
        for statement in self.syntax.items.statements(statements) {
            if let StatementNode::Call(call) = self.syntax.statements.statement(*statement) {
                self.invocation_arguments(&call.machine_arguments);
            }
            let mut expressions = HashSet::new();
            collect_statement_expression_handles(self.syntax, *statement, &mut expressions);
            self.expressions(expressions, false);
            for reference in
                crate::preparation::generic_data::statement_type_inputs(self.syntax, *statement)
            {
                self.retain(reference, false);
            }
        }
    }

    fn signature(&mut self, signature: &syntax_trees::item::StateSignature, public: bool) {
        self.parameters(signature.type_parameters, public);
        self.retain(signature.return_type, public);
        for parameter in self.syntax.items.state_parameters(signature.parameters) {
            self.retain(
                self.syntax.items.state_parameter(*parameter).type_reference,
                public,
            );
        }
        self.contracts(signature.contracts, public);
        self.facts(signature.where_facts, public);
        self.statements(signature.default_body);
    }

    fn operator(&mut self, operator: &syntax_trees::item::OperatorDefinition) {
        self.parameters(operator.type_parameters, operator.is_public);
        self.retain(operator.return_type, operator.is_public);
        for parameter in self.syntax.items.state_parameters(operator.parameters) {
            self.retain(
                self.syntax.items.state_parameter(*parameter).type_reference,
                operator.is_public,
            );
        }
        self.contracts(operator.contracts, operator.is_public);
    }

    fn machine(&mut self, machine: &Machine) {
        self.parameters(machine.type_parameters, machine.is_public);
        for bound in &machine.conformance_bounds {
            self.trait_application(&bound.trait_name, bound.arguments, machine.is_public);
        }
        for clause in self.syntax.items.satisfies_clauses(machine.satisfies) {
            self.trait_application(&clause.trait_name, clause.arguments, machine.is_public);
            self.expression(clause.via_expression, machine.is_public);
        }
        self.facts(machine.where_facts, machine.is_public);
        self.contracts(machine.contracts, machine.is_public);
        for expression in self
            .syntax
            .expressions
            .expression_handles(machine.ranking_subjects)
        {
            self.expression(*expression, false);
        }
        for expression in self
            .syntax
            .expressions
            .expression_handles(machine.ranking_view_arguments)
        {
            self.expression(*expression, false);
        }
        self.expression(machine.ranking_range, false);
        for (ordinal, state) in self
            .syntax
            .items
            .state_handles(machine.states)
            .iter()
            .enumerate()
        {
            let state = self.syntax.items.state(*state);
            let public = ordinal == 0 && machine.is_public;
            self.retain(state.return_type, public);
            for parameter in self.syntax.items.state_parameters(state.parameters) {
                self.retain(
                    self.syntax.items.state_parameter(*parameter).type_reference,
                    public,
                );
            }
            self.contracts(state.contracts, public);
            self.statements(state.statements);
        }
    }

    fn mathematical_type(
        &mut self,
        reference: syntax_trees::item::MathematicalTypeHandle,
        public: bool,
        visited: &mut Vec<syntax_trees::item::MathematicalTypeHandle>,
    ) {
        if !reference.is_valid() || visited.contains(&reference) {
            return;
        }
        visited.push(reference);
        match self.syntax.items.mathematical_type(reference) {
            syntax_trees::item::MathematicalTypeNode::Ordinary(reference) => {
                self.retain(*reference, public)
            }
            syntax_trees::item::MathematicalTypeNode::Arrow {
                domain, codomain, ..
            } => {
                self.mathematical_type(*domain, public, visited);
                self.mathematical_type(*codomain, public, visited);
            }
            syntax_trees::item::MathematicalTypeNode::Application { callee, arguments } => {
                self.mathematical_type(*callee, public, visited);
                for expression in self.syntax.expressions.expression_handles(*arguments) {
                    self.expression(*expression, public);
                }
            }
        }
    }
}

pub(in crate::preparation::generic_data) fn collect_lexical_type_reference_inventory(
    syntax: &SyntaxTrees,
) -> LexicalTypeReferenceInventory {
    use syntax_trees::item::{CapabilityMember, ConformanceMember, PropositionBody};
    let mut collector = LexicalTypePositions {
        syntax,
        inventory: LexicalTypeReferenceInventory {
            positions: Vec::new(),
            public_positions: Vec::new(),
            trait_applications: Vec::new(),
            invocation_arguments: Vec::new(),
            container_arguments: Vec::new(),
        },
    };
    for item in syntax.root_items() {
        match item {
            Item::Data(definition) => {
                collector.parameters(definition.type_parameters, definition.is_public);
                collector.facts(definition.where_facts, definition.is_public);
                if let Some(quotient) = &definition.quotient {
                    collector.retain(quotient.carrier, definition.is_public);
                    if let Some(equivalence) = &quotient.equivalence {
                        collector.trait_application(
                            &equivalence.trait_name,
                            equivalence.trait_arguments,
                            definition.is_public,
                        );
                    }
                }
                for member in syntax.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => {
                            collector.retain(field.type_reference, definition.is_public)
                        }
                        DataMember::Variant(variant) => {
                            for field in syntax.items.data_payload_fields(variant.payload) {
                                collector.retain(field.type_reference, definition.is_public);
                            }
                            collector.facts(variant.where_facts, definition.is_public);
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Const(definition) => {
                collector.retain(definition.type_reference, definition.is_public);
                collector.expression(definition.value, definition.is_public);
            }
            Item::Operator(operator) => collector.operator(operator),
            Item::Domain(definition) => {
                collector.parameters(definition.type_parameters, definition.is_public);
                collector.retain(definition.target_type, definition.is_public);
                for argument in syntax
                    .type_references
                    .type_reference_handles(definition.index_arguments)
                {
                    collector.retain(*argument, definition.is_public);
                }
                collector.facts(definition.facts, definition.is_public);
                for operator in syntax.items.operators(definition.operators) {
                    collector.operator(operator);
                }
            }
            Item::Trait(definition) => {
                collector.parameters(definition.type_parameters, definition.is_public);
                for bound in &definition.conformance_bounds {
                    collector.trait_application(
                        &bound.trait_name,
                        bound.arguments,
                        definition.is_public,
                    );
                }
                for parent in syntax
                    .type_references
                    .type_reference_handles(definition.parents)
                {
                    collector.trait_reference(*parent, definition.is_public);
                }
                if let Some(refines) = definition.refines {
                    collector.trait_reference(refines, definition.is_public);
                }
                for signature in syntax.items.state_signatures(definition.machines) {
                    collector.signature(
                        syntax.items.state_signature(*signature),
                        definition.is_public,
                    );
                }
                for clause in &definition.refinement_clauses {
                    collector.signature(&clause.signature, definition.is_public);
                }
            }
            Item::Conformance(definition) => {
                collector.parameters(definition.type_parameters, definition.is_public);
                collector.trait_application(
                    &definition.trait_name,
                    definition.trait_arguments,
                    definition.is_public,
                );
                for member in syntax.items.conformance_members(definition.members) {
                    match member {
                        ConformanceMember::Machine(machine)
                        | ConformanceMember::Synthesized(machine)
                        | ConformanceMember::TraitDefault { machine, .. } => {
                            collector.machine(machine)
                        }
                        ConformanceMember::Reference { .. } => {}
                    }
                }
            }
            Item::Machine(machine) => collector.machine(machine),
            Item::Proposition(definition) => {
                collector.parameters(definition.type_parameters, definition.is_public);
                for parameter in syntax.items.state_parameters(definition.parameters) {
                    collector.retain(
                        syntax.items.state_parameter(*parameter).type_reference,
                        definition.is_public,
                    );
                }
                match &definition.body {
                    PropositionBody::Witness { evidence } => {
                        collector.retain(*evidence, definition.is_public)
                    }
                    PropositionBody::Transparent { proposition } => {
                        collector.expression(*proposition, definition.is_public)
                    }
                    PropositionBody::Primitive => {}
                }
            }
            Item::Measure(definition) => {
                collector.retain(
                    syntax
                        .items
                        .state_parameter(definition.parameter)
                        .type_reference,
                    false,
                );
                collector.retain(definition.return_type, false);
                for expression in syntax.expressions.expression_handles(definition.body) {
                    collector.expression(*expression, false);
                }
            }
            Item::Capability(definition) => {
                for member in syntax.items.capability_members(definition.members) {
                    match member {
                        CapabilityMember::Field(field) => {
                            collector.retain(field.type_reference, false)
                        }
                        CapabilityMember::State(state) => {
                            collector.signature(&state.signature, false);
                            collector.contracts(state.contracts, false);
                        }
                    }
                }
            }
            Item::Module(_) | Item::Package(_) | Item::Use(_) => {}
        }
    }
    for handle in syntax.root_mathematical_definition_handles() {
        let definition = syntax.items.mathematical_definition(*handle);
        collector.parameters(definition.type_parameters, definition.is_public);
        let mut visited = Vec::new();
        for parameter in syntax.items.mathematical_parameters(definition.parameters) {
            collector.mathematical_type(
                syntax.items.mathematical_parameter(*parameter).ty,
                definition.is_public,
                &mut visited,
            );
        }
        collector.mathematical_type(definition.result, definition.is_public, &mut visited);
        if let syntax_trees::item::MathematicalDefinitionBody::Definition(expression) =
            &definition.body
        {
            collector.expression(*expression, definition.is_public);
        }
    }
    collector.inventory
}
