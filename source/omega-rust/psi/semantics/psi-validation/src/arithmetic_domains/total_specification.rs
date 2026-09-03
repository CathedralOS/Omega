//! Total-proposition arithmetic formation checks.
//!
//! Runtime overflow analysis remains in the parent. This module owns the
//! separate rule that every arithmetic term admitted into a proposition has a
//! total denotation, for both concrete and abstract contract owners.

use super::guard_narrowing::{comparison_bound, narrow_env_by_condition};
use super::*;

/// Reject runtime-control arithmetic from proof positions. A contract is a
/// total proposition: it may compare or bitwise-inspect a Trapping-qualified
/// value, but an arithmetic operation selected by that qualification cannot
/// become a proposition term. Explicit Exact casts remove the qualification
/// before this check; Wrapping and Saturating retain their total denotations.
fn validate_total_specification_arithmetic(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    env: &ValueEnv,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    exact_division_definedness::validate_concrete(
        program,
        machine,
        state,
        expression,
        env,
        owner,
        diagnostics,
    );
    // Proposition terms retain the ordinary Exact formation judgment. Run it
    // before admitting this fact into `env`: a bound written around the very
    // operation being formed cannot prove that operation total.
    let mut formation_diagnostics = Vec::new();
    analyze(
        program,
        machine,
        state,
        expression,
        env,
        None,
        ArithmeticDomain::Exact,
        owner,
        &mut formation_diagnostics,
    );
    // Runtime-only unconditional-Trapping warnings have no meaning in Prop;
    // the totality walker below emits the directed hard error instead.
    diagnostics.extend(
        formation_diagnostics
            .into_iter()
            .filter(Diagnostic::is_error),
    );
    let expression_domain = |candidate| {
        let mut ignored_diagnostics = Vec::new();
        analyze(
            program,
            machine,
            state,
            candidate,
            env,
            None,
            ArithmeticDomain::Exact,
            owner,
            &mut ignored_diagnostics,
        )
        .domain
    };
    let expression_interval = |candidate| {
        let mut ignored_diagnostics = Vec::new();
        Some(
            analyze(
                program,
                machine,
                state,
                candidate,
                env,
                None,
                ArithmeticDomain::Exact,
                owner,
                &mut ignored_diagnostics,
            )
            .interval,
        )
    };
    validate_total_specification_arithmetic_with_domain_lookup(
        program,
        expression,
        owner,
        &expression_domain,
        &expression_interval,
        true,
        diagnostics,
    );
}

