use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

mod literal_widths;
pub(crate) use literal_widths::validate_literal_widths;

/// CR4 (carrier, ch5 two-phase law): a width-SUFFIXED literal (`5i8`) landed
/// at parse time, so a destination whose declared type DISAGREES with the
/// suffix is a loud error -- silently stripping the suffix (yesterday's
/// behavior) let a wrong suffix mean nothing; silently honoring it would
/// steer signedness/width decisions against the declared type. Domain is NOT
/// checked (a suffix lands the TYPE; the destination's arithmetic domain is
/// contextual and governs its own folds). Concrete destinations include
/// declared storage, resolved call parameters, and state results, with the
/// literal read through borrow wrappers.
/// CR4 (suffixed-magnitude fit): a width suffix is the literal's OWN claim of
/// type, so the spelled value must fit that type's range wherever the literal
/// sits -- `200i8` is a loud error even in an anonymous position. Runs after
/// the parse-time negative fold, so `-128i8` is ONE literal valued -128 (fits)
/// while a bare `128i8` does not -- the negation caveat resolves itself.
/// Value semantics throughout (ch5 exact anonymous values): `0xFFi8` is 255
/// and does not fit i8 -- a bit-pattern intent spells `0xFFu8` or `-1i8`.
pub(crate) fn validate_suffix_magnitudes(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for (_, node) in program.expression_table.expression_entries() {
        let ExpressionNode::Integer(literal) = node else {
            continue;
        };
        let Some(landing) = literal.landing() else {
            continue;
        };
        let landed = landing.landed_type;
        let width = landed.bit_width();
        let fits = if landed.is_signed() {
            literal.value_i64().is_some_and(|value| {
                if width == 64 {
                    true
                } else {
                    let min = -(1i64 << (width - 1));
                    let max = (1i64 << (width - 1)) - 1;
                    (min..=max).contains(&value)
                }
            })
        } else {
            // A negative spelling never fits an unsigned suffix; beyond that,
            // the value must sit inside the width's window. u64/addr accept
            // the full u64 window (an even larger spelling fails value_u64
            // and lands here too).
            !literal.text().starts_with('-')
                && literal.value_u64().is_some_and(|value| {
                    if width == 64 {
                        true
                    } else {
                        value < (1u64 << width)
                    }
                })
        };
        if !fits {
            diagnostics.push(Diagnostic::error(format!(
                "literal `{}` does not fit its `{}` suffix -- a width suffix chooses the \
                 literal's type at the spelling, and the spelled value must fit that type's \
                 range (suffixes read VALUES, not bit patterns: spell `-1i8`, not `0xFFi8`)",
                literal.text(),
                landed.name(),
            )));
        }
    }
}

mod float_landing;
pub use float_landing::land_float_literal_destinations;
mod integer_landing;
mod integer_remainder;
pub(crate) use integer_landing::anonymous_integer_landing_warnings;
pub(crate) use integer_landing::{anonymous_numeric_value, land_integer_value};
pub use integer_landing::{has_anonymous_operator_meaning, land_anonymous_integer_expression};
pub(crate) use integer_remainder::validate_anonymous_remainders;

