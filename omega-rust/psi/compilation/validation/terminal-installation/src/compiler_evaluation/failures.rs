//! Refuse a constant call before execution when a checked crash route may occur.
//!
//! The emitted root's verified contract already covers its called bodies with
//! their exact actuals. Combining callee contracts would lose those guards.

use proof_admission::{ClosedIntegerEvaluator, check_predicate_evaluation_size};
use semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_failures;
use semantic_vocabulary::{
    IntegerMathTerm, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, ProofCallableAssociation, ProofDeclarationId,
    TargetMachineCallable, TerminalModule,
};

pub(crate) fn require_root_failure_admission(
    module: &TerminalModule,
    root: ProofDeclarationId,
) -> Result<(), String> {
    let scope = &module.scalar_applications.proof_scope;
    let header = super::header(scope, root)?;
    let facts = header.compiler_evaluation.as_ref().map_err(Clone::clone)?;
    let mut bodies = scope
        .callables
        .iter()
        .filter_map(|(_, association)| match association {
            ProofCallableAssociation::Callable {
                declaration,
                callable,
            } if *declaration == root => Some(*callable),
            _ => None,
        });
    let Some(TargetMachineCallable::Machine(machine)) = bodies.next() else {
        return Err("compiler crash admission requires an exact emitted machine body".into());
    };
    if bodies.next().is_some() {
        return Err("compiler crash admission has competing emitted body associations".into());
    }
    let machine = module
        .machine(machine)
        .ok_or("compiler crash admission has no emitted root body")?;
    let context = machine.entry_proposition_context().map_err(|reason| {
        format!("compiler crash admission has an invalid entry context: {reason:?}")
    })?;
    let failures = exclude_failure_routes(&machine.contract.crash_routes, &context);
    require_compiler_evaluation_failures(
        &facts.name,
        failures.as_ref().map(|_| ()).map_err(String::as_str),
    )
}

fn exclude_failure_routes(
    routes: &[CrashRouteBucket],
    context: &PropositionContext,
) -> Result<(), String> {
    if routes.windows(2).any(|pair| pair[0].cause >= pair[1].cause)
        || routes.iter().any(|bucket| {
            bucket.alternatives.is_empty()
                || bucket
                    .alternatives
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                    && bucket.alternatives != [CrashRouteGuard::Truth])
        })
    {
        return Err("the checked crash summary is malformed".into());
    }
    // Check the whole input before recursive evaluation, including guards whose
    // truth could otherwise let evaluation skip an unformed or oversized child.
    check_predicate_evaluation_size(routes.iter().flat_map(|bucket| {
        bucket.alternatives.iter().filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
    }))
    .map_err(|reason| format!("the checked crash summary exceeds evaluation limits: {reason:?}"))?;
    let mut integers = ClosedIntegerEvaluator::default();
    for guard in routes.iter().flat_map(|bucket| &bucket.alternatives) {
        if let CrashRouteGuard::Predicate(predicate) = guard {
            context
                .validate(predicate.proposition())
                .map_err(|reason| format!("the checked crash guard is malformed: {reason:?}"))?;
            require_formed_operands(predicate.proposition(), &mut integers)?;
        }
    }
    for bucket in routes {
        for guard in &bucket.alternatives {
            let possible = match guard {
                CrashRouteGuard::Truth => Some(true),
                CrashRouteGuard::Predicate(predicate) => {
                    closed_guard(predicate.proposition(), &mut integers)?
                }
            };
            match possible {
                Some(false) => (),
                Some(true) => {
                    return Err(format!(
                        "the checked {:?} crash route can occur",
                        bucket.cause
                    ));
                }
                None => {
                    return Err(format!(
                        "the checked {:?} crash route cannot be proved impossible from the available invocation facts",
                        bucket.cause
                    ));
                }
            }
        }
    }
    Ok(())
}

