//! Type inference for the synthetic `let __hoist_N = <indexed read>;` temps the
//! operand-hoisting normalization synthesizes (see the syntax -> symbol-resolved
//! `statement::hoist_operand_indexed_reads`). Those temps carry a `Unit`
//! declared type (the inference sentinel) because the language has no local type
//! inference and the element type is not knowable at the hoist site. Here, with
//! the resolved data-field types available, the temp's element type is derived
//! from its indexed-read initializer so the later domain/range/layout checks and
//! codegen see a concrete declared type that matches the element read.

use crate::lowerer::Lowerer;
use crate::type_reference::lower_element_applicable_constraints;
use crate::type_reference::lower_type_reference_into_table;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use psi_typed_trees as typed;

use resolved::expression::{ExpressionHandle, ExpressionNode};

/// Infers the typed declared type for a hoist temp whose `initial_value` is a
/// runtime-indexed read. Returns `None` (the caller then keeps `Unit`) for any
/// shape it cannot resolve -- that preserves the pre-existing behavior for those
/// shapes rather than guessing.
pub(super) fn infer_hoist_temp_type(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    initial_value: ExpressionHandle,
) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;

    // A pure-builtin guard subject the syntax->symbol-resolved lowering hoisted
    // (`let __hoist = min(self.a, self.b)`): the temp's type is its FIRST
    // argument's type. min/max/sqrt all return their operand's type (abs desugars
    // to `max(x, 0-x)`; clamp's outer call has a non-place first arg and is not
    // hoisted). Only a `self.<field>` first arg is typeable here, which is exactly
    // what the hoist predicate requires; any other call shape yields no type.
    let builtin_first_arg: Option<Option<ExpressionHandle>> = match expressions
        .expression(initial_value)
    {
        ExpressionNode::Call(call)
            if !call.receiver.is_valid()
                && matches!(call.target.as_str(), "min" | "max" | "sqrt") =>
        {
            Some(
                expressions
                    .expression_handles(call.arguments)
                    .first()
                    .copied(),
            )
        }
        // A USER value-machine call hoisted out of a guard comparison
        // (`let __hoist = self.dbl(5)`, syntax->symbol-resolved
        // `hoist_scalar_value_call_comparison`): the temp's type is the
        // callee's DECLARED return type, resolved by target symbol over
        // the machine-state arena. An inferred-return callee cannot be
        // typed at this phase -- reject it with the actionable fix
        // instead of the confusing Unit frame-slot layout error.
        ExpressionNode::Call(call) => {
            let target_symbol = call.target_symbol;
            let target_name = call.target.as_str().to_string();
            let mut machine_substitutions = Vec::new();
            let declared_state = target_symbol
                .is_valid()
                .then(|| {
                    lowerer
                        .source_trees
                        .tables
                        .declarations
                        .machine_states
                        .iter()
                        .find(|(_, candidate)| candidate.symbol == target_symbol)
                        .map(|(_, candidate)| candidate)
                })
                .flatten();
            let declared_return = declared_state
                    .and_then(|candidate| candidate.storage.return_type.clone())
                    .or_else(|| {
                        static_conformance_requirement_return(lowerer, state, call)
                    })
                    .or_else(|| {
                        // MP3/MP4: a call through a static machine parameter
                        // has the authored `where machine F(..) -> R` return
                        // type even before a concrete selection is known.
                        lowerer
                            .source_trees
                            .roots
                            .machines
                            .iter()
                            .find_map(|machine| {
                                lowerer
                                    .source_trees
                                    .machine_type_parameters(machine)
                                    .iter()
                                    .find_map(|parameter| match &parameter.kind {
                                        resolved::data::TypeParameterKind::Machine { contract }
                                            if parameter.symbol == target_symbol =>
                                        {
                                            let contract = lowerer
                                                .source_trees
                                                .machine_parameter_contract_view(contract)?
                                                .signature();
                                            machine_substitutions.extend(
                                                lowerer
                                                    .source_trees
                                                    .data_type_parameters(contract.type_parameters)
                                                    .iter()
                                                    .filter(|parameter| {
                                                        matches!(
                                                            parameter.kind,
                                                            resolved::data::TypeParameterKind::Machine { .. }
                                                        )
                                                    })
                                                    .zip(call.machine_arguments.iter())
                                                    .filter_map(|(parameter, argument)| {
                                                        argument.path.last().map(|name| {
                                                            (
                                                                parameter.symbol,
                                                                argument.symbol,
                                                                crate::name::lower_name(name),
                                                            )
                                                        })
                                                    }),
                                            );
                                            contract.return_type.clone()
                                        }
                                        _ => None,
                                    })
                            })
                    });
            let Some(declared_return) = declared_return else {
                return Err(Diagnostic::error(format!(
                    "a value-machine call in a guard comparison needs the callee's \
                         return type declared: annotate `{target_name}` with an explicit \
                         `-> Type`, or bind the call to a typed `let` first and guard \
                         the local"
                )));
            };
            let declared_return = lower_type_reference_into_table(lowerer, &declared_return)?;
            substitute_machine_parameters_in_type(
                &mut lowerer.typed_trees.type_reference_table,
                declared_return,
                &machine_substitutions,
            );
            return Ok(Some(declared_return));
        }
        _ => None,
    };
    if let Some(first) = builtin_first_arg {
        let Some(first) = first else {
            return Ok(None);
        };
        let Some(place_type) = collection_type_reference(lowerer, attached_data, state, first)
        else {
            return Ok(None);
        };
        return Ok(Some(lower_type_reference_into_table(lowerer, &place_type)?));
    }

    // A member read hoisted through a SHARED reference-to-named param
    // (`let __hoist = table.con_out` with `table: &EfiSystemTable`): the temp's
    // type is the FIELD's declared type in the referee data definition. The
    // guard/assignment hoists synthesize exactly this shape so the read routes
    // through the pointee-deref path; the temp is slot-backed and must carry a
    // concrete type.
    if let ExpressionNode::Member(member) = expressions.expression(initial_value)
        && let ExpressionNode::Name(path) = expressions.expression(member.receiver)
        && let [only] = expressions.name_path_members(path.members)
        && let Some(TypeReference::Reference(reference)) =
            parameter_type(lowerer.source_trees, state, only.as_str())
        && reference.access == psi_language_semantics::ReferenceAccess::Shared
        && let TypeReference::Named { name, .. } =
            lowerer.source_trees.child_type_reference(reference.referee)
    {
        let data_name = name.clone();
        let member_name = member.member.as_str().to_string();
        if let Some(field_type) =
            attached_field_type(lowerer.source_trees, &data_name, &member_name)
        {
            return Ok(Some(lower_type_reference_into_table(lowerer, &field_type)?));
        }
        return Ok(None);
    }

    // A FIELD read of a runtime-indexed ELEMENT (`cells[k].v`, hoisted whole):
    // the temp's type is the FIELD's declared type on the element's Named data
    // type. (Single-level member only -- deeper chains keep Unit and fall back
    // to the pre-existing behavior.)
    if let ExpressionNode::Member(member) = expressions.expression(initial_value)
        && let ExpressionNode::Indexed(inner) = expressions.expression(member.receiver)
    {
        let member_name = member.member.as_str().to_string();
        let inner_collection = inner.collection;
        if let Some(collection_type) =
            collection_type_reference(lowerer, attached_data, state, inner_collection)
            && let Some((element_type, _)) = element_type_of(lowerer.source_trees, &collection_type)
            && let TypeReference::Named { name, .. } = &element_type
        {
            let data_name = name.clone();
            if let Some(field_type) =
                attached_field_type(lowerer.source_trees, &data_name, &member_name)
            {
                return Ok(Some(lower_type_reference_into_table(lowerer, &field_type)?));
            }
        }
        return Ok(None);
    }

    // A COMPUTED-INDEX temp (`let __hoist = self.k + 1`, hoisted by the
    // syntax->resolved index hoist so `arr[k + 1]` indexes a slotted plain
    // place): the temp's type is the BASE SCALAR of the first typeable place
    // operand -- the operand's OWN range does not hold for the computed value
    // (`k in [0..=3]` but `k + 1` reaches 4) -- carrying a SYNTHESIZED range
    // computed by interval arithmetic over the operands' Exact declared
    // ranges. The range is what lets the index prover discharge the bound
    // (`expression_enforced_declared_range`), and it is SOUND because a
    // declared range is store-enforced: the temp's one store (the
    // initializer) is range-proved, which accepts `k + 1` outright and
    // rejects an unguarded `k - 1` (underflow at k=0) unless a dominating
    // guard narrows k -- exactly the manual let-temp idiom's behavior. An
    // operand with no Exact range yields a bare scalar temp (the index prover
    // then reports its clean cannot-prove error).
    if let ExpressionNode::Binary(binary) = expressions.expression(initial_value) {
        for operand in [binary.left, binary.right] {
            if let Some(place_type) =
                collection_type_reference(lowerer, attached_data, state, operand)
            {
                let base = base_scalar_type(lowerer.source_trees, &place_type).clone();
                let base_handle = lower_type_reference_into_table(lowerer, &base)?;
                let Some((low, high)) = computed_index_interval(
                    lowerer,
                    attached_data,
                    state,
                    binary.operator,
                    binary.left,
                    binary.right,
                ) else {
                    return Ok(Some(base_handle));
                };
                // A negative bound cannot ride an UNSIGNED scalar's range;
                // clamp the low end to 0 (the store proof still gates every
                // value, so clamping never widens what is accepted).
                let low = if scalar_is_unsigned(&base) {
                    if high < 0 {
                        return Ok(Some(base_handle));
                    }
                    low.max(0)
                } else {
                    low
                };
                let minimum = lowerer.typed_trees.expression_table.insert(
                    typed::expression::ExpressionNode::Integer(
                        psi_numerics::literals::IntegerLiteral::from_value(low),
                    ),
                );
                let maximum = lowerer.typed_trees.expression_table.insert(
                    typed::expression::ExpressionNode::Integer(
                        psi_numerics::literals::IntegerLiteral::from_value(high),
                    ),
                );
                let constraints = lowerer
                    .typed_trees
                    .type_reference_table
                    .insert_constraints([typed::types::TypeConstraintNode::Range {
                        minimum,
                        maximum,
                    }]);
                return Ok(Some(lowerer.typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Constrained {
                        base_type: base_handle,
                        constraints,
                    },
                )));
            }
        }
        return Ok(None);
    }

    let ExpressionNode::Indexed(indexed) = expressions.expression(initial_value) else {
        return Ok(None);
    };
    // A NESTED indexed read (`grid[i][j]`, `rows[i].data[j]` -- the
    // double-indexed hoist temps) descends one ELEMENT level per `Indexed`
    // layer and one FIELD level per `Member` link. Walk the chain to the base
    // place collecting the steps (they come out innermost-last, so apply them
    // in reverse), then re-derive the leaf type from the base.
    enum NestedStep<'walk> {
        Element,
        Field(&'walk resolved::name::DiagnosticName),
    }
    let mut steps: Vec<NestedStep<'_>> = vec![NestedStep::Element];
    let mut collection = indexed.collection;
    loop {
        match expressions.expression(collection) {
            ExpressionNode::Indexed(inner) => {
                steps.push(NestedStep::Element);
                collection = inner.collection;
            }
            ExpressionNode::Member(member) => {
                // The base place itself may be a `self.<field>` member --
                // stop when the receiver is `self` (collection_type_reference
                // resolves that shape directly).
                if matches!(
                    expressions.expression(member.receiver),
                    ExpressionNode::Name(path)
                        if expressions
                            .name_path_members(path.members)
                            .first()
                            .is_some_and(|name| name.as_str() == "self")
                ) {
                    break;
                }
                steps.push(NestedStep::Field(&member.member));
                collection = member.receiver;
            }
            ExpressionNode::Borrow(inner) => collection = inner.target,
            _ => break,
        }
    }

    let Some(collection_type) =
        collection_type_reference(lowerer, attached_data, state, collection)
    else {
        return Ok(None);
    };

    // Re-apply the steps outermost-base-first. Element steps keep the LAST
    // non-empty collection-level constraint span (the innermost declared
    // domain characterizes the scalar element); a Field step replaces the
    // type wholly (the field's declared type carries its own constraints).
    let mut element_type = collection_type;
    let mut constraints = HandleSpan::empty();
    for step in steps.iter().rev() {
        match step {
            NestedStep::Element => {
                let Some((next_element, next_constraints)) =
                    element_type_of(lowerer.source_trees, &element_type)
                else {
                    return Ok(None);
                };
                element_type = next_element;
                if !next_constraints.is_empty() {
                    constraints = next_constraints;
                }
            }
            NestedStep::Field(field_name) => {
                let TypeReference::Named { name, .. } = &element_type else {
                    return Ok(None);
                };
                let data_name = name.clone();
                let Some(field_type) =
                    attached_field_type(lowerer.source_trees, &data_name, field_name.as_str())
                else {
                    return Ok(None);
                };
                element_type = field_type;
                constraints = HandleSpan::empty();
            }
        }
    }

    let element_handle = lower_type_reference_into_table(lowerer, &element_type)?;
    if constraints.is_empty() {
        return Ok(Some(element_handle));
    }

    // Re-apply only the collection constraints that characterize the ELEMENT
    // (arithmetic domains like `Trapping`); an encoding `Domain` (`[u8] in Utf8`)
    // is bound to the collection's storage type, not to `u8`, so it is dropped.
    let constraints = lower_element_applicable_constraints(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        constraints,
    )?;
    if constraints.is_empty() {
        return Ok(Some(element_handle));
    }
    Ok(Some(lowerer.typed_trees.type_reference_table.insert(
        typed::types::TypeReferenceNode::Constrained {
            base_type: element_handle,
            constraints,
        },
    )))
}