/// Check an already-landed literal against one exact declared destination.
/// Explicit casts are conversions, not transparent literal wrappers.
pub(crate) fn validate_suffix_landing(
    program: &TypedTrees,
    value: ExpressionHandle,
    declared: typed_trees::types::TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use numerics::literals::LandedIntegerType;

    let landed_of_primitive = |primitive: PrimitiveType| -> Option<LandedIntegerType> {
        Some(match primitive {
            PrimitiveType::I8 => LandedIntegerType::I8,
            PrimitiveType::I16 => LandedIntegerType::I16,
            PrimitiveType::I32 => LandedIntegerType::I32,
            PrimitiveType::I64 => LandedIntegerType::I64,
            PrimitiveType::U8 => LandedIntegerType::U8,
            PrimitiveType::U16 => LandedIntegerType::U16,
            PrimitiveType::U32 => LandedIntegerType::U32,
            PrimitiveType::U64 => LandedIntegerType::U64,
            PrimitiveType::Addr => LandedIntegerType::Addr,
            _ => return None,
        })
    };

    let literal_landing =
        |expression: ExpressionHandle| -> Option<(ExpressionHandle, LandedIntegerType)> {
            let mut current = expression;
            loop {
                match program.expression_table.expression(current) {
                    ExpressionNode::Borrow(inner) => current = inner.target,
                    ExpressionNode::Integer(literal) => {
                        return literal
                            .landing()
                            .map(|landing| (current, landing.landed_type));
                    }
                    _ => return None,
                }
            }
        };

    // The FLOAT twin (F2a): a width-suffixed float literal landed its FORMAT
    // at the spelling; a destination declaring the other format is the same
    // loud error.
    let float_landing = |expression: ExpressionHandle| -> Option<(
        ExpressionHandle,
        numerics::literals::FloatFormat,
    )> {
        let mut current = expression;
        loop {
            match program.expression_table.expression(current) {
                ExpressionNode::Borrow(inner) => current = inner.target,
                ExpressionNode::Float(literal) => {
                    return literal.landing().map(|landing| (current, landing));
                }
                _ => return None,
            }
        }
    };

    let Some(unwrapped) = crate::places::unwrapped_type_reference(program, declared) else {
        return;
    };
    let Some(primitive) = program.primitive_type_reference(unwrapped) else {
        return;
    };
    if let Some((literal_handle, suffix_type)) = literal_landing(value) {
        let Some(declared_type) = landed_of_primitive(primitive) else {
            return;
        };
        if declared_type != suffix_type {
            let literal = program.expression_table.display_name(literal_handle);
            diagnostics.push(Diagnostic::error(format!(
                "literal `{literal}` is suffixed `{suffix}` but lands in a `{declared}` place -- \
                 a width suffix chooses the literal's type at the spelling, so it must agree \
                 with the destination's declared type (drop the suffix or fix one side)",
                suffix = suffix_type.name(),
                declared = primitive.name(),
            )));
        }
        return;
    }
    if let Some((literal_handle, suffix_format)) = float_landing(value) {
        use numerics::literals::FloatFormat;
        let declared_format = match primitive {
            PrimitiveType::F32 => FloatFormat::F32,
            PrimitiveType::F64 => FloatFormat::F64,
            _ => return,
        };
        if declared_format != suffix_format {
            let literal = program.expression_table.display_name(literal_handle);
            diagnostics.push(Diagnostic::error(format!(
                "literal `{literal}` is suffixed `{suffix}` but lands in a `{declared}` place -- \
                 a width suffix chooses the literal's format at the spelling, so it must agree \
                 with the destination's declared type (drop the suffix or fix one side)",
                suffix = suffix_format.name(),
                declared = primitive.name(),
            )));
        }
    }
}

pub(crate) fn validate_suffix_landings(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for (_, node) in program.expression_table.expression_entries() {
        let ExpressionNode::StructLiteral(literal) = node else {
            continue;
        };
        let Some(data_definition) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == literal.type_name.as_str())
        else {
            continue;
        };
        for field in program.expression_table.struct_fields(literal.fields) {
            let Some(field_type) = crate::struct_literals::construction_field_type(
                program,
                data_definition,
                literal.case_name.as_ref().map(|name| name.as_str()),
                field.name.as_str(),
            ) else {
                continue;
            };
            validate_suffix_landing(program, field.value, field_type, diagnostics);
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(value) if state.return_type.is_valid() => {
                        validate_suffix_landing(program, *value, state.return_type, diagnostics);
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) {
                            validate_suffix_landing(
                                program,
                                assignment.value,
                                declared,
                                diagnostics,
                            );
                        }
                    }
                    StatementNode::LocalData(local)
                        if local.initial_value.is_valid() && local.type_reference.is_valid() =>
                    {
                        validate_suffix_landing(
                            program,
                            local.initial_value,
                            local.type_reference,
                            diagnostics,
                        );
                    }
                    StatementNode::Transition(transition) if state.return_type.is_valid() => {
                        for target in [transition.target, transition.continuation] {
                            if target.is_valid()
                                && let typed_trees::statement::TransitionTargetNode::Value(value) =
                                    program.statement_table.transition_target(target)
                            {
                                validate_suffix_landing(
                                    program,
                                    *value,
                                    state.return_type,
                                    diagnostics,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
