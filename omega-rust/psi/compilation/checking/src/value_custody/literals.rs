//! Literals: numeric literals and the destinations they land in. The checks
//! here reject anonymous `%` and anonymous division by zero, and width suffixes
//! that disagree with the destination or do not fit their own carrier. Scalar
//! destination validation checks exact anonymous integrality and representability
//! for storage, aggregate leaves, calls, transitions and returns. No source
//! position or arithmetic policy supplies permission to truncate a literal.
//! The landing queries give an anonymous numeric literal, or a tree of literal
//! arithmetic, the concrete type of its
//! destination.
//!
//! `program_validation::validate` runs this module's program-wide checks at
//! three points. First, before any other validator,
//! `validate_anonymous_remainders` (`integer_remainder`),
//! `validate_anonymous_bitwise` (`integer_bitwise`) and
//! `validate_anonymous_divisions` (`float_landing`); validation stops if
//! any reports a diagnostic. Later it runs `validate_suffix_landings` and
//! `validate_suffix_magnitudes`. `validate_suffix_landings` checks each
//! struct-literal field, state result, assignment, local initializer and
//! transition value against its declared type through
//! `validate_suffix_landing`, which call validation and
//! `expression_types::match_dispatch` also call directly. Last, after every
//! other check passes, it reports `anonymous_integer_landing_warnings`
//! (`integer_landing`).
//!
//! The landing queries are used outside validation too.
//! `land_float_literal_destinations` (`float_landing`) gives unsuffixed float
//! literals their destination format on the typed tree before validation
//! runs, and the `integer_landing` functions evaluate and land anonymous
//! integer expressions for checking, lowering, proof and interpretation.
//! `constant_arrays` and `record_projection` answer the declared type and the
//! closed values of constant arrays and record constructor projections.
//! Width and landing checks use `crate::expression_traversal` for immediate
//! edges while retaining their own destination and execution rules.

use diagnostics::Diagnostic;
use numerics::literals::LandedIntegerType;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::PrimitiveType;

/// The named transition target's spelled arguments paired with the target
/// state's parameters, or `None` when the target is not a singleton-named
/// state. The `&mut self` receiver rides the param list but never pairs
/// with a spelled argument -- the zip pairs only the value params.
/// The landed integer type a primitive lands as, or `None` for a primitive
/// without an integer landing.
pub(crate) fn primitive_landing_type(primitive: PrimitiveType) -> Option<LandedIntegerType> {
    primitive.landed_landing()
}

fn named_transition_arguments<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    target_node: &typed_trees::statement::TransitionTargetNode,
) -> Option<
    Vec<(
        &'program typed_trees::signature::StateParameter,
        ExpressionHandle,
    )>,
> {
    let typed_trees::statement::TransitionTargetNode::Named {
        path, arguments, ..
    } = target_node
    else {
        return None;
    };
    // The Named target's path members live in the STATEMENT table's
    // identifier arena (the target node's home), not the expression table's.
    let target_members = program.statement_table.name_path_members(path.members);
    let [target_name] = target_members else {
        return None;
    };
    let target_state = program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.name.as_str() == target_name.as_str())?;
    Some(
        program
            .state_parameters(target_state)
            .iter()
            .zip(
                program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .copied(),
            )
            .collect(),
    )
}

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
pub(crate) fn validate_suffix_magnitudes(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
pub(crate) use float_landing::validate_anonymous_divisions;
mod constant_arrays;
mod record_projection;
pub use constant_arrays::builtin_constant_array_projection_type;
pub use constant_arrays::declared_constant_array_type;
pub(crate) use constant_arrays::validate_constant_projection_destination;
pub use constant_arrays::{
    ScalarArrayElements, closed_array_store_type, closed_constant_array_elements,
    closed_literal_array_elements, is_closed_primitive_array_type, peel_arithmetic_policy,
    scalar_array_elements, scalar_array_preserves_memberships,
};
pub(crate) use record_projection::closed_record_integer_projection;
pub use record_projection::closed_record_scalar_projection;
mod integer_bitwise;
mod integer_landing;
mod integer_remainder;
pub(crate) use integer_bitwise::validate_anonymous_bitwise;
pub(crate) use integer_landing::anonymous_integer_landing_warnings;
pub(crate) use integer_landing::{anonymous_numeric_value, evaluate_anonymous_numeric_equality};
pub use integer_landing::{
    evaluate_anonymous_numeric_comparison, evaluate_anonymous_numeric_expression,
    has_anonymous_numeric_results, has_anonymous_operator_meaning,
    land_anonymous_integer_expression, land_integer_value, select_anonymous_numeric_match_arm,
};
pub(crate) use integer_remainder::validate_anonymous_remainders;

/// Check an already-landed literal against one exact declared destination.
/// Explicit casts are conversions, not transparent literal wrappers.
pub(crate) fn validate_suffix_landing(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
    declared: typed_trees::types::TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(dispatch) = program.expression_table.match_node(value) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            validate_suffix_landing(program, arm.value, declared, diagnostics);
        }
        return;
    }
    constant_arrays::validate_declared_array_destination(program, value, declared, diagnostics);

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

    let Some(unwrapped) = crate::value_custody::places::unwrapped_type_reference(program, declared)
    else {
        return;
    };
    if let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
        program.type_reference_table.type_reference(unwrapped)
        && let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(value)
    {
        for element in program.expression_table.expression_handles(*elements) {
            validate_suffix_landing(program, *element, *element_type, diagnostics);
        }
        return;
    }
    let Some(primitive) = program.primitive_type_reference(unwrapped) else {
        return;
    };
    float_landing::validate_numeric_tree_destination(program, value, primitive, diagnostics);
    if let Some((literal_handle, suffix_type)) = literal_landing(value) {
        let Some(declared_type) = primitive_landing_type(primitive) else {
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

pub(crate) fn validate_suffix_landings(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, node) in program.expression_table.expression_entries() {
        let ExpressionNode::StructLiteral(literal) = node else {
            continue;
        };
        crate::value_custody::struct_literals::each_selected_literal_field(
            program,
            literal,
            |value, field_type| validate_suffix_landing(program, value, field_type, diagnostics),
        );
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(value) if state.return_type.is_valid() => {
                        validate_suffix_landing(program, *value, state.return_type, diagnostics);
                    }
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) =
                            crate::value_custody::places::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                        {
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
                        for target in transition.targets() {
                            if let typed_trees::statement::TransitionTargetNode::Value(value) =
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
