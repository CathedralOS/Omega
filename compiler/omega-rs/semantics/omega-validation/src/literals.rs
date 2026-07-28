//! D14 literal-width gate: which positions may hold a u64-magnitude literal.
//!
//! Literals are anonymous payloads with no parse-time ceiling (see
//! `omega_core::literals::IntegerLiteral`), so a u64-magnitude spelling like
//! `18446744073709551615` PARSES. Every consumer reads literals through the
//! i64 value window (`value_i64()`) and defers/degrades on `None`; the backend
//! write path additionally materializes full 8-byte patterns via `bits_u64()`.
//! That is only sound because THIS gate guarantees an oversize literal reaches
//! exactly the positions that handle it:
//!
//! - **Accepted (fire C):** the direct RHS of an assignment whose target's
//!   declared primitive is u64-classed (`u64`/`usize`/`addr`) -- an 8-byte
//!   slot, so the two's-complement bit pattern the write path emits is the
//!   value.
//! - **Accepted (fire D):** a struct-literal FIELD value whose declared field
//!   type is u64-classed (`Duration { seconds: 18446744073709551615, ... }`)
//!   -- same 8-byte-slot argument; the construction write cascade reads
//!   literals through the same bits-capable resolvers. (The interval side already agrees: an oversize-positive literal's
//!   honest over-approximation `[i64::MAX, +inf)` fits only u64-classed
//!   target ranges. The gate must stay PRECISE regardless -- a `u32 in
//!   Wrapping` target bypasses the interval store-check entirely, and only
//!   this gate stands between such a slot and a silent truncation.)
//! - **Accepted (fire E, 2026-07-11n):** a LET initializer into a local whose
//!   declared type is u64-classed -- same 8-byte-slot argument; the frame
//!   write path reads through the bits-capable static resolver.
//! - **Accepted (fire F, 2026-07-11n):** an EQUALITY (`==`/`!=` ONLY) guard
//!   leg whose other operand is a u64-classed place, at any depth of the
//!   guard's boolean structure (the multi-arm desugar nests the spelled
//!   compare inside `(subject) == true`). Ordering stays refused: the
//!   compare the encoder emits is SIGNED, and a bit-pattern u64 under it is
//!   sign-blind.
//! - **Accepted (fire G, 2026-07-11, math roster N2):** literals inside
//!   CONTRACT FACT positions (`requires`/`ensures` facts, `decreases`
//!   measures) at ANY magnitude -- contracts never lower to runtime bytes;
//!   their one consumer is the proof engine, which reads literals exactly
//!   (`value_bignum`, N2 bignum coefficients). This is the position where
//!   the old i64 window silently downgraded provable facts.
//! - **Everything else** (arithmetic operands, ordering guards,
//!   call/transition arguments, narrower or signed targets): one clear
//!   error.
//!
//! Growing acceptance to a new position = extend `u64_blessed_literals` AND
//! the consumer that materializes it, in the SAME change.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::PrimitiveType;

pub(crate) fn validate_literal_widths(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let blessed = u64_blessed_literals(program);
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Integer(literal) = node
            && literal.value_i64().is_none()
            && !blessed.contains(&handle)
        {
            diagnostics.push(Diagnostic::error(format!(
                "integer literal `{literal}` exceeds the i64 range; only a direct \
                 assignment into a `u64`/`usize`/`addr` place accepts a u64-magnitude \
                 literal so far (bind it to such a place first, or wait for this \
                 position's typed lowering -- TASKS_TIME.md D14)"
            )));
        }
    }
}

