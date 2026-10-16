//! Call argument substitution into published crash routes.

use crate::facts::crash_calls::private_summaries::crash_route_expressions_by_identity;
use crate::facts::crash_calls::summary_predicates::{
    CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate, SummaryCrashRouteGuard,
    concrete_guard_scalar_value, normalize_summary_buckets, normalize_summary_guards,
    scalar_guard_is_integer_comparison, scalar_guard_proves_false, summary_boolean_value,
};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;

/// `owner`'s machine and state once `argument` clears the builtin-bound
/// meaning gate: checked scalar evidence folds and annotates under builtin
/// laws, and an actual whose own operator occurrence selected an authored
/// meaning cannot be re-read under semantics its selection rejected.
fn builtin_bound_owner<'o>(
    program: &dyn ::validation::CheckingInput,
    owner: Option<(
        &'o typed_trees::machine::Machine,
        &'o typed_trees::state::State,
    )>,
    argument: ExpressionHandle,
) -> Option<(
    &'o typed_trees::machine::Machine,
    &'o typed_trees::state::State,
)> {
    let (machine, state) = owner?;
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), argument)
        .then_some((machine, state))
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
pub(crate) fn call_argument_substitution<'program>(
    program: &'program dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &facts::FactPlan,
    flow: &'program checked_trees::FlowFacts,
    state_flow: &checked_trees::FlowStateFact,
    call_flow: &checked_trees::FlowCallFact,
    target_parameters: &'program [typed_trees::signature::StateParameter],
    arguments: &[typed_trees::expression::ExpressionHandle],
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> CallArgumentSubstitution<'program> {
    // Both direct published routes and private/transitive summaries cross the
    // same namespace boundary. Source spelling and statement position cannot
    // prove that a current actual still denotes the caller's entry value.
    // Retain exact immutable entry inputs/literals; unsupported current-value
    // provenance leaves a missing substitution, which widens the cause below.
    let caller_state = state_flow.state_symbol;
    let before_statement = call_flow.statement_index;
    let owner = program.machines().iter().find_map(|machine| {
        program
            .state_in_machine(machine, caller_state)
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
            .statement_at(state, before_statement)
    });
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
    let mut values = Vec::with_capacity(target_parameters.len());
    for parameter in target_parameters {
        let argument = arguments.get(argument_index).copied();
        argument_index = argument_index.saturating_add(1);
        let entry_identity = argument.and_then(|argument| {
            let (machine, state) = owner?;
            crate::facts::crash_entry_values::entry_operand(
                program,
                machine.symbol,
                state.symbol,
                before_statement,
                argument,
            )
            .or_else(|| {
                crate::facts::crash_entry_values::checked_projected_entry_value(
                    program,
                    operators,
                    semantic,
                    flow,
                    state_flow,
                    before_statement,
                    argument,
                )
            })
        });
        // Scalar annotations live in the dense primitive namespace: only
        // primitive-typed formals occupy a slot, regardless of aggregate
        // parameters or receivers before them. Checked scalar evidence is
        // retained independently of entry custody so arithmetic actuals and
        // unprovable origins can still discharge once they become concrete.
        // The dense-unit lowering is primary: a caller mixing structural and
        // primitive parameters still describes its scalar actuals there, while
        // the state lowering remains the fallback that can fold an exact-cast
        // actual against the validation-only `exact_integer_casts` facts.
        if let Some(expected) = program.primitive_type_reference(parameter.type_reference) {
            scalar.push(argument.and_then(|argument| {
                let (_, state) = builtin_bound_owner(program, owner, argument)?;
                let lowered = crate::values::lower_unit_scalar_argument(
                    program,
                    operators,
                    state,
                    before_statement,
                    argument,
                    expected,
                )
                .or_else(|| {
                    crate::values::lower_state_scalar_expression(
                        program,
                        operators,
                        state,
                        before_statement,
                        argument,
                        expected,
                        exact_integer_casts,
                    )
                })?;
                scalar_evidence_is_crash_lane_lowerable(&lowered).then_some(lowered)
            }));
            values.push(argument.and_then(|argument| {
                let (machine, state) = builtin_bound_owner(program, owner, argument)?;
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
                        value_at_place: |place: &validation::CanonicalPlace| {
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
                            .or_else(|| {
                                crate::checks::contracts::boolean_value_at_place(
                                    program,
                                    semantic,
                                    &entry_contexts,
                                    place,
                                )
                                .map(facts::ScalarValue::Boolean)
                            })
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
        fields: StructuralArgumentSubstitution {
            program,
            target_parameters,
            flow,
            operands: crate::flow::captured_call_operands(program, flow, state_flow, call_flow),
            machine: state_flow.machine_symbol,
            state: caller_state,
            before_statement,
        },
        values,
    }
}

/// Invocation-owned structural actuals. Each predicate leaf authenticates its
/// complete projected observation, not the whole actual or a frozen-root flag.
/// A sibling write therefore preserves an unaffected field, while an overlapping
/// write (including another argument's effects) cannot donate caller-entry facts.
/// Copies use their completion epoch; view contents use the final operand's
/// prefix, before the receiving call runs. Both canonical keys and annotations
/// consume this same retained provenance, not a scan of the enclosing statement.
#[derive(Clone)]
pub(crate) struct StructuralArgumentSubstitution<'program> {
    program: &'program dyn ::validation::CheckingInput,
    target_parameters: &'program [StateParameter],
    flow: &'program checked_trees::FlowFacts,
    operands: Option<(&'program [checked_trees::FlowOperandFact], usize)>,
    machine: SymbolHandle,
    state: SymbolHandle,
    before_statement: usize,
}

impl StructuralArgumentSubstitution<'_> {
    pub(crate) fn entry_operand(
        &self,
        whole: &[Option<CrashPredicateExpression>],
        ordinal: u32,
        members: &[String],
    ) -> Option<CrashPredicateExpression> {
        let ordinal = ordinal as usize;
        let parameter = self.target_parameters.get(ordinal)?;
        let projection = crate::facts::crash_entry_values::formal_member_place_projection(
            self.program,
            parameter.type_reference,
            members,
        )?;
        let Some(place) = self.observation_place(ordinal, &projection) else {
            // Non-place literals and arithmetic retain the existing exact
            // whole-value channel; it never invents a projected identity.
            return members
                .is_empty()
                .then(|| whole.get(ordinal).cloned().flatten())
                .flatten();
        };
        let mut expression = crate::facts::crash_entry_values::entry_place_before_statement(
            self.program,
            self.machine,
            self.state,
            self.before_statement,
            &place,
        )?;
        // The canonical substitution reapplies its own formal member spine.
        // Provenance authenticated that complete projection above; return only
        // the actual's root spelling so the spine is not appended twice.
        for expected in members.iter().rev() {
            let CrashPredicateExpression::Member { receiver, member } = expression else {
                return None;
            };
            if member != *expected {
                return None;
            }
            expression = *receiver;
        }
        Some(expression)
    }

    fn observation_place(
        &self,
        ordinal: usize,
        suffix: &[facts::PlaceSegment],
    ) -> Option<validation::CanonicalPlace> {
        let parameter = self.target_parameters.get(ordinal)?;
        let (operands, receiver_offset) = self.operands?;
        let operand = operands.get(ordinal.checked_add(receiver_offset)?)?;
        let (mut place, epoch) =
            if crate::flow::operand_binds_live_view(self.program, parameter.type_reference) {
                let [referent] = self
                    .flow
                    .control
                    .operand_referents
                    .span(operand.referents)?
                else {
                    return None;
                };
                (
                    validation::CanonicalPlace {
                        root: referent.root,
                        segments: self
                            .flow
                            .control
                            .operand_referent_segments
                            .span(referent.segments)?
                            .to_vec(),
                    },
                    operands.last()?,
                )
            } else if crate::flow::operand_binds_stable_copy(self.program, parameter.type_reference)
            {
                (
                    validation::canonical_place_from_expression_in_state(
                        self.program,
                        operand.state_symbol,
                        operand.statement_index,
                        operand.expression,
                    )?,
                    operand,
                )
            } else {
                return None;
            };
        place.segments.extend_from_slice(suffix);
        crate::flow::operand_entry_origin_is_stable(self.program, self.flow, epoch, &place)
            .then_some(place)
    }

    fn subject(
        &self,
        leaf: &checked_trees::CheckedStructuralParameterField,
    ) -> Option<checked_trees::CheckedStructuralParameterField> {
        let (_, suffix, _, _) = crate::values::resolve_structural_parameter_path(
            self.program,
            self.target_parameters,
            leaf.parameter_position,
            &leaf.path,
        )?;
        let place = self.observation_place(leaf.parameter_position as usize, &suffix)?;
        let (parameter_position, path) =
            crate::facts::crash_entry_values::entry_parameter_place_before_statement(
                self.program,
                self.machine,
                self.state,
                self.before_statement,
                &place,
            )?;
        Some(checked_trees::CheckedStructuralParameterField {
            parameter_position,
            path,
        })
    }
}

/// A scalar annotation crossing the call boundary may only carry terms a
/// crash lane can still lower once substituted. Byte observations —
/// `StructuralParameterIndexedRead` and `StructuralSequenceLength` —
/// resolve structural storage views neither crash-route lane carries, and the
/// contract namespace already accepts them: inserting one would keep the
/// guarded route but turn its conservative widening into a downstream
/// rejection, so the annotation stays empty instead. `Local`, `StorageRead`,
/// `IeeeFloatLiteral` leaves stay admissible — the
/// contract namespace refuses them, so they widen rather than pin an
/// unsupported term. A cast actual carries no entry identity, so its scalar
/// can only discharge a closed-false guard in the missing-identity arm and is
/// never kept where the route survives.
fn scalar_evidence_is_crash_lane_lowerable(
    expression: &checked_trees::CheckedScalarExpression,
) -> bool {
    use checked_trees::CheckedScalarExpression;
    match expression {
        // The crash lane replays no representation re-view.
        CheckedScalarExpression::StructuralSequenceLength { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
        | CheckedScalarExpression::RepresentationView { .. } => false,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar_evidence_is_crash_lane_lowerable(left)
                && scalar_evidence_is_crash_lane_lowerable(right)
        }
        checked_trees::integer_widen_triple_pattern!(operand, ..) => {
            scalar_evidence_is_crash_lane_lowerable(operand)
        }
        CheckedScalarExpression::Boolean(expression) => {
            boolean_evidence_is_crash_lane_lowerable(expression)
        }
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::ErasedParameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::ImplementationSelection { .. } => true,
    }
}

fn boolean_evidence_is_crash_lane_lowerable(
    expression: &checked_trees::CheckedBooleanExpression,
) -> bool {
    use checked_trees::CheckedBooleanExpression;
    match expression {
        CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
            [left, right].into_iter().all(|expression| {
                expression.terms.iter().all(|(_, term)| match term {
                    checked_trees::CheckedIntegerMathTerm::Embed { operand, .. } => {
                        scalar_evidence_is_crash_lane_lowerable(operand)
                    }
                    _ => true,
                })
            })
        }

        CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            scalar_evidence_is_crash_lane_lowerable(left)
                && scalar_evidence_is_crash_lane_lowerable(right)
        }
        CheckedBooleanExpression::Not(operand) => boolean_evidence_is_crash_lane_lowerable(operand),
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
            boolean_evidence_is_crash_lane_lowerable(left)
                && boolean_evidence_is_crash_lane_lowerable(right)
        }
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::Local { .. }
        | CheckedBooleanExpression::StorageRead { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::StructuralIdentity { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::ByteSequencePredicate { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
    }
}

pub(crate) fn substitute_checked_boolean_expression(
    expression: &checked_trees::CheckedBooleanExpression,
    arguments: &[Option<checked_trees::CheckedScalarExpression>],
    fields: &StructuralArgumentSubstitution<'_>,
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};

    Some(match expression {
        CheckedBooleanExpression::IntegerMathComparison { kind, left, right } => {
            let substitute = |expression: &checked_trees::CheckedIntegerMathExpression| {
                let mut result = expression.clone();
                for (handle, term) in expression.terms.iter() {
                    if let checked_trees::CheckedIntegerMathTerm::Embed {
                        primitive_type,
                        operand,
                    } = term
                    {
                        *result.terms.get_mut(handle) =
                            checked_trees::CheckedIntegerMathTerm::Embed {
                                primitive_type: *primitive_type,
                                operand: substitute_checked_scalar_expression(
                                    operand, arguments, fields,
                                )?,
                            };
                    }
                }
                Some(result)
            };
            CheckedBooleanExpression::IntegerMathComparison {
                kind: *kind,
                left: substitute(left)?,
                right: substitute(right)?,
            }
        }
        CheckedBooleanExpression::Constant(value) => CheckedBooleanExpression::Constant(*value),
        CheckedBooleanExpression::Parameter { position } => {
            let CheckedScalarExpression::Boolean(expression) =
                arguments.get(*position)?.as_ref()?.clone()
            else {
                return None;
            };
            *expression
        }
        // Erased formals index the contract's proof-only roster, not the
        // runtime argument lane; no caller actual reaches this substitution.
        CheckedBooleanExpression::ErasedParameter { .. } => return None,
        // A callee contract is parameter-relative. A local can appear only
        // after composing a private body summary; it cannot be rebound by the
        // outer call and therefore deliberately loses portable structure.
        CheckedBooleanExpression::Local { .. } | CheckedBooleanExpression::StorageRead { .. } => {
            return None;
        }
        // An IEEE float comparison is atomic over structural field leaves.
        // Each leaf re-roots through the invocation's projected entry custody:
        // the callee position binds the actual's caller parameter and member
        // spine only when the complete observation survives intervening writes.
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
        CheckedBooleanExpression::StructuralIdentity { left, right } => {
            CheckedBooleanExpression::StructuralIdentity {
                left: substitute_structural_parameter_field(left, fields)?,
                right: substitute_structural_parameter_field(right, fields)?,
            }
        }
        // A byte-sequence content equality is atomic over the same
        // structural leaves: both subjects re-root through the caller's
        // projected entry custody. The leaf carries no roster or case
        // identity to transport — content equality compares the bytes the
        // resolved subjects hold — so the re-rooted leaves are the whole
        // term, and the lowering rechecks each against the caller
        // subject's retained byte-sequence carrier before emitting the
        // atomic proposition.
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            CheckedBooleanExpression::ByteSequenceEqual {
                left: substitute_structural_parameter_field(&left.parameter_field()?, fields)?
                    .into(),
                right: substitute_structural_parameter_field(&right.parameter_field()?, fields)?
                    .into(),
            }
        }
        // A sum case-membership test is atomic over the same structural
        // leaves: the subject re-roots through the caller's projected
        // entry custody while the case identity names the resolved
        // sum's declared case — a type-level fact the re-root cannot
        // change, and one the lowering rechecks against the resolved
        // subject's declared cases before emitting the atomic
        // proposition.
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            CheckedBooleanExpression::StructuralCaseMembership {
                subject: substitute_structural_parameter_field(
                    &subject.parameter_field()?,
                    fields,
                )?
                .into(),
                case: case.clone(),
            }
        }
        // A byte-predicate evaluation is atomic over the same structural
        // operand: a parameter-rooted subject re-roots through the caller's
        // projected entry custody and its path appends below the bound
        // member spine. A predicate rooted at local storage cannot be rebound
        // by the outer call and loses portable structure like a local read.
        CheckedBooleanExpression::ByteSequencePredicate {
            predicate,
            root,
            path,
        } => {
            let checked_trees::CheckedStorageRoot::Parameter { index } = root else {
                return None;
            };
            let leaf = substitute_structural_parameter_field(
                &checked_trees::CheckedStructuralParameterField {
                    parameter_position: *index,
                    path: path.clone(),
                },
                fields,
            )?;
            CheckedBooleanExpression::ByteSequencePredicate {
                predicate: *predicate,
                root: checked_trees::CheckedStorageRoot::Parameter {
                    index: leaf.parameter_position,
                },
                path: leaf.path,
            }
        }
        // A standalone Boolean field read uses the same projected entry custody
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
        CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
            CheckedBooleanExpression::ScalarIeeeFloatComparison {
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
    fields: &StructuralArgumentSubstitution<'_>,
) -> Option<checked_trees::CheckedStructuralParameterField> {
    fields.subject(leaf)
}

fn substitute_checked_scalar_expression(
    expression: &checked_trees::CheckedScalarExpression,
    arguments: &[Option<checked_trees::CheckedScalarExpression>],
    fields: &StructuralArgumentSubstitution<'_>,
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
        CheckedScalarExpression::ErasedParameter { .. } => return None,
        CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::StructuralSequenceLength { .. }
        | CheckedScalarExpression::RepresentationView { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => return None,
        // A standalone integer field leaf uses the same projected entry custody
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
        CheckedScalarExpression::ImplementationSelection { .. } => expression.clone(),
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            ..
        } => CheckedScalarExpression::IntegerBinary {
            kind: *kind,
            primitive_type: *primitive_type,
            left: Box::new(substitute_checked_scalar_expression(
                left, arguments, fields,
            )?),
            right: Box::new(substitute_checked_scalar_expression(
                right, arguments, fields,
            )?),
            // A substituted callee term is a different occurrence.
            formation_decided: false,
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
            ..
        } => CheckedScalarExpression::IntegerExactCast {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(
                operand, arguments, fields,
            )?),
            range: range.clone(),
            // A substituted callee term is a different occurrence.
            formation_decided: false,
        },
        CheckedScalarExpression::Boolean(expression) => CheckedScalarExpression::Boolean(Box::new(
            substitute_checked_boolean_expression(expression, arguments, fields)?,
        )),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refine_published_crash_routes(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    state_flow: &checked_trees::FlowStateFact,
    call_flow: &checked_trees::FlowCallFact,
    call_site: &validation::CallSite<'_>,
    target_state_symbol: SymbolHandle,
    target_parameters: &[typed_trees::signature::StateParameter],
    target_parameter_names: &[String],
    buckets: &[checked_trees::CrashRouteBucket],
    contracts: &[typed_trees::signature::SignatureContract],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> Vec<SummaryCrashBucket> {
    let route_expressions = crash_route_expressions_by_identity(
        program,
        contracts,
        target_parameters,
        target_parameter_names,
        content_conservation,
    );
    let arguments = validation::call_site_argument_expressions(program, call_site);
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
    // Ordinary argument lists omit self. Keep formal positions aligned without
    // inventing receiver-entry custody; a guard that reads self must still use
    // the existing receiver-aware evidence paths.
    let mut ordinary_arguments = arguments.iter().copied();
    let operands = target_parameters
        .iter()
        .map(|_parameter| ordinary_arguments.next().unwrap_or_default())
        .collect::<Vec<_>>();
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
                    let builtin_meaning = !identity
                        .expression()
                        .is_some_and(CrashPredicateExpression::contains_structural_identity)
                        && program
                            .machines()
                            .iter()
                            .find_map(|machine| {
                                program.state_in_machine(machine, target_state_symbol).map(
                                    |state| {
                                        validation::has_builtin_bound_expression_meaning(
                                            program,
                                            machine,
                                            Some(state),
                                            expression,
                                        )
                                    },
                                )
                            })
                            .unwrap_or(false);
                    if builtin_meaning
                        && crate::facts::operator_crashes::statement_entry_route_is_false(
                            program,
                            flow,
                            semantic,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            call_flow.statement_index,
                            target_parameters,
                            &operands,
                            expression,
                        )
                    {
                        continue;
                    }
                    let Some(predicate) = identity.expression() else {
                        guards.push(SummaryCrashRouteGuard::Truth);
                        continue;
                    };
                    // The direct evaluator follows actuals and local initializers.
                    // Only referenced formals with retained immutable entry origins
                    // may enter it; callee meaning cannot authorize caller operators.
                    // A projected leaf keeps the per-field caller name when the
                    // read projection stayed pristine across sibling writes.
                    let entry_predicate =
                        crate::facts::crash_entry_values::substitute_entry_projected(
                            predicate,
                            &mut |ordinal, members| {
                                substitution.fields.entry_operand(
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
                                None,
                                state_flow,
                                call_flow,
                                call_site,
                                target_state_symbol,
                                target_parameters,
                                expression,
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
                                predicate,
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
                            let scalar = identity.scalar_expression().and_then(|scalar| {
                                substitute_checked_boolean_expression(
                                    scalar,
                                    &substitution.scalar,
                                    &substitution.fields,
                                )
                            });
                            let Some(predicate) = entry_predicate else {
                                // A current storage read with no entry-value
                                // custody cannot become a caller Parameter.
                                // The annotation still substitutes on its own
                                // lane, so a guard it proves false dies here
                                // instead of widening; an undecided one keeps
                                // the unconditional ceiling without inventing
                                // a guard in the caller namespace.
                                if !scalar_guard_proves_false(builtin_meaning, scalar.as_ref()) {
                                    guards.push(SummaryCrashRouteGuard::Truth);
                                }
                                continue;
                            };
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
