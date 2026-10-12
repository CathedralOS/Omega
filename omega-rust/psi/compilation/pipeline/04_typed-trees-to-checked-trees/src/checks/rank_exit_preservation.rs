//! Exact target metric observations preserved across selected cleanup.
//!
//! Consume the already issued normalized computation and original call frame;
//! a captured scalar metric is not proof of unchanged descriptor-backed reads.
use checked_trees::{CheckFacts, CheckedScalarExpression as Scalar, FlowCallFact, FlowStateFact};
use validation::CanonicalPlace;

/// Rank roots are issued only in final execution preparation. Observe that
/// exact normalized computation once, after issuance, beside the original
/// whole-exit call. This does not infer descent or replay an operand.
pub(crate) fn record_rank_preservation(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    frames: Option<&validation::CallFrameResolver<'_>>,
) {
    let mut issued = Vec::new();
    for (handle, row) in facts.call_exit_preservations.iter() {
        let Some(source) = facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|source| {
                source.machine_symbol == row.machine && source.state_symbol == row.exit.state_symbol
            })
        else {
            continue;
        };
        if !facts.flow.control.calls.is_valid(row.call)
            || !source.calls.handles().any(|call| call == row.call)
        {
            continue;
        }
        let call = facts.flow.control.calls.get(row.call);
        let Some((owner, target)) = validation::find_state(program, row.target_state) else {
            continue;
        };
        if validation::call_target_state(program, call.target_symbol).map(|state| state.symbol)
            != Some(target.symbol)
        {
            continue;
        }
        // The rank producer compares complete selected descriptors for all
        // fused views before issuing this unique state/root coordinate.
        let Some(root) = facts
            .values
            .scalar_computations
            .root_at(
                target.symbol,
                0,
                checked_trees::CheckedScalarExpressionRole::RankObservation,
            )
            .filter(|root| root.machine == owner.symbol)
        else {
            continue;
        };
        let preserved = rank_reads(program, facts, source, call, target, root.root, frames)
            .is_some_and(|reads| {
                super::cleanup_preservation::reads_survive(
                    program,
                    facts,
                    row.machine,
                    row.exit,
                    &reads,
                )
            });
        issued.push((handle, root.root, preserved));
    }
    for (handle, root, preserved) in issued {
        let row = facts.call_exit_preservations.get_mut(handle);
        row.rank_observation = root;
        row.rank_preserved = preserved;
    }
}

fn rank_reads(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: &FlowStateFact,
    call: &FlowCallFact,
    target: &typed_trees::state::State,
    root: checked_trees::CheckedScalarComputationHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    use checked_trees::CheckedScalarComputationKind as Kind;
    let plans = &facts.values.scalar_computations;
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut complete = Vec::new();
    let mut reads = Vec::new();
    while let Some((handle, finishing)) = pending.pop() {
        if finishing {
            active.retain(|active| *active != handle);
            complete.push(handle);
            continue;
        }
        if complete.contains(&handle) {
            continue;
        }
        if !plans.nodes.is_valid(handle) || active.contains(&handle) {
            return None;
        }
        active.push(handle);
        pending.push((handle, true));
        let node = plans.nodes.get(handle);
        let children = match &node.kind {
            Kind::Value(expression) => {
                rank_scalar_reads(
                    program, source, call, target, expression, frames, &mut reads,
                )?;
                Vec::new()
            }
            Kind::Apply {
                expression,
                operands,
                ..
            } => {
                let operands = if operands.is_empty() {
                    &[]
                } else {
                    plans.operands.span(*operands)?
                };
                let mut template = expression.clone();
                // An Apply template's Parameters name its completed operands,
                // never a second source namespace or hidden storage read.
                crate::facts::boolean_predicates::map_scalar(&mut template, &mut |leaf| {
                    matches!(leaf, Scalar::Parameter { position, .. } if *position < operands.len())
                        .then_some(())
                })?;
                operands.to_vec()
            }
            Kind::Qualification { operand, .. }
            | Kind::AuthoredCast { operand, .. }
            | Kind::BooleanToInteger { operand, .. } => vec![*operand],
            Kind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => vec![*condition, *when_true, *when_false],
            Kind::Dispatch { subject, arms, .. } => {
                let mut children = vec![*subject];
                for arm in plans.dispatch_arms.span(*arms)? {
                    // Selected comparison bodies may observe storage not named
                    // by their two operands; no absence-as-purity assumption.
                    if arm.equality_use.is_valid() {
                        return None;
                    }
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) =
                        &arm.pattern
                    {
                        children.push(*pattern);
                    }
                    children.push(arm.value);
                }
                children
            }
            Kind::StructuralField {
                subject,
                receiver_value,
                field,
                field_identity,
                ..
            } => {
                if receiver_value.is_valid() {
                    return None;
                }
                let (position, mut path) = rank_structural_place(program, target, subject)?;
                let declaration =
                    crate::execution::terminal_unit::types::terminal_field_declaration(
                        program, *field,
                    )?;
                if checked_trees::CheckedMemberIdentity::of_member(
                    declaration.identity,
                    declaration.name.clone(),
                ) != *field_identity
                {
                    return None;
                }
                if let Some(variant) = facts::payload_variant_for_field(program, *field) {
                    if !matches!(path.last(), Some(facts::PlaceSegment::Case { variant: selected }) if *selected == variant)
                    {
                        path.push(facts::PlaceSegment::Case { variant });
                    }
                }
                path.push(facts::PlaceSegment::Field { symbol: *field });
                rank_actual_read(program, source, call, position, path, frames, &mut reads)?;
                Vec::new()
            }
            // A call, byte predicate, or indexed observation needs a complete
            // read projection of its own. Missing projection cannot certify
            // unchanged backing merely because the metric result is scalar.
            _ => return None,
        };
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Some(reads)
}

