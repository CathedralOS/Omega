//! Range premises use the declaration reached through the receiver's type.
//! Field identities belong to their data declaration, never a machine alias.
//! A global same-spelling search can
//! both reject a valid index and borrow another record's range to admit an
//! invalid one. Ordinary projections retain this checker's scoped root lookup.

use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Whether `expression`'s type is provably an UNSIGNED integer primitive. An
/// unsigned index is non-negative by construction, so it discharges the lower
/// half of the index obligation (`0 <= i`) by type and never needs a separate
/// `>= 0` proof. A signed index -- or one whose type cannot be resolved -- is
/// NOT exempt (closed-world: exempt only when provably unsigned).
pub(super) fn expression_is_unsigned_integer(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        expression_type_reference(program, machine, state, expression)
            .and_then(|type_reference| primitive_of_type_reference(program, type_reference)),
        Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
    )
}

/// Carrier bounds hold under every arithmetic policy. This is deliberately
/// separate from declared refinements: Wrapping does not enforce a declared
/// interval, but its u8 result still cannot exceed 255. Signed nonnegativity
/// remains a separate obligation at the access.
pub(in crate::checks) fn expression_integer_carrier_maximum(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<u64> {
    let reference =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    match program.primitive_type_reference(reference)? {
        PrimitiveType::U8 => Some(u64::from(u8::MAX)),
        PrimitiveType::U16 => Some(u64::from(u16::MAX)),
        PrimitiveType::U32 => Some(u64::from(u32::MAX)),
        PrimitiveType::U64 => Some(u64::MAX),
        PrimitiveType::I8 => u64::try_from(i8::MAX).ok(),
        PrimitiveType::I16 => u64::try_from(i16::MAX).ok(),
        PrimitiveType::I32 => u64::try_from(i32::MAX).ok(),
        PrimitiveType::I64 => u64::try_from(i64::MAX).ok(),
        PrimitiveType::Bool | PrimitiveType::Addr | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

/// The carrier floor symmetric to `expression_integer_carrier_maximum`:
/// unsigned carriers bottom at 0 and signed ones at their type minimum.
/// BigInt so `i64::MIN` and every unsigned bound share one comparison type.
/// Non-integer carriers contribute no floor.
pub(in crate::checks) fn expression_integer_carrier_minimum(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<BigInt> {
    let reference =
        validation::expression_result_type_reference(program, machine, state, expression)?;
    match program.primitive_type_reference(reference)? {
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64 => {
            Some(BigInt::from_u64(0))
        }
        PrimitiveType::I8 => Some(BigInt::from_i64(i64::from(i8::MIN))),
        PrimitiveType::I16 => Some(BigInt::from_i64(i64::from(i16::MIN))),
        PrimitiveType::I32 => Some(BigInt::from_i64(i64::from(i32::MIN))),
        PrimitiveType::I64 => Some(BigInt::from_i64(i64::MIN)),
        PrimitiveType::Bool | PrimitiveType::Addr | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

/// Resolves a type reference (through references and constraints) to its
/// underlying primitive, when it names one.
fn primitive_of_type_reference(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => primitive_of_type_reference(program, *referee),
        TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name.as_str()),
        _ => None,
    }
}

pub(super) fn expression_is_slice(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    expression_type_reference(program, machine, state, expression)
        .is_some_and(|type_reference| type_reference_is_slice(program, type_reference))
}

/// The DECLARED range constraint of `expression`'s type (`i: usize [0..=4]`)
/// as inclusive i64 bounds -- ONLY when that range is a true invariant the
/// index prover may rely on:
/// - the base type is a plain INTEGER primitive (an atomic's hardware
///   semantics wrap; a float range never indexes), and
/// - the arithmetic domain is EXACT: a `in Wrapping`/`Saturating` store is
///   deliberately permissive (probed live: `[0..=4] in Wrapping` accepts a
///   store of 100), so a non-Exact range must never discharge an index bound.
///
/// Every store into an Exact ranged place is range-checked (the narrowing
/// keystone) and ZII requires 0 in range, so the declared interval holds at
/// every read. An exactly selected ordinary call contributes its declared
/// return interval and enforced result contract, intersected with its carrier.
/// Checking the callee's return remains an independent duty.
pub(in crate::checks) fn expression_enforced_declared_range(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let (type_reference, is_call_result) = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            // A private trait realization is not the public requirement's
            // proof interface. Its result needs that interface's own reader.
            if call.static_requirement_dispatch.is_some() {
                return None;
            }
            (
                validation::declared_place_type_raw(program, machine, Some(state), expression)?,
                true,
            )
        }
        _ => (
            expression_type_reference(program, machine, state, expression)?,
            false,
        ),
    };
    let primitive = primitive_of_type_reference(program, type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let declared = enforced_range_of_type_reference(program, type_reference).or_else(|| {
        // `self.count` in a callee result is not the caller's same-named field.
        // Relational call-entry substitution belongs to the call proof owner.
        if is_call_result {
            None
        } else {
            dependent_range_substituted(program, machine, type_reference)
        }
    });
    let Some((low, high)) = super::expressions::ensured_call_result_bounds(program, expression)
    else {
        return declared;
    };
    let minimum = declared.map(|(minimum, _)| minimum).or_else(|| {
        expression_integer_carrier_minimum(program, machine, state, expression)?.to_i64()
    });
    let maximum = declared.map(|(_, maximum)| maximum).or_else(|| {
        i64::try_from(expression_integer_carrier_maximum(
            program, machine, state, expression,
        )?)
        .ok()
    });
    let minimum = match (minimum, low) {
        (Some(carrier), Some(ensured)) => Some(carrier.max(ensured)),
        (carrier, ensured) => carrier.or(ensured),
    }?;
    let maximum = match (maximum, high) {
        (Some(carrier), Some(ensured)) => Some(carrier.min(ensured)),
        (carrier, ensured) => carrier.or(ensured),
    }?;
    (minimum <= maximum).then_some((minimum, maximum))
}

/// R1 dependent range (`i: u32 [0..=self.count]`): the SUBSTITUTED literal
/// interval. The caller-side proof obligation established `i <= count@entry
/// plus `offset`; `count`'s own enforced literal range holds at EVERY write
/// (store-enforced), so `count@entry <= high(count)` and the substituted
/// `(min, high(count) + offset)` is a true invariant of `i` -- immune to the
/// field being reassigned after entry. `None` when the named field is
/// unranged, non-Exact, or the arithmetic overflows (the index prover then
/// reports its clean cannot-prove error).
fn dependent_range_substituted(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    handle: TypeReferenceHandle,
) -> Option<(i64, i64)> {
    let (minimum, symbolic) = dependent_range_of_type_reference(program, handle)?;
    // The symbolic bound retains a field spelling, but its owner is the exact
    // attached declaration. Another module's same-named record cannot supply
    // the store-enforced bound used to justify this parameter's indexes.
    let field_type = validation::exact_attached_field(
        program,
        machine,
        SymbolHandle::invalid(),
        symbolic.field.as_str(),
    )?
    .type_reference;
    // The field itself must be an enforced-integer-ranged place (same gate
    // this module applies to any range an index proof may trust).
    if primitive_of_type_reference(program, field_type).is_none_or(|primitive| {
        !matches!(
            primitive,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        )
    }) {
        return None;
    }
    let (_, field_high) = enforced_range_of_type_reference(program, field_type)?;
    let substituted_high = field_high.checked_add(symbolic.offset)?;
    (minimum <= substituted_high).then_some((minimum, substituted_high))
}

/// A Range constraint whose minimum literal-folds and whose maximum is the
/// R1a-admissible symbolic shape, under Exact shells only (mirrors
/// `enforced_range_of_type_reference`'s domain kill).

/// A shell's constraint set spells a non-Exact arithmetic domain — a
/// deliberately permissive layer neither dependent-range nor exact-shell
/// walks may read through.
fn has_non_exact_domain(constraints: &[typed_trees::types::TypeConstraintNode]) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            typed_trees::types::TypeConstraintNode::ArithmeticDomain(domain)
                if *domain != numerics::arithmetic::ArithmeticDomain::Exact
        )
    })
}