/// The u64-magnitude literals sitting in an ACCEPTED position: the direct RHS
/// of an assignment to a u64-classed place, or a struct-literal field whose
/// declared field type is u64-classed.
fn u64_blessed_literals(program: &TypedTrees) -> Vec<ExpressionHandle> {
    // A handful of entries at most -- a Vec beats hashing arena handles.
    let mut blessed = Vec::new();

    // Struct-literal fields (position-independent: wherever the literal is
    // constructed, the field slot's declared type is what matters).
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
            let ExpressionNode::Integer(value) = program.expression_table.expression(field.value)
            else {
                continue;
            };
            if value.value_i64().is_some() || value.value_u64().is_none() {
                continue;
            }
            let Some(field_type) = crate::struct_literals::construction_field_type(
                program,
                data_definition,
                literal.case_name.as_ref().map(|name| name.as_str()),
                field.name.as_str(),
            ) else {
                continue;
            };
            let Some(unwrapped) = crate::places::unwrapped_type_reference(program, field_type)
            else {
                continue;
            };
            let Some(primitive) = program.primitive_type_reference(unwrapped) else {
                continue;
            };
            if matches!(primitive, PrimitiveType::U64 | PrimitiveType::Addr) {
                blessed.push(field.value);
            }
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if oversize_literal(program, assignment.value)
                            && place_is_u64_classed(program, machine, state, assignment.target)
                        {
                            blessed.push(assignment.value);
                        }
                    }
                    // Fire E (2026-07-11n): a LET initializer into a local whose
                    // DECLARED type is u64-classed -- same 8-byte-slot argument
                    // as the assignment fire; the frame-slot write path reads
                    // literals through the bits-capable static resolver.
                    StatementNode::LocalData(local) => {
                        if !oversize_literal(program, local.initial_value) {
                            continue;
                        }
                        let Some(unwrapped) =
                            crate::places::unwrapped_type_reference(program, local.type_reference)
                        else {
                            continue;
                        };
                        if primitive_is_u64_classed(program, unwrapped) {
                            blessed.push(local.initial_value);
                        }
                    }
                    // Fire F (2026-07-11n): an EQUALITY compare (== / != ONLY --
                    // ordering under two's-complement bit patterns is
                    // sign-blind and stays refused) in a transition GUARD whose
                    // other operand is a u64-classed place. The guard's static
                    // compare is an 8-byte bit-pattern equality on both
                    // engines, so the bit-cast IS the value.
                    StatementNode::Transition(transition) => {
                        if let omega_typed_trees::statement::TransitionGuardNode::When(guard) =
                            transition.guard
                        {
                            bless_equality_guard_literals(
                                program,
                                machine,
                                state,
                                guard,
                                &mut blessed,
                            );
                        }
                        // Fire H (2026-07-16, the CR3 remaining face): a
                        // TRANSITION ARGUMENT into a u64-classed target-state
                        // parameter -- the arg IS a delivery into that
                        // declared slot (the same law the F2c float stamping
                        // rides), and the frame-slot arg writer already reads
                        // literals through the bits-capable static resolver.
                        // Same-machine Named targets only, receiver filtered.
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            let omega_typed_trees::statement::TransitionTargetNode::Named {
                                path,
                                arguments,
                            } = program.statement_table.transition_target(target)
                            else {
                                continue;
                            };
                            let target_members =
                                program.statement_table.name_path_members(path.members);
                            let [target_name] = target_members else {
                                continue;
                            };
                            let Some(target_state) = program
                                .machine_states(machine)
                                .iter()
                                .find(|candidate| candidate.name.as_str() == target_name.as_str())
                            else {
                                continue;
                            };
                            let parameters = program
                                .state_parameters(target_state)
                                .iter()
                                .filter(|parameter| !parameter.is_self);
                            let argument_handles =
                                program.statement_table.expression_handles(*arguments);
                            for (parameter, argument) in
                                parameters.zip(argument_handles.iter().copied())
                            {
                                if !oversize_literal(program, argument) {
                                    continue;
                                }
                                let Some(unwrapped) = crate::places::unwrapped_type_reference(
                                    program,
                                    parameter.type_reference,
                                ) else {
                                    continue;
                                };
                                if primitive_is_u64_classed(program, unwrapped) {
                                    blessed.push(argument);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Fire G: contract facts and decreases measures live in the proof
        // domain only; the exact engine is their sole reader.
        for contract in program.machine_contracts(machine) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let omega_typed_trees::domain::ProofFact::Expression(expression) = fact {
                    bless_fact_literals(program, *expression, &mut blessed);
                }
            }
        }
        for measure in program
            .expression_table
            .expression_handles(machine.decreases)
        {
            bless_fact_literals(program, *measure, &mut blessed);
        }
    }
    blessed
}

