//! Total-proposition arithmetic formation checks.
//!
//! Runtime overflow analysis remains in the parent. This module owns the
//! separate rule that every arithmetic term admitted into a proposition has a
//! total denotation, for both concrete and abstract contract owners.
//! Concrete and abstract contracts discharge selected call requirements here,
//! through `contract_entailment::specification_calls`, before any fact enters
//! the shared prior-fact context. A second pass after intake would permit a
//! partial call to establish the condition needed to form itself.

use super::guard_narrowing::{comparison_bound, narrow_environment_by_condition};
use super::{
    ArithmeticDomain, BinaryOperator, Diagnostic, ExpressionHandle, ExpressionNode, Interval,
    Machine, PrimitiveType, ProofFact, SignatureContractKind, State, TypeReferenceHandle,
    TypeReferenceNode, ValueEnvironment, abstract_shift_count, division_definedness, is_arithmetic,
    literal_i64, place_path, range_constraint_interval,
};
use crate::machine_calls::machine_parameters::contract_facts::contract_kind_name;
use crate::proof_contracts::arithmetic_domains::expression_analysis::analyze;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    integer_interval_fits_primitive, primitive_name, primitive_range,
    validate_anonymous_integer_primitive_range,
};
use crate::proof_contracts::arithmetic_domains::operand_reports::arithmetic_operator_spelling;
use language_core::is_self_receiver;

/// Reject runtime-control arithmetic from proof positions. A contract is a
/// total proposition: it may compare or bitwise-inspect a Trapping-qualified
/// value, but an arithmetic operation selected by that qualification cannot
/// become a proposition term. Explicit Exact casts remove the qualification
/// before this check; Wrapping and Saturating retain their total denotations.
pub(crate) fn validate_total_specification_arithmetic(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(state) = state {
        crate::value_custody::expression_types::validate_float_expression_formats(
            program,
            state.symbol,
            expression,
            &|operand| crate::expression_result_type_reference(program, machine, state, operand),
            diagnostics,
        );
    }
    // The ordinary branch-sensitive walk owns integer formation for every
    // policy, including nonzero divisors under Wrapping and Saturating. Run it
    // before admitting this fact into `environment`: an operation cannot use
    // its own proposed contract to establish its definedness. A second walk
    // from the outer environment would lose the facts of a selected match arm.
    let mut formation_diagnostics = Vec::new();
    analyze(
        program,
        machine,
        state,
        expression,
        environment,
        None,
        ArithmeticDomain::Exact,
        owner,
        true,
        &mut formation_diagnostics,
    )
    .value;
    // Runtime-only unconditional-Trapping warnings have no meaning in Prop;
    // the totality walker below emits the directed hard error instead.
    diagnostics.extend(
        formation_diagnostics
            .into_iter()
            .filter(Diagnostic::is_error),
    );
    let analyzed = |candidate| {
        let mut ignored_diagnostics = Vec::new();
        analyze(
            program,
            machine,
            state,
            candidate,
            environment,
            None,
            ArithmeticDomain::Exact,
            owner,
            true,
            &mut ignored_diagnostics,
        )
    };
    let expression_domain = |candidate| analyzed(candidate).value.domain;
    let expression_interval = |candidate| Some(analyzed(candidate).value.interval);
    validate_total_specification_arithmetic_for_domains(
        program,
        expression,
        owner,
        &expression_domain,
        &expression_interval,
        &|candidate| {
            crate::proof_contracts::bound_expression_meaning::builtin_arithmetic(
                program, machine, state, candidate,
            )
        },
        &|_, _, _| {},
        true,
        diagnostics,
    );
}

