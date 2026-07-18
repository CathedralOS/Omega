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
//!   CONTRACT FACT positions (`requires`/`ensures` facts, ranking-witness
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
                    }
                    _ => {}
                }
            }
        }

        // Fire G: contract facts and ranking witnesses live in the proof
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
            .expression_handles(machine.ranking_witness.subjects)
        {
            bless_fact_literals(program, *measure, &mut blessed);
        }
        for argument in program
            .expression_table
            .expression_handles(machine.ranking_witness.view_arguments)
        {
            bless_fact_literals(program, *argument, &mut blessed);
        }
        if machine.ranking_witness.range.is_present() {
            bless_fact_literals(program, machine.ranking_witness.range.start, &mut blessed);
            bless_fact_literals(program, machine.ranking_witness.range.end, &mut blessed);
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
/// place lands that format ON ITS TEXT CARRIER, so every downstream read --
/// native store, guard compare, argument materialization, AND the
/// interpreter's eval -- rounds ONCE, correctly, from the decimal spelling.
/// Without the stamp an anonymous literal at an f32 place takes the
/// transitional f64-then-narrow route (double rounding; the
/// 8388609.499999999999999 witness lands on the wrong side of the tie).
///
/// The walk enumerates EXACTLY the destinations of `validate_suffix_landings`
/// below -- keep the two in lockstep. It propagates the destination format
/// through a homogeneous arithmetic spine. A wholly anonymous constant
/// subtree evaluates as exact rational arithmetic and rounds once where it
/// meets that format; a landed operand instead keeps its riding format and
/// makes subsequent operations round per node. Runs on the still-mutable typed
/// tree BEFORE validation, so both engines consume one stamped/folded tree.
pub fn land_float_literal_destinations(program: &mut TypedTrees) {
    let pairs = literal_destination_pairs(program);

    for (value, declared) in pairs {
        let Some(format) = destination_float_format(program, declared) else {
            continue;
        };
        land_float_expression(program, value, format);
    }

    // Guards are not value destinations, but a float place supplies the
    // contextual format for the opposite adaptive constant/expression leg.
    // Collect first, then mutate the table, so native and interpreter receive
    // the same landed/exact-folded guard tree before their pipeline fork.
    let mut guard_landings = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Transition(transition) = statement
                    && let omega_typed_trees::statement::TransitionGuardNode::When(guard) =
                        transition.guard
                {
                    collect_float_guard_landings(
                        program,
                        machine,
                        state,
                        guard,
                        &mut guard_landings,
                    );
                }
            }
        }
    }
    for (expression, format) in guard_landings {
        land_float_expression(program, expression, format);
    }
    fold_anonymous_float_comparisons(program);
}

fn destination_float_format(
    program: &TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<omega_core::literals::FloatFormat> {
    use omega_typed_trees::types::TypeReferenceNode;

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => destination_float_format(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            destination_float_format(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            destination_float_format(program, *element_type)
        }
        TypeReferenceNode::Named { name, .. } => match PrimitiveType::from_name(name.as_str())? {
            PrimitiveType::F32 => Some(omega_core::literals::FloatFormat::F32),
            PrimitiveType::F64 => Some(omega_core::literals::FloatFormat::F64),
            _ => None,
        },
        _ => None,
    }
}

fn collect_float_guard_landings(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    expression: ExpressionHandle,
    landings: &mut Vec<(ExpressionHandle, omega_core::literals::FloatFormat)>,
) {
    use omega_typed_trees::expression::BinaryOperator;

    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        collect_float_guard_landings(program, machine, state, binary.left, landings);
        collect_float_guard_landings(program, machine, state, binary.right, landings);
        return;
    }
    // Multi-arm guard desugaring wraps the spelled comparison as
    // `(subject) == true`; peel that boolean shell to reach the actual float
    // comparison and its place-supplied format.
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        if matches!(
            program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(_)
        ) {
            collect_float_guard_landings(program, machine, state, binary.right, landings);
            return;
        }
        if matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(_)
        ) {
            collect_float_guard_landings(program, machine, state, binary.left, landings);
            return;
        }
    }
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        return;
    }
    let left = float_place_format(program, machine, state, binary.left);
    let right = float_place_format(program, machine, state, binary.right);
    if let Some(format) = left
        && right.is_none()
    {
        landings.push((binary.right, format));
    }
    if let Some(format) = right
        && left.is_none()
    {
        landings.push((binary.left, format));
    }
}

fn float_place_format(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<omega_core::literals::FloatFormat> {
    let declared =
        crate::places::declared_place_type_raw(program, machine, Some(state), expression)?;
    destination_float_format(program, declared)
}

fn land_float_expression(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    format: omega_core::literals::FloatFormat,
) {
    let node = program.expression_table.expression(expression).clone();
    match node {
        ExpressionNode::Mutable(inner) => land_float_expression(program, inner, format),
        ExpressionNode::ArrayLiteral(values) => {
            let values = program.expression_table.expression_handles(values).to_vec();
            for value in values {
                land_float_expression(program, value, format);
            }
        }
        ExpressionNode::Cast(cast) => {
            let cast_format = program
                .expression_table
                .name_path_members(cast.target_type)
                .last()
                .and_then(|name| PrimitiveType::from_name(name.as_str()))
                .and_then(|primitive| match primitive {
                    PrimitiveType::F32 => Some(omega_core::literals::FloatFormat::F32),
                    PrimitiveType::F64 => Some(omega_core::literals::FloatFormat::F64),
                    _ => None,
                });
            if let Some(cast_format) = cast_format {
                land_float_expression(program, cast.value, cast_format);
            }
        }
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            *program.expression_table.expression_mut(expression) =
                ExpressionNode::Float(literal.with_landing(format));
        }
        ExpressionNode::Binary(binary) => {
            if let Some(exact) =
                exact_anonymous_float_expression(&program.expression_table, expression)
            {
                let rounded = match format {
                    omega_core::literals::FloatFormat::F32 => f64::from(exact.to_f32()),
                    omega_core::literals::FloatFormat::F64 => exact.to_f64(),
                };
                let literal =
                    omega_core::literals::FloatLiteral::from_f64(rounded).with_landing(format);
                *program.expression_table.expression_mut(expression) =
                    ExpressionNode::Float(literal);
            } else {
                land_float_expression(program, binary.left, format);
                land_float_expression(program, binary.right, format);
            }
        }
        _ => {}
    }
}

