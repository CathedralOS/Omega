//! Call argument substitution into published crash routes.

use crate::facts::crash_calls::crash_predicate_from_expression;
use crate::facts::crash_calls::private_summaries::crash_route_expressions_by_identity;
use crate::facts::crash_calls::summary_predicates::{
    CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate, SummaryCrashRouteGuard,
    concrete_guard_scalar_value, normalize_summary_buckets, normalize_summary_guards,
    scalar_guard_is_integer_comparison, summary_boolean_value,
};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;

/// One call argument's entry operand under the projection a surviving guard
/// reads. `ordinal` indexes the callee parameter telescope; `members` is the
/// contiguous member spine above its `Parameter` leaf. A bare leaf keeps the
/// precomputed whole-operand substitution (`whole`), while a projected leaf
/// asks `entry_operand_projected` for exactly that field path — a mutable
/// actual written only in a sibling field still supplies the read field's
/// saved actual instead of widening the route to `Truth`. `is_self`
/// parameters keep no substitution: receiver-entry identity needs retained
/// referent custody a bare argument cannot supply.
#[allow(clippy::too_many_arguments)]
pub(crate) fn call_argument_entry_operand(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    whole: &[Option<CrashPredicateExpression>],
    ordinal: u32,
    members: &[String],
) -> Option<CrashPredicateExpression> {
    let ordinal = ordinal as usize;
    let parameter = parameters.get(ordinal)?;
    if members.is_empty() {
        return whole.get(ordinal)?.clone();
    }
    if parameter.is_self {
        return None;
    }
    // Transition/call arguments bind only the non-self parameters, in order.
    let argument_index = parameters[..ordinal]
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let argument = arguments.get(argument_index).copied()?;
    let projection = crate::facts::crash_entry_values::formal_member_projection(
        program,
        parameter.type_reference,
        members,
    );
    crate::facts::crash_entry_values::entry_operand_projected(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        argument,
        &projection,
    )
}

