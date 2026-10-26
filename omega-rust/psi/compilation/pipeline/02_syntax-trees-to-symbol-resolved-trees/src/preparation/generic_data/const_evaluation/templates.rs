//! Constant evaluation: templates.
mod captures;
use super::super::{
    DataMember, Diagnostic, ExpressionNode, HashMap, HashSet, Identifier, Item, StatementNode,
    SyntaxTrees, TypeConstraintNode, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};
use crate::preparation::generic_data::uses::{
    ConstructorFrontier, collect_expression_handles, collect_statement_expression_handles,
    concrete_machine_expression_handles, visit_expression_type_inputs,
    visit_static_argument_type_inputs,
};
pub(in crate::preparation::generic_data) use captures::{
    RuntimeTemplateCaptures, capture_domain_machine_names, capture_machine_runtime_template_names,
};

use crate::preparation::generic_data::EvaluatedConst;

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
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            collect_type_expression_handles(syntax, *element_type, expressions, type_references);
            if let syntax_trees::types::FixedArrayLength::Expression(value) = length {
                collect_type_expression_handles(syntax, *value, expressions, type_references);
            }
        }
        TypeReferenceNode::Slice { element_type } => {
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
                .get(name.as_str(syntax))
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
                .map(|parameter| parameter.name.as_str(syntax).to_string())
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
            let integer_types = generic_const_integer_types(syntax, base_name.as_str(syntax));
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
    positions.push(type_reference);
    match syntax.tables.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            if traversal == TypePositionTraversal::Identity {
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
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            visit_type_positions(
                syntax,
                *element_type,
                positions,
                include_domain_arguments,
                traversal,
            );
            if let syntax_trees::types::FixedArrayLength::Expression(value) = length {
                visit_type_positions(
                    syntax,
                    *value,
                    positions,
                    include_domain_arguments,
                    traversal,
                );
            }
        }
        TypeReferenceNode::Slice { element_type } => visit_type_positions(
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
        TypeReferenceNode::ConstExpression(expression) => {
            // A static expression can itself call an applied machine. Its
            // argument positions belong to the same concrete owner and must
            // settle before the enclosing invocation can execute.
            let mut expressions = HashSet::new();
            super::super::uses::collect_expression_handles(syntax, *expression, &mut expressions);
            for expression in expressions {
                visit_expression_type_inputs(syntax, expression, &mut |reference| {
                    visit_type_positions(
                        syntax,
                        reference,
                        positions,
                        include_domain_arguments,
                        traversal,
                    );
                });
            }
        }
        TypeReferenceNode::Named(_) | TypeReferenceNode::Unit => {}
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
    for machine in super::super::uses::concrete_machine_declarations(syntax, declarations) {
        // Satisfier arguments and signatures select the same closed carrier.
        // All states, including inline conformance states, own their type uses.
        for conformance in syntax.items.satisfies_clauses(machine.satisfies) {
            for argument in syntax
                .type_references
                .type_reference_handles(conformance.arguments)
            {
                visit_type_positions(syntax, *argument, &mut positions, false, traversal);
            }
        }
        for state_handle in syntax.items.state_handles(machine.states) {
            let state = syntax.items.state(*state_handle);
            visit_type_positions(syntax, state.return_type, &mut positions, false, traversal);
            for parameter in syntax.items.state_parameters(state.parameters) {
                visit_type_positions(
                    syntax,
                    syntax.items.state_parameter(*parameter).type_reference,
                    &mut positions,
                    false,
                    traversal,
                );
            }
            for statement in syntax.items.statements(state.statements) {
                for reference in super::super::uses::statement_type_inputs(syntax, *statement) {
                    visit_type_positions(syntax, reference, &mut positions, false, traversal);
                }
            }
        }
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

/// Type owners admitted for original-scope machine constant selection. Public
/// exposure belongs only to the entry signature, never local/state storage.
pub(in crate::preparation::generic_data) fn collect_machine_type_reference_positions(
    syntax: &SyntaxTrees,
) -> (Vec<TypeReferenceHandle>, Vec<TypeReferenceHandle>) {
    let mut positions = Vec::new();
    let mut public_positions = Vec::new();
    for machine in
        super::super::uses::concrete_machine_declarations(syntax, syntax.root_item_handles())
    {
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
                for reference in super::super::uses::statement_type_inputs(syntax, *statement) {
                    collect_type_positions(syntax, reference, &mut positions, true);
                }
            }
        }
    }
    let concrete_expressions = concrete_machine_expression_handles(syntax);
    for (handle, _) in syntax.expressions.iter_expressions() {
        if concrete_expressions.contains(handle) {
            visit_expression_type_inputs(syntax, handle, &mut |reference| {
                collect_type_positions(syntax, reference, &mut positions, true);
            });
        }
    }
    (positions, public_positions)
}