/// Fire G's walk: every integer literal under a fact expression, any
/// magnitude (beyond-u64 included -- the engine is exact).
fn bless_fact_literals(
    program: &TypedTrees,
    expression: ExpressionHandle,
    blessed: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            if literal.value_i64().is_none() {
                blessed.push(expression);
            }
        }
        ExpressionNode::Binary(binary) => {
            bless_fact_literals(program, binary.left, blessed);
            bless_fact_literals(program, binary.right, blessed);
        }
        ExpressionNode::Unary(unary) => {
            bless_fact_literals(program, unary.operand, blessed);
        }
        ExpressionNode::Mutable(inner) => {
            bless_fact_literals(program, *inner, blessed);
        }
        ExpressionNode::Call(call) => {
            for argument in program.expression_table.expression_handles(call.arguments) {
                bless_fact_literals(program, *argument, blessed);
            }
        }
        _ => {}
    }
}

/// CR4 (carrier, ch5 two-phase law): a width-SUFFIXED literal (`5i8`) landed
/// at parse time, so a destination whose declared type DISAGREES with the
/// suffix is a loud error -- silently stripping the suffix (yesterday's
/// behavior) let a wrong suffix mean nothing; silently honoring it would
/// steer signedness/width decisions against the declared type. Domain is NOT
/// checked (a suffix lands the TYPE; the destination's arithmetic domain is
/// contextual and governs its own folds). Checked at the same destination
/// positions the width gate enumerates: let initializers, assignments, and
/// struct-literal fields, with the literal read through `Mutable` wrappers.
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
                        value <= (1u64 << width) - 1
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