fn static_conformance_requirement_return(
    lowerer: &Lowerer,
    state: &resolved::state::State,
    call: &resolved::expression::TableCallExpression,
) -> Option<TypeReference> {
    let ExpressionNode::Name(receiver) = lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression(call.receiver)
    else {
        return None;
    };
    let owner = lowerer.source_trees.roots.machines.iter().find(|machine| {
        lowerer
            .source_trees
            .machine_state_handles(machine.states)
            .iter()
            .any(|handle| lowerer.source_trees.machine_state(*handle).symbol == state.symbol)
    })?;
    let mut bounds = owner
        .conformance_bounds
        .iter()
        .filter(|bound| bound.binder == Some(receiver.symbol));
    let bound = bounds.next()?;
    if bounds.next().is_some() {
        return None;
    }
    let trait_definition = lowerer
        .source_trees
        .roots
        .traits
        .iter()
        .find(|definition| definition.symbol == bound.carrier)?;
    let mut requirements = lowerer
        .source_trees
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .filter(|requirement| requirement.name.as_str() == call.target.as_str());
    let requirement = requirements.next()?;
    if requirements.next().is_some() {
        return None;
    }
    requirement.storage.return_type.clone()
}

fn substitute_machine_parameters_in_type(
    table: &mut typed::types::TypeReferenceTable,
    handle: typed::types::TypeReferenceHandle,
    substitutions: &[(
        psi_symbols::SymbolHandle,
        psi_symbols::SymbolHandle,
        typed::name::Identifier,
    )],
) {
    if !handle.is_valid() || substitutions.is_empty() {
        return;
    }
    let node = table.type_reference(handle).clone();
    match node {
        typed::types::TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, replacement, name)) = substitutions
                .iter()
                .find(|(parameter, _, _)| *parameter == symbol)
            {
                table.substitute_node(
                    handle,
                    typed::types::TypeReferenceNode::Named {
                        symbol: *replacement,
                        name: name.clone(),
                    },
                );
            }
        }
        typed::types::TypeReferenceNode::Reference { referee, .. } => {
            substitute_machine_parameters_in_type(table, referee, substitutions);
        }
        typed::types::TypeReferenceNode::Constrained { base_type, .. } => {
            substitute_machine_parameters_in_type(table, base_type, substitutions);
        }
        typed::types::TypeReferenceNode::FixedArray { element_type, .. }
        | typed::types::TypeReferenceNode::Slice { element_type } => {
            substitute_machine_parameters_in_type(table, element_type, substitutions);
        }
        typed::types::TypeReferenceNode::Generic { arguments, .. } => {
            let arguments = table.type_reference_handles(arguments).to_vec();
            for argument in arguments {
                substitute_machine_parameters_in_type(table, argument, substitutions);
            }
        }
        typed::types::TypeReferenceNode::DynamicTrait { .. }
        | typed::types::TypeReferenceNode::ConstExpression(_)
        | typed::types::TypeReferenceNode::Unit => {}
    }
}