pub(crate) enum SelectedTargetCrashRoutes<'a> {
    Published {
        buckets: &'a [checked_trees::CrashRouteBucket],
        contracts: &'a [typed_trees::signature::SignatureContract],
    },
    Private(&'a [SummaryCrashBucket]),
    Empty,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn call_argument_substitution(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    state_flow: &checked_trees::FlowStateFact,
    call_flow: &checked_trees::FlowCallFact,
    target_parameters: &[typed_trees::signature::StateParameter],
    arguments: &[typed_trees::expression::ExpressionHandle],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> CallArgumentSubstitution {
    // Both direct published routes and private/transitive summaries cross the
    // same namespace boundary. Source spelling and statement position cannot
    // prove that a current actual still denotes the caller's entry value.
    // Retain exact immutable entry inputs/literals; unsupported current-value
    // provenance leaves a missing substitution, which widens the cause below.
    let caller_state = state_flow.state_symbol;
    let before_statement = call_flow.statement_index;
    let owner = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == caller_state)
            .map(|state| (machine, state))
    });
    // Each actual's proven current value is read only from this call's own
    // entry contexts — the semantic facts flow proves live exactly when the
    // invocation begins, after every operand already ran. A place the
    // containing statement may still overwrite through a sibling operand
    // stays unproven: the entry snapshot postdates those effects and would
    // describe newer storage than the argument carried.
    let entry_contexts: Vec<facts::FactContextHandle> = flow
        .contexts
        .semantic_context_refs
        .span_or_empty(call_flow.entry_semantic_contexts)
        .iter()
        .map(|reference| reference.context)
        .collect();
    let containing_statement = owner.and_then(|(_, state)| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .get(before_statement)
    });
    // Structural guard leaves name the authored parameter telescope. Each
    // actual's root is the frozen caller parameter (plus its member spine)
    // whose storage it reads, so a callee leaf can re-root into the caller's
    // contract namespace. A mutable or exclusively borrowed root keeps
    // `None`: the surviving annotation is an entry snapshot and could not
    // speak for the bound operand.
    let caller_parameters = owner.map(|(_, state)| program.state_parameters(state));
    // The dense binding namespace of `lower_unit_scalar_argument`: primitive
    // parameters, then immutable primitive locals in declaration order.
    // Mutable storage resolves through named StorageRead leaves instead.
    let caller_symbols: Vec<SymbolHandle> = owner
        .map(|(_, state)| {
            program
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .map(|parameter| parameter.symbol)
                .chain(
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .get(..before_statement)
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(|statement| match statement {
                            typed_trees::statement::StatementNode::LocalData(local)
                                if !local.is_mutable
                                    && local.initial_value.is_valid()
                                    && program
                                        .primitive_type_reference(local.type_reference)
                                        .is_some() =>
                            {
                                Some(local.symbol)
                            }
                            _ => None,
                        }),
                )
                .collect()
        })
        .unwrap_or_default();
    let mut argument_index = 0usize;
    let mut identity = Vec::with_capacity(target_parameters.len());
    let mut scalar = Vec::with_capacity(target_parameters.len());
    let mut fields = Vec::with_capacity(target_parameters.len());
    let mut values = Vec::with_capacity(target_parameters.len());
    for parameter in target_parameters {
        if parameter.is_self {
            // Receiver-entry identity needs retained referent custody. A name
            // alone must not impersonate a caller entry value.
            identity.push(None);
            fields.push(None);
            values.push(None);
            continue;
        }
        let argument = arguments.get(argument_index).copied();
        argument_index = argument_index.saturating_add(1);
        fields.push(argument.and_then(|argument| {
            structural_actual_root(program, caller_parameters.unwrap_or(&[]), argument)
        }));
        let entry_identity = argument.and_then(|argument| {
            let (machine, state) = owner?;
            crate::facts::crash_entry_values::entry_operand(
                program,
                machine.symbol,
                state.symbol,
                before_statement,
                argument,
            )
        });
        // Scalar annotations live in the dense primitive namespace: only
        // primitive-typed formals occupy a slot, regardless of aggregate
        // parameters or receivers before them. Checked scalar evidence is
        // retained independently of entry custody so arithmetic actuals and
        // unprovable origins can still discharge once they become concrete.
        if let Some(expected) = program.primitive_type_reference(parameter.type_reference) {
            scalar.push(argument.and_then(|argument| {
                crate::values::lower_state_scalar_expression(
                    program,
                    operators,
                    owner?.1,
                    before_statement,
                    argument,
                    expected,
                    exact_integer_casts,
                )
            }));
            values.push(argument.and_then(|argument| {
                let (machine, state) = owner?;
                // Live evaluation folds under builtin laws. An actual whose
                // own operator occurrence selected an authored meaning cannot
                // be re-interpreted this way; it keeps no proven value.
                if !validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    argument,
                ) {
                    return None;
                }
                let lowered = crate::values::lower_unit_scalar_argument(
                    program,
                    operators,
                    state,
                    before_statement,
                    argument,
                    expected,
                )?;
                crate::values::evaluate_checked_scalar(
                    &lowered,
                    &mut crate::values::PlaceScalarValues {
                        program,
                        parameters: program.state_parameters(state),
                        symbols: &caller_symbols,
                        value_at_place: |place: &crate::flow::CanonicalPlace| {
                            if containing_statement.is_none_or(|statement| {
                                crate::facts::crash_entry_values::statement_may_overwrite_place(
                                    program,
                                    machine.symbol,
                                    statement,
                                    place,
                                )
                            }) {
                                return None;
                            }
                            crate::values::scalar_value_at_place(
                                program,
                                semantic,
                                entry_contexts
                                    .iter()
                                    .map(|handle| semantic.contexts.get(*handle)),
                                place,
                            )
                        },
                    },
                )
                .and_then(|value| match value {
                    facts::ScalarValue::Boolean(value) => {
                        Some(CrashPredicateExpression::Boolean(value))
                    }
                    facts::ScalarValue::Integer(value) => {
                        Some(CrashPredicateExpression::Integer(value.to_string()))
                    }
                    facts::ScalarValue::Unknown => None,
                })
            }));
        } else {
            values.push(None);
        }
        identity.push(entry_identity);
    }
    CallArgumentSubstitution {
        identity,
        scalar,
        fields,
        values,
    }
}