/// F2b -- float DESTINATION stamping (ch5 two-phase constants, the float
/// half): an UNSUFFIXED float literal initializing a declared `f32`/`f64`
/// place lands that format ON ITS VALUE. A wholly anonymous literal-arithmetic
/// tree is first evaluated as an exact rational and then replaced by one
/// landed literal, so every downstream reader -- native store, guard compare,
/// argument materialization, AND the interpreter -- consumes the same single
/// rounding. A nonconstant tree instead stamps its anonymous literal leaves;
/// its runtime operations then execute at the destination format.
/// Without this landing pass an anonymous literal at an f32 place takes the
/// transitional f64-then-narrow route (double rounding; the
/// 8388609.499999999999999 witness lands on the wrong side of the tie).
///
/// The walk enumerates EXACTLY the destinations of `validate_suffix_landings`
/// below (struct-literal fields, assignments, let locals) -- keep the two in
/// lockstep. Already-landed (suffixed) literals are untouched: their landing
/// was chosen at the spelling, and the suffix-vs-destination check owns any
/// disagreement. Runs on the still-mutable typed tree BEFORE validation, so
/// both engines consume one stamped tree.
pub fn land_float_literal_destinations(program: &mut TypedTrees) {
    use omega_core::literals::FloatFormat;

    let mut pairs: Vec<(
        ExpressionHandle,
        omega_typed_trees::types::TypeReferenceHandle,
    )> = Vec::new();
    let mut direct_formats: Vec<(ExpressionHandle, FloatFormat)> = Vec::new();
    let mut anonymous_comparisons: Vec<ExpressionHandle> = Vec::new();

    for (handle, node) in program.expression_table.expression_entries() {
        match node {
            ExpressionNode::StructLiteral(literal) => {
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
                    pairs.push((field.value, field_type));
                }
            }
            ExpressionNode::Cast(cast) => {
                let format =
                    program
                        .primitive_type_reference(cast.target_type)
                        .and_then(|primitive| match primitive {
                            PrimitiveType::F32 => Some(FloatFormat::F32),
                            PrimitiveType::F64 => Some(FloatFormat::F64),
                            _ => None,
                        });
                if let Some(format) = format {
                    direct_formats.push((cast.value, format));
                }
            }
            ExpressionNode::Call(call) => {
                let Some(callee) = program.machines().iter().find_map(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == call.target_symbol)
                }) else {
                    continue;
                };
                let parameters = program
                    .state_parameters(callee)
                    .iter()
                    .filter(|parameter| !parameter.is_self);
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied();
                for (parameter, argument) in parameters.zip(arguments) {
                    if parameter.type_reference.is_valid() {
                        pairs.push((argument, parameter.type_reference));
                    }
                }
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    omega_typed_trees::expression::BinaryOperator::Equal
                        | omega_typed_trees::expression::BinaryOperator::NotEqual
                        | omega_typed_trees::expression::BinaryOperator::Less
                        | omega_typed_trees::expression::BinaryOperator::LessOrEqual
                        | omega_typed_trees::expression::BinaryOperator::Greater
                        | omega_typed_trees::expression::BinaryOperator::GreaterOrEqual
                ) =>
            {
                anonymous_comparisons.push(handle);
            }
            _ => {}
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) {
                            pairs.push((assignment.value, declared));
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() && local.type_reference.is_valid() {
                            pairs.push((local.initial_value, local.type_reference));
                        }
                    }
                    // A float COMPARISON in a transition guard adopts the
                    // PLACE side's format (the operand-derived landing law,
                    // float flavor): `self.f == 16777216.0 + 1.0` with an f32
                    // place must fold/evaluate its literal side per-op at f32
                    // -- unstamped, the tree computed in the anonymous f64
                    // window and the engines diverged at the f32 precision
                    // cliff (2^24 + 1.0). Recursive like
                    // bless_equality_guard_literals: the multi-arm desugar
                    // wraps the spelled compare as `(subject) == true`, and
                    // conjunctions nest comparisons under And/Or legs.
                    // Suffixed literals keep their own landing (stamp-if-none
                    // in the shared loop below).
                    StatementNode::Transition(transition) => {
                        if let omega_typed_trees::statement::TransitionGuardNode::When(guard) =
                            transition.guard
                        {
                            collect_guard_float_comparison_pairs(
                                program, machine, state, guard, &mut pairs,
                            );
                        }
                        // A transition ARG adopts the TARGET state's declared
                        // param type (the arg IS a delivery into that
                        // destination -- same law as a `let`): `check(2.0e0 +
                        // tiny)` into `got: f32` folds/evaluates per-op at
                        // f32. Same-machine targets only (value-machine call
                        // args ride the Call statement, a later face).
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            let target_node = program.statement_table.transition_target(target);
                            if let omega_typed_trees::statement::TransitionTargetNode::Value(value) =
                                target_node
                                && state.return_type.is_valid()
                            {
                                pairs.push((*value, state.return_type));
                                continue;
                            }
                            let omega_typed_trees::statement::TransitionTargetNode::Named {
                                path,
                                arguments,
                            } = target_node
                            else {
                                continue;
                            };
                            // The Named target's path members live in the
                            // STATEMENT table's identifier arena (the target
                            // node's home), not the expression table's.
                            let target_members =
                                program.statement_table.name_path_members(path.members);
                            let [target_name] = target_members else {
                                continue;
                            };
                            let Some(target_state) = program
                                .machine_states(machine)
                                .iter()
                                .find(|candidate| candidate.name.as_str() == target_name.as_str())
                            else {
                                continue;
                            };
                            // The `&mut self` receiver rides the param list but
                            // never pairs with a spelled argument -- zip only the
                            // value params.
                            let parameters = program
                                .state_parameters(target_state)
                                .iter()
                                .filter(|parameter| !parameter.is_self);
                            let argument_handles =
                                program.statement_table.expression_handles(*arguments);
                            for (parameter, argument) in
                                parameters.zip(argument_handles.iter().copied())
                            {
                                if parameter.type_reference.is_valid() {
                                    pairs.push((argument, parameter.type_reference));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for (value, format) in direct_formats {
        land_float_tree(program, value, format);
    }
    for (value, declared) in pairs {
        land_float_value_for_type(program, value, declared);
    }
    // A comparison with no typed operand is itself the first destination: it
    // produces bool. Its anonymous float operands therefore compare as exact
    // values (including format-independent NaN/infinity), rather than each
    // independently falling through the transitional f64 window.
    for comparison in anonymous_comparisons {
        fold_anonymous_float_comparison(program, comparison);
    }
}

fn land_float_value_for_type(
    program: &mut TypedTrees,
    value: ExpressionHandle,
    declared: omega_typed_trees::types::TypeReferenceHandle,
) {
    use omega_typed_trees::types::TypeReferenceNode;

    let declared_node = program
        .type_reference_table
        .type_reference(declared)
        .clone();
    match declared_node {
        TypeReferenceNode::Reference { referee, .. } => {
            land_float_value_for_type(program, value, referee);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            land_float_value_for_type(program, value, base_type);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            let ExpressionNode::ArrayLiteral(elements) =
                program.expression_table.expression(value).clone()
            else {
                return;
            };
            let elements = program
                .expression_table
                .expression_handles(elements)
                .to_vec();
            for element in elements {
                land_float_value_for_type(program, element, element_type);
            }
        }
        TypeReferenceNode::Named { .. } => {
            let format = match program.primitive_type_reference(declared) {
                Some(PrimitiveType::F32) => omega_core::literals::FloatFormat::F32,
                Some(PrimitiveType::F64) => omega_core::literals::FloatFormat::F64,
                _ => return,
            };
            land_float_tree(program, value, format);
        }
        TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn land_float_tree(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    format: omega_core::literals::FloatFormat,
) {
    if let Some(exact) = anonymous_exact_float_tree(program, expression) {
        let semantic_format = match format {
            omega_core::literals::FloatFormat::F32 => {
                omega_core::float_semantics::FloatFormat::BINARY32
            }
            omega_core::literals::FloatFormat::F64 => {
                omega_core::float_semantics::FloatFormat::BINARY64
            }
        };
        let value =
            omega_core::float_semantics::FloatSemantics::round_exact(semantic_format, exact)
                .to_interpreter_value(semantic_format);
        *program.expression_table.expression_mut(expression) = ExpressionNode::Float(
            omega_core::literals::FloatLiteral::from_f64(value).with_landing(format),
        );
    } else {
        stamp_float_tree(program, expression, format);
    }
}

fn anonymous_exact_float_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<omega_core::bignum::ExactFloat> {
    use omega_typed_trees::expression::BinaryOperator;

    match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            omega_core::bignum::ExactFloat::from_decimal_str(literal.text())
        }
        ExpressionNode::Mutable(inner) => anonymous_exact_float_tree(program, *inner),
        ExpressionNode::Binary(binary) => {
            let left = anonymous_exact_float_tree(program, binary.left)?;
            let right = anonymous_exact_float_tree(program, binary.right)?;
            Some(match binary.operator {
                BinaryOperator::Add => left.add(&right),
                BinaryOperator::Subtract => left.sub(&right),
                BinaryOperator::Multiply => left.mul(&right),
                BinaryOperator::Divide => left.div(&right),
                _ => return None,
            })
        }
        _ => None,
    }
}

fn fold_anonymous_float_comparison(program: &mut TypedTrees, expression: ExpressionHandle) {
    use omega_typed_trees::expression::BinaryOperator;
    use std::cmp::Ordering;

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression).clone()
    else {
        return;
    };
    let Some(left) = anonymous_exact_float_tree(program, binary.left) else {
        return;
    };
    let Some(right) = anonymous_exact_float_tree(program, binary.right) else {
        return;
    };
    let value = match binary.operator {
        BinaryOperator::Equal => left.equal_value(&right),
        BinaryOperator::NotEqual => !left.equal_value(&right),
        BinaryOperator::Greater => left.partial_cmp_value(&right) == Some(Ordering::Greater),
        BinaryOperator::GreaterOrEqual => matches!(
            left.partial_cmp_value(&right),
            Some(Ordering::Greater | Ordering::Equal)
        ),
        BinaryOperator::Less => left.partial_cmp_value(&right) == Some(Ordering::Less),
        BinaryOperator::LessOrEqual => matches!(
            left.partial_cmp_value(&right),
            Some(Ordering::Less | Ordering::Equal)
        ),
        _ => return,
    };
    *program.expression_table.expression_mut(expression) = ExpressionNode::Boolean(value);
}

/// Collect (float-literal-tree, place-declared-type) pairs from every
/// comparison reachable in a guard expression: And/Or legs recurse, and each
/// Equal/NotEqual/ordering node pairs its literal side with its place side's
/// declared type (either orientation). The multi-arm desugar wraps the spelled
/// compare as `(subject) == true`, so comparisons nest inside equality legs --
/// recurse through comparison legs too, exactly like
/// bless_equality_guard_literals.
fn collect_guard_float_comparison_pairs(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    pairs: &mut Vec<(
        ExpressionHandle,
        omega_typed_trees::types::TypeReferenceHandle,
    )>,
) {
    if !guard.is_valid() {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };
    use omega_typed_trees::expression::BinaryOperator;
    match binary.operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => {
            if let Some(declared) =
                crate::places::declared_place_type_raw(program, machine, Some(state), binary.left)
            {
                pairs.push((binary.right, declared));
            } else if let Some(declared) =
                crate::places::declared_place_type_raw(program, machine, Some(state), binary.right)
            {
                pairs.push((binary.left, declared));
            }
            collect_guard_float_comparison_pairs(program, machine, state, binary.left, pairs);
            collect_guard_float_comparison_pairs(program, machine, state, binary.right, pairs);
        }
        BinaryOperator::And | BinaryOperator::Or => {
            collect_guard_float_comparison_pairs(program, machine, state, binary.left, pairs);
            collect_guard_float_comparison_pairs(program, machine, state, binary.right, pairs);
        }
        _ => {}
    }
}

