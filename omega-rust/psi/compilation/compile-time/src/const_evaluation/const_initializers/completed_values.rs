//! Apply independently replayed scalar declaration values to exact source uses.
//!
//! Omega supplies the completed private evaluation forest, never encoded scalar
//! semantics. Psi rejoins each retained declaration and materializes its value.
//! Initializer roots remain evidence; selected use occurrences receive literals.

use diagnostics::Diagnostic;
use language_core::constant_value::EvaluatedConstant;
use language_semantics::const_value::{
    CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue,
};
use typed_trees::{TypedTrees, constant::ConstDeclaration};

use crate::const_evaluation::const_generic_expressions::{
    scalar_probe_destination, value::ScalarValue,
};

fn same_source(left: &ConstDeclaration, right: &ConstDeclaration) -> bool {
    left.symbol == right.symbol
        && left.is_public == right.is_public
        && left.declared_type == right.declared_type
        && left.initializer_source_span == right.initializer_source_span
        && left.authored_initializer == right.authored_initializer
        && left.materialized_initializer == right.materialized_initializer
}

fn exact_declaration<'program>(
    program: &'program TypedTrees,
    expected: &ConstDeclaration,
) -> Result<&'program ConstDeclaration, String> {
    let mut declarations = program
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.symbol == expected.symbol);
    let declaration = declarations
        .next()
        .ok_or("pending constant lost its exact declaration")?;
    if declarations.next().is_some() || !same_source(declaration, expected) {
        return Err("pending constant changed its declaration or source owner".into());
    }
    let table = &program.expression_table;
    if !table.expression_is_valid(declaration.materialized_initializer)
        || table.source_span(declaration.materialized_initializer)
            != declaration.initializer_source_span
        || (declaration.authored_initializer.is_valid()
            && (!table.expression_is_valid(declaration.authored_initializer)
                || table.source_span(declaration.authored_initializer)
                    != declaration.initializer_source_span))
    {
        return Err("pending constant lost its exact initializer source".into());
    }
    Ok(declaration)
}

pub(crate) fn read_back(
    receiving: &mut TypedTrees,
    evaluated: &TypedTrees,
    pending: &[ConstDeclaration],
) -> Result<(), Vec<Diagnostic>> {
    let mut completed = Vec::new();
    for expected in pending {
        let failure = |reason| super::failure(expected.initializer_source_span, reason);
        let current = exact_declaration(receiving, expected).map_err(&failure)?;
        let declaration = exact_declaration(evaluated, expected).map_err(&failure)?;
        let meaning = declaration.evaluated_value.as_ref().ok_or_else(|| {
            failure("pending constant did not complete under selected execution".into())
        })?;
        if current
            .evaluated_value
            .as_ref()
            .is_some_and(|value| value != meaning)
        {
            return Err(failure(
                "pending constant readback changed an already completed value".into(),
            ));
        }
        let carrier = scalar_probe_destination(evaluated, declaration.declared_type)
            .ok_or_else(|| failure("pending constant lost its scalar declared carrier".into()))?;
        let value = match meaning {
            EvaluatedConstant::UndeterminedNaN(format) => ScalarValue::UndeterminedNaN(*format),
            EvaluatedConstant::Encoded(encoding) => {
                let identity = CanonicalConstIdentity {
                    type_name: carrier.name().into(),
                    encoding: encoding.clone(),
                };
                match identity.decode_declaration_encoding().ok_or_else(|| {
                    failure("pending constant has no canonical declaration value".into())
                })? {
                    DecodedCanonicalConstValue::Float { format, bits } => {
                        ScalarValue::Float { format, bits }
                    }
                    decoded @ (DecodedCanonicalConstValue::Integer { .. }
                    | DecodedCanonicalConstValue::Boolean(_)) => {
                        let value = CanonicalConstValue::from_decoded(
                            &decoded,
                            "completed scalar declaration",
                        )
                        .ok_or_else(|| {
                            failure("pending constant has no exact scalar value".into())
                        })?;
                        if value.type_name != carrier.name() {
                            return Err(failure(
                                "pending constant value differs from its declared carrier".into(),
                            ));
                        }
                        ScalarValue::Index(value)
                    }
                    _ => return Err(failure("pending constant result is not scalar".into())),
                }
            }
        };
        completed.push((expected.clone(), value));
    }
    for (declaration, value) in completed {
        install(receiving, &declaration, &value)
            .map_err(|reason| super::failure(declaration.initializer_source_span, reason))?;
    }
    Ok(())
}

pub(super) fn install(
    typed: &mut TypedTrees,
    declaration: &ConstDeclaration,
    value: &ScalarValue,
) -> Result<(), String> {
    let destination = scalar_probe_destination(typed, declaration.declared_type)
        .ok_or("completed constant lost its scalar carrier")?;
    let materialized = match value {
        ScalarValue::UndeterminedNaN(format) => {
            if !matches!(
                (format, destination),
                (
                    numerics::literals::FloatFormat::F32,
                    typed_trees::types::PrimitiveType::F32
                ) | (
                    numerics::literals::FloatFormat::F64,
                    typed_trees::types::PrimitiveType::F64
                )
            ) {
                return Err("completed NaN lost its exact scalar carrier".into());
            }
            typed_trees::expression::ExpressionNode::UndeterminedNaN(*format)
        }
        _ => super::invocations::scalar_snapshot(value, destination)?,
    };
    let uses = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, _)| {
            let reference = typed.expression_table.source_span(expression);
            typed
                .expression_table
                .authored_selection_occurrences(expression)
                .any(|occurrence| {
                    typed
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|selection| {
                            selection.source_span() == reference
                                && selection.target().as_resolved().is_some_and(|selected| {
                                    selected.selected_symbol() == declaration.symbol
                                })
                        })
                })
                .then_some(expression)
        })
        .collect::<Vec<_>>();
    for expression in uses {
        *typed.expression_table.expression_mut(expression) = materialized.clone();
    }
    typed
        .tables
        .const_declarations
        .for_each_mut(|_, candidate| {
            if candidate.symbol == declaration.symbol {
                candidate.evaluated_value = Some(value.declaration_value());
            }
        });
    Ok(())
}