/// Resolves the resolved `TypeReference` of an indexed read's COLLECTION.
///
/// Handles `self.<field>` (resolved against the machine's attached data) and a
/// bare state parameter or explicitly typed local `<name>` (the slice/array a
/// `[i]` indexes). Other shapes (nested fields, deeper chains) return `None`.
fn collection_type_reference(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    collection: ExpressionHandle,
) -> Option<TypeReference> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    match expressions.expression(collection) {
        // `self.<field>`
        ExpressionNode::Member(member) => {
            let receiver_is_self = matches!(
                expressions.expression(member.receiver),
                ExpressionNode::Name(path)
                    if expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|name| name.as_str() == "self")
            );
            if !receiver_is_self {
                return None;
            }
            let data_name = attached_data?;
            attached_field_type(lowerer.source_trees, data_name, member.member.as_str())
        }
        // A bare `<name>` path -- a state parameter or explicitly typed local
        // naming the indexed collection. Locals remain fully explicit in Psi:
        // this only recovers the authored type of the collection that the
        // normalization pass moved behind a synthetic Unit-typed hoist temp.
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let [only] = members else {
                return None;
            };
            parameter_type(lowerer.source_trees, state, only.as_str()).or_else(|| {
                local_type(
                    lowerer.source_trees,
                    state,
                    path.head_symbol,
                    path.symbol,
                    only.as_str(),
                )
            })
        }
        _ => None,
    }
}

