//! Closed machine applications discharge the same structural equations as data.
//!
//! Ordinary provisional resolution supplies exact selected declaration handles.
//! The lowering pass records private source links while producing those handles;
//! source coordinates and leaf spellings never reconstruct call identity. This
//! bounded preparation route completes explicit static tuples, including full
//! tuples, before ordinary checked monomorphization. Open applications remain
//! obligations and reject rather than entering later inference unchecked.

use super::{
    generic_data::{constant_selection::ConstantSelection, module_constants},
    type_equations::{self, EquationTemplate},
};
use arena::Handle;
use diagnostics::Diagnostic;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::{
    SyntaxTrees,
    expression::StaticMachineArgument,
    item::{Item, Machine, TypeParameterKind},
    types::{TypeReferenceHandle, TypeReferenceNode},
};

#[derive(Default)]
pub(crate) struct SourceLinks {
    pub(crate) declarations: Vec<(usize, Machine)>,
    pub(crate) expressions: Vec<(
        syntax_trees::expression::ExpressionHandle,
        symbol_resolved_trees::expression::ExpressionHandle,
    )>,
    pub(crate) statements: Vec<(
        syntax_trees::statement::StatementHandle,
        Handle<symbol_resolved_trees::statement::Statement>,
    )>,
}

pub(crate) fn required(syntax: &SyntaxTrees, base: Option<&SymbolResolvedTrees>) -> bool {
    syntax
        .root_items()
        .any(|item| matches!(item, Item::Machine(machine) if !machine.where_facts.is_empty()))
        || base.is_some_and(|base| {
            base.machines
                .iter()
                .any(|machine| machine.has_structural_type_equations)
        })
}

pub(crate) fn validate_declarations(syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        if machine.where_facts.is_empty() {
            continue;
        }
        if machine.attached_data.is_some()
            || machine.spelling.is_some()
            || !machine.satisfies.is_empty()
        {
            return Err(vec![Diagnostic::error("machine structural equations currently require explicit calls to a free machine; attached, operator, and conformance supplies retain undisclosed equations").with_source_span(machine.name.source_span())]);
        }
        if machine.type_parameters.is_empty() {
            return Err(vec![
                Diagnostic::error("machine structural equations require a generic declaration")
                    .with_source_span(machine.name.source_span()),
            ]);
        }
        let equations = type_equations::classify_type_equations(
            syntax,
            syntax.items.type_parameters(machine.type_parameters),
            syntax.items.proof_facts(machine.where_facts),
        );
        if equations.len() != machine.where_facts.count() as usize {
            return Err(vec![Diagnostic::error("machine where clause requires a structural type equation or a named conformance bound").with_source_span(machine.name.source_span())]);
        }
        for equation in equations {
            equation
                .validate_kind("machine")
                .map_err(|error| vec![error])?;
        }
    }
    Ok(())
}

fn selected_machine(trees: &SymbolResolvedTrees, symbol: symbols::SymbolHandle) -> Option<usize> {
    let declaration = trees.symbols.get(symbol);
    let owner = match declaration.kind {
        symbols::SymbolKind::Machine => symbol,
        symbols::SymbolKind::State => declaration.parent,
        _ => return None,
    };
    trees
        .machines
        .iter()
        .position(|machine| machine.symbol == owner)
}