/// Stamp every UNSTAMPED float literal reachable through Mutable/Unary/Binary
/// wrappers with `format` when the tree is not wholly anonymous and constant.
/// This is the runtime-expression case: operations execute per-op at the
/// landed width. Suffixed literals keep their own landing (disagreement is
/// validate_suffix_landings' domain). Deliberately does NOT descend into
/// calls/indexes/places.
fn stamp_float_tree(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    format: omega_core::literals::FloatFormat,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            let inner = *inner;
            stamp_float_tree(program, inner, format);
        }
        ExpressionNode::Unary(unary) => {
            let operand = unary.operand;
            stamp_float_tree(program, operand, format);
        }
        ExpressionNode::Binary(binary) => {
            let (left, right) = (binary.left, binary.right);
            stamp_float_tree(program, left, format);
            stamp_float_tree(program, right, format);
        }
        ExpressionNode::Float(literal) => {
            if literal.landing().is_none() {
                let landed = literal.with_landing(format);
                *program.expression_table.expression_mut(expression) =
                    ExpressionNode::Float(landed);
            }
        }
        _ => {}
    }
}

pub(crate) fn validate_suffix_landings(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    use omega_core::literals::LandedIntegerType;

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
                    ExpressionNode::Mutable(inner) => current = *inner,
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
        omega_core::literals::FloatFormat,
    )> {
        let mut current = expression;
        loop {
            match program.expression_table.expression(current) {
                ExpressionNode::Mutable(inner) => current = *inner,
                ExpressionNode::Float(literal) => {
                    return literal.landing().map(|landing| (current, landing));
                }
                _ => return None,
            }
        }
    };

    let check = |value: ExpressionHandle,
                 declared: omega_typed_trees::types::TypeReferenceHandle,
                 diagnostics: &mut Vec<Diagnostic>| {
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
            use omega_core::literals::FloatFormat;
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
    };

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
            check(field.value, field_type, diagnostics);
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Assignment(assignment) => {
                        if let Some(declared) = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        ) {
                            check(assignment.value, declared, diagnostics);
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() && local.type_reference.is_valid() {
                            check(local.initial_value, local.type_reference, diagnostics);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn oversize_literal(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            literal.value_i64().is_none() && literal.value_u64().is_some()
        }
        _ => false,
    }
}