fn dependent_range_of_type_reference(
    program: &typed_trees::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<(i64, typed_trees::dependent_ranges::SymbolicMaxBound)> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            dependent_range_of_type_reference(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if has_non_exact_domain(constraints) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    typed_trees::types::TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive,
                    } => {
                        let minimum =
                            validation::closed_integer_range_bound(program, *minimum)?.to_i64()?;
                        let symbolic = typed_trees::dependent_ranges::symbolic_range_maximum(
                            program,
                            *maximum,
                            *end_inclusive,
                        )?;
                        Some((minimum, symbolic))
                    }
                    _ => None,
                })
                .or_else(|| dependent_range_of_type_reference(program, *base_type))
        }
        _ => None,
    }
}

/// Peel `Reference`/`Constrained` layers of `handle`, running `visit` on
/// each surviving shell's constraints. A shell carrying a non-Exact
/// arithmetic domain is deliberately permissive and kills the walk
/// (`None`); `visit` returning `false` stops early.
pub(in crate::checks::ranges) fn exact_domain_shell_constraints(
    program: &typed_trees::TypedTrees,
    mut handle: TypeReferenceHandle,
    mut visit: impl FnMut(&[typed_trees::types::TypeConstraintNode]) -> bool,
) -> Option<()> {
    loop {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => handle = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = program.type_reference_table.constraints(*constraints);
                if has_non_exact_domain(constraints) {
                    return None;
                }
                if !visit(constraints) {
                    return Some(());
                }
                handle = *base_type;
            }
            _ => return Some(()),
        }
    }
}

