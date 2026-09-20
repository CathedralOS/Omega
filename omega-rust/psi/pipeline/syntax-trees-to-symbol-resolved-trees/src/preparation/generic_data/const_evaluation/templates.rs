//! Constant evaluation: templates.
use super::super::{
    DataMember, Diagnostic, ExpressionNode, HashMap, HashSet, Identifier, Item, StatementNode,
    SyntaxTrees, TypeConstraintNode, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};

use crate::preparation::generic_data::ConstructorFrontier;
use crate::preparation::generic_data::EvaluatedConst;
use crate::preparation::generic_data::collect_expression_handles;
use crate::preparation::generic_data::collect_statement_expression_handles;
use crate::preparation::generic_data::concrete_machine_expression_handles;
use crate::preparation::generic_data::evaluate_const_argument_expression;
use crate::preparation::generic_data::generic_const_integer_types;
use arena::HandleSpan;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::StaticMachineArgument;
use syntax_trees::item::CapabilityContract;
use syntax_trees::item::Machine;
use syntax_trees::item::ProofFact;

pub(in crate::preparation::generic_data) fn replace_const_expression_names_from(
    syntax: &mut SyntaxTrees,
    expression_watermark: u32,
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
    type_watermark: u32,
    expression_watermark: u32,
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
        if handle.arena_index() >= expression_watermark
            && let ExpressionNode::Name(path) = expression
            && header_frontier.captures(syntax, *path)
        {
            captured.expressions.insert(handle);
        }
    }
    for (handle, _) in syntax.type_references.named_nodes_from(type_watermark) {
        if let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(handle)
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
    expression_watermark: u32,
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
            match syntax.expressions.expression(handle) {
                ExpressionNode::Cast(cast) => {
                    collect_type_expression_handles(
                        syntax,
                        cast.target_type,
                        expressions,
                        type_references,
                    );
                    for argument in syntax
                        .type_references
                        .type_reference_handles(cast.semantic_domain_arguments)
                    {
                        collect_type_expression_handles(
                            syntax,
                            *argument,
                            expressions,
                            type_references,
                        );
                    }
                }
                ExpressionNode::TypeExpression(reference)
                | ExpressionNode::ZeroValue(reference) => {
                    collect_type_expression_handles(
                        syntax,
                        *reference,
                        expressions,
                        type_references,
                    );
                }
                ExpressionNode::Call(call) => {
                    collect_static_argument_expression_handles(
                        syntax,
                        &call.machine_arguments,
                        expressions,
                        type_references,
                    );
                }
                _ => {}
            }
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
        if let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(*reference) {
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
    for argument in arguments {
        if argument.type_reference.is_valid() {
            collect_type_expression_handles(
                syntax,
                argument.type_reference,
                expressions,
                type_references,
            );
        }
        if let Some(application) = &argument.application {
            collect_static_argument_expression_handles(
                syntax,
                &application.arguments,
                expressions,
                type_references,
            );
        }
    }
}

fn collect_contract_expression_handles(
    syntax: &SyntaxTrees,
    contracts: HandleSpan<CapabilityContract>,
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
) {
    for contract in syntax.items.capability_contracts(contracts) {
        for fact in syntax.items.proof_facts(contract.facts) {
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
        TypeReferenceNode::Generic { arguments, .. } => {
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
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => {}
    }
}

fn replace_uncaptured_const_names(
    syntax: &mut SyntaxTrees,
    expression_watermark: u32,
    const_expressions: &HashMap<String, ExpressionNode>,
    captured: &HashSet<ExpressionHandle>,
) {
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() >= expression_watermark)
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

/// Generic definitions remain in the tree after their concrete records are
/// synthesized so the normal frontend can validate the template. A symbolic
/// const expression cannot cross that boundary yet, so reduce each template
/// expression to either its concrete value or one declared const-parameter
/// dependency. The concrete clones already carry the fully evaluated value;
/// this placeholder exists only to preserve the established generic type/kind
/// checks on the source template.
pub(in crate::preparation::generic_data) fn normalize_generic_template_const_expressions(
    syntax: &mut SyntaxTrees,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    let templates: Vec<(String, HashSet<String>, Vec<TypeReferenceHandle>)> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
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
                    let placeholder = evaluate_const_argument_expression(
                        syntax,
                        expression,
                        const_values,
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
                        const_values,
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
                const_values,
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
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => Ok(()),
    }
}

/// Every TYPE-REFERENCE position a generic-data spelling can appear in: data
/// FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN types. Run
/// afresh each fixpoint round so newly-synthesized records' fields are seen.
pub(in crate::preparation::generic_data) fn collect_type_reference_positions(
    syntax: &SyntaxTrees,
) -> Vec<TypeReferenceHandle> {
    collect_owned_type_reference_positions(syntax, true, false)
}

pub(in crate::preparation::generic_data) fn collect_data_type_reference_positions(
    syntax: &SyntaxTrees,
    public_only: bool,
) -> Vec<TypeReferenceHandle> {
    collect_owned_type_reference_positions(syntax, false, public_only)
}

pub(in crate::preparation) fn collect_type_positions(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    positions: &mut Vec<TypeReferenceHandle>,
    include_domain_arguments: bool,
) {
    positions.push(type_reference);
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_positions(syntax, *referee, positions, include_domain_arguments)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_positions(syntax, *base_type, positions, include_domain_arguments);
            // The legacy synthesis walk must not capture machine-domain
            // locals. Original-scope discovery may visit these children,
            // but its caller must resolve their lexical bindings first.
            if !include_domain_arguments {
                return;
            }
            for constraint in syntax.type_references.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for argument in syntax
                        .type_references
                        .type_reference_handles(domain.arguments)
                    {
                        collect_type_positions(
                            syntax,
                            *argument,
                            positions,
                            include_domain_arguments,
                        );
                    }
                }
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_positions(syntax, *element_type, positions, include_domain_arguments)
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in syntax
                .tables
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_type_positions(syntax, *argument, positions, include_domain_arguments);
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => {}
    }
}

fn collect_owned_type_reference_positions(
    syntax: &SyntaxTrees,
    include_machines: bool,
    public_only: bool,
) -> Vec<TypeReferenceHandle> {
    let mut positions: Vec<TypeReferenceHandle> = Vec::new();
    for item in syntax.root_items() {
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
                        DataMember::Field(field) => collect_type_positions(
                            syntax,
                            field.type_reference,
                            &mut positions,
                            true,
                        ),
                        DataMember::Variant(variant) => {
                            for field in syntax.tables.items.data_payload_fields(variant.payload) {
                                collect_type_positions(
                                    syntax,
                                    field.type_reference,
                                    &mut positions,
                                    true,
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
                collect_type_positions(syntax, definition.type_reference, &mut positions, true)
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
                        collect_type_positions(syntax, *argument, &mut positions, false);
                    }
                }
                for state_handle in syntax.tables.items.state_handles(machine.states) {
                    let state = syntax.tables.items.state(*state_handle);
                    collect_type_positions(syntax, state.return_type, &mut positions, false);
                    for parameter_handle in syntax.tables.items.state_parameters(state.parameters) {
                        collect_type_positions(
                            syntax,
                            syntax
                                .tables
                                .items
                                .state_parameter(*parameter_handle)
                                .type_reference,
                            &mut positions,
                            false,
                        );
                    }
                    for statement_handle in syntax.tables.items.statements(state.statements) {
                        if let StatementNode::LocalData(local) =
                            syntax.tables.statements.statement(*statement_handle)
                        {
                            collect_type_positions(
                                syntax,
                                local.type_reference,
                                &mut positions,
                                false,
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
    let concrete_expressions = concrete_machine_expression_handles(syntax);
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if concrete_expressions.contains(&handle.arena_index())
            && let ExpressionNode::Cast(cast) = expression
        {
            collect_type_positions(syntax, cast.target_type, &mut positions, false);
        }
    }
    positions
}

/// Type owners admitted for original-scope machine constant selection. Public
/// exposure belongs only to the entry signature, never local/state storage.
pub(in crate::preparation::generic_data) fn collect_machine_type_reference_positions(
    syntax: &SyntaxTrees,
) -> (Vec<TypeReferenceHandle>, Vec<TypeReferenceHandle>) {
    let mut positions = Vec::new();
    let mut public_positions = Vec::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        if !machine.type_parameters.is_empty() {
            continue;
        }
        for (ordinal, state_handle) in syntax
            .items
            .state_handles(machine.states)
            .iter()
            .enumerate()
        {
            let state = syntax.items.state(*state_handle);
            let start = positions.len();
            collect_type_positions(syntax, state.return_type, &mut positions, true);
            for parameter in syntax.items.state_parameters(state.parameters) {
                collect_type_positions(
                    syntax,
                    syntax.items.state_parameter(*parameter).type_reference,
                    &mut positions,
                    true,
                );
            }
            if ordinal == 0 && machine.is_public {
                public_positions.extend_from_slice(&positions[start..]);
            }
            for statement in syntax.items.statements(state.statements) {
                if let StatementNode::LocalData(local) = syntax.statements.statement(*statement) {
                    collect_type_positions(syntax, local.type_reference, &mut positions, true);
                }
            }
        }
    }
    let concrete_expressions = concrete_machine_expression_handles(syntax);
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if concrete_expressions.contains(&handle.arena_index())
            && let ExpressionNode::Cast(cast) = expression
        {
            collect_type_positions(syntax, cast.target_type, &mut positions, true);
            for argument in syntax
                .type_references
                .type_reference_handles(cast.semantic_domain_arguments)
            {
                collect_type_positions(syntax, *argument, &mut positions, true);
            }
        }
    }
    (positions, public_positions)
}
