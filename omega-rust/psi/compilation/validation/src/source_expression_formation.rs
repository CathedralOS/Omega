//! Source formation uses the ordinary lowered proof operation. This adapter
//! owns diagnostics only; it supplies no inferred body values or proof facts.

use diagnostics::Diagnostic;
use structure_elaboration::{
    SourceExpressionAssumptions, SourceExpressionContext, SourceExpressionGoal,
};
use symbols::SymbolHandle;
use typed_trees::{
    expression::ExpressionHandle, machine::Machine, state::State, types::TypeReferenceHandle,
};

#[derive(Clone, Copy)]
pub(crate) struct LocalValueFormation<'a> {
    pub machine: &'a Machine,
    pub state: Option<&'a State>,
}

pub(crate) fn validate(
    program: &dyn crate::CheckingInput,
    lexical_owner: SymbolHandle,
    prior_facts: Option<&[ExpressionHandle]>,
    local: Option<LocalValueFormation<'_>>,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let typed = program.typed_program();
    let mut context = SourceExpressionContext::declaration(lexical_owner);
    if let Some(local) = local {
        context.lexical_owner = local.machine.symbol;
        context.state = local.state.map(|state| state.symbol);
    } else if let Some(machine) = typed.machine_holding_state(lexical_owner) {
        context.lexical_owner = machine.symbol;
        context.state = Some(lexical_owner);
    }
    context.assumptions = match prior_facts {
        Some(facts) => SourceExpressionAssumptions::Explicit(facts),
        None => SourceExpressionAssumptions::Published,
    };
    if let Err(refused) = lowered_discharge::prove_source_expression(
        typed,
        expression,
        SourceExpressionGoal::Formed { expected },
        context,
        &mut Default::default(),
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "cannot establish formation of {owner}: {} ({:?})",
            refused.detail, refused.reason,
        )));
    }
}

/// Prove a source requirement's complete static tuple in the supplier's actual
/// namespace. Every operand is read before the required expression uses it.
pub(crate) fn static_requirements(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    arguments: &[typed_trees::expression::StaticMachineArgument],
    lexical_owner: SymbolHandle,
    caller_state: Option<&State>,
) -> Result<(), String> {
    use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
    use structure_elaboration::{SourceExpressionBinding, SourceExpressionValue};
    use typed_trees::{data::TypeParameterKind, types::TypeReferenceNode};
    enum Actual {
        Constant(CanonicalConstIdentity),
        Expression(ExpressionHandle),
        Symbol(SymbolHandle),
    }
    let typed = program.typed_program();
    let parameters = typed.state_signature_type_parameters(signature);
    if parameters.len() != arguments.len() {
        return Err("static requirement tuple does not match its exact declaration".into());
    }
    let requirements = structure_elaboration::static_tuple_requirements(typed, signature)
        .map_err(str::to_owned)?;
    let mut types = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if parameter.kind == TypeParameterKind::Type {
            if !typed
                .type_reference_table
                .contains_type_reference(argument.type_reference)
            {
                return Err("static requirement type operand is missing or stale".into());
            }
            types.push((parameter.symbol, argument.type_reference));
        }
    }
    let mut actuals = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let (TypeParameterKind::Const {
            type_reference: declared,
        }
        | TypeParameterKind::Value {
            type_reference: declared,
        }) = parameter.kind
        else {
            continue;
        };
        let carrier =
            typed_trees::static_applications::project_type_operand(typed, declared, &types);
        let actual = if typed
            .type_reference_table
            .contains_type_reference(argument.type_reference)
        {
            match typed
                .type_reference_table
                .type_reference(argument.type_reference)
            {
                TypeReferenceNode::ConstExpression(expression) => Actual::Expression(*expression),
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                    Actual::Symbol(*symbol)
                }
                _ => {
                    return Err(
                        "static requirement value operand has no retained source computation"
                            .into(),
                    );
                }
            }
        } else if argument.type_reference.is_valid() {
            return Err("static requirement value operand has a stale source carrier".into());
        } else if let Some(literal) = &argument.const_literal {
            let value = literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                .ok_or("static integer operand exceeds the represented canonical range")?;
            let primitive = typed
                .primitive_type_reference(carrier)
                .ok_or("static integer operand has no exact applied carrier")?;
            Actual::Constant(CanonicalConstIdentity::integer(primitive.name(), value))
        } else if argument.symbol.is_valid() {
            if let Some(value) =
                typed_trees::canonical_const_declaration_value(typed, argument.symbol)
            {
                Actual::Constant(value.identity())
            } else {
                Actual::Symbol(argument.symbol)
            }
        } else if let [atom] = argument.path.as_ref() {
            if let Some(value) = CanonicalConstValue::from_atom(atom.as_str()) {
                Actual::Constant(value.identity())
            } else if let Some(value) =
                language_semantics::const_value::boolean_literal_spelling(atom.as_str())
            {
                Actual::Constant(
                    language_semantics::const_value::CanonicalConstValue::boolean(value).identity(),
                )
            } else {
                return Err(
                    "static value operand has no canonical value or resolved source subject".into(),
                );
            }
        } else {
            return Err("static requirement value operand lost its exact supplied identity".into());
        };
        actuals.push((parameter.symbol, carrier, actual));
    }
    let bindings = actuals
        .iter()
        .map(|(symbol, carrier, actual)| SourceExpressionBinding {
            symbol: *symbol,
            carrier: *carrier,
            signature: Some(signature),
            value: match actual {
                Actual::Constant(value) => SourceExpressionValue::Constant(value),
                Actual::Expression(expression) => SourceExpressionValue::Expression(*expression),
                Actual::Symbol(symbol) => SourceExpressionValue::Symbol(*symbol),
            },
        })
        .collect::<Vec<_>>();
    let mut context = SourceExpressionContext::declaration(lexical_owner);
    context.state = caller_state.map(|state| state.symbol);
    context.types = &types;
    context.bindings = &bindings;
    for expression in requirements {
        lowered_discharge::prove_source_expression(
            typed,
            expression,
            SourceExpressionGoal::Establish,
            context,
            &mut Default::default(),
        )
        .map_err(|refused| format!("{} ({:?})", refused.detail, refused.reason))?;
    }
    Ok(())
}
