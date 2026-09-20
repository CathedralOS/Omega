//! Normalize explicit const selections into the existing erased value leaves.

use super::{CalleeState, Candidate};
use crate::monomorphization::selection::resolve_callee;
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::literals::LandedIntegerType;
use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::statement::StatementNode;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(super) fn spelling(program: &TypedTrees, argument: &StaticMachineArgument) -> Option<String> {
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return None;
    }
    if let Some(literal) = &argument.const_literal {
        return Some(
            literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                .map_or_else(|| literal.text().to_owned(), |value| value.to_string()),
        );
    }
    if argument.symbol.is_valid() {
        let declaration = program
            .const_declarations()
            .iter()
            .find(|declaration| declaration.symbol == argument.symbol)?;
        let value = CanonicalConstValue::new(
            program.display_type_reference(declaration.declared_type),
            declaration.canonical_value_encoding.as_ref()?.clone(),
            argument.display_name(),
        );
        // Integer inference already uses decimal leaves. Declaration-aware
        // argument validation retains the independently declared carrier.
        return Some(match value.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
            _ => value.atom(),
        });
    }
    let [name] = argument.path.as_ref() else {
        return None;
    };
    match name.as_str() {
        "true" => Some(CanonicalConstValue::boolean(true).atom()),
        "false" => Some(CanonicalConstValue::boolean(false).atom()),
        // A previous specialization may forward a compiler-created atom.
        spelling => CanonicalConstValue::from_atom(spelling).map(|value| value.atom()),
    }
}

pub(super) fn validate_authored(
    program: &TypedTrees,
    candidates: &[Candidate],
    callees: &[CalleeState],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    validate_structural_type_arguments(program, callees, &mut diagnostics);
    let mut validate = |target, name: &str, arguments: &[StaticMachineArgument]| {
        let Some(callee) = resolve_callee(callees, target, name) else {
            return;
        };
        let candidate = &candidates[callee.candidate_index];
        if let Err(error) = validate_arguments(program, candidate, arguments) {
            diagnostics.push(error);
        }
    };
    for (_, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate(
                call.target_symbol,
                call.target.as_str(),
                &call.machine_arguments,
            );
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    validate(
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                    );
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub(super) fn forwarded_type(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
) -> TypeReferenceHandle {
    if !argument.symbol.is_valid() {
        return TypeReferenceHandle::invalid();
    }
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_type_parameters(machine))
        .find_map(|parameter| match parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference }
                if parameter.symbol == argument.symbol =>
            {
                Some(type_reference)
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// The shape of one ordinary runtime subject admissible into a `Value`
/// binder: a single-segment name carrying no static payload. A spelled name
/// that resolved to no static declaration still qualifies; the caller's value
/// scope decides whether it denotes a subject (`resolve_runtime_subject`).
pub(super) fn is_runtime_value_subject(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
) -> bool {
    !argument.type_reference.is_valid()
        && argument.application.is_none()
        && argument.evidence_projection.is_none()
        && argument.const_literal.is_none()
        && argument.path.len() == 1
        && (!argument.symbol.is_valid()
            || matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            ))
}

/// Resolve the exact value-scope subject one runtime `Value` argument
/// denotes: the caller's own parameter or the nearest `let` of the same name
/// declared before the call. Only a single-segment spelling without static
/// payload can resolve, and only an ordinary `Local`/`Parameter` symbol
/// counts — a runtime subject is never a type, machine, or evidence name.
pub(super) fn resolve_runtime_subject(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    scope_limit: usize,
    argument: &StaticMachineArgument,
) -> Option<symbols::SymbolHandle> {
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
    {
        return None;
    }
    if argument.symbol.is_valid() {
        let kind = program.symbols.get(argument.symbol).kind;
        // A resolved parameter is exact: a realized generic binder forwarded
        // into a specialization names that parameter, never a same-named
        // local. A resolved local re-derives its nearest binding by scope
        // below, since the symbol alone cannot order shadowed `let`s.
        if kind == SymbolKind::Parameter {
            return Some(argument.symbol);
        }
        if kind != SymbolKind::Local {
            return None;
        }
    }
    let [name] = argument.path.as_ref() else {
        return None;
    };
    let mut resolved = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name.as_str())
        .map(|parameter| parameter.symbol);
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope_limit)
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.name.as_str() == name.as_str() {
            resolved = Some(local.symbol);
        }
    }
    resolved
}