fn primitive_is_u64_classed(
    program: &TypedTrees,
    unwrapped: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        program.primitive_type_reference(unwrapped),
        Some(PrimitiveType::U64 | PrimitiveType::Addr)
    )
}

fn place_is_u64_classed(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    place: ExpressionHandle,
) -> bool {
    let Some(declared) =
        crate::places::declared_place_type_raw(program, machine, Some(state), place)
    else {
        return false;
    };
    let Some(unwrapped) = crate::places::unwrapped_type_reference(program, declared) else {
        return false;
    };
    primitive_is_u64_classed(program, unwrapped)
}

/// Fire F's walk: descend a guard expression's `&&`/`||` conjunctions and
/// bless oversize literals sitting in `==`/`!=` legs whose OTHER operand is
/// a u64-classed place.
fn bless_equality_guard_literals(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    blessed: &mut Vec<ExpressionHandle>,
) {
    if !guard.is_valid() {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return;
    };
    use omega_typed_trees::expression::BinaryOperator;
    match binary.operator {
        BinaryOperator::And | BinaryOperator::Or => {
            bless_equality_guard_literals(program, machine, state, binary.left, blessed);
            bless_equality_guard_literals(program, machine, state, binary.right, blessed);
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            for (literal, other) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if oversize_literal(program, literal)
                    && place_is_u64_classed(program, machine, state, other)
                {
                    blessed.push(literal);
                }
            }
            // The multi-arm desugar wraps an arm's guard as
            // `(subject) == true` -- the spelled compare NESTS inside an
            // equality leg. Recurse so `self.stored == <u64 literal>` is
            // found at any depth of the boolean structure.
            bless_equality_guard_literals(program, machine, state, binary.left, blessed);
            bless_equality_guard_literals(program, machine, state, binary.right, blessed);
        }
        _ => {}
    }
}