/// The caller structural root one actual binds: the frozen caller parameter
/// whose storage the actual reads, plus the actual's own member spine below
/// it. A surviving `StructuralParameterField` leaf keeps that root and
/// appends the callee leaf's path — `inner(pair.cell)` under a `left.narrow`
/// guard reads `pair.cell.narrow`. The same field-identity spelling
/// `structural_parameter_place` produces keeps the checked path exact, and
/// the `resolve_structural_parameter_path` round trip proves the converted
/// spine resolves to the canonical place the actual occupies. Only `frozen`
/// roots qualify: a mutable parameter or an exclusive borrow can hold
/// storage newer than the entry snapshot the contract namespace names, so
/// their rows stay `None` rather than naming stale storage.
fn structural_actual_root(
    program: &TypedTrees,
    caller_parameters: &[StateParameter],
    actual: ExpressionHandle,
) -> Option<checked_trees::CheckedStructuralParameterField> {
    let place = crate::flow::canonical_place_from_expression(program, actual)?;
    let root = crate::flow::normalized_event_place_root(program, place.root);
    if !matches!(root, facts::PlaceRoot::Symbol(_)) {
        return None;
    }
    let parameter_position = caller_parameters.iter().position(|parameter| {
        crate::flow::normalized_event_place_root(
            program,
            facts::PlaceRoot::Symbol(parameter.symbol),
        ) == root
    })?;
    let mut path = Vec::with_capacity(place.segments.len());
    for segment in &place.segments {
        path.push(match *segment {
            facts::PlaceSegment::Field { symbol } => {
                checked_trees::CheckedStructuralPredicatePathSegment::Field(
                    structural_member_identity(program, symbol)?,
                )
            }
            facts::PlaceSegment::Case { variant } => {
                checked_trees::CheckedStructuralPredicatePathSegment::Case(
                    structural_member_identity(program, variant)?,
                )
            }
            facts::PlaceSegment::FixedIndex { .. }
            | facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::Index { .. } => return None,
        });
    }
    let parameter_position = u32::try_from(parameter_position).ok()?;
    let (resolved_root, segments, _, frozen) = crate::values::resolve_structural_parameter_path(
        program,
        caller_parameters,
        parameter_position,
        &path,
    )?;
    (frozen
        && crate::flow::normalized_event_place_root(
            program,
            facts::PlaceRoot::Symbol(resolved_root),
        ) == root
        && segments == place.segments)
        .then_some(checked_trees::CheckedStructuralParameterField {
            parameter_position,
            path,
        })
}

/// The checked identity string a place segment's member symbol carries: the
/// field's declared `#identity` when present, else its authored name. This
/// mirrors the spelling `structural_parameter_place` produces so a
/// transported leaf resolves to the same canonical storage.
fn structural_member_identity(program: &TypedTrees, symbol: SymbolHandle) -> Option<String> {
    program.data_definitions().iter().find_map(|data| {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => Some(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ),
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    Some(
                        variant
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| variant.name.as_str().to_owned()),
                    )
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == symbol)
                    .map(|field| {
                        field
                            .identity
                            .map(|identity| format!("#{identity}"))
                            .unwrap_or_else(|| field.name.as_str().to_owned())
                    }),
                typed_trees::data::DataMember::Field(_) => None,
            })
    })
}