fn validate_arguments(
    program: &TypedTrees,
    candidate: &Candidate,
    arguments: &[StaticMachineArgument],
) -> Result<(), Diagnostic> {
    let mut const_index = 0;
    for (ordinal, argument) in arguments.iter().enumerate() {
        // Proposals are grouped by kind after validation. Named types must
        // retain their authored slot just like structural types; otherwise
        // `<7, u8>` can silently become `<u8, 7>`. A const-only template keeps
        // its more specific "requires a const value" diagnostic below.
        let named_type = argument.symbol.is_valid()
            && matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::BuiltinType | SymbolKind::Data
            );
        if argument.type_reference.is_valid()
            || (named_type && !candidate.template.type_parameters.is_empty())
        {
            let template = &program.machines()[candidate.template.machine_index];
            if !program
                .machine_type_parameters(template)
                .get(ordinal)
                .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` static argument {} requires its declared binder kind, not a type argument",
                    candidate.template.template_name,
                    ordinal + 1
                )));
            }
        }
        if argument.type_reference.is_valid() {
            if !program
                .type_reference_table
                .contains_type_reference(argument.type_reference)
                || !argument.path.is_empty()
                || argument.symbol.is_valid()
                || argument.const_literal.is_some()
                || argument.application.is_some()
                || argument.evidence_projection.is_some()
            {
                return Err(Diagnostic::error(
                    "structural static type argument has invalid or mixed argument custody",
                ));
            }
            continue;
        }
        // An unused type binder is still a type, not an unsupplied data
        // constructor. Check the selected declaration before specialization
        // can erase this argument from the executable call.
        if named_type
            && argument.application.is_none()
            && program.symbols.get(argument.symbol).kind == SymbolKind::Data
            && let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == argument.symbol)
            && (!definition.type_parameters.is_empty()
                || !definition.lifetime_parameters.is_empty())
        {
            return Err(Diagnostic::error(format!(
                "static type argument `{}` selects generic data without its complete type/lifetime arguments",
                definition.name,
            )));
        }
        let declaration = program.const_declarations().iter().find(|declaration| {
            argument.symbol.is_valid() && declaration.symbol == argument.symbol
        });
        let forwarded = forwarded_type(program, argument);
        let forwarded_type = forwarded.is_valid().then_some(forwarded);
        if declaration.is_none()
            && forwarded_type.is_none()
            && spelling(program, argument).is_none()
        {
            let type_shaped = argument.type_reference.is_valid()
                || argument.symbol.is_valid()
                    && matches!(
                        program.symbols.get(argument.symbol).kind,
                        SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                    );
            if !candidate.template.const_parameters.is_empty()
                && candidate.template.type_parameters.is_empty()
                && type_shaped
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` requires a const value, not type `{}`",
                    candidate.template.template_name,
                    argument.display_name()
                )));
            }
            // A `Value` binder admits static arguments through the ordinary
            // const specialization path and ordinary runtime subjects through
            // the dynamic realization path. Anything else spelled here is
            // neither.
            if candidate
                .template
                .value_const_parameters
                .contains(&const_index)
                && argument.application.is_none()
                && argument.evidence_projection.is_none()
                && !type_shaped
            {
                let parameter_name = &candidate.template.const_parameters[const_index].1;
                if is_runtime_value_subject(program, argument) {
                    const_index += 1;
                    continue;
                }
                return Err(Diagnostic::error(format!(
                    "value parameter `{parameter_name}` of machine `{}` received `{}`, which \
                     is neither a static const value nor an ordinary runtime value subject",
                    candidate.template.template_name,
                    argument.display_name()
                )));
            }
            continue;
        }
        let Some((_, parameter_name, required)) =
            candidate.template.const_parameters.get(const_index)
        else {
            return Err(Diagnostic::error(format!(
                "machine `{}` has no const parameter for extra argument `{}`",
                candidate.template.template_name,
                argument.display_name()
            )));
        };
        const_index += 1;
        if argument.type_reference.is_valid()
            || argument.application.is_some()
            || argument.evidence_projection.is_some()
        {
            return Err(Diagnostic::error(format!(
                "const argument `{}` must select a closed value or a const binder",
                argument.display_name()
            )));
        }
        if let Some(actual) = declaration
            .map(|declaration| declaration.declared_type)
            .or(forwarded_type)
            && program.normalized_type_identity(actual)
                != program.normalized_type_identity(*required)
        {
            return Err(Diagnostic::error(format!(
                "const parameter `{parameter_name}` of `{}` requires `{}`, but `{}` declares `{}`",
                candidate.template.template_name,
                program.display_type_reference(*required),
                argument.display_name(),
                program.display_type_reference(actual)
            )));
        }
        // Decimal specialization keys cannot erase a literal's earlier typed
        // landing. Only anonymous literals may land in the binder's carrier.
        if let Some(landing) = argument
            .const_literal
            .as_ref()
            .and_then(|literal| literal.landing())
        {
            let actual = match landing.landed_type {
                LandedIntegerType::I8 => PrimitiveType::I8,
                LandedIntegerType::I16 => PrimitiveType::I16,
                LandedIntegerType::I32 => PrimitiveType::I32,
                LandedIntegerType::I64 => PrimitiveType::I64,
                LandedIntegerType::U8 => PrimitiveType::U8,
                LandedIntegerType::U16 => PrimitiveType::U16,
                LandedIntegerType::U32 => PrimitiveType::U32,
                LandedIntegerType::U64 => PrimitiveType::U64,
                LandedIntegerType::Addr => PrimitiveType::Addr,
            };
            if program.type_reference_table.primitive_type(*required) != Some(actual) {
                return Err(Diagnostic::error(format!(
                    "const parameter `{parameter_name}` of `{}` requires `{}`, but its literal already landed as `{}`",
                    candidate.template.template_name,
                    program.display_type_reference(*required),
                    landing.landed_type.name(),
                )));
            }
        }
        // Public declaration identity is broader than proof-static atom identity.
        // Decode through the ordinary specialization reader even for unused
        // binders, whose Unit calls may otherwise need no substitution.
        if declaration.is_some() && spelling(program, argument).is_none() {
            return Err(Diagnostic::error(format!(
                "const argument `{}` has no eligible canonical value",
                argument.display_name()
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_bindings(
    program: &TypedTrees,
    candidate: &Candidate,
) -> Result<(), Diagnostic> {
    let machine = &program.machines()[candidate.template.machine_index];
    for (index, ((symbol, _, _), binding)) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
        .enumerate()
    {
        // A runtime-bound `Value` slot carries its declared carrier, not a
        // closed static value; its subject is checked as an ordinary argument.
        if candidate.runtime_value_bindings[index].is_some() {
            continue;
        }
        let Some(binding) = binding else { continue };
        let Some(parameter) = program
            .machine_type_parameters(machine)
            .iter()
            .find(|parameter| parameter.symbol == *symbol)
        else {
            return Err(Diagnostic::error(
                "const specialization lost its declared binder",
            ));
        };
        validation::validate_closed_const_argument(
            program,
            &candidate.template.template_name,
            parameter,
            *binding,
        )
        .map_err(|errors| {
            errors.into_iter().next().unwrap_or_else(|| {
                Diagnostic::error("const argument validation failed without a diagnostic")
            })
        })?;
    }
    Ok(())
}

// A range-bound call belongs to the declaration containing its type, not
// to whichever machine happens to share its arena. Follow only that
// caller's type roots; named data does not lend its separate binder scope.
fn collect_type_expressions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    visited: &mut Vec<TypeReferenceHandle>,
    expressions: &mut Vec<ExpressionHandle>,
) {
    if !reference.is_valid() || visited.contains(&reference) {
        return;
    }
    visited.push(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_expressions(program, *referee, visited, expressions);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_expressions(program, *element_type, visited, expressions);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_expressions(program, *argument, visited, expressions);
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_expressions(program, *base_type, visited, expressions);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        super::selection::collect_expression_tree(program, *minimum, expressions);
                        super::selection::collect_expression_tree(program, *maximum, expressions);
                    }
                    TypeConstraintNode::Domain(domain) => {
                        for argument in &domain.arguments {
                            collect_type_expressions(program, *argument, visited, expressions);
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            super::selection::collect_expression_tree(program, *expression, expressions);
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => {}
    }
}

pub(super) fn has_pending_type_arguments(
    program: &TypedTrees,
    arguments: &[Option<TypeReferenceHandle>],
) -> bool {
    if program.pending_const_range_endpoints.is_empty() {
        return false;
    }
    let mut visited = Vec::new();
    let mut expressions = Vec::new();
    for argument in arguments.iter().flatten() {
        collect_type_expressions(program, *argument, &mut visited, &mut expressions);
    }
    expressions
        .iter()
        .any(|expression| program.pending_const_range_endpoints.contains(expression))
}

/// A static type is checked under the source caller even when the callee never
/// uses its type binder. Unowned arena expressions cannot borrow another scope.
fn validate_structural_type_arguments(
    program: &TypedTrees,
    callees: &[CalleeState],
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn contains_type(arguments: &[StaticMachineArgument]) -> bool {
        arguments.iter().any(|argument| {
            argument.type_reference.is_valid()
                || argument
                    .application
                    .as_ref()
                    .is_some_and(|application| contains_type(&application.arguments))
        })
    }
    fn validate_types(
        program: &TypedTrees,
        caller: &typed_trees::machine::Machine,
        arguments: &[StaticMachineArgument],
        symbols: &validation::TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        visited_types: &mut Vec<TypeReferenceHandle>,
        expressions: &mut Vec<ExpressionHandle>,
    ) {
        for argument in arguments {
            if argument.type_reference.is_valid() {
                validation::validate_static_type_argument(
                    program,
                    caller,
                    argument.type_reference,
                    symbols,
                    diagnostics,
                );
                collect_type_expressions(
                    program,
                    argument.type_reference,
                    visited_types,
                    expressions,
                );
            }
            if let Some(application) = &argument.application {
                validate_types(
                    program,
                    caller,
                    &application.arguments,
                    symbols,
                    diagnostics,
                    visited_types,
                    expressions,
                );
            }
        }
    }
    let has_types = program.expression_table.iter_expressions().any(|(_, expression)| matches!(expression, ExpressionNode::Call(call) if contains_type(&call.machine_arguments)))
        || program.machines().iter().any(|machine| program.machine_states(machine).iter().any(|state| program.statement_table.statements(state.statement_nodes).iter().any(|statement| matches!(statement, StatementNode::Call(call) if contains_type(&call.machine_arguments)))));
    if !has_types {
        return;
    }
    let symbols = validation::TopLevelSymbols::build(program, diagnostics);
    let mut owned = Vec::new();
    for machine in program.machines() {
        let mut expressions = Vec::new();
        let mut visited_types = Vec::new();
        for item in program.machine_owned_data(machine) {
            collect_type_expressions(
                program,
                item.type_reference,
                &mut visited_types,
                &mut expressions,
            );
            super::selection::collect_expression_tree(
                program,
                item.initial_value,
                &mut expressions,
            );
        }
        for call in program
            .proof_output_calls
            .iter()
            .filter(|call| call.machine_symbol == machine.symbol)
        {
            super::selection::collect_expression_tree(program, call.call, &mut expressions);
        }
        for contract in program.machine_contracts(machine) {
            super::selection::collect_contract_facts(program, contract.facts, &mut expressions);
        }
        for state in program.machine_states(machine) {
            collect_type_expressions(
                program,
                state.return_type,
                &mut visited_types,
                &mut expressions,
            );
            for parameter in program.state_parameters(state) {
                collect_type_expressions(
                    program,
                    parameter.type_reference,
                    &mut visited_types,
                    &mut expressions,
                );
            }
            for contract in program.state_contracts(state) {
                super::selection::collect_contract_facts(program, contract.facts, &mut expressions);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::LocalData(local) = statement {
                    collect_type_expressions(
                        program,
                        local.type_reference,
                        &mut visited_types,
                        &mut expressions,
                    );
                }
                super::collect_statement_expression_trees(program, statement, &mut expressions);
                if let StatementNode::Call(call) = statement {
                    validate_types(
                        program,
                        machine,
                        &call.machine_arguments,
                        &symbols,
                        diagnostics,
                        &mut visited_types,
                        &mut expressions,
                    );
                }
            }
        }
        let mut expression_position = 0;
        while let Some(expression) = expressions.get(expression_position).copied() {
            expression_position += 1;
            if let ExpressionNode::Call(call) = program.expression_table.expression(expression) {
                validate_types(
                    program,
                    machine,
                    &call.machine_arguments,
                    &symbols,
                    diagnostics,
                    &mut visited_types,
                    &mut expressions,
                );
                if !owned.contains(&expression) {
                    owned.push(expression);
                }
            }
        }
    }
    for (expression, node) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = node
            && contains_type(&call.machine_arguments)
            && !owned.contains(&expression)
            && resolve_callee(callees, call.target_symbol, call.target.as_str()).is_some()
        {
            diagnostics.push(Diagnostic::error(
                "structural static type argument has no retained caller type/lifetime context",
            ));
        }
    }
}