fn validate_total_specification_arithmetic_for_domains(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    owner: &str,
    expression_domain: &impl Fn(ExpressionHandle) -> Option<ArithmeticDomain>,
    expression_interval: &impl Fn(ExpressionHandle) -> Option<Interval>,
    builtin_arithmetic: &impl Fn(ExpressionHandle) -> bool,
    numeric_coercion: &impl Fn(ExpressionHandle, TypeReferenceHandle, &mut Vec<Diagnostic>),
    integer_formation_prevalidated: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        owner: &str,
        expression_domain: &impl Fn(ExpressionHandle) -> Option<ArithmeticDomain>,
        expression_interval: &impl Fn(ExpressionHandle) -> Option<Interval>,
        builtin_arithmetic: &impl Fn(ExpressionHandle) -> bool,
        numeric_coercion: &impl Fn(ExpressionHandle, TypeReferenceHandle, &mut Vec<Diagnostic>),
        integer_formation_prevalidated: bool,
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
                builtin_arithmetic,
                numeric_coercion,
                integer_formation_prevalidated,
                diagnostics,
                visited,
            );
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                recurse(binary.left, diagnostics, visited);
                recurse(binary.right, diagnostics, visited);
                if !is_arithmetic(binary.operator) || !builtin_arithmetic(expression) {
                    return;
                }

                // The selected result owner already distinguishes builtin
                // operations from declarations and supplies their policy.
                // Operand qualifications cannot reconstruct that selection.
                let selected = expression_domain(expression);
                if selected == Some(ArithmeticDomain::Trapping) {
                    diagnostics.push(Diagnostic::error(format!(
                        "direct Trapping arithmetic `{}` is illegal in {owner}: specification terms are total and cannot transfer runtime control. Use `embed(..)` for unbounded proof `Int` mathematics, or explicitly cast the operands to the same unqualified carrier and discharge the resulting Exact formation obligations",
                        arithmetic_operator_spelling(binary.operator),
                    )));
                }
                // Concrete formation already checked the operation under its
                // branch-local environment. Rechecking its divisor here would
                // discard that environment. Abstract declaration carriers use
                // their separate formation lookup until they share that walk.
                if !integer_formation_prevalidated
                    && matches!(
                        selected,
                        Some(ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating)
                    )
                    && matches!(
                        binary.operator,
                        BinaryOperator::Divide | BinaryOperator::Modulo
                    )
                {
                    let divisor = expression_interval(binary.right);
                    let proven_nonzero = divisor.is_some_and(Interval::excludes_zero);
                    if !proven_nonzero {
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
                // An authored coercion preserves denotation. Its destination
                // policy governs enclosing operations, not this conversion.
                // Concrete formation already owns this obligation; abstract
                // owners supply the same judgment with their prior-fact
                // interval and resolved source carrier.
                if integer_formation_prevalidated || cast.form.is_recast() {
                    return;
                }
                numeric_coercion(cast.value, cast.target_type, diagnostics);
            }
            ExpressionNode::Call(call) => {
                recurse(call.receiver, diagnostics, visited);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    recurse(*argument, diagnostics, visited);
                }
                if !integer_formation_prevalidated
                    && call.exact_coercion_target.is_valid()
                    && let [operand] = program.expression_table.expression_handles(call.arguments)
                {
                    numeric_coercion(*operand, call.exact_coercion_target, diagnostics);
                }
                // A call's declared result policy governs enclosing operations.
                // Operand qualification does not synthesize a trapping adapter
                // for an ordinary named call, and returning a qualified value
                // does not by itself execute a trapping operation.
            }
            other => crate::expression_traversal::children(program, other, |child| {
                recurse(child, diagnostics, visited);
            }),
        }
    }

    walk(
        program,
        expression,
        owner,
        expression_domain,
        expression_interval,
        builtin_arithmetic,
        numeric_coercion,
        integer_formation_prevalidated,
        diagnostics,
        &mut Vec::new(),
    );
}