fn rank_scalar_reads(
    program: &dyn validation::CheckingInput,
    source: &FlowStateFact,
    call: &FlowCallFact,
    target: &typed_trees::state::State,
    expression: &Scalar,
    frames: Option<&validation::CallFrameResolver<'_>>,
    reads: &mut Vec<CanonicalPlace>,
) -> Option<()> {
    let parameters = program.state_parameters(target);
    let mut expression = expression.clone();
    crate::facts::boolean_predicates::map_scalar(&mut expression, &mut |leaf| {
        let (position, path) = match leaf {
            Scalar::Parameter { position, .. } => {
                let (position, parameter) = parameters
                    .iter()
                    .enumerate()
                    .filter(|(_, parameter)| {
                        crate::values::occupies_scalar_position(program, parameter)
                    })
                    .nth(*position)?;
                // Arrival scalar formals are completed values. A source field
                // read used to produce that value is not a later target read.
                if validation::reference_shell(program, parameter.type_reference).is_none()
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                {
                    return Some(());
                }
                (position, Vec::new())
            }
            Scalar::StorageRead { symbol, .. } => (
                parameters
                    .iter()
                    .position(|parameter| parameter.symbol == *symbol)?,
                Vec::new(),
            ),
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                let (_, segments, _, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    parameters,
                    *parameter_position,
                    path,
                )?;
                (usize::try_from(*parameter_position).ok()?, segments)
            }
            _ => return None,
        };
        rank_actual_read(program, source, call, position, path, frames, reads)
    })
}

fn rank_actual_read(
    program: &dyn validation::CheckingInput,
    source: &FlowStateFact,
    call: &FlowCallFact,
    position: usize,
    path: Vec<facts::PlaceSegment>,
    frames: Option<&validation::CallFrameResolver<'_>>,
    reads: &mut Vec<CanonicalPlace>,
) -> Option<()> {
    let actual = super::contracts::call_values::actual_at(program, source, call, position, path)?;
    let place = crate::semantic::facts::projected_argument_place(
        program,
        source.state_symbol,
        call.statement_index,
        actual.argument?,
        actual.parameter.type_reference,
        &actual.path,
    )?;
    let place = crate::flow::rebase_exact_local_place(
        program,
        source.state_symbol,
        call.statement_index,
        place,
        frames,
    )?;
    if !reads.contains(&place) {
        reads.push(place);
    }
    Some(())
}

/// Reverse the checked member identities only in the exact target carrier.
/// Same-spelled declarations elsewhere and unavailable geometry never donate
/// a root. Referent segments retain descriptor crossing via the carrier walk;
/// canonical places encode the same crossing through their field types.
fn rank_structural_place(
    program: &dyn validation::CheckingInput,
    target: &typed_trees::state::State,
    subject: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Option<(usize, Vec<facts::PlaceSegment>)> {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        &subject.source
    else {
        return None;
    };
    let parameters = program.state_parameters(target);
    let (position, parameter) = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            crate::execution::terminal_unit::types::structural_parameter_candidate(
                program, parameter,
            )
        })
        .nth(usize::try_from(*parameter_index).ok()?)?;
    let mut carrier = parameter.type_reference;
    let mut path = Vec::new();
    let mut selected_case = symbols::SymbolHandle::invalid();
    for segment in &subject.path {
        carrier = validation::peel_declared_constraints(program, carrier);
        if let Segment::Referent = segment {
            carrier = validation::reference_shell(program, carrier)?.referee;
            continue;
        }
        if let Some(shell) = validation::reference_shell(program, carrier) {
            carrier = shell.referee;
        }
        carrier = validation::peel_declared_constraints(program, carrier);
        match segment {
            Segment::Field(identity) => {
                let data = program.closed_data_definition(carrier)?;
                let fields = if selected_case.is_valid() {
                    let case = program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .find(|case| case.symbol == selected_case)?;
                    program.data_payload_fields(case).iter().collect::<Vec<_>>()
                } else {
                    program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Field(field) => Some(field),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                };
                let mut fields = fields.into_iter().filter(|field| {
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ) == *identity
                });
                let field = fields.next()?;
                if fields.next().is_some() {
                    return None;
                }
                path.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                carrier = field.type_reference;
                selected_case = symbols::SymbolHandle::invalid();
            }
            Segment::Case(identity) => {
                let data = program.closed_data_definition(carrier)?;
                let mut cases = program
                    .data_members(data)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .filter(|case| {
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ) == *identity
                    });
                let case = cases.next()?;
                if cases.next().is_some() || selected_case.is_valid() {
                    return None;
                }
                selected_case = case.symbol;
                path.push(facts::PlaceSegment::Case {
                    variant: case.symbol,
                });
            }
            Segment::FixedIndex(index) => {
                let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                path.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                });
                carrier = *element_type;
            }
            _ => return None,
        }
    }
    Some((position, path))
}