pub(crate) fn complete(
    syntax: &mut SyntaxTrees,
    trees: &SymbolResolvedTrees,
    sources: SourceLinks,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    for bound in trees
        .machines
        .iter()
        .flat_map(|machine| &machine.conformance_bounds)
        .chain(
            trees
                .traits
                .iter()
                .flat_map(|definition| &definition.conformance_bounds),
        )
    {
        if let Some(argument) = &bound.selected_conformance {
            reject_machine_arguments(trees, std::slice::from_ref(argument))?;
        }
    }
    for conformance in &trees.conformances {
        if let symbol_resolved_trees::trait_definition::ConformanceImplementation::Closed { rows } =
            &conformance.implementation
        {
            for row in rows {
                if [row.realization_machine, row.realization_state]
                    .into_iter()
                    .any(|symbol| {
                        selected_machine(trees, symbol).is_some_and(|index| {
                            trees.machines[index].has_structural_type_equations
                        })
                    })
                {
                    return Err(vec![Diagnostic::error(
                        "conformance realization retains an undisclosed machine structural equation",
                    )]);
                }
            }
        }
    }
    for (original, resolved) in &sources.expressions {
        match trees.tables.bodies.expressions.expression(*resolved) {
            symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
                reject_machine_arguments(trees, &call.machine_arguments)?;
                let Some(index) = selected_machine(trees, call.target_symbol) else {
                    continue;
                };
                if !trees.machines[index].has_structural_type_equations {
                    continue;
                }
                require_closed_selected_arguments(trees, &call.machine_arguments)?;
                let syntax_trees::expression::ExpressionNode::Call(mut authored) =
                    syntax.expressions.expression(*original).clone()
                else {
                    return Err(vec![Diagnostic::error(
                        "machine equation application lost its authored call",
                    )]);
                };
                authored.machine_arguments = complete_arguments(
                    syntax,
                    index,
                    &authored.target,
                    &authored.machine_arguments,
                    &sources,
                    selection,
                )
                .map_err(|error| vec![error])?;
                syntax.expressions.replace_expression(
                    *original,
                    syntax_trees::expression::ExpressionNode::Call(authored),
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::Name(path)
                if selected_machine(trees, path.symbol)
                    .is_some_and(|index| trees.machines[index].has_structural_type_equations) =>
            {
                return Err(vec![Diagnostic::error(
                    "machine with structural equations requires a closed explicit call; noncall selection retains an unsolved equation",
                )]);
            }
            _ => {}
        }
    }
    for (original, resolved) in &sources.statements {
        let symbol_resolved_trees::statement::Statement::Call(call) =
            trees.tables.declarations.state_statements.get(*resolved)
        else {
            continue;
        };
        reject_machine_arguments(trees, &call.machine_arguments)?;
        let Some(index) = selected_machine(trees, call.target_symbol) else {
            continue;
        };
        if !trees.machines[index].has_structural_type_equations {
            continue;
        }
        require_closed_selected_arguments(trees, &call.machine_arguments)?;
        let syntax_trees::statement::StatementNode::Call(mut authored) =
            syntax.statements.statement(*original).clone()
        else {
            return Err(vec![Diagnostic::error(
                "machine equation application lost its authored statement",
            )]);
        };
        authored.machine_arguments = complete_arguments(
            syntax,
            index,
            &authored.target,
            &authored.machine_arguments,
            &sources,
            selection,
        )
        .map_err(|error| vec![error])?;
        syntax.statements.replace_statement(
            *original,
            syntax_trees::statement::StatementNode::Call(authored),
        );
    }
    Ok(())
}

fn reject_machine_arguments(
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    for argument in arguments {
        if selected_machine(trees, argument.symbol)
            .is_some_and(|index| trees.machines[index].has_structural_type_equations)
        {
            return Err(vec![Diagnostic::error(
                "machine structural equation remains unsolved in a noncall static argument",
            )]);
        }
        if let Some(application) = &argument.application {
            reject_machine_arguments(trees, &application.arguments)?;
        }
    }
    Ok(())
}

/// Syntax identity must not reinterpret an exact caller binder as a same-named
/// global declaration. Inspect the selected type tree before structural matching;
/// this is a closedness fence, not a second equation or type inference solver.
fn require_closed_selected_arguments(
    trees: &SymbolResolvedTrees,
    arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    use symbol_resolved_trees::types::{FixedArrayLength, TypeReference};
    let is_binder = |symbol| {
        matches!(
            trees.symbols.get(symbol).kind,
            symbols::SymbolKind::TypeParameter
                | symbols::SymbolKind::Parameter
                | symbols::SymbolKind::MachineParameter
                | symbols::SymbolKind::ConformanceParameter
        )
    };
    let reject = || {
        vec![Diagnostic::error(
            "machine equation retains an open caller binder; supply a closed explicit static argument",
        )]
    };
    let mut pending = Vec::new();
    for argument in arguments {
        if is_binder(argument.symbol) {
            return Err(reject());
        }
        if argument.type_reference.is_valid() {
            pending.push(argument.type_reference);
        }
    }
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        match trees.child_type_reference(handle) {
            TypeReference::Named { symbol, .. }
            | TypeReference::SelfType { symbol }
            | TypeReference::DynamicTrait { symbol, .. } => {
                if is_binder(*symbol) {
                    return Err(reject());
                }
            }
            TypeReference::Reference(reference) => pending.push(reference.referee),
            TypeReference::Slice(slice) => pending.push(slice.element_type),
            TypeReference::FixedArray(array) => {
                if !matches!(array.length, FixedArrayLength::Literal(_)) {
                    return Err(reject());
                }
                pending.push(array.element_type);
            }
            TypeReference::Constrained(constrained) => {
                pending.push(constrained.base_type);
                for constraint in trees
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                {
                    if let symbol_resolved_trees::types::TypeConstraint::Domain(domain) = constraint
                    {
                        append_type_children(domain.arguments, &mut pending);
                    }
                }
            }
            TypeReference::Generic(generic) => {
                if is_binder(generic.base_symbol) {
                    return Err(reject());
                }
                append_type_children(generic.arguments, &mut pending);
            }
            TypeReference::ConstExpression(_) => return Err(reject()),
            TypeReference::Unit => {}
        }
    }
    Ok(())
}

fn append_type_children(
    span: arena::HandleSpan<symbol_resolved_trees::types::TypeReference>,
    pending: &mut Vec<Handle<symbol_resolved_trees::types::TypeReference>>,
) {
    for offset in 0..span.count() {
        pending.push(Handle::from_parts(
            span.start().arena_index() + offset,
            span.start().generation(),
        ));
    }
}