fn enforced_range_of_type_reference(
    program: &typed_trees::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<(i64, i64)> {
    let mut bounds: Option<(BigInt, BigInt)> = None;
    exact_domain_shell_constraints(program, handle, |constraints| {
        for constraint in constraints {
            // A DECLARED domain on the place carries its predicates as
            // an invariant of every value that reaches it: membership
            // requires every predicate, and a place declared `in D`
            // establishes D at every write. So each predicate the
            // reader recognizes as a closed integer bound is a true
            // bound of the place, exactly as a bracketed range is.
            if let typed_trees::types::TypeConstraintNode::Domain(domain) = constraint
                && let Some((minimum, maximum)) =
                    validation::declared_domain_predicate_bounds(program, domain)
            {
                bounds = Some(match bounds.take() {
                    Some((prior_minimum, prior_maximum)) => {
                        (minimum.max(prior_minimum), maximum.min(prior_maximum))
                    }
                    None => (minimum, maximum),
                });
                continue;
            }
            let typed_trees::types::TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                continue;
            };
            let Some((minimum, maximum)) =
                validation::closed_integer_range_bound(program, *minimum).zip(
                    validation::closed_integer_range_maximum(program, *maximum, *end_inclusive),
                )
            else {
                // This reader contributes only closed facts. Dependent
                // endpoint substitution retains its separate owner.
                continue;
            };
            bounds = Some(match bounds.take() {
                Some((prior_minimum, prior_maximum)) => {
                    (minimum.max(prior_minimum), maximum.min(prior_maximum))
                }
                None => (minimum, maximum),
            });
        }
        true
    })?;
    let (minimum, maximum) = bounds?;
    // Intersect every closed shell before bounded conversion. A later empty
    // constraint cannot disappear behind a prior nonempty interval, and an
    // exclusive signed-minimum predecessor need not fit i64 to prove bottom.
    if minimum > maximum {
        Some((1, 0))
    } else {
        Some((minimum.to_i64()?, maximum.to_i64()?))
    }
}