fn validate_total_specification_arithmetic_with_domain_lookup(
    program: &TypedTrees,
    expression: ExpressionHandle,
    owner: &str,
    expression_domain: &impl Fn(ExpressionHandle) -> Option<ArithmeticDomain>,
    expression_interval: &impl Fn(ExpressionHandle) -> Option<Interval>,
    provably_zero_is_prevalidated: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn selected_domain(
        program: &TypedTrees,
        expression: ExpressionHandle,
        expression_domain: &impl Fn(ExpressionHandle) -> Option<ArithmeticDomain>,
    ) -> Option<ArithmeticDomain> {
        if !expression.is_valid() {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast) if !cast.form.is_recast() => Some(cast.domain),
            ExpressionNode::Borrow(value) => {
                selected_domain(program, value.target, expression_domain)
            }
            ExpressionNode::Unary(unary) => {
                selected_domain(program, unary.operand, expression_domain)
            }
            ExpressionNode::Binary(binary) => {
                let left = selected_domain(program, binary.left, expression_domain);
                if matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) {
                    return left;
                }
                let right = selected_domain(program, binary.right, expression_domain);
                match (left, right) {
                    (Some(ArithmeticDomain::Exact), Some(right)) => Some(right),
                    (Some(left), Some(_)) => Some(left),
                    (Some(domain), None) | (None, Some(domain)) => Some(domain),
                    (None, None) => None,
                }
            }
            ExpressionNode::Call(call)
                if resolve_named_float_arithmetic(program, call).is_some() =>
            {
                let mut selected = None;
                for argument in program.expression_table.expression_handles(call.arguments) {
                    let Some(domain) = selected_domain(program, *argument, expression_domain)
                    else {
                        continue;
                    };
                    selected = match selected {
                        None | Some(ArithmeticDomain::Exact) => Some(domain),
                        Some(current) => Some(current),
                    };
                }
                selected
            }
            _ => expression_domain(expression),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: &str,
        expression_domain: &impl Fn(ExpressionHandle) -> Option<ArithmeticDomain>,
        expression_interval: &impl Fn(ExpressionHandle) -> Option<Interval>,
        provably_zero_is_prevalidated: bool,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);

        let recurse = |child, diagnostics: &mut Vec<Diagnostic>, visited: &mut Vec<_>| {
            walk(
                program,
                child,
                owner,
                expression_domain,
                expression_interval,
                provably_zero_is_prevalidated,
                diagnostics,
                visited,
            );
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => {
                for child in program.expression_table.expression_handles(*values) {
                    recurse(*child, diagnostics, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                recurse(atomic.value, diagnostics, visited);
                recurse(atomic.result, diagnostics, visited);
            }
            ExpressionNode::Binary(binary) => {
                recurse(binary.left, diagnostics, visited);
                recurse(binary.right, diagnostics, visited);
                if !is_arithmetic(binary.operator) {
                    return;
                }

                let left = selected_domain(program, binary.left, expression_domain);
                let right = selected_domain(program, binary.right, expression_domain);
                let selected = if matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                ) {
                    left
                } else {
                    match (left, right) {
                        (Some(ArithmeticDomain::Exact), Some(right)) => Some(right),
                        (Some(left), Some(_)) => Some(left),
                        (Some(domain), None) | (None, Some(domain)) => Some(domain),
                        (None, None) => None,
                    }
                };
                if selected == Some(ArithmeticDomain::Trapping) {
                    diagnostics.push(Diagnostic::error(format!(
                        "direct Trapping arithmetic `{}` is illegal in {owner}: specification terms are total and cannot transfer runtime control. Use `embed(..)` for unbounded proof `Int` mathematics, or explicitly cast the operands to the same unqualified carrier and discharge the resulting Exact formation obligations",
                        arithmetic_operator_spelling(binary.operator),
                    )));
                }
                if matches!(
                    selected,
                    Some(ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating)
                ) && matches!(
                    binary.operator,
                    BinaryOperator::Divide | BinaryOperator::Modulo
                ) {
                    let divisor = expression_interval(binary.right);
                    let proven_nonzero = divisor.is_some_and(Interval::excludes_zero);
                    let provably_zero = divisor.is_some_and(Interval::is_exactly_zero);
                    if !(proven_nonzero || provably_zero_is_prevalidated && provably_zero) {
                        let operation = if binary.operator == BinaryOperator::Divide {
                            "division"
                        } else {
                            "remainder"
                        };
                        diagnostics.push(Diagnostic::error(format!(
                            "partial {operation} in {owner}: `{}` defines carrier overflow but not a zero divisor; the divisor must be proven nonzero by an independently accepted prior fact",
                            selected.expect("selected total arithmetic policy").name(),
                        )));
                    }
                }
            }
            ExpressionNode::Cast(cast) => {
                recurse(cast.value, diagnostics, visited);
                if !cast.form.is_recast() && cast.domain == ArithmeticDomain::Trapping {
                    diagnostics.push(Diagnostic::error(format!(
                        "direct Trapping conversion is illegal in {owner}: specification terms are total and cannot transfer runtime control. Use a total Exact or Saturating conversion and discharge its formation obligations",
                    )));
                }
            }
            ExpressionNode::Call(call) => {
                recurse(call.receiver, diagnostics, visited);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    recurse(*argument, diagnostics, visited);
                }
                if resolve_named_float_arithmetic(program, call).is_some()
                    && selected_domain(program, expression, expression_domain)
                        == Some(ArithmeticDomain::Trapping)
                {
                    diagnostics.push(Diagnostic::error(format!(
                            "direct Trapping named float operation `{}` is illegal in {owner}: specification terms are total and cannot transfer runtime control. Use `Float::meaning32`/`Float::meaning64` or an explicitly total policy",
                            call.target,
                        )));
                }
            }
            ExpressionNode::Indexed(indexed) => {
                recurse(indexed.collection, diagnostics, visited);
                recurse(indexed.index, diagnostics, visited);
            }
            ExpressionNode::Member(member) => recurse(member.receiver, diagnostics, visited),
            ExpressionNode::Borrow(value) => recurse(value.target, diagnostics, visited),
            ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics, visited),
            ExpressionNode::Range(range) => {
                recurse(range.start, diagnostics, visited);
                recurse(range.end, diagnostics, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    recurse(field.value, diagnostics, visited);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    walk(
        program,
        expression,
        owner,
        expression_domain,
        expression_interval,
        provably_zero_is_prevalidated,
        diagnostics,
        &mut Vec::new(),
    );
}

pub(crate) fn validate_machine_total_specification_arithmetic(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn kind_label(kind: &SignatureContractKind) -> &'static str {
        match kind {
            SignatureContractKind::Requires => "requires",
            SignatureContractKind::Ensures => "ensures",
            SignatureContractKind::EnsuresForResultCase { .. } => "outcome-specific ensures",
            SignatureContractKind::Crashes { .. } => "crashes",
        }
    }

    let entry_state = program.machine_states(machine).first();
    let mut machine_env = ValueEnv::new();
    for contract in program.machine_contracts(machine) {
        let owner = format!(
            "machine `{}` {} contract",
            machine.name,
            kind_label(&contract.kind),
        );
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                let diagnostics_before = diagnostics.len();
                validate_total_specification_arithmetic(
                    program,
                    machine,
                    entry_state,
                    *expression,
                    &machine_env,
                    &owner,
                    diagnostics,
                );
                if contract.kind == SignatureContractKind::Requires
                    && diagnostics.len() == diagnostics_before
                    && let Some(entry_state) = entry_state
                {
                    narrow_env_by_condition(
                        program,
                        machine,
                        Some(entry_state),
                        &mut machine_env,
                        *expression,
                        true,
                    );
                }
            }
        }
    }
    for state in program.machine_states(machine) {
        let mut state_env = machine_env.clone();
        for contract in program.state_contracts(state) {
            let owner = format!(
                "machine `{}` state `{}` {} contract",
                machine.name,
                state.name,
                kind_label(&contract.kind),
            );
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(expression) = fact {
                    let diagnostics_before = diagnostics.len();
                    validate_total_specification_arithmetic(
                        program,
                        machine,
                        Some(state),
                        *expression,
                        &state_env,
                        &owner,
                        diagnostics,
                    );
                    if contract.kind == SignatureContractKind::Requires
                        && diagnostics.len() == diagnostics_before
                    {
                        narrow_env_by_condition(
                            program,
                            machine,
                            Some(state),
                            &mut state_env,
                            *expression,
                            true,
                        );
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct AbstractSpecificationBindings<'program> {
    parameters: &'program [psi_typed_trees::signature::StateParameter],
    result_type: Option<TypeReferenceHandle>,
    self_type: Option<TypeReferenceHandle>,
    data: Option<&'program psi_typed_trees::data::DataDefinition>,
}

pub(super) fn abstract_specification_place_type(
    program: &TypedTrees,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    fn segments(
        program: &TypedTrees,
        expression: ExpressionHandle,
        output: &mut Vec<String>,
    ) -> Option<()> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                output.extend(
                    program
                        .expression_table
                        .name_path_members(path.members)
                        .iter()
                        .map(ToString::to_string),
                );
                Some(())
            }
            ExpressionNode::Member(member) => {
                segments(program, member.receiver, output)?;
                output.push(member.member.to_string());
                Some(())
            }
            ExpressionNode::Borrow(value) => segments(program, value.target, output),
            _ => None,
        }
    }

    fn data_field_type(
        program: &TypedTrees,
        data: &psi_typed_trees::data::DataDefinition,
        name: &str,
    ) -> Option<TypeReferenceHandle> {
        program.data_members(data).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == name).then_some(field.type_reference)
        })
    }

    fn named_data(
        program: &TypedTrees,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<&psi_typed_trees::data::DataDefinition> {
        let (symbol, name) = loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => type_reference = *referee,
                TypeReferenceNode::Named { symbol, name }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    base_name: name,
                    ..
                } => break (*symbol, name),
                _ => return None,
            }
        };
        if symbol.is_valid() {
            return program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol);
        }
        let mut candidates = program
            .data_definitions()
            .iter()
            .filter(|data| data.name.as_str() == name.as_str());
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first)
    }

    let mut path = Vec::new();
    segments(program, expression, &mut path)?;
    let (mut type_reference, consumed) = match path.as_slice() {
        [name, ..] if name == "result" => (bindings.result_type?, 1),
        [name, ..] if name == "self" => {
            if let Some(type_reference) = bindings.self_type {
                (type_reference, 1)
            } else if bindings.data.is_some() {
                let field = path.get(1)?;
                (data_field_type(program, bindings.data?, field)?, 2)
            } else {
                return None;
            }
        }
        [name, ..] => {
            if let Some(parameter) = bindings.parameters.iter().find(|parameter| {
                parameter.name.as_str() == name.as_str()
                    || (parameter.symbol.is_valid()
                        && matches!(program.expression_table.expression(expression),
                            ExpressionNode::Name(path) if path.symbol == parameter.symbol))
            }) {
                (parameter.type_reference, 1)
            } else if let Some(data) = bindings.data {
                (data_field_type(program, data, name)?, 1)
            } else {
                return None;
            }
        }
        [] => return None,
    };

    for field in &path[consumed..] {
        type_reference = data_field_type(program, named_data(program, type_reference)?, field)?;
    }
    Some(type_reference)
}

