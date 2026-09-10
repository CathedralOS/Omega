//! Normalize explicit const selections into the existing erased value leaves.

use super::{CalleeState, Candidate, resolve_callee};
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn spelling(program: &TypedTrees, argument: &StaticMachineArgument) -> Option<String> {
    if argument.application.is_some() || argument.evidence_projection.is_some() {
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
            TypeParameterKind::Const { type_reference } if parameter.symbol == argument.symbol => {
                Some(type_reference)
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn validate_arguments(
    program: &TypedTrees,
    candidate: &Candidate,
    arguments: &[StaticMachineArgument],
) -> Result<(), Diagnostic> {
    let mut const_index = 0;
    for argument in arguments {
        let declaration = program.const_declarations().iter().find(|declaration| {
            argument.symbol.is_valid() && declaration.symbol == argument.symbol
        });
        let forwarded = forwarded_type(program, argument);
        let forwarded_type = forwarded.is_valid().then_some(forwarded);
        if declaration.is_none()
            && forwarded_type.is_none()
            && spelling(program, argument).is_none()
        {
            if !candidate.const_parameters.is_empty()
                && candidate.type_parameters.is_empty()
                && argument.symbol.is_valid()
                && matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::BuiltinType | SymbolKind::Data | SymbolKind::TypeParameter
                )
            {
                return Err(Diagnostic::error(format!(
                    "machine `{}` requires a const value, not type `{}`",
                    candidate.template_name,
                    argument.display_name()
                )));
            }
            continue;
        }
        let Some((_, parameter_name, required)) = candidate.const_parameters.get(const_index)
        else {
            return Err(Diagnostic::error(format!(
                "machine `{}` has no const parameter for extra argument `{}`",
                candidate.template_name,
                argument.display_name()
            )));
        };
        const_index += 1;
        if argument.application.is_some() || argument.evidence_projection.is_some() {
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
                candidate.template_name,
                program.display_type_reference(*required),
                argument.display_name(),
                program.display_type_reference(actual)
            )));
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
    let machine = &program.machines()[candidate.machine_index];
    for ((symbol, _, _), binding) in candidate
        .const_parameters
        .iter()
        .zip(&candidate.const_bindings)
    {
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
            &candidate.template_name,
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