/// A generic field's extent belongs to its data declaration; its actual belongs
/// to the receiver application. Join them by declaration and argument position,
/// never by the two binders' possibly identical spellings.
pub(in crate::checks::ranges) fn collection_symbolic_extent_floor(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    collection: ExpressionHandle,
    extent: &(SymbolHandle, typed_trees::name::Identifier),
) -> Option<i64> {
    let actual = (|| {
        let ExpressionNode::Member(member) = program.expression_table.expression(collection) else {
            return None;
        };
        let mut receiver = expression_type_reference(program, machine, state, member.receiver)?;
        while let TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } = program.type_reference_table.type_reference(receiver)
        {
            receiver = *referee;
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(receiver)
        else {
            return None;
        };
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *base_symbol)?;
        // Open generic members may not yet carry a member symbol. Resolve
        // only inside this exact receiver declaration, then rejoin the field's
        // declared extent symbol before substituting the actual argument.
        let mut fields = program.data_members(data).iter().filter_map(|definition| {
            let typed_trees::data::DataMember::Field(field) = definition else {
                return None;
            };
            (if member.member_symbol.is_valid() {
                field.symbol == member.member_symbol
            } else {
                field.name == member.member
            })
            .then_some(field)
        });
        let field = fields.next()?;
        if fields.next().is_some()
            || super::arrays::fixed_array_type_symbolic_extent(program, field.type_reference)?.0
                != extent.0
        {
            return None;
        }
        let position = program
            .data_type_parameters(data)
            .iter()
            .position(|parameter| {
                parameter.symbol == extent.0
                    && extent.0.is_valid()
                    && matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Const { .. }
                    )
            })?;
        program
            .type_reference_table
            .type_reference_handles(*arguments)
            .get(position)
            .copied()
    })();
    let Some(actual) = actual else {
        return symbolic_extent_floor(program, machine, state, extent.0);
    };
    match program.type_reference_table.type_reference(actual) {
        TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
            symbolic_extent_floor(program, machine, state, *symbol)
        }
        // `validate_closed_const_argument` admits closed values as symbol-less
        // Named atoms; ShapeCollector's const-length reader uses this encoding.
        // This decodes retained const data, never an identified declaration.
        TypeReferenceNode::Named { name, .. } => name.as_str().parse::<i64>().ok(),
        TypeReferenceNode::ConstExpression(expression) => {
            match program.expression_table.expression(*expression) {
                ExpressionNode::Name(path) if path.symbol.is_valid() => {
                    symbolic_extent_floor(program, machine, state, path.symbol)
                }
                ExpressionNode::Integer(literal) => literal.value_bignum()?.to_i64(),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The floor a const-parameter extent's declared type guarantees — the closed
/// declared range minimum of the binder (`const N: u64[1..=18446744073709551615]` contributes 1),
/// else its unsigned carrier's zero. `None` means the binder contributes no
/// usable floor: an unranged signed carrier could in principle specialize a
/// negative "length", so nothing below it is provable. Checking admits a
/// symbolic extent only when its symbol is an in-scope binder's, so the
/// binder is found by identity alone.
fn symbolic_extent_floor(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    extent_symbol: SymbolHandle,
) -> Option<i64> {
    let reference = type_reference_for_symbol(program, machine, state, extent_symbol)
        .or_else(|| attached_data_const_parameter_reference(program, machine, extent_symbol))?;
    let declared = constrained_minimum_of_type_reference(program, reference).or_else(|| {
        primitive_of_type_reference(program, reference).and_then(|primitive| {
            matches!(
                primitive,
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
            )
            .then_some(0)
        })
    });
    let required = const_extent_requires_floor(program, machine, state, extent_symbol);
    match (declared, required) {
        (Some(declared), Some(required)) => Some(declared.max(required)),
        (declared, required) => declared.or(required),
    }
}

/// An immutable const binder can use its exact state's arrival contract.
/// Selected builtin meaning, not comparison spelling, supplies the ordering;
/// unrelated binders and disjunctive alternatives supply no bound.
fn const_extent_requires_floor(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    extent: SymbolHandle,
) -> Option<i64> {
    use typed_trees::expression::BinaryOperator;

    if !extent.is_valid()
        || !program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                parameter.symbol == extent
                    && matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Const { .. }
                    )
            })
    {
        return None;
    }
    let mut floor = None;
    for fact in super::requirements::state_requires_facts(program, machine, state) {
        let typed_trees::domain::ProofFact::Expression(expression) = program.proof_facts.get(fact)
        else {
            continue;
        };
        let mut pending = vec![*expression];
        while let Some(expression) = pending.pop() {
            if !validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                continue;
            }
            let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            else {
                continue;
            };
            if binary.operator == BinaryOperator::And {
                pending.extend([binary.left, binary.right]);
                continue;
            }
            for (subject, bound, left) in [
                (binary.left, binary.right, true),
                (binary.right, binary.left, false),
            ] {
                if !matches!(program.expression_table.expression(subject),
                    ExpressionNode::Name(path) if path.symbol == extent
                        && path.head_symbol == extent
                        && program.expression_table.name_path_members(path.members).len() == 1)
                {
                    continue;
                }
                let ExpressionNode::Integer(literal) = program.expression_table.expression(bound)
                else {
                    continue;
                };
                let Some(bound) = literal.value_bignum().and_then(|value| value.to_i64()) else {
                    continue;
                };
                let candidate = match (binary.operator, left) {
                    (BinaryOperator::GreaterOrEqual, true)
                    | (BinaryOperator::LessOrEqual, false)
                    | (BinaryOperator::Equal, _) => Some(bound),
                    (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
                        bound.checked_add(1)
                    }
                    _ => None,
                };
                if let Some(candidate) = candidate {
                    floor = Some(floor.map_or(candidate, |prior: i64| prior.max(candidate)));
                }
            }
        }
    }
    floor
}