fn fold_anonymous_float_comparisons(program: &mut TypedTrees) {
    use omega_typed_trees::expression::BinaryOperator;

    let mut folded = Vec::new();
    for (expression, node) in program.expression_table.expression_entries() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        ) {
            continue;
        }
        let Some(left) = exact_anonymous_float_expression(&program.expression_table, binary.left)
        else {
            continue;
        };
        let Some(right) = exact_anonymous_float_expression(&program.expression_table, binary.right)
        else {
            continue;
        };
        let ordering = left.partial_cmp_value(&right);
        let value = match binary.operator {
            BinaryOperator::Equal => left.equal_value(&right),
            BinaryOperator::NotEqual => !left.equal_value(&right),
            BinaryOperator::Less => ordering == Some(std::cmp::Ordering::Less),
            BinaryOperator::LessOrEqual => {
                matches!(
                    ordering,
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                )
            }
            BinaryOperator::Greater => ordering == Some(std::cmp::Ordering::Greater),
            BinaryOperator::GreaterOrEqual => matches!(
                ordering,
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            ),
            _ => unreachable!(),
        };
        folded.push((expression, value));
    }
    for (expression, value) in folded {
        *program.expression_table.expression_mut(expression) = ExpressionNode::Boolean(value);
    }
}

fn exact_anonymous_float_expression(
    table: &omega_typed_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> Option<omega_core::bignum::ExactFloat> {
    use omega_core::bignum::ExactFloat;

    match table.expression(expression) {
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            ExactFloat::from_decimal_str(literal.text())
        }
        ExpressionNode::Mutable(inner) => exact_anonymous_float_expression(table, *inner),
        ExpressionNode::Binary(binary) => {
            let left = exact_anonymous_float_expression(table, binary.left)?;
            let right = exact_anonymous_float_expression(table, binary.right)?;
            Some(match binary.operator {
                omega_typed_trees::expression::BinaryOperator::Add => left.add(&right),
                omega_typed_trees::expression::BinaryOperator::Subtract => left.sub(&right),
                omega_typed_trees::expression::BinaryOperator::Multiply => left.mul(&right),
                omega_typed_trees::expression::BinaryOperator::Divide => left.div(&right),
                _ => return None,
            })
        }
        _ => None,
    }
}

fn literal_destination_pairs(
    program: &TypedTrees,
) -> Vec<(
    ExpressionHandle,
    omega_typed_trees::types::TypeReferenceHandle,
)> {
    let mut pairs = Vec::new();

    for (_, node) in program.expression_table.expression_entries() {
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
            ExpressionNode::Call(call) => append_call_destination_pairs(
                program,
                call.target_symbol,
                program.expression_table.expression_handles(call.arguments),
                &mut pairs,
            ),
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
                    StatementNode::Call(call) => append_call_destination_pairs(
                        program,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                        &mut pairs,
                    ),
                    StatementNode::LocalData(local) => {
                        if local.initial_value.is_valid() && local.type_reference.is_valid() {
                            pairs.push((local.initial_value, local.type_reference));
                        }
                    }
                    StatementNode::Transition(transition) => {
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                omega_typed_trees::statement::TransitionTargetNode::Named {
                                    path,
                                    arguments,
                                } => append_call_destination_pairs(
                                    program,
                                    path.symbol,
                                    program.statement_table.expression_handles(*arguments),
                                    &mut pairs,
                                ),
                                omega_typed_trees::statement::TransitionTargetNode::Value(
                                    value,
                                ) if state.return_type.is_valid() => {
                                    pairs.push((*value, state.return_type));
                                }
                                _ => {}
                            }
                        }
                    }
                    StatementNode::Expression(_) => {}
                }
            }
        }
    }
    pairs
}

fn append_call_destination_pairs(
    program: &TypedTrees,
    target_symbol: omega_core::symbols::SymbolHandle,
    arguments: &[ExpressionHandle],
    pairs: &mut Vec<(
        ExpressionHandle,
        omega_typed_trees::types::TypeReferenceHandle,
    )>,
) {
    let Some(parameters) = call_target_parameters(program, target_symbol) else {
        return;
    };
    pairs.extend(
        arguments
            .iter()
            .zip(parameters.iter().filter(|parameter| !parameter.is_self))
            .map(|(argument, parameter)| (*argument, parameter.type_reference)),
    );
}

fn call_target_parameters(
    program: &TypedTrees,
    target_symbol: omega_core::symbols::SymbolHandle,
) -> Option<&[omega_typed_trees::signature::StateParameter]> {
    if !target_symbol.is_valid() {
        return None;
    }
    for machine in program.machines() {
        if let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
        {
            return Some(program.state_parameters(state));
        }
    }
    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
        {
            return Some(program.state_signature_parameters(signature));
        }
    }
    for platform in program.platforms() {
        if let Some(signature) = program
            .platform_state_signatures(platform)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
        {
            return Some(program.state_signature_parameters(signature));
        }
    }
    None
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

    for (value, declared) in literal_destination_pairs(program) {
        check(value, declared, diagnostics);
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