pub(crate) fn validate_machine_total_specification_arithmetic(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_state = program.machine_states(machine).first();
    let mut machine_environment = ValueEnvironment::new();
    let mut machine_prior_facts = Vec::new();
    for contract in program.machine_contracts(machine) {
        let owner = format!(
            "machine `{}` {} contract",
            machine.name,
            contract_kind_name(&contract.kind),
        );
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let diagnostics_before = diagnostics.len();
            crate::proof_contracts::contract_entailment::validate_specification_call_requirements(
                program,
                machine,
                entry_state,
                fact,
                &machine_prior_facts,
                &owner,
                diagnostics,
            );
            crate::proof_contracts::contract_entailment::validate_proof_fact_integer_casts(
                program,
                fact,
                &machine_prior_facts,
                diagnostics,
            );
            if let Some(expression) = fact.as_expression() {
                validate_total_specification_arithmetic(
                    program,
                    machine,
                    entry_state,
                    *expression,
                    &machine_environment,
                    &owner,
                    diagnostics,
                );
                if contract.kind == SignatureContractKind::Requires
                    && diagnostics.len() == diagnostics_before
                {
                    machine_prior_facts.push(*expression);
                    if let Some(entry_state) = entry_state {
                        narrow_environment_by_condition(
                            program,
                            machine,
                            Some(entry_state),
                            &mut machine_environment,
                            *expression,
                            true,
                        );
                    }
                }
            }
        }
    }
    for state in program.machine_states(machine) {
        let mut state_environment = machine_environment.clone();
        let mut state_prior_facts = machine_prior_facts.clone();
        for contract in program.state_contracts(state) {
            let owner = format!(
                "machine `{}` state `{}` {} contract",
                machine.name,
                state.name,
                contract_kind_name(&contract.kind),
            );
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let diagnostics_before = diagnostics.len();
                crate::proof_contracts::contract_entailment::validate_specification_call_requirements(
                    program, machine, Some(state), fact, &state_prior_facts, &owner, diagnostics,
                );
                crate::proof_contracts::contract_entailment::validate_proof_fact_integer_casts(
                    program,
                    fact,
                    &state_prior_facts,
                    diagnostics,
                );
                if let Some(expression) = fact.as_expression() {
                    validate_total_specification_arithmetic(
                        program,
                        machine,
                        Some(state),
                        *expression,
                        &state_environment,
                        &owner,
                        diagnostics,
                    );
                    if contract.kind == SignatureContractKind::Requires
                        && diagnostics.len() == diagnostics_before
                    {
                        state_prior_facts.push(*expression);
                        narrow_environment_by_condition(
                            program,
                            machine,
                            Some(state),
                            &mut state_environment,
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
    owner_symbol: symbols::SymbolHandle,
    parameters: &'program [typed_trees::signature::StateParameter],
    result_type: Option<TypeReferenceHandle>,
    domain: Option<&'program typed_trees::domain::DomainDefinition>,
    data: Option<&'program typed_trees::data::DataDefinition>,
}

fn abstract_specification_result_type(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    crate::value_custody::expression_types::declaration_expression_result_type_reference(
        program,
        bindings.owner_symbol,
        &|expression| abstract_specification_place_type(program, bindings, expression),
        expression,
    )
}

pub(super) fn abstract_specification_builtin_arithmetic(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    crate::proof_contracts::bound_expression_meaning::builtin_arithmetic_for_operand_types(
        program,
        bindings.owner_symbol,
        expression,
        &[
            abstract_specification_result_type(program, bindings, binary.left),
            abstract_specification_result_type(program, bindings, binary.right),
        ],
    )
}

fn abstract_specification_constant_has_builtin_meaning(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> bool {
    crate::proof_contracts::bound_expression_meaning::folded_constant_has_builtin_meaning(
        program,
        expression,
        &|expression| abstract_specification_builtin_arithmetic(program, bindings, expression),
    )
}

pub(super) fn abstract_specification_place_type(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    fn segments(
        program: &dyn crate::CheckingInput,
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
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        name: &str,
    ) -> Option<TypeReferenceHandle> {
        program
            .data_field_named(data, name)
            .map(|field| field.type_reference)
    }

    fn named_data(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let peeled = program
            .type_reference_table
            .peeled_type_reference(type_reference);
        let (symbol, name) = match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::Named { symbol, name }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                base_name: name,
                ..
            } => (*symbol, name),
            _ => return None,
        };
        if symbol.is_valid() {
            return crate::declarations::data_queries::data_definition_by_symbol(program, symbol);
        }
        let mut candidates = program
            .data_definitions()
            .iter()
            .filter(|data| data.name.as_str() == name.as_str());
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first)
    }

    if let Some(domain) = bindings.domain {
        fn domain_place_type(
            program: &dyn crate::CheckingInput,
            domain: &typed_trees::domain::DomainDefinition,
            expression: ExpressionHandle,
        ) -> Option<TypeReferenceHandle> {
            if let Some(reference) = crate::exact_domain_subject_type(program, domain, expression) {
                return Some(reference);
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => domain_place_type(program, domain, borrow.target),
                ExpressionNode::Member(member) => {
                    let reference = domain_place_type(program, domain, member.receiver)?;
                    let data = named_data(program, reference)?;
                    crate::value_custody::places::exact_data_member_field(
                        program,
                        data,
                        member.member_symbol,
                        member.member.as_str(),
                        None,
                    )
                    .map(|field| field.type_reference)
                }
                _ => None,
            }
        }
        if let Some(reference) = domain_place_type(program, domain, expression) {
            return Some(reference);
        }
    }
    let mut path = Vec::new();
    segments(program, expression, &mut path)?;
    let (mut type_reference, consumed) = match path.as_slice() {
        [name, ..] if language_core::is_reserved_result(name) => (bindings.result_type?, 1),
        [name, ..] if is_self_receiver(name) => {
            if bindings.data.is_some() {
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
            } else {
                (data_field_type(program, bindings.data?, name)?, 1)
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
/// signatures do not have an executable machine/state analyzer. Literals and
/// live place facts refine the selected source's declared range; computations
/// retain their selected carrier without borrowing input predicates.
pub(super) fn abstract_specification_interval(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    expression: ExpressionHandle,
) -> Option<Interval> {
    if let Some(literal) = literal_i64(program, expression)
        && abstract_specification_constant_has_builtin_meaning(program, bindings, expression)
    {
        return Some(Interval::constant(literal));
    }
    let type_reference = abstract_specification_result_type(program, bindings, expression)?;
    let primitive = program.primitive_type_reference(type_reference)?;
    place_path(program, expression)
        .and_then(|path| environment.get(&path))
        .or_else(|| range_constraint_interval(program, type_reference))
        .or_else(|| primitive_range(primitive))
}

/// Form an abstract authored numeric coercion before exporting its result
/// qualification. The declaration supplies source types and prior facts;
/// integer landing, carrier fit, float denotation and target predicates remain
/// judgments of their existing shared owners.
fn validate_abstract_numeric_coercion(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    operand: ExpressionHandle,
    target_reference: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target) = program.primitive_type_reference(target_reference) else {
        return;
    };
    let result_type =
        |expression| abstract_specification_result_type(program, bindings, expression);
    let mut source =
        result_type(operand).and_then(|reference| program.primitive_type_reference(reference));
    let mut source_interval =
        abstract_specification_interval(program, bindings, environment, operand)
            .or_else(|| source.and_then(primitive_range));
    let anonymous = if primitive_range(target).is_some() {
        validate_anonymous_integer_primitive_range(program, target, operand, owner, diagnostics)
    } else {
        None
    };
    if let Some((interval, primitive, _)) = anonymous {
        source = primitive;
        source_interval = Some(interval);
    }
    let float_bounds =
        |expression| super::float_source_bounds(program, environment, expression, &result_type);
    let integer_bounds = || {
        super::integer_ranges::integer_source_bounds_from_interval(
            program,
            environment,
            operand,
            source?,
            source_interval?,
        )
    };
    super::cast_ranges::validate_target_range_membership(
        program,
        operand,
        target_reference,
        source,
        integer_bounds(),
        &float_bounds,
        owner,
        diagnostics,
    );
    if anonymous.is_some() {
        return;
    }
    if let Some(proven) = source.and_then(|source| {
        crate::value_custody::expression_types::float_coercion_preserves_denotation(
            source,
            target,
            || float_bounds(operand),
            integer_bounds,
        )
    }) {
        if !proven {
            diagnostics.push(Diagnostic::error(format!(
                "Exact numeric cast in {owner} to `{}` is not provably denotation preserving; \
                 prove source integrality and exact representability independently or use a named numeric conversion",
                primitive_name(target),
            )).with_source_span(program.expression_table.source_span(operand)));
        }
        return;
    }
    if primitive_range(target).is_none() {
        return;
    }
    let fits = source.is_some_and(|source| {
        source == PrimitiveType::Bool
            || source_interval
                .is_some_and(|interval| integer_interval_fits_primitive(interval, source, target))
    });
    if !fits {
        diagnostics.push(Diagnostic::error(format!(
            "Exact integer cast in {owner} to `{}` is not provably representable; \
             constrain the source with an independently accepted prior fact or use a named numeric conversion",
            primitive_name(target),
        )).with_source_span(program.expression_table.source_span(operand)));
    }
}

/// The settled abstract slice is deliberately narrower than executable
/// `analyze`: it recognizes only explicit same-carrier policy erasure, the
/// surface which makes a formerly Trapping operand Exact in Prop. It shares
/// `Interval` and the prior-fact `ValueEnvironment`, but does not guess call results or
/// duplicate the executable expression analyzer.
fn validate_abstract_exact_policy_erasure_formation(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    owner: &str,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn direct_operand(
        program: &dyn crate::CheckingInput,
        bindings: AbstractSpecificationBindings<'_>,
        environment: &ValueEnvironment,
        expression: ExpressionHandle,
    ) -> Option<(Option<PrimitiveType>, Interval, bool)> {
        if let Some(literal) = literal_i64(program, expression)
            && abstract_specification_constant_has_builtin_meaning(program, bindings, expression)
        {
            return Some((None, Interval::constant(literal), false));
        }
        let (type_reference, place, erased) = match program.expression_table.expression(expression)
        {
            ExpressionNode::Cast(cast)
                if !cast.form.is_recast()
                    && cast.semantic_domain.is_empty()
                    && cast.domain == ArithmeticDomain::Exact =>
            {
                let source_type =
                    abstract_specification_result_type(program, bindings, cast.value)?;
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
                    abstract_specification_result_type(program, bindings, expression)?;
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
            .and_then(|path| environment.get(&path))
            .or_else(|| range_constraint_interval(program, type_reference))
            .or_else(|| primitive_range(primitive))?;
        Some((Some(primitive), interval, erased))
    }

    let mut check = |expression, binary: &typed_trees::expression::TableBinaryExpression| {
        if !abstract_specification_builtin_arithmetic(program, bindings, expression) {
            return;
        }
        let Some((left_primitive, left, left_erased)) =
            direct_operand(program, bindings, environment, binary.left)
        else {
            return;
        };
        let Some((right_primitive, right, right_erased)) =
            direct_operand(program, bindings, environment, binary.right)
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
    };
    super::visit_binary_operators(
        program,
        expression,
        &[
            BinaryOperator::Add,
            BinaryOperator::Subtract,
            BinaryOperator::Multiply,
            BinaryOperator::ShiftLeft,
        ],
        &mut check,
    );
}

fn narrow_abstract_specification_environment(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    environment: &mut ValueEnvironment,
    expression: ExpressionHandle,
) {
    let source_type =
        |expression| abstract_specification_result_type(program, bindings, expression);
    let Some(comparison) = crate::proof_contracts::bound_expression_meaning::guard_comparison(
        program,
        bindings.owner_symbol,
        expression,
        &source_type,
    ) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        narrow_abstract_specification_environment(program, bindings, environment, comparison.left);
        narrow_abstract_specification_environment(program, bindings, environment, comparison.right);
        return;
    }
    if crate::proof_contracts::bound_expression_meaning::has_builtin_or_sealed_float_comparison_meaning(
        program, bindings.owner_symbol, expression, comparison.operator,
        &[source_type(comparison.left), source_type(comparison.right)],
    ) && super::float_arithmetic::record_float_comparison_bounds(
        program, environment, &comparison, &source_type,
    ) {
        return;
    }
    if !abstract_specification_constant_has_builtin_meaning(program, bindings, comparison.left)
        || !abstract_specification_constant_has_builtin_meaning(program, bindings, comparison.right)
    {
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
    environment.narrow(path, interval);
}

/// Validate every abstract Prop-bearing owner exactly once. Concrete machine
/// and state contracts are deliberately absent: their place environments and
/// diagnostics remain owned by `validate_machine_total_specification_arithmetic`.
/// Standalone invariant definitions contain type constraints rather than
/// `ProofFact`s, and trait-requirement/conformance declarations contain names
/// or rows rather than a second contract body.
pub(crate) fn validate_abstract_total_specification_arithmetic(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    fn validate_facts(
        program: &dyn crate::CheckingInput,
        facts: &[ProofFact],
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        admit_to_environment: bool,
        environment: &mut ValueEnvironment,
        prior_facts: &mut Vec<ExpressionHandle>,
        seen: &mut Vec<ExpressionHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // The declaration's binders are real operands, not a fabricated
        // machine frame. Reuse their exact symbols for call substitution.
        let mut parameters = bindings.parameters.to_vec();
        if let Some(domain) = bindings.domain {
            parameters.push(typed_trees::signature::StateParameter {
                symbol: domain.subject_symbol,
                name: domain.subject_name.clone(),
                type_reference: domain.target_type,
                ..Default::default()
            });
        }
        if let Some(data) = bindings.data {
            parameters.extend(program.data_members(data).iter().filter_map(|member| {
                let typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                Some(typed_trees::signature::StateParameter {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_reference: field.type_reference,
                    ..Default::default()
                })
            }));
        }
        for fact in facts {
            let diagnostics_before = diagnostics.len();
            crate::proof_contracts::contract_entailment::specification_calls::validate_declaration_call_requirements(
                program, bindings.owner_symbol, &parameters, fact, prior_facts, owner, diagnostics,
            );
            crate::proof_contracts::contract_entailment::validate_proof_fact_integer_casts(
                program,
                fact,
                prior_facts,
                diagnostics,
            );
            let Some(expression) = fact.as_expression() else {
                continue;
            };
            if seen.contains(expression) {
                continue;
            }
            seen.push(*expression);
            crate::value_custody::expression_types::validate_float_expression_formats(
                program,
                bindings.owner_symbol,
                *expression,
                &|operand| abstract_specification_result_type(program, bindings, operand),
                diagnostics,
            );
            abstract_shift_count::validate(
                program,
                *expression,
                owner,
                bindings,
                environment,
                diagnostics,
            );
            division_definedness::validate_abstract(
                program,
                *expression,
                owner,
                bindings,
                environment,
                diagnostics,
            );
            validate_abstract_exact_policy_erasure_formation(
                program,
                *expression,
                owner,
                bindings,
                environment,
                diagnostics,
            );
            let expression_domain = |candidate| {
                abstract_specification_result_type(program, bindings, candidate).map(
                    |type_reference| program.arithmetic_domain_for_type_reference(type_reference),
                )
            };
            let expression_interval = |candidate| {
                abstract_specification_interval(program, bindings, environment, candidate)
            };
            let builtin_arithmetic =
                |candidate| abstract_specification_builtin_arithmetic(program, bindings, candidate);
            let numeric_coercion = |operand, target, diagnostics: &mut Vec<Diagnostic>| {
                validate_abstract_numeric_coercion(
                    program,
                    bindings,
                    environment,
                    operand,
                    target,
                    owner,
                    diagnostics,
                );
            };
            validate_total_specification_arithmetic_for_domains(
                program,
                *expression,
                owner,
                &expression_domain,
                &expression_interval,
                &builtin_arithmetic,
                &numeric_coercion,
                false,
                diagnostics,
            );
            if admit_to_environment && diagnostics.len() == diagnostics_before {
                prior_facts.push(*expression);
                narrow_abstract_specification_environment(
                    program,
                    bindings,
                    environment,
                    *expression,
                );
            }
        }
    }

    fn validate_contracts(
        program: &dyn crate::CheckingInput,
        contracts: &[typed_trees::signature::SignatureContract],
        owner: &str,
        bindings: AbstractSpecificationBindings<'_>,
        seen: &mut Vec<ExpressionHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut environment = ValueEnvironment::new();
        let mut prior_facts = Vec::new();
        for contract in contracts {
            let contract_owner = format!("{owner} {} contract", contract_kind_name(&contract.kind));
            validate_facts(
                program,
                program.proof_facts.span_or_empty(contract.facts),
                &contract_owner,
                bindings,
                contract.kind == SignatureContractKind::Requires,
                &mut environment,
                &mut prior_facts,
                seen,
                diagnostics,
            );
        }
    }

    let mut seen = Vec::new();

    for definition in program.mathematical_definitions() {
        let parameters = crate::mathematical_applications::ordinary_parameters(program, definition);
        for expression in crate::mathematical_applications::definition_roots(program, definition) {
            validate_facts(
                program,
                &[ProofFact::Expression(expression)],
                &format!("mathematical definition `{}`", definition.name),
                AbstractSpecificationBindings {
                    owner_symbol: definition.symbol,
                    parameters: &parameters,
                    ..Default::default()
                },
                false,
                &mut ValueEnvironment::new(),
                &mut Vec::new(),
                &mut seen,
                diagnostics,
            );
        }
    }

    for proposition in program.propositions() {
        use typed_trees::proposition::{PropositionBody, PropositionFormula};
        if let PropositionBody::Transparent {
            proposition: formula,
        } = &proposition.body
        {
            let bindings = AbstractSpecificationBindings {
                owner_symbol: proposition.symbol,
                parameters: program.proposition_parameters(proposition),
                ..Default::default()
            };
            let owner = format!("proposition `{}`", proposition.name);
            let mut environment = ValueEnvironment::new();
            let mut prior_facts = Vec::new();
            let mut validate_term = |expression| {
                validate_facts(
                    program,
                    &[ProofFact::Expression(expression)],
                    &owner,
                    bindings,
                    false,
                    &mut environment,
                    &mut prior_facts,
                    &mut seen,
                    diagnostics,
                );
            };
            match formula {
                PropositionFormula::BooleanExpression(expression) => {
                    validate_term(*expression);
                }
                PropositionFormula::Application(application) => {
                    for argument in program
                        .expression_table
                        .expression_handles(application.arguments)
                    {
                        validate_term(*argument);
                    }
                }
            }
        }
    }

    for domain in program.domain_definitions() {
        let mut environment = ValueEnvironment::new();
        let mut prior_facts = Vec::new();
        validate_facts(
            program,
            program.proof_facts(domain),
            &format!("domain `{}` predicate", domain.name),
            AbstractSpecificationBindings {
                owner_symbol: domain.symbol,
                domain: Some(domain),
                ..Default::default()
            },
            true,
            &mut environment,
            &mut prior_facts,
            &mut seen,
            diagnostics,
        );
    }

    for data in program.data_definitions() {
        let mut environment = ValueEnvironment::new();
        let mut prior_facts = Vec::new();
        validate_facts(
            program,
            program.proof_facts.span_or_empty(data.where_facts),
            &format!("data `{}` default-domain predicate", data.name),
            AbstractSpecificationBindings {
                owner_symbol: data.symbol,
                data: Some(data),
                ..Default::default()
            },
            true,
            &mut environment,
            &mut prior_facts,
            &mut seen,
            diagnostics,
        );
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                continue;
            };
            let payload = program
                .data_payload_fields(variant)
                .iter()
                .map(|field| typed_trees::signature::StateParameter {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_reference: field.type_reference,
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            // Common predicates precede every case; one case's payload and
            // facts must never become another case's formation premises.
            validate_facts(
                program,
                program.proof_facts.span_or_empty(variant.where_facts),
                &format!("data `{}` case `{}` predicate", data.name, variant.name),
                AbstractSpecificationBindings {
                    owner_symbol: variant.symbol,
                    parameters: &payload,
                    data: Some(data),
                    ..Default::default()
                },
                true,
                &mut environment.clone(),
                &mut prior_facts.clone(),
                &mut seen,
                diagnostics,
            );
        }
    }

    fn validate_state_signature(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
        owner: String,
        seen: &mut Vec<ExpressionHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_contracts(
            program,
            program.state_signature_contracts(signature),
            &owner,
            AbstractSpecificationBindings {
                owner_symbol: signature.symbol,
                parameters: program.state_signature_parameters(signature),
                result_type: signature
                    .return_type
                    .is_valid()
                    .then_some(signature.return_type),
                ..Default::default()
            },
            seen,
            diagnostics,
        );
    }

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            validate_state_signature(
                program,
                signature,
                format!(
                    "trait `{}` state `{}`",
                    trait_definition.name, signature.name
                ),
                &mut seen,
                diagnostics,
            );
        }
    }

    for (_, parameter) in program.data_type_parameters.iter() {
        let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        let Some(signature) = program.machine_parameter_contract_view(contract) else {
            continue;
        };
        let signature = signature.signature();
        validate_state_signature(
            program,
            signature,
            format!("machine-parameter requirement `{}`", parameter.name),
            &mut seen,
            diagnostics,
        );
    }

    for operator in program.all_operators() {
        let name = program.operator_path_string(operator.name);
        validate_contracts(
            program,
            program.operator_contracts(operator),
            &format!("operator `{name}`"),
            AbstractSpecificationBindings {
                owner_symbol: operator.symbol,
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

/// The source expression's selected carrier and policy in the owning abstract
/// declaration. Computed operands use the same result typing as executable
/// expressions; the cast destination contributes no source information.
pub(super) fn direct_abstract_operand(
    program: &dyn crate::CheckingInput,
    bindings: AbstractSpecificationBindings<'_>,
    expression: ExpressionHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    let type_reference = abstract_specification_result_type(program, bindings, expression)?;
    Some((
        program.primitive_type_reference(type_reference)?,
        program.arithmetic_domain_for_type_reference(type_reference),
    ))
}