/// The declared carrier of a const binder that lives on the machine's ATTACHED
/// data (`data Buffer<const N: u64>` in `Buffer::push`), where the machine's own
/// parameter list is empty. The binder is matched by symbol; the data
/// declaration's name is only a fallback when its symbol never resolved.
fn attached_data_const_parameter_reference(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    extent_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    let data = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == machine.attached_data_symbol)
        .or_else(|| {
            machine.attached_data.as_ref().and_then(|name| {
                program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name == *name)
            })
        })?;
    program
        .data_type_parameters(data)
        .iter()
        .find_map(|parameter| match &parameter.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference }
                if extent_symbol.is_valid() && parameter.symbol == extent_symbol =>
            {
                Some(*type_reference)
            }
            _ => None,
        })
}

/// The declared minimum across a type's closed Range constraints, under the
/// same Exact-shell gate as `enforced_range_of_type_reference` but tolerating
/// a symbolic or absent maximum — a `u64[1..=18446744073709551615]`/`u64[0..N]` lower bound is
/// still an enforced floor.
fn constrained_minimum_of_type_reference(
    program: &typed_trees::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<i64> {
    let mut minimum: Option<numerics::bignum::BigInt> = None;
    exact_domain_shell_constraints(program, handle, |constraints| {
        for constraint in constraints {
            let typed_trees::types::TypeConstraintNode::Range {
                minimum: range_minimum,
                ..
            } = constraint
            else {
                continue;
            };
            let Some(range_minimum) =
                validation::closed_integer_range_bound(program, *range_minimum)
            else {
                continue;
            };
            minimum = Some(match minimum.take() {
                Some(prior) => range_minimum.max(prior),
                None => range_minimum,
            });
        }
        true
    })?;
    minimum?.to_i64()
}

/// `Some(end_inclusive)` when `expression`'s declared type carries a Range
/// constraint whose maximum names the const binder `extent` — the
/// `u64[0..N]`/`u64[0..=N]` binder-bounded shape. `end_inclusive == false`
/// asserts `value < N` (a strict index bound); `true` only `value <= N`.
/// The binder is matched by symbol. A maximum stage 02 left unbound is not
/// that binder even when spelled like it; checking refuses such a maximum in
/// an integer range of every declared type and requires a binder's carrier
/// range to close.
pub(in crate::checks::ranges) fn declared_bound_names_symbolic_extent(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    extent_symbol: SymbolHandle,
) -> Option<bool> {
    let handle = expression_type_reference(program, machine, state, expression)?;
    let mut found = None;
    exact_domain_shell_constraints(program, handle, |constraints| {
        for constraint in constraints {
            let typed_trees::types::TypeConstraintNode::Range {
                maximum,
                end_inclusive,
                ..
            } = constraint
            else {
                continue;
            };
            let ExpressionNode::Name(path) = program.expression_table.expression(*maximum) else {
                continue;
            };
            // A same-spelled binder in another scope cannot borrow
            // this discharge.
            if extent_symbol.is_valid() && path.symbol == extent_symbol {
                found = Some(*end_inclusive);
                return false;
            }
        }
        true
    })?;
    found
}

pub(in crate::checks::ranges) fn expression_type_reference(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(_) => {
            validation::declared_constant_array_type(program, expression)
        }
        ExpressionNode::StructLiteral(_) => {
            validation::expression_result_type_reference(program, machine, state, expression)
        }
        ExpressionNode::Borrow(inner) => {
            expression_type_reference(program, machine, state, inner.target)
        }
        ExpressionNode::Call(call) => {
            // A call result's declared type is its target state's return type —
            // the same reader `expression_enforced_declared_range` uses. A
            // `u64` result is non-negative by construction, discharging the
            // lower half of an index obligation by type alone.
            if call.static_requirement_dispatch.is_some() {
                return None;
            }
            validation::declared_place_type_raw(program, machine, Some(state), expression)
        }
        ExpressionNode::Indexed(indexed)
            if !matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            let collection =
                expression_type_reference(program, machine, state, indexed.collection)?;
            crate::flow::project_type_reference_from_segments(
                program,
                collection,
                &[facts::PlaceSegment::Index {
                    expression: indexed.index,
                }],
            )
        }
        // Checking refuses a value name that resolved to no in-scope binder
        // before ranges run. A same-spelled binder elsewhere in the machine is
        // a different declaration and lends no type.
        ExpressionNode::Name(path) => {
            type_reference_for_symbol(program, machine, state, path.symbol)
        }
        ExpressionNode::Member(member) => {
            validation::exact_self_field(program, machine, expression)
                .map(|field| field.type_reference)
                .or_else(|| {
                    expression_type_reference(program, machine, state, member.receiver).and_then(
                        |receiver_type| {
                            data_field_type_reference(
                                program,
                                receiver_type,
                                member.member_symbol,
                                &member.member,
                            )
                        },
                    )
                })
        }
        // An arithmetic node carries the same result type as its operands'
        // resolved operation — `pos - 1` on u64 IS a u64, which is how its
        // non-negativity discharges by type alone. The generic reader owns
        // the operator typing; callers only need the carrier.
        ExpressionNode::Binary(_) => {
            validation::expression_result_type_reference(program, machine, state, expression)
        }
        _ => None,
    }
}