/// The BASE SCALAR of a place type, peeling `Constrained` wrappers: a declared
/// range or domain describes that PLACE's stored values, not a value computed
/// FROM it.
fn base_scalar_type<'trees>(
    source_trees: &'trees resolved::SymbolResolvedTrees,
    type_reference: &'trees TypeReference,
) -> &'trees TypeReference {
    match type_reference {
        TypeReference::Constrained(constrained) => base_scalar_type(
            source_trees,
            source_trees.child_type_reference(constrained.base_type),
        ),
        other => other,
    }
}

fn scalar_is_unsigned(type_reference: &TypeReference) -> bool {
    matches!(
        type_reference,
        TypeReference::Named { name, .. }
            if matches!(name.as_str(), "u8" | "u16" | "u32" | "u64" | "usize")
    )
}

/// The inclusive interval of a hoisted computed index, by interval arithmetic
/// over the two operands' intervals. `None` when an operand has no usable
/// interval, the operator is not +/-/*, or the arithmetic overflows i64.
fn computed_index_interval(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    operator: resolved::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<(i64, i64)> {
    // R3b: the DIRECT product spelling `pixels[y * self.cols + x]` -- the
    // typed-side bounded-product rule mirrored at the hoist synthesis, so
    // the temp's range carries the coupling bound and the index prover
    // discharges without an explicit ranged temp.
    if let Some(interval) = dependent_product_index_interval(lowerer, state, operator, left, right)
    {
        return Some(interval);
    }
    let left = operand_declared_interval(lowerer, attached_data, state, left)?;
    let right = operand_declared_interval(lowerer, attached_data, state, right)?;
    match operator {
        resolved::expression::BinaryOperator::Add => {
            Some((left.0.checked_add(right.0)?, left.1.checked_add(right.1)?))
        }
        resolved::expression::BinaryOperator::Subtract => {
            Some((left.0.checked_sub(right.1)?, left.1.checked_sub(right.0)?))
        }
        resolved::expression::BinaryOperator::Multiply => {
            let products = [
                left.0.checked_mul(right.0)?,
                left.0.checked_mul(right.1)?,
                left.1.checked_mul(right.0)?,
                left.1.checked_mul(right.1)?,
            ];
            Some((
                *products.iter().min().expect("non-empty"),
                *products.iter().max().expect("non-empty"),
            ))
        }
        _ => None,
    }
}

/// An OPERAND's inclusive interval: an integer literal's exact value, or a
/// place's declared Exact range (a non-Exact domain's range is deliberately
/// permissive -- probed live: `[0..=4] in Wrapping` accepts a store of 100 --
/// so it must never seed an index-bound synthesis).
fn operand_declared_interval(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    operand: ExpressionHandle,
) -> Option<(i64, i64)> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    if let ExpressionNode::Integer(literal) = expressions.expression(operand) {
        let value = literal.value_i64()?;
        return Some((value, value));
    }
    // A nested binary operand (`y * 4` inside `y * 4 + x`) composes through
    // the same interval arithmetic -- depth-bounded by the hoist predicate,
    // which admits exactly one nested level (R0's row-major shape).
    if let ExpressionNode::Binary(inner) = expressions.expression(operand) {
        return computed_index_interval(
            lowerer,
            attached_data,
            state,
            inner.operator,
            inner.left,
            inner.right,
        );
    }
    let place_type = collection_type_reference(lowerer, attached_data, state, operand)?;
    declared_exact_range(lowerer.source_trees, &place_type).or_else(|| {
        dependent_exact_range_substituted(lowerer.source_trees, attached_data, &place_type)
    })
}