fn complete_arguments(
    syntax: &mut SyntaxTrees,
    selected: usize,
    target: &syntax_trees::identifier::Identifier,
    supplied: &[StaticMachineArgument],
    sources: &SourceLinks,
    selection: &ConstantSelection<'_>,
) -> Result<Box<[StaticMachineArgument]>, Diagnostic> {
    let Some((_, declaration)) = sources
        .declarations
        .iter()
        .find(|(index, _)| *index == selected)
    else {
        return Err(Diagnostic::error(
            "retained machine structural equation requires its original syntax for discharge",
        )
        .with_source_span(target.source_span()));
    };
    let parameters = syntax
        .items
        .type_parameters(declaration.type_parameters)
        .to_vec();
    let equations = type_equations::classify_type_equations(
        syntax,
        &parameters,
        syntax.items.proof_facts(declaration.where_facts),
    );
    if equations.len() != declaration.where_facts.count() as usize {
        return Err(Diagnostic::error(
            "machine where clause requires a structural type equation or a named conformance bound",
        )
        .with_source_span(target.source_span()));
    }
    if supplied.len() > parameters.len() {
        return Err(Diagnostic::error(
            "machine equation application supplies too many static arguments",
        )
        .with_source_span(target.source_span()));
    }
    let parameter_names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut const_parameter_types = Vec::new();
    for parameter in &parameters {
        const_parameter_types.push(match parameter.kind {
            TypeParameterKind::Type => None,
            TypeParameterKind::Const { type_reference } => Some(type_reference),
            _ => {
                return Err(Diagnostic::error(
                    "machine equation completion requires type and const binders",
                )
                .with_source_span(target.source_span()));
            }
        });
    }
    let mut arguments = Vec::new();
    for (argument, parameter) in supplied.iter().zip(&parameters) {
        let reference = match &parameter.kind {
            TypeParameterKind::Type if argument.type_reference.is_valid() => argument.type_reference,
            TypeParameterKind::Type if argument.const_literal.is_none() && argument.application.is_none() && argument.evidence_projection.is_none() && !argument.path.is_empty() => {
                let name = argument.path.iter().map(|part| part.as_str()).collect::<Vec<_>>().join("::");
                syntax.type_references.insert(TypeReferenceNode::Named(syntax_trees::identifier::Identifier::new(name, argument.path[0].source_span())))
            }
            TypeParameterKind::Const { .. } if argument.const_literal.is_some() => {
                let value = argument.const_literal.as_ref().and_then(IntegerLiteral::value_bignum).ok_or_else(|| Diagnostic::error("machine equation const argument is not an exact integer"))?;
                syntax.type_references.insert(TypeReferenceNode::Named(syntax_trees::identifier::Identifier::generated(value.to_string())))
            }
            _ => return Err(Diagnostic::error("machine equation argument mixes type and value kinds or retains an open static argument").with_source_span(target.source_span())),
        };
        if matches!(parameter.kind, TypeParameterKind::Type)
            && super::generic_data::closed_argument_identity(
                syntax,
                Some(selection),
                reference,
                false,
            )
            .is_none()
        {
            return Err(Diagnostic::error(
                "machine equation requires a closed explicit type argument",
            )
            .with_source_span(target.source_span()));
        }
        arguments.push(reference);
    }
    let const_values = module_constants::lexical_integer_const_values(syntax);
    let mut warnings = Vec::new();
    let tuple = type_equations::complete_equation_arguments(
        syntax,
        EquationTemplate {
            kind: "machine",
            name: declaration.name.as_str(),
            parameters: declaration.type_parameters,
            parameter_names: &parameter_names,
            const_parameter_types: &const_parameter_types,
            type_equations: &equations,
        },
        target,
        &arguments,
        &const_values,
        Some(selection),
        &mut warnings,
    )?;
    for warning in warnings {
        eprintln!("{warning}");
    }
    let mut completed = supplied.to_vec();
    for (reference, parameter) in tuple.into_iter().zip(parameters).skip(supplied.len()) {
        let mut argument = StaticMachineArgument {
            type_reference: TypeReferenceHandle::invalid(),
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
        };
        match parameter.kind {
            TypeParameterKind::Type => argument.type_reference = reference,
            TypeParameterKind::Const { .. } => {
                let TypeReferenceNode::Named(value) =
                    syntax.type_references.type_reference(reference)
                else {
                    return Err(Diagnostic::error(
                        "machine equation did not produce a closed const argument",
                    ));
                };
                let text = value.as_str();
                argument.const_literal = Some(
                    IntegerLiteral::from_parts(
                        text.starts_with('-'),
                        IntegerRadix::Decimal,
                        text.trim_start_matches('-'),
                    )
                    .map_err(Diagnostic::error)?,
                );
            }
            _ => unreachable!("parameter kinds were admitted above"),
        }
        completed.push(argument);
    }
    Ok(completed.into_boxed_slice())
}