fn type_reference_for_symbol(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, symbol)
                .map(|(_, local)| local.type_reference)
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.symbol == symbol)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            // A const/value binder denotes a symbolic value of its declared
            // carrier while the template is checked, so `values[Count]`
            // proves its lower bound from `u32` like any other unsigned
            // subject. Specialization replaces the binder before runtime.
            program
                .machine_type_parameters(machine)
                .iter()
                .find_map(|parameter| match &parameter.kind {
                    typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference }
                        if parameter.symbol == symbol =>
                    {
                        Some(*type_reference)
                    }
                    _ => None,
                })
        })
        .or_else(|| {
            // State bindings are WHOLE-MACHINE scope: a param declared on the
            // machine's entry state is readable from every sub-state, so its
            // declared range must feed the prover there too (`pick(k: usize
            // [0..=3]) { ... state go { self.arr[k] } }`). Symbol match is
            // exact, so a sibling state's same-named param cannot alias.
            program.machine_states(machine).iter().find_map(|other| {
                program
                    .state_parameters(other)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
                    .map(|parameter| parameter.type_reference)
            })
        })
}

fn type_reference_is_slice(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => type_reference_is_slice(program, *referee),
        TypeReferenceNode::Slice { .. } => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn data_field_type_reference(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => data_field_type_reference(program, *referee, member_symbol, member_name),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
            |data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            },
        ),
        TypeReferenceNode::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(|data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            })
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &typed_trees::name::Identifier,
) -> Option<&'program typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        if symbol.is_valid() {
            data_definition.symbol == symbol
        } else {
            data_definition.name == *name
        }
    })
}

fn data_field_in_definition(
    program: &typed_trees::TypedTrees,
    data_definition: &typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}