/// R1a dependent maximum as an INTERVAL operand: `y: u32 [0..=self.rows]`
/// contributes `(0, high(rows) + offset)` -- the named field's own
/// store-enforced literal range caps the dependent bound at every moment,
/// so the substituted interval is sound for the temp's synthesized range
/// (the same substitution the checker's index prover applies). The field
/// resolves on the ENCLOSING machine's attached data (`attached_data`).
fn dependent_exact_range_substituted(
    source_trees: &resolved::SymbolResolvedTrees,
    attached_data: Option<&resolved::name::DiagnosticName>,
    type_reference: &TypeReference,
) -> Option<(i64, i64)> {
    let TypeReference::Constrained(constrained) = type_reference else {
        return None;
    };
    let constraints = source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints);
    if constraints.iter().any(|constraint| {
        matches!(
            constraint,
            TypeConstraint::ArithmeticDomain(domain)
                if *domain != psi_numerics::arithmetic::ArithmeticDomain::Exact
        )
    }) {
        return None;
    }
    let data_name = attached_data?;
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            return None;
        };
        let expressions = &source_trees.tables.bodies.expressions;
        let low = match expressions.expression(*minimum) {
            ExpressionNode::Integer(literal) => literal.value_i64()?,
            _ => return None,
        };
        let (field, offset) = resolved_symbolic_max_bound(expressions, *maximum)?;
        let field_type = attached_field_type(source_trees, data_name, &field)?;
        let (_, field_high) = declared_exact_range(source_trees, &field_type)?;
        let high = field_high.checked_add(offset)?;
        (low <= high).then_some((low, high))
    })
}