/// A direct abstract specification operand's current integer interval. Abstract
/// signatures do not have an executable machine/state analyzer, so this stays
/// deliberately limited to literals and resolved places. Missing structure is
/// not evidence of definedness: callers fail closed when this returns `None`.
pub(super) fn abstract_specification_interval(
    program: &TypedTrees,
    bindings: AbstractSpecificationBindings<'_>,
    env: &ValueEnv,
    expression: ExpressionHandle,
) -> Option<Interval> {
    if let Some(literal) = literal_i64(program, expression) {
        return Some(Interval::constant(literal));
    }
    let type_reference = abstract_specification_place_type(program, bindings, expression)?;
    let primitive = program.primitive_type_reference(type_reference)?;
    place_path(program, expression)
        .and_then(|path| env.get(&path))
        .or_else(|| range_constraint_interval(program, type_reference))
        .or_else(|| primitive_range(primitive))
}

/// The settled abstract slice is deliberately narrower than executable
/// `analyze`: it recognizes only explicit same-carrier policy erasure, the
/// surface which makes a formerly Trapping operand Exact in Prop. It shares
/// `Interval` and the prior-fact `ValueEnv`, but does not guess call results or
/// duplicate the executable expression analyzer.
fn validate_abstract_exact_policy_erasure_formation(
    program: &TypedTrees,
    expression: ExpressionHandle,
    owner: &str,
    bindings: AbstractSpecificationBindings<'_>,
    env: &ValueEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn direct_operand(
        program: &TypedTrees,
        bindings: AbstractSpecificationBindings<'_>,
        env: &ValueEnv,
        expression: ExpressionHandle,
    ) -> Option<(Option<PrimitiveType>, Interval, bool)> {
        if let Some(literal) = literal_i64(program, expression) {
            return Some((None, Interval::constant(literal), false));
        }
        let (type_reference, place, erased) = match program.expression_table.expression(expression)
        {
            ExpressionNode::Cast(cast)
                if !cast.form.is_recast()
                    && cast.semantic_domain.is_empty()
                    && cast.domain == ArithmeticDomain::Exact =>
            {
                let source_type = abstract_specification_place_type(program, bindings, cast.value)?;
                if program.arithmetic_domain_for_type_reference(source_type)
                    == ArithmeticDomain::Exact
                {
                    return None;
                }
                let source_primitive = program.primitive_type_reference(source_type)?;
                let target_primitive = program.primitive_type_reference(cast.target_type)?;
                if source_primitive != target_primitive {
                    return None;
                }
                (source_type, cast.value, true)
            }
            _ => {
                let type_reference =
                    abstract_specification_place_type(program, bindings, expression)?;
                if program.arithmetic_domain_for_type_reference(type_reference)
                    != ArithmeticDomain::Exact
                {
                    return None;
                }
                (type_reference, expression, false)
            }
        };
        let primitive = program.primitive_type_reference(type_reference)?;
        let interval = place_path(program, place)
            .and_then(|path| env.get(&path))
            .or_else(|| range_constraint_interval(program, type_reference))
            .or_else(|| primitive_range(primitive))?;
        Some((Some(primitive), interval, erased))
    }

    fn walk(
        program: &TypedTrees,
        expression: ExpressionHandle,
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        env: &ValueEnv,
        diagnostics: &mut Vec<Diagnostic>,
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !expression.is_valid() || visited.contains(&expression) {
            return;
        }
        visited.push(expression);
        let recurse = |child, diagnostics: &mut Vec<Diagnostic>, visited: &mut Vec<_>| {
            walk(program, child, owner, bindings, env, diagnostics, visited);
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                recurse(binary.left, diagnostics, visited);
                recurse(binary.right, diagnostics, visited);
                if !matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::ShiftLeft
                ) {
                    return;
                }
                let Some((left_primitive, left, left_erased)) =
                    direct_operand(program, bindings, env, binary.left)
                else {
                    return;
                };
                let Some((right_primitive, right, right_erased)) =
                    direct_operand(program, bindings, env, binary.right)
                else {
                    return;
                };
                if !left_erased && !right_erased {
                    return;
                }
                let primitive = match (left_primitive, right_primitive) {
                    (Some(left), Some(right)) if left != right => return,
                    (Some(primitive), _) | (_, Some(primitive)) => primitive,
                    (None, None) => return,
                };
                let interval = match binary.operator {
                    BinaryOperator::Add => left.add(right),
                    BinaryOperator::Subtract => left.subtract(right),
                    BinaryOperator::Multiply => left.multiply(right),
                    BinaryOperator::ShiftLeft => left.shift_left(right),
                    _ => return,
                };
                if let Some(range) = primitive_range(primitive)
                    && !range.contains(interval)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "exact arithmetic in {owner} may overflow `{}`: explicit same-carrier policy erasure retains the ordinary representability obligation, which must be discharged by prior facts",
                        primitive_name(primitive),
                    )));
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for child in program.expression_table.expression_handles(*values) {
                    recurse(*child, diagnostics, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                recurse(atomic.value, diagnostics, visited);
                recurse(atomic.result, diagnostics, visited);
            }
            ExpressionNode::Cast(cast) => recurse(cast.value, diagnostics, visited),
            ExpressionNode::Call(call) => {
                recurse(call.receiver, diagnostics, visited);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    recurse(*argument, diagnostics, visited);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                recurse(indexed.collection, diagnostics, visited);
                recurse(indexed.index, diagnostics, visited);
            }
            ExpressionNode::Member(member) => recurse(member.receiver, diagnostics, visited),
            ExpressionNode::Borrow(value) => recurse(value.target, diagnostics, visited),
            ExpressionNode::Unary(unary) => recurse(unary.operand, diagnostics, visited),
            ExpressionNode::Range(range) => {
                recurse(range.start, diagnostics, visited);
                recurse(range.end, diagnostics, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    recurse(field.value, diagnostics, visited);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    walk(
        program,
        expression,
        owner,
        bindings,
        env,
        diagnostics,
        &mut Vec::new(),
    );
}

fn narrow_abstract_specification_env(
    program: &TypedTrees,
    bindings: AbstractSpecificationBindings<'_>,
    env: &mut ValueEnv,
    expression: ExpressionHandle,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        narrow_abstract_specification_env(program, bindings, env, comparison.left);
        narrow_abstract_specification_env(program, bindings, env, comparison.right);
        return;
    }
    let Some((path, low, high)) = comparison_bound(program, expression) else {
        return;
    };
    let place = if literal_i64(program, comparison.right).is_some() {
        comparison.left
    } else {
        comparison.right
    };
    let Some(type_reference) = abstract_specification_place_type(program, bindings, place) else {
        return;
    };
    let mut interval = Interval { low, high };
    if let Some(carrier) = program
        .primitive_type_reference(type_reference)
        .and_then(primitive_range)
    {
        interval = interval.intersect(carrier);
    }
    if let Some(declared) = range_constraint_interval(program, type_reference) {
        interval = interval.intersect(declared);
    }
    env.narrow(path, interval);
}

/// Validate every abstract Prop-bearing owner exactly once. Concrete machine
/// and state contracts are deliberately absent: their place environments and
/// diagnostics remain owned by `validate_machine_total_specification_arithmetic`.
/// Standalone invariant definitions contain type constraints rather than
/// `ProofFact`s, and trait-requirement/conformance declarations contain names
/// or rows rather than a second contract body.
pub(crate) fn validate_abstract_total_specification_arithmetic(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn kind_label(kind: &SignatureContractKind) -> &'static str {
        match kind {
            SignatureContractKind::Requires => "requires",
            SignatureContractKind::Ensures => "ensures",
            SignatureContractKind::EnsuresForResultCase { .. } => "outcome-specific ensures",
            SignatureContractKind::Crashes { .. } => "crashes",
        }
    }

    fn validate_facts(
        program: &TypedTrees,
        facts: &[ProofFact],
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        admit_to_env: bool,
        env: &mut ValueEnv,
        seen: &mut Vec<ExpressionHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for fact in facts {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if seen.contains(expression) {
                continue;
            }
            seen.push(*expression);
            let diagnostics_before = diagnostics.len();
            abstract_shift_count::validate(program, *expression, owner, bindings, env, diagnostics);
            exact_division_definedness::validate_abstract(
                program,
                *expression,
                owner,
                bindings,
                env,
                diagnostics,
            );
            validate_abstract_exact_policy_erasure_formation(
                program,
                *expression,
                owner,
                bindings,
                env,
                diagnostics,
            );
            let expression_domain = |candidate| {
                abstract_specification_place_type(program, bindings, candidate).map(
                    |type_reference| program.arithmetic_domain_for_type_reference(type_reference),
                )
            };
            let expression_interval =
                |candidate| abstract_specification_interval(program, bindings, env, candidate);
            validate_total_specification_arithmetic_with_domain_lookup(
                program,
                *expression,
                owner,
                &expression_domain,
                &expression_interval,
                false,
                diagnostics,
            );
            if admit_to_env && diagnostics.len() == diagnostics_before {
                narrow_abstract_specification_env(program, bindings, env, *expression);
            }
        }
    }

    fn validate_contracts(
        program: &TypedTrees,
        contracts: &[psi_typed_trees::signature::SignatureContract],
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        seen: &mut Vec<ExpressionHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut env = ValueEnv::new();
        for contract in contracts {
            let contract_owner = format!("{owner} {} contract", kind_label(&contract.kind));
            validate_facts(
                program,
                program.proof_facts.span_or_empty(contract.facts),
                &contract_owner,
                bindings,
                contract.kind == SignatureContractKind::Requires,
                &mut env,
                seen,
                diagnostics,
            );
        }
    }

    let mut seen = Vec::new();

    for domain in program.domain_definitions() {
        let mut env = ValueEnv::new();
        validate_facts(
            program,
            program.proof_facts(domain),
            &format!("domain `{}` predicate", domain.name),
            AbstractSpecificationBindings {
                self_type: Some(domain.target_type),
                ..Default::default()
            },
            true,
            &mut env,
            &mut seen,
            diagnostics,
        );
    }

    for data in program.data_definitions() {
        let mut env = ValueEnv::new();
        validate_facts(
            program,
            program.proof_facts.span_or_empty(data.where_facts),
            &format!("data `{}` default-domain predicate", data.name),
            AbstractSpecificationBindings {
                data: Some(data),
                ..Default::default()
            },
            true,
            &mut env,
            &mut seen,
            diagnostics,
        );
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            validate_contracts(
                program,
                program.state_signature_contracts(signature),
                &format!(
                    "trait `{}` state `{}`",
                    trait_definition.name, signature.name
                ),
                AbstractSpecificationBindings {
                    parameters: program.state_signature_parameters(signature),
                    result_type: signature
                        .return_type
                        .is_valid()
                        .then_some(signature.return_type),
                    ..Default::default()
                },
                &mut seen,
                diagnostics,
            );
        }
    }

    for (_, parameter) in program.data_type_parameters.iter() {
        let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        let Some(signature) = program.machine_parameter_contract_view(contract) else {
            continue;
        };
        let signature = signature.signature();
        validate_contracts(
            program,
            program.state_signature_contracts(signature),
            &format!("machine-parameter requirement `{}`", parameter.name),
            AbstractSpecificationBindings {
                parameters: program.state_signature_parameters(signature),
                result_type: signature
                    .return_type
                    .is_valid()
                    .then_some(signature.return_type),
                ..Default::default()
            },
            &mut seen,
            diagnostics,
        );
    }

    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        let name = program
            .operator_path_members(operator.name)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("::");
        validate_contracts(
            program,
            program.operator_contracts(operator),
            &format!("operator `{name}`"),
            AbstractSpecificationBindings {
                parameters: program.operator_parameters(operator),
                result_type: operator
                    .return_type
                    .is_valid()
                    .then_some(operator.return_type),
                ..Default::default()
            },
            &mut seen,
            diagnostics,
        );
    }
}