// A false guard cannot justify forming an undefined operand. Visit every child
// first; open observations stay open, while existing kernels decide closed terms.
fn require_formed_operands(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<(), String> {
    match proposition {
        semantic_vocabulary::scalar_comparison_operands!(left, right) => {
            require_formed_scalar(left)?;
            require_formed_scalar(right)?;
        }
        semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
            require_formed_integer_math(left, integers)?;
            require_formed_integer_math(right, integers)?;
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            for child in children {
                require_formed_operands(child, integers)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            require_formed_operands(premise, integers)?;
            require_formed_operands(conclusion, integers)?;
        }
        _ => (),
    }
    Ok(())
}

fn require_formed_scalar(term: &ScalarTerm) -> Result<bool, String> {
    let mut closed = true;
    for operand in term.operands() {
        // Do not short circuit: an open left operand cannot hide a bad right one.
        closed &= require_formed_scalar(operand)?;
    }
    match term {
        ScalarTerm::ExactIntegerDivide { right, .. }
        | ScalarTerm::ExactIntegerRemainder { right, .. }
        | ScalarTerm::WrappingIntegerDivide { right, .. }
        | ScalarTerm::WrappingIntegerRemainder { right, .. }
        | ScalarTerm::SaturatingIntegerDivide { right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { right, .. }
            if matches!(
                right.integer_value(),
                Some((_, IntegerValue::Signed(0) | IntegerValue::Unsigned(0)))
            ) =>
        {
            return Err(
                "the checked crash guard contains an undefined zero-divisor operand".into(),
            );
        }
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            count,
            ..
        }
        | ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            count,
            ..
        } => {
            // A right shift of an admitted value isolates count validity from
            // the value-dependent representability needed by a left shift.
            if let Some((_, count)) = count.integer_value()
                && value_type
                    .exact_shift_right(value_type.minimum_value(), *count_type, count)
                    .is_none()
            {
                return Err(
                    "the checked crash guard contains an undefined exact-shift count".into(),
                );
            }
        }
        _ => (),
    }
    match term {
        ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) | ScalarTerm::IeeeFloatLiteral(_) => {
            Ok(true)
        }
        ScalarTerm::Application { .. }
        | ScalarTerm::Value { .. }
        | ScalarTerm::StructuralRead { .. }
        | ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::IeeeFloatField { .. }
        | ScalarTerm::ViewExtent { .. }
        | ScalarTerm::ByteSequencePredicate { .. } => Ok(false),
        _ if !closed => Ok(false),
        _ => {
            let defined = match term.scalar_type() {
                ScalarType::Boolean => term.boolean_value().is_some(),
                ScalarType::Integer(_) => term.integer_value().is_some(),
                _ => false,
            };
            if !defined {
                return Err(
                    "the checked crash guard contains an undefined closed scalar operand".into(),
                );
            }
            Ok(true)
        }
    }
}

fn require_formed_integer_math(
    term: &IntegerMathTerm,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<bool, String> {
    if let IntegerMathTerm::Embed(scalar) = term {
        return require_formed_scalar(scalar);
    }
    let mut closed = !matches!(term, IntegerMathTerm::MathValue { .. });
    for operand in term.operands() {
        closed &= require_formed_integer_math(operand, integers)?;
    }
    if let IntegerMathTerm::ShiftLeft { count, .. } = term {
        if let Some(count) = integers
            .evaluate_closed(count)
            .map_err(|reason| format!("the checked crash guard could not be formed: {reason}"))?
            && count.is_negative()
        {
            return Err(
                "the checked crash guard contains an undefined negative mathematical shift".into(),
            );
        }
    }
    if closed
        && integers
            .evaluate_closed(term)
            .map_err(|reason| format!("the checked crash guard could not be formed: {reason}"))?
            .is_none()
    {
        return Err(
            "the checked crash guard contains an undefined closed mathematical operand".into(),
        );
    }
    Ok(closed)
}

/// Use the existing scalar and mathematical integer denotations. An unavailable
/// value remains unknown; executing the entry to discover it cannot prove safety.
fn closed_guard(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<Option<bool>, String> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => left
            .boolean_value()
            .zip(right.boolean_value())
            .map(|(left, right)| left == right),
        semantic_vocabulary::ordered_comparison_operands!(left, right) => left
            .integer_value()
            .zip(right.integer_value())
            .and_then(|((left_type, left), (right_type, right))| {
                (left_type == right_type)
                    .then(|| left_type.compare(left, right))
                    .flatten()
            })
            .map(|ordering| match proposition {
                Proposition::Equal(..) => ordering.is_eq(),
                Proposition::LessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            }),
        semantic_vocabulary::integer_math_comparison_operands!(left, right) => integers
            .compare(left, right)
            .map_err(|reason| format!("the checked crash guard could not be evaluated: {reason}"))?
            .map(|ordering| match proposition {
                Proposition::IntegerMathEqual(..) => ordering.is_eq(),
                Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            }),
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let identity = matches!(proposition, Proposition::Conjunction(_));
            let mut unknown = false;
            for child in children {
                match closed_guard(child, integers)? {
                    Some(value) if value != identity => return Ok(Some(value)),
                    Some(_) => (),
                    None => unknown = true,
                }
            }
            (!unknown).then_some(identity)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => match closed_guard(premise, integers)? {
            Some(false) => Some(true),
            Some(true) => closed_guard(conclusion, integers)?,
            None => closed_guard(conclusion, integers)?.filter(|value| *value),
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests;