/// Resolved-tree twin of `psi_typed_trees::dependent_ranges`'s recognizer
/// (`self.<field>` plus an optional literal offset). Kept to the same
/// admissible class -- the typed-level gate has already validated it.
fn resolved_symbolic_max_bound(
    expressions: &resolved::expression::ExpressionTable,
    bound: ExpressionHandle,
) -> Option<(String, i64)> {
    let self_field = |handle: ExpressionHandle| -> Option<String> {
        let ExpressionNode::Member(member) = expressions.expression(handle) else {
            return None;
        };
        let ExpressionNode::Name(path) = expressions.expression(member.receiver) else {
            return None;
        };
        let [only] = expressions.name_path_members(path.members) else {
            return None;
        };
        (only.as_str() == "self").then(|| member.member.as_str().to_owned())
    };
    match expressions.expression(bound) {
        ExpressionNode::Member(_) => Some((self_field(bound)?, 0)),
        ExpressionNode::Binary(binary) => {
            let field = self_field(binary.left)?;
            let ExpressionNode::Integer(literal) = expressions.expression(binary.right) else {
                return None;
            };
            let magnitude = literal.value_i64()?;
            let offset = match binary.operator {
                resolved::expression::BinaryOperator::Add => magnitude,
                resolved::expression::BinaryOperator::Subtract => magnitude.checked_neg()?,
                _ => return None,
            };
            Some((field, offset))
        }
        _ => None,
    }
}

/// The Range constraint of a resolved place type, ONLY when every arithmetic
/// domain in its Constrained shells is Exact (mirrors the checker's
/// `enforced_range_of_type_reference`).
fn declared_exact_range(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<(i64, i64)> {
    let TypeReference::Constrained(constrained) = type_reference else {
        return None;
    };
    let constraints = source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints);
    if constraints.iter().any(|constraint| {
        matches!(
            constraint,
            TypeConstraint::ArithmeticDomain(domain)
                if *domain != psi_numerics::arithmetic::ArithmeticDomain::Exact
        )
    }) {
        return None;
    }
    constraints
        .iter()
        .find_map(|constraint| match constraint {
            TypeConstraint::Range { minimum, maximum } => {
                let expressions = &source_trees.tables.bodies.expressions;
                let low = match expressions.expression(*minimum) {
                    ExpressionNode::Integer(literal) => literal.value_i64()?,
                    _ => return None,
                };
                let high = match expressions.expression(*maximum) {
                    ExpressionNode::Integer(literal) => literal.value_i64()?,
                    _ => return None,
                };
                Some((low, high))
            }
            _ => None,
        })
        .or_else(|| {
            declared_exact_range(
                source_trees,
                source_trees.child_type_reference(constrained.base_type),
            )
        })
}

/// The declared resolved type of `field_name` in the data definition named
/// `data_name`.
fn attached_field_type(
    source_trees: &resolved::SymbolResolvedTrees,
    data_name: &resolved::name::DiagnosticName,
    field_name: &str,
) -> Option<TypeReference> {
    let data = source_trees
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == data_name.as_str())?;
    source_trees
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            resolved::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                Some(field.type_reference.clone())
            }
            _ => None,
        })
}

/// The declared resolved type of state parameter `name`.
fn parameter_type(
    source_trees: &resolved::SymbolResolvedTrees,
    state: &resolved::state::State,
    name: &str,
) -> Option<TypeReference> {
    source_trees
        .state_parameters(state.parameters)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
        .map(|parameter| parameter.type_reference.clone())
}