pub(crate) fn substitute_checked_boolean_expression(
    expression: &checked_trees::CheckedBooleanExpression,
    arguments: &[Option<checked_trees::CheckedScalarExpression>],
    fields: &[Option<checked_trees::CheckedStructuralParameterField>],
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};

    Some(match expression {
        CheckedBooleanExpression::Constant(value) => CheckedBooleanExpression::Constant(*value),
        CheckedBooleanExpression::Parameter { position } => {
            let CheckedScalarExpression::Boolean(expression) =
                arguments.get(*position)?.as_ref()?.clone()
            else {
                return None;
            };
            *expression
        }
        // A callee contract is parameter-relative. A local can appear only
        // after composing a private body summary; it cannot be rebound by the
        // outer call and therefore deliberately loses portable structure.
        CheckedBooleanExpression::Local { .. } | CheckedBooleanExpression::StorageRead { .. } => {
            return None;
        }
        // An IEEE float comparison is atomic over structural field leaves.
        // Each leaf re-roots through the caller's frozen structural channel:
        // the callee position binds the actual's own caller parameter and
        // member spine, and the leaf path appends below it. The remaining
        // structural terms still refuse — the standalone field leaf keeps
        // its own transport gap.
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => CheckedBooleanExpression::IeeeFloatComparison {
            kind: *kind,
            primitive_type: *primitive_type,
            left: substitute_structural_parameter_field(left, fields)?,
            right: substitute_structural_parameter_field(right, fields)?,
        },
        // A payload-less sum equality is atomic over the same structural
        // leaves: both subjects re-root through the caller channel while the
        // closed case roster names the sum's declared cases — a type-level
        // identity the re-root cannot change, and one the lowering rechecks
        // against the resolved subject before expanding the roster.
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            CheckedBooleanExpression::PayloadlessSumEqual {
                left: substitute_structural_parameter_field(left, fields)?,
                right: substitute_structural_parameter_field(right, fields)?,
                cases: cases.clone(),
            }
        }
        // A byte-sequence content equality is atomic over the same
        // structural leaves: both subjects re-root through the caller's
        // frozen structural channel. The leaf carries no roster or case
        // identity to transport — content equality compares the bytes the
        // resolved subjects hold — so the re-rooted leaves are the whole
        // term, and the lowering rechecks each against the caller
        // subject's retained byte-sequence carrier before emitting the
        // atomic proposition.
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            CheckedBooleanExpression::ByteSequenceEqual {
                left: substitute_structural_parameter_field(left, fields)?,
                right: substitute_structural_parameter_field(right, fields)?,
            }
        }
        // A sum case-membership test is atomic over the same structural
        // leaves: the subject re-roots through the caller's frozen
        // structural channel while the case identity names the resolved
        // sum's declared case — a type-level fact the re-root cannot
        // change, and one the lowering rechecks against the resolved
        // subject's declared cases before emitting the atomic
        // proposition.
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            CheckedBooleanExpression::StructuralCaseMembership {
                subject: substitute_structural_parameter_field(subject, fields)?,
                case: case.clone(),
            }
        }
        // A standalone Boolean field read is the same frozen structural leaf
        // the atomic propositions carry: the callee position binds the
        // actual's caller parameter and member spine, and the leaf path
        // appends below it. The lowering rechecks the re-rooted path ends at
        // a retained Boolean field before emitting the scalar term, so a
        // redirected leaf stays fail-closed downstream.
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            let leaf = substitute_structural_parameter_field(
                &checked_trees::CheckedStructuralParameterField {
                    parameter_position: *parameter_position,
                    path: path.clone(),
                },
                fields,
            )?;
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position: leaf.parameter_position,
                path: leaf.path,
            }
        }
        CheckedBooleanExpression::Not(operand) => CheckedBooleanExpression::Not(Box::new(
            substitute_checked_boolean_expression(operand, arguments, fields)?,
        )),
        CheckedBooleanExpression::Equal { left, right } => CheckedBooleanExpression::Equal {
            left: Box::new(substitute_checked_boolean_expression(
                left, arguments, fields,
            )?),
            right: Box::new(substitute_checked_boolean_expression(
                right, arguments, fields,
            )?),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            CheckedBooleanExpression::IntegerComparison {
                kind: *kind,
                left: Box::new(substitute_checked_scalar_expression(
                    left, arguments, fields,
                )?),
                right: Box::new(substitute_checked_scalar_expression(
                    right, arguments, fields,
                )?),
            }
        }
        CheckedBooleanExpression::And { left, right } => CheckedBooleanExpression::And {
            left: Box::new(substitute_checked_boolean_expression(
                left, arguments, fields,
            )?),
            right: Box::new(substitute_checked_boolean_expression(
                right, arguments, fields,
            )?),
        },
        CheckedBooleanExpression::Or { left, right } => CheckedBooleanExpression::Or {
            left: Box::new(substitute_checked_boolean_expression(
                left, arguments, fields,
            )?),
            right: Box::new(substitute_checked_boolean_expression(
                right, arguments, fields,
            )?),
        },
    })
}

/// Re-root one structural leaf through the call: the callee parameter
/// position binds the actual's caller root (`parameter_position` plus the
/// actual's own member spine) and the leaf's authored path extends below it.
/// `None` where the actual's root stayed unproven — the guard then loses its
/// checked scalar form rather than naming storage the entry snapshot cannot
/// describe.
fn substitute_structural_parameter_field(
    leaf: &checked_trees::CheckedStructuralParameterField,
    fields: &[Option<checked_trees::CheckedStructuralParameterField>],
) -> Option<checked_trees::CheckedStructuralParameterField> {
    let root = fields.get(leaf.parameter_position as usize)?.as_ref()?;
    let mut path = root.path.clone();
    path.extend_from_slice(&leaf.path);
    Some(checked_trees::CheckedStructuralParameterField {
        parameter_position: root.parameter_position,
        path,
    })
}