/// The authored resolved type of a state local. Prefer symbol identity so a
/// shadowed name cannot retag a hoist; the name fallback is retained for the
/// generated-tree phase where a path may not yet carry its final symbol.
fn local_type(
    source_trees: &resolved::SymbolResolvedTrees,
    state: &resolved::state::State,
    head_symbol: psi_symbols::SymbolHandle,
    symbol: psi_symbols::SymbolHandle,
    name: &str,
) -> Option<TypeReference> {
    source_trees
        .state_statements(state.statements)
        .iter()
        .find_map(|statement| {
            let resolved::statement::Statement::LocalData(local) = statement else {
                return None;
            };
            let symbol_matches = (head_symbol.is_valid() && head_symbol == local.symbol)
                || (symbol.is_valid() && symbol == local.symbol);
            let symbols_are_absent = !head_symbol.is_valid() && !symbol.is_valid();
            (symbol_matches || (symbols_are_absent && local.name.as_str() == name))
                .then(|| local.type_reference.clone())
        })
}

/// The element type of an indexable collection type plus the COLLECTION-LEVEL
/// constraint span to re-apply to it.
///
/// `[T; N]` / `&[T]` / `[T]` yield `(T, empty)`. A leading reference (`&[T]`) is
/// looked through. A `Constrained{inner, [c..]}` collection (`[i32; N] in
/// Trapping`) yields the element type of `inner` paired with `[c..]`, so an
/// element read inherits the collection's arithmetic/encoding domain -- exactly
/// as a runtime read of one element does.
fn element_type_of(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<(TypeReference, HandleSpan<TypeConstraint>)> {
    match type_reference {
        TypeReference::FixedArray(fixed_array) => Some((
            source_trees
                .child_type_reference(fixed_array.element_type)
                .clone(),
            HandleSpan::empty(),
        )),
        TypeReference::Slice(slice) => Some((
            source_trees
                .child_type_reference(slice.element_type)
                .clone(),
            HandleSpan::empty(),
        )),
        TypeReference::Reference(reference) => element_type_of(
            source_trees,
            source_trees.child_type_reference(reference.referee),
        ),
        TypeReference::Constrained(constrained) => {
            let base = source_trees.child_type_reference(constrained.base_type);
            let (element, _inner_constraints) = element_type_of(source_trees, base)?;
            // The collection's own constraints (`in Trapping`) apply to each
            // element read; an inner element does not carry collection-level
            // constraints, so this span fully describes the element's domain.
            Some((element, constrained.constraints))
        }
        _ => None,
    }
}

/// R3b: `a * self.Fb + c` with STRICT dependent params (`a < self.Fa`,
/// `c < self.Fb`, resolved-tree recognizers) and the owning machine's
/// `requires self.Fa * self.Fb <= K` coupling -> `(0, K - 1)`. Mirrors
/// psi-validation's refine_dependent_product; kept in lockstep. The
/// preservation gate is the typed-side rule's job at USE time -- the
/// synthesized range is store-proved on the temp's own initializer, which
/// re-runs the typed-side rule (a machine that writes the dims fails THAT
/// proof, so a stale synthesis here never survives to an index).
fn dependent_product_index_interval(
    lowerer: &Lowerer,
    state: &resolved::state::State,
    operator: resolved::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<(i64, i64)> {
    if operator != resolved::expression::BinaryOperator::Add {
        return None;
    }
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let ExpressionNode::Binary(product) = expressions.expression(left) else {
        return None;
    };
    if product.operator != resolved::expression::BinaryOperator::Multiply {
        return None;
    }
    let (a_expr, fb_expr) = {
        let left_is_field = resolved_symbolic_max_bound(expressions, product.left)
            .is_some_and(|(_, offset)| offset == 0);
        if left_is_field {
            (product.right, product.left)
        } else {
            (product.left, product.right)
        }
    };
    let (fb, fb_offset) = resolved_symbolic_max_bound(expressions, fb_expr)?;
    if fb_offset != 0 {
        return None;
    }
    let fa = strict_dependent_param_field(lowerer, state, a_expr)?;
    let c_field = strict_dependent_param_field(lowerer, state, right)?;
    if c_field != fb {
        return None;
    }
    let machine = lowerer.source_trees.machines.iter().find(|machine| {
        lowerer
            .source_trees
            .machine_state_handles(machine.storage.states)
            .iter()
            .any(|handle| lowerer.source_trees.machine_state(*handle).symbol == state.symbol)
    })?;
    let k = resolved_product_coupling(lowerer, machine, &fa, &fb)?;
    let high = k.checked_sub(1)?;
    (high >= 0).then_some((0, high))
}

/// The field a STRICT dependent PARAM names on the resolved trees: the
/// expression is a bare param name whose declared range maximum is
/// `self.<field> - 1`.
fn strict_dependent_param_field(
    lowerer: &Lowerer,
    state: &resolved::state::State,
    expression: ExpressionHandle,
) -> Option<String> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let ExpressionNode::Name(path) = expressions.expression(expression) else {
        return None;
    };
    let [only] = expressions.name_path_members(path.members) else {
        return None;
    };
    let parameter = parameter_type(lowerer.source_trees, state, only.as_str())?;
    let TypeReference::Constrained(constrained) = parameter else {
        return None;
    };
    let constraints = lowerer
        .source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints);
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { maximum, .. } = constraint else {
            return None;
        };
        let (field, offset) = resolved_symbolic_max_bound(expressions, *maximum)?;
        (offset == -1).then_some(field)
    })
}

/// The owning machine's `requires self.Fa * self.Fb <= K` conjunct on the
/// RESOLVED trees (either multiply order; strict `<` gives K - 1).
fn resolved_product_coupling(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
    fa: &str,
    fb: &str,
) -> Option<i64> {
    for contract in lowerer.source_trees.machine_contracts(machine) {
        if contract.kind != resolved::signature::SignatureContractKind::Requires {
            continue;
        }
        for fact in lowerer.source_trees.proof_facts(contract.facts) {
            let resolved::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(k) = resolved_product_conjunct(lowerer, machine, *expression, fa, fb) {
                return Some(k);
            }
        }
    }
    None
}

fn resolved_product_conjunct(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
    guard: ExpressionHandle,
    fa: &str,
    fb: &str,
) -> Option<i64> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    use resolved::expression::BinaryOperator;
    match binary.operator {
        BinaryOperator::And => resolved_product_conjunct(lowerer, machine, binary.left, fa, fb)
            .or_else(|| resolved_product_conjunct(lowerer, machine, binary.right, fa, fb)),
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            let ExpressionNode::Binary(product) = expressions.expression(binary.left) else {
                return None;
            };
            if product.operator != BinaryOperator::Multiply {
                return None;
            }
            let lhs = resolved_product_coupling_operand(lowerer, machine, product.left)?;
            let rhs = resolved_product_coupling_operand(lowerer, machine, product.right)?;
            let matches = (lhs == fa && rhs == fb) || (lhs == fb && rhs == fa);
            if !matches {
                return None;
            }
            let ExpressionNode::Integer(literal) = expressions.expression(binary.right) else {
                return None;
            };
            let k = literal.value_i64()?;
            if binary.operator == BinaryOperator::Less {
                k.checked_sub(1)
            } else {
                Some(k)
            }
        }
        _ => None,
    }
}

/// Resolved-tree twin of psi-validation's bounded-product operand projection.
/// The direct field spelling remains canonical. An Exact widening cast is
/// transparent because validation independently proves it preserves the
/// source value and makes the specification product total.
fn resolved_product_coupling_operand(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
    expression: ExpressionHandle,
) -> Option<String> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    if let Some((field, 0)) = resolved_symbolic_max_bound(expressions, expression) {
        return Some(field);
    }
    let ExpressionNode::Cast(cast) = expressions.expression(expression) else {
        return None;
    };
    if cast.form.is_recast()
        || !cast.semantic_domain.is_empty()
        || cast.domain != psi_numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let (field, 0) = resolved_symbolic_max_bound(expressions, cast.value)? else {
        return None;
    };
    let target = lowerer
        .source_trees
        .child_type_reference(cast.target_type)
        .primitive_type()?;
    let source = resolved_primitive_type(
        lowerer.source_trees,
        &attached_field_type(
            lowerer.source_trees,
            machine.attached_data.as_ref()?,
            &field,
        )?,
    )?;
    let width = |primitive| match primitive {
        resolved::types::PrimitiveType::U8 => Some(8),
        resolved::types::PrimitiveType::U16 => Some(16),
        resolved::types::PrimitiveType::U32 => Some(32),
        resolved::types::PrimitiveType::U64 => Some(64),
        _ => None,
    };
    if width(source)? >= width(target)? {
        return None;
    }
    Some(field)
}

fn resolved_primitive_type(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<resolved::types::PrimitiveType> {
    match type_reference {
        TypeReference::Constrained(constrained) => resolved_primitive_type(
            source_trees,
            source_trees.child_type_reference(constrained.base_type),
        ),
        other => other.primitive_type(),
    }
}