fn substitute_checked_scalar_expression(
    expression: &checked_trees::CheckedScalarExpression,
    arguments: &[Option<checked_trees::CheckedScalarExpression>],
    fields: &[Option<checked_trees::CheckedStructuralParameterField>],
) -> Option<checked_trees::CheckedScalarExpression> {
    use checked_trees::CheckedScalarExpression;

    Some(match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            let substituted = arguments.get(*position)?.as_ref()?.clone();
            (crate::values::scalar_expression_type(&substituted) == Some(*primitive_type))
                .then_some(substituted)?
        }
        CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. }
        | CheckedScalarExpression::IntegerWrappingCast { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => return None,
        // A standalone integer field leaf is the same frozen structural leaf
        // the Boolean channel carries: the callee position binds the actual's
        // caller parameter and member spine, and the leaf path appends below
        // it. The lowering rechecks the re-rooted path ends at a retained
        // integer field of the declared type before emitting the scalar term,
        // so a redirected leaf stays fail-closed downstream.
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            let leaf = substitute_structural_parameter_field(
                &checked_trees::CheckedStructuralParameterField {
                    parameter_position: *parameter_position,
                    path: path.clone(),
                },
                fields,
            )?;
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: leaf.parameter_position,
                path: leaf.path,
                primitive_type: *primitive_type,
            }
        }
        CheckedScalarExpression::IntegerLiteral { literal } => {
            CheckedScalarExpression::IntegerLiteral {
                literal: literal.clone(),
            }
        }
        CheckedScalarExpression::IeeeFloatLiteral { value } => {
            CheckedScalarExpression::IeeeFloatLiteral { value: *value }
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => CheckedScalarExpression::IntegerBinary {
            kind: *kind,
            primitive_type: *primitive_type,
            left: Box::new(substitute_checked_scalar_expression(
                left, arguments, fields,
            )?),
            right: Box::new(substitute_checked_scalar_expression(
                right, arguments, fields,
            )?),
        },
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(
                operand, arguments, fields,
            )?),
        },
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => CheckedScalarExpression::IntegerWiden {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(
                operand, arguments, fields,
            )?),
        },
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => CheckedScalarExpression::IntegerExactCast {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(
                operand, arguments, fields,
            )?),
            range: range.clone(),
        },
        CheckedScalarExpression::Boolean(expression) => CheckedScalarExpression::Boolean(Box::new(
            substitute_checked_boolean_expression(expression, arguments, fields)?,
        )),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refine_published_crash_routes(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    state_flow: &checked_trees::FlowStateFact,
    call_flow: &checked_trees::FlowCallFact,
    call_site: &crate::semantic_calls::CallSite<'_>,
    target_state_symbol: SymbolHandle,
    target_parameters: &[typed_trees::signature::StateParameter],
    target_parameter_names: &[String],
    buckets: &[checked_trees::CrashRouteBucket],
    contracts: &[typed_trees::signature::SignatureContract],
    content_conservation: &[validation::ContentConservationSourcePlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<SummaryCrashBucket> {
    let route_expressions = crash_route_expressions_by_identity(
        program,
        contracts,
        target_parameter_names,
        content_conservation,
    );
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, call_site);
    let substitution = call_argument_substitution(
        program,
        operators,
        semantic,
        flow,
        state_flow,
        call_flow,
        target_parameters,
        arguments,
        exact_integer_casts,
    );
    let mut surviving = Vec::new();
    for bucket in buckets {
        let mut guards = Vec::new();
        for guard in bucket.alternative_guards() {
            match guard {
                checked_trees::CrashRouteGuard::Truth => {
                    guards.push(SummaryCrashRouteGuard::Truth);
                }
                checked_trees::CrashRouteGuard::Predicate(identity) => {
                    let expression = *route_expressions.get(identity).expect(
                        "a canonical published crash route retains its typed producer expression",
                    );
                    // The token-only predicate identity does not select operator
                    // meaning. Retain authority only from this exact local owner.
                    let builtin_meaning = program
                        .machines()
                        .iter()
                        .find_map(|machine| {
                            program
                                .machine_states(machine)
                                .iter()
                                .find(|state| state.symbol == target_state_symbol)
                                .map(|state| {
                                    validation::has_builtin_bound_expression_meaning(
                                        program,
                                        machine,
                                        Some(state),
                                        expression,
                                    )
                                })
                        })
                        .unwrap_or(false);
                    let predicate = crash_predicate_from_expression(
                        program,
                        expression,
                        target_parameter_names,
                        Some(content_conservation),
                    );
                    // The direct evaluator follows actuals and local initializers.
                    // Only referenced formals with retained immutable entry origins
                    // may enter it; callee meaning cannot authorize caller operators.
                    // A projected leaf keeps the per-field caller name when the
                    // read projection stayed pristine across sibling writes.
                    let entry_predicate =
                        crate::facts::crash_entry_values::substitute_entry_projected(
                            &predicate,
                            &mut |ordinal, members| {
                                call_argument_entry_operand(
                                    program,
                                    state_flow.machine_symbol,
                                    state_flow.state_symbol,
                                    call_flow.statement_index,
                                    target_parameters,
                                    arguments,
                                    &substitution.identity,
                                    ordinal,
                                    members,
                                )
                            },
                        );
                    let concrete_value = (builtin_meaning && entry_predicate.is_some())
                        .then(|| {
                            crate::checks::contracts::call_site_boolean_contract_expression_value(
                                program,
                                state_flow,
                                call_flow,
                                call_site,
                                target_state_symbol,
                                target_parameters,
                                expression,
                                call_frames,
                            )
                        })
                        .flatten();
                    match concrete_value {
                        Some(false) => {}
                        Some(true) => guards.push(SummaryCrashRouteGuard::Truth),
                        None => {
                            // Live storage evidence decides a guard entry
                            // custody cannot carry: each referenced formal is
                            // replaced by the one literal this call's own
                            // entry contexts prove for its actual. The fold
                            // honors this route's own selected meaning, and a
                            // missing value leaves the substitution missing
                            // rather than inventing an origin.
                            if let Some(value) = crate::facts::crash_entry_values::substitute_entry(
                                &predicate,
                                &substitution.values,
                            )
                            .and_then(|substituted| {
                                if builtin_meaning {
                                    summary_boolean_value(&substituted)
                                } else {
                                    substituted.boolean_value()
                                }
                            }) {
                                if value {
                                    guards.push(SummaryCrashRouteGuard::Truth);
                                }
                                continue;
                            }
                            let Some(predicate) = entry_predicate else {
                                guards.push(SummaryCrashRouteGuard::Truth);
                                continue;
                            };
                            let scalar = identity.scalar_expression().and_then(|scalar| {
                                substitute_checked_boolean_expression(
                                    scalar,
                                    &substitution.scalar,
                                    &substitution.fields,
                                )
                            });
                            let folded = if builtin_meaning {
                                summary_boolean_value(&predicate)
                            } else {
                                predicate.boolean_value()
                            };
                            // Domain-free folding stands down on arithmetic
                            // operands: a checked integer-comparison
                            // annotation decides them under their selected
                            // domains once every actual is concrete.
                            let value = folded.or_else(|| {
                                (builtin_meaning
                                    && scalar
                                        .as_ref()
                                        .is_some_and(scalar_guard_is_integer_comparison))
                                .then(|| scalar.as_ref().and_then(concrete_guard_scalar_value))
                                .flatten()
                            });
                            match value {
                                Some(false) => {}
                                Some(true) => guards.push(SummaryCrashRouteGuard::Truth),
                                None => guards.push(SummaryCrashRouteGuard::Predicate(
                                    SummaryCrashPredicate {
                                        identity: predicate,
                                        builtin_meaning,
                                        scalar,
                                    },
                                )),
                            }
                        }
                    }
                }
            }
        }
        normalize_summary_guards(&mut guards);
        if !guards.is_empty() {
            surviving.push(SummaryCrashBucket {
                cause: bucket.cause(),
                alternative_guards: guards,
            });
        }
    }
    normalize_summary_buckets(surviving)
}
