//! Returned partition invocations and the wrappers composed from them.

use crate::checks::content::content_paths::{
    content_path, content_segments_to_fact_path, unique_entry_claim_identity,
};
use checked_trees::{
    CheckFacts, ContentPartitionCompositionFact, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, FlowClaimOutcomeSource,
};
use language_semantics::content::{
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentStructuralPlace, conservation_report_fingerprint,
};
use language_semantics::{
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

#[derive(Debug, Clone)]
pub(crate) struct ReturnedPartitionInvocation {
    statement_index: usize,
    pub(crate) target_symbol: SymbolHandle,
    receiver: Option<ExpressionHandle>,
    arguments: Vec<ExpressionHandle>,
    form: ReturnedPartitionInvocationForm,
}

#[derive(Debug, Clone, Copy)]
enum ReturnedPartitionInvocationForm {
    Expression(ExpressionHandle),
    NamedTransition,
    StagedLocal {
        call_expression: ExpressionHandle,
        local_symbol: SymbolHandle,
    },
}

#[derive(Debug, Default)]
struct PartitionCompositionEvidence {
    call_ordinal: Option<usize>,
    input_claim_identities: Vec<PermissionClaimIdentity>,
    input_claim_bindings: Vec<checked_trees::ContentPartitionInputClaimBinding>,
    result_rewrites: Vec<ContentPartitionResultRewrite>,
    substitutions: Vec<ContentPartitionPlaceSubstitution>,
    observed_entry_projection: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AvailablePartitionSource {
    pub(crate) plan: ContentConservationPlan,
    pub(crate) derivation_depth: u32,
}

pub(crate) fn returned_partition_invocations(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Vec<ReturnedPartitionInvocation> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut invocations = statements
        .iter()
        .enumerate()
        .filter_map(|(statement_index, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            let mut invocation =
                direct_expression_invocation(program, statement_index, local.initial_value)?;
            invocation.form = ReturnedPartitionInvocationForm::StagedLocal {
                call_expression: local.initial_value,
                local_symbol: local.symbol,
            };
            Some(invocation)
        })
        .filter(|invocation| invocation.target_symbol.is_valid())
        .collect::<Vec<_>>();
    let mut returned = Vec::new();

    for (statement_index, statement) in statements.iter().enumerate() {
        match statement {
            StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
                if let Some(invocation) =
                    direct_expression_invocation(program, statement_index, *expression)
                {
                    returned.push(invocation);
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                {
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named {
                            path, arguments, ..
                        } => {
                            returned.push(ReturnedPartitionInvocation {
                                statement_index,
                                target_symbol: path.symbol,
                                receiver: None,
                                arguments: program
                                    .statement_table
                                    .expression_handles(*arguments)
                                    .to_vec(),
                                form: ReturnedPartitionInvocationForm::NamedTransition,
                            });
                        }
                        TransitionTargetNode::Value(expression) => {
                            if let Some(invocation) =
                                direct_expression_invocation(program, statement_index, *expression)
                            {
                                returned.push(invocation);
                            }
                        }
                        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                    }
                }
            }
            _ => {}
        }
    }

    if let [invocation] = returned.as_slice()
        && invocation.target_symbol.is_valid()
    {
        invocations.push(invocation.clone());
    }
    invocations
}

fn direct_expression_invocation(
    program: &TypedTrees,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<ReturnedPartitionInvocation> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    Some(ReturnedPartitionInvocation {
        statement_index,
        target_symbol: call.target_symbol,
        receiver: call.receiver.is_valid().then_some(call.receiver),
        arguments: program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec(),
        form: ReturnedPartitionInvocationForm::Expression(expression),
    })
}

pub(crate) fn equation_contains_partition(equation: &ContentConservationEquation) -> bool {
    term_contains_partition(equation.left()) || term_contains_partition(equation.right())
}

fn term_contains_partition(term: &ContentConservationTerm) -> bool {
    match term {
        ContentConservationTerm::Projection { .. } => false,
        ContentConservationTerm::Separate(_) => true,
    }
}

pub(crate) fn instantiate_partition_wrapper(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    source: &ContentConservationPlan,
    source_derivation_depth: u32,
) -> Option<ContentPartitionCompositionFact> {
    let target_parameters =
        crate::semantic::calls::call_target_parameters(program, invocation.target_symbol)?;
    let call_ordinal = partition_invocation_call_ordinal(
        program,
        facts,
        machine_symbol,
        state.symbol,
        invocation,
    )?;
    let mut evidence = PartitionCompositionEvidence {
        call_ordinal: Some(call_ordinal),
        ..PartitionCompositionEvidence::default()
    };
    let left = instantiate_partition_term(
        program,
        facts,
        state,
        invocation,
        target_parameters,
        source.equation.left(),
        &mut evidence,
    )?;
    let right = instantiate_partition_term(
        program,
        facts,
        state,
        invocation,
        target_parameters,
        source.equation.right(),
        &mut evidence,
    )?;
    if !evidence.observed_entry_projection {
        return None;
    }
    evidence.input_claim_bindings.sort_by_key(|binding| {
        (
            format!("{:?}", binding.claim_identity),
            format!("{:?}", binding.entry_place),
        )
    });
    evidence.input_claim_bindings.dedup();
    if evidence
        .input_claim_bindings
        .iter()
        .enumerate()
        .any(|(index, binding)| {
            evidence.input_claim_bindings[index + 1..]
                .iter()
                .any(|later| {
                    later.claim_identity == binding.claim_identity
                        && later.entry_place != binding.entry_place
                })
        })
    {
        return None;
    }
    evidence.input_claim_identities = evidence
        .input_claim_bindings
        .iter()
        .map(|binding| binding.claim_identity)
        .collect();
    evidence.input_claim_identities.dedup();
    evidence.result_rewrites.sort_by_key(|rewrite| {
        (
            format!("{:?}", rewrite.source),
            format!("{:?}", rewrite.target),
            format!("{:?}", rewrite.claim_identity),
        )
    });
    evidence.result_rewrites.dedup();
    evidence.substitutions.sort_by_key(|substitution| {
        (
            format!("{:?}", substitution.source),
            format!("{:?}", substitution.target),
        )
    });
    evidence.substitutions.dedup();
    let equation = ContentConservationEquation::new(left, right);
    let report_fingerprint = conservation_report_fingerprint(&source.algebra, &equation);
    let plan = ContentConservationPlan {
        owner_kind: ContentConservationOwnerKind::Machine,
        owner: machine_symbol,
        callable: state.symbol,
        algebra: source.algebra.clone(),
        equation,
        report_fingerprint,
    };
    Some(ContentPartitionCompositionFact {
        machine_symbol,
        state_symbol: state.symbol,
        source_callable: source.callable,
        source_report_fingerprint: source.report_fingerprint,
        source_derivation_depth,
        source_plan: source.clone(),
        statement_index: invocation.statement_index,
        call_ordinal,
        input_claim_identities: evidence.input_claim_identities,
        input_claim_bindings: evidence.input_claim_bindings,
        result_rewrites: evidence.result_rewrites,
        substitutions: evidence.substitutions,
        plan,
    })
}

fn partition_invocation_call_ordinal(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    invocation: &ReturnedPartitionInvocation,
) -> Option<usize> {
    let state_flow = facts.flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })?;
    let ordinals = facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .filter(|call| {
            call.statement_index == invocation.statement_index
                && call.target_symbol == invocation.target_symbol
        })
        .filter_map(|call| {
            let call_site = crate::semantic::calls::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            let exact = match (&invocation.form, call_site) {
                (
                    ReturnedPartitionInvocationForm::Expression(expected),
                    crate::semantic::calls::CallSite::Expression { expression, .. },
                ) => *expected == expression,
                (
                    ReturnedPartitionInvocationForm::NamedTransition,
                    crate::semantic::calls::CallSite::TransitionNamed { arguments, .. },
                ) => {
                    program.statement_table.expression_handles(arguments)
                        == invocation.arguments.as_slice()
                }
                (
                    ReturnedPartitionInvocationForm::StagedLocal {
                        call_expression, ..
                    },
                    crate::semantic::calls::CallSite::Expression { expression, .. },
                ) => *call_expression == expression,
                _ => false,
            };
            exact.then_some(call.call_ordinal)
        })
        .fold(Vec::new(), |mut ordinals, ordinal| {
            if !ordinals.contains(&ordinal) {
                ordinals.push(ordinal);
            }
            ordinals
        });
    let [ordinal] = ordinals.as_slice() else {
        return None;
    };
    Some(*ordinal)
}

#[allow(clippy::too_many_arguments)]
fn instantiate_partition_term(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    target_parameters: &[StateParameter],
    term: &ContentConservationTerm,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentConservationTerm> {
    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            projection_report_fingerprint,
            subject,
        } => Some(ContentConservationTerm::Projection {
            domain: *domain,
            semantic_domain: *semantic_domain,
            projection_machine: *projection_machine,
            projection_report_fingerprint: *projection_report_fingerprint,
            subject: instantiate_partition_subject(
                program,
                facts,
                caller_state,
                invocation,
                target_parameters,
                subject,
                evidence,
            )?,
        }),
        ContentConservationTerm::Separate(children) => Some(ContentConservationTerm::separate(
            children
                .iter()
                .map(|child| {
                    instantiate_partition_term(
                        program,
                        facts,
                        caller_state,
                        invocation,
                        target_parameters,
                        child,
                        evidence,
                    )
                })
                .collect::<Option<Vec<_>>>()?,
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn instantiate_partition_subject(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    target_parameters: &[StateParameter],
    subject: &ContentStructuralPlace,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentStructuralPlace> {
    let target = match (&subject.root, subject.version) {
        (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
            instantiate_partition_result_subject(
                program,
                facts,
                caller_state,
                invocation,
                subject,
                evidence,
            )?
        }
        (
            ContentPlaceRoot::Parameter {
                position, symbol, ..
            },
            ContentPlaceVersion::Entry,
        ) => {
            evidence.observed_entry_projection = true;
            let parameter = target_parameters
                .get(usize::try_from(*position).ok()?)
                .filter(|parameter| !symbol.is_valid() || parameter.symbol == *symbol)
                .or_else(|| {
                    symbol
                        .is_valid()
                        .then(|| {
                            target_parameters
                                .iter()
                                .find(|parameter| parameter.symbol == *symbol)
                        })
                        .flatten()
                })?;
            let argument = argument_for_target_parameter(
                target_parameters,
                &invocation.arguments,
                invocation.receiver,
                parameter.symbol,
            )?;
            let actual = partition_argument_place(
                program,
                caller_state.symbol,
                invocation.statement_index,
                argument,
                &subject.segments,
            )?;
            let facts::PlaceRoot::Symbol(actual_root) = actual.root else {
                return None;
            };
            let (caller_position, caller_parameter) = program
                .state_parameters(caller_state)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == actual_root)?;
            let claim_identity = unique_entry_claim_identity(
                facts,
                caller_state.symbol,
                actual_root,
                &actual.segments,
            )?;
            let call_ordinal = evidence.call_ordinal?;
            let transferred_to_invocation =
                facts.flow.ownership.permissions.iter().any(|(_, event)| {
                    let source_matches = matches!(
                        event.source,
                        PermissionEventSource::Call {
                            statement_index,
                            call_ordinal: event_call_ordinal,
                            target_symbol,
                            ..
                        } if statement_index == invocation.statement_index
                            && event_call_ordinal == call_ordinal
                            && target_symbol == invocation.target_symbol
                    ) || event.source
                        == PermissionEventSource::Statement {
                            statement_index: invocation.statement_index,
                        };
                    event.state_symbol == caller_state.symbol
                        && source_matches
                        && event.kind == PermissionEventKind::Transfer
                        && event.access == PermissionAccess::Owned
                        && event.obligation_live
                        && event.claim_identity == claim_identity
                        && event.root == actual.root
                        && facts.flow.ownership.segments.span_or_empty(event.segments)
                            == actual.segments
                });
            if !transferred_to_invocation {
                return None;
            }
            let entry_place = ContentStructuralPlace {
                version: ContentPlaceVersion::Entry,
                root: ContentPlaceRoot::Parameter {
                    position: u32::try_from(caller_position).ok()?,
                    symbol: caller_parameter.symbol,
                    name: caller_parameter.name.as_str().to_owned(),
                    is_self: caller_parameter.is_self,
                },
                segments: content_path(program, &actual.segments)?,
            };
            evidence
                .input_claim_bindings
                .push(checked_trees::ContentPartitionInputClaimBinding {
                    claim_identity,
                    entry_place: entry_place.clone(),
                });
            entry_place
        }
        _ => return None,
    };
    if let Some(previous) = evidence
        .substitutions
        .iter()
        .find(|substitution| substitution.source == *subject)
    {
        if previous.target != target {
            return None;
        }
    } else {
        evidence
            .substitutions
            .push(ContentPartitionPlaceSubstitution {
                source: subject.clone(),
                target: target.clone(),
            });
    }
    Some(target)
}

fn instantiate_partition_result_subject(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: &typed_trees::state::State,
    invocation: &ReturnedPartitionInvocation,
    subject: &ContentStructuralPlace,
    evidence: &mut PartitionCompositionEvidence,
) -> Option<ContentStructuralPlace> {
    let ReturnedPartitionInvocationForm::StagedLocal { local_symbol, .. } = invocation.form else {
        return Some(subject.clone());
    };
    let local_segments = content_segments_to_fact_path(&subject.segments)?;
    let call_ordinal = evidence.call_ordinal?;
    let identities = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            let source_matches = matches!(
                event.source,
                PermissionEventSource::Call {
                    statement_index,
                    call_ordinal: event_call_ordinal,
                    target_symbol,
                    ..
                } if statement_index == invocation.statement_index
                    && event_call_ordinal == call_ordinal
                    && target_symbol == invocation.target_symbol
            ) || event.source
                == PermissionEventSource::Statement {
                    statement_index: invocation.statement_index,
                };
            event.state_symbol == caller_state.symbol
                && source_matches
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.obligation_live
                && event.claim_identity != PermissionClaimIdentity::Unknown
                && event.root == facts::PlaceRoot::Symbol(local_symbol)
                && facts.flow.ownership.segments.span_or_empty(event.segments) == local_segments
        })
        .map(|(_, event)| event.claim_identity)
        .fold(Vec::new(), |mut identities, identity| {
            if !identities.contains(&identity) {
                identities.push(identity);
            }
            identities
        });
    let [claim_identity] = identities.as_slice() else {
        return None;
    };
    let output_paths = facts
        .flow
        .ownership
        .claim_outcome_maps
        .iter()
        .filter(|(_, map)| map.state_symbol == caller_state.symbol)
        .flat_map(|(_, map)| {
            facts
                .flow
                .ownership
                .claim_outcome_entries
                .span_or_empty(map.entries)
        })
        .filter_map(|entry| match entry.source {
            FlowClaimOutcomeSource::Established {
                claim_identity: outcome_identity,
                ..
            } if outcome_identity == *claim_identity => Some(
                facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(entry.output_segments)
                    .to_vec(),
            ),
            FlowClaimOutcomeSource::Input { .. }
            | FlowClaimOutcomeSource::Established { .. }
            | FlowClaimOutcomeSource::Unknown => None,
        })
        .fold(Vec::new(), |mut paths, path| {
            if !paths.contains(&path) {
                paths.push(path);
            }
            paths
        });
    let [output_path] = output_paths.as_slice() else {
        return None;
    };
    let target = ContentStructuralPlace {
        version: ContentPlaceVersion::Current,
        root: ContentPlaceRoot::Result,
        segments: content_path(program, output_path)?,
    };
    evidence
        .result_rewrites
        .push(ContentPartitionResultRewrite {
            claim_identity: *claim_identity,
            source: subject.clone(),
            target: target.clone(),
        });
    Some(target)
}

fn argument_for_target_parameter(
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    receiver: Option<ExpressionHandle>,
    parameter_symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    let includes_explicit_self =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == parameters.len();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let argument = if parameter.is_self && !includes_explicit_self {
            receiver
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            argument
        };
        if parameter.symbol == parameter_symbol {
            return argument;
        }
    }
    None
}

fn partition_argument_place(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    argument: ExpressionHandle,
    projection_path: &[ContentPlaceSegment],
) -> Option<crate::flow::CanonicalPlace> {
    let mut direct = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        argument,
    )?;
    if matches!(direct.root, facts::PlaceRoot::Symbol(_)) {
        direct
            .segments
            .extend(content_segments_to_fact_path(projection_path)?);
        return Some(direct);
    }
    let leaf = aggregate_argument_projection(program, argument, projection_path)?;
    let leaf = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        leaf,
    )?;
    matches!(leaf.root, facts::PlaceRoot::Symbol(_)).then_some(leaf)
}

fn aggregate_argument_projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
    projection_path: &[ContentPlaceSegment],
) -> Option<ExpressionHandle> {
    let Some((head, tail)) = projection_path.split_first() else {
        return Some(expression);
    };
    match (head, program.expression_table.expression(expression)) {
        (ContentPlaceSegment::Case(expected), ExpressionNode::StructLiteral(literal))
            if literal
                .case_name
                .as_ref()
                .is_some_and(|case| case.as_str() == expected.name) =>
        {
            aggregate_argument_projection(program, expression, tail)
        }
        (ContentPlaceSegment::Field(expected), ExpressionNode::StructLiteral(literal)) => {
            let field = program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == expected.name)?;
            aggregate_argument_projection(program, field.value, tail)
        }
        (ContentPlaceSegment::FixedIndex(index), ExpressionNode::ArrayLiteral(values)) => {
            let value = program
                .expression_table
                .expression_handles(*values)
                .get(usize::try_from(*index).ok()?)?;
            aggregate_argument_projection(program, *value, tail)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        argument_for_target_parameter, equation_contains_partition, term_contains_partition,
    };
    use language_semantics::SemanticDomainId;
    use language_semantics::content::{
        ContentConservationEquation, ContentConservationTerm, ContentPlaceRoot,
        ContentPlaceVersion, ContentStructuralPlace,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;

    fn projection_term() -> ContentConservationTerm {
        ContentConservationTerm::Projection {
            domain: SymbolHandle::invalid(),
            semantic_domain: SemanticDomainId::NULL,
            projection_machine: SymbolHandle::invalid(),
            projection_report_fingerprint: 0,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root: ContentPlaceRoot::Result,
                segments: Vec::new(),
            },
        }
    }

    #[test]
    fn term_contains_partition_flags_only_separate_terms() {
        assert!(!term_contains_partition(&projection_term()));
        assert!(term_contains_partition(&ContentConservationTerm::separate(
            [projection_term(),]
        )));
        assert!(term_contains_partition(&ContentConservationTerm::separate(
            [
                projection_term(),
                ContentConservationTerm::separate([projection_term()]),
            ]
        )));
    }

    #[test]
    fn equation_contains_partition_flags_either_side() {
        let bare = ContentConservationEquation::new(projection_term(), projection_term());
        assert!(!equation_contains_partition(&bare));
        let separated = ContentConservationEquation::new(
            projection_term(),
            ContentConservationTerm::separate([projection_term()]),
        );
        assert!(equation_contains_partition(&separated));
    }

    fn parameter(symbol_index: u32, is_self: bool) -> StateParameter {
        StateParameter {
            symbol: SymbolHandle::from_arena_index(symbol_index),
            is_self,
            ..StateParameter::default()
        }
    }

    fn argument(index: u32) -> ExpressionHandle {
        ExpressionHandle::from_arena_index(index)
    }

    #[test]
    fn argument_for_target_parameter_binds_implicit_self_to_the_receiver() {
        let parameters = [parameter(1, true), parameter(2, false), parameter(3, false)];
        let arguments = [argument(10), argument(11)];
        assert_eq!(
            argument_for_target_parameter(
                &parameters,
                &arguments,
                Some(argument(99)),
                SymbolHandle::from_arena_index(1)
            ),
            Some(argument(99)),
            "an implicit self parameter takes the receiver, not a positional argument"
        );
        assert_eq!(
            argument_for_target_parameter(
                &parameters,
                &arguments,
                Some(argument(99)),
                SymbolHandle::from_arena_index(3)
            ),
            Some(argument(11)),
            "trailing parameters consume positions without the receiver's slot"
        );
    }

    #[test]
    fn argument_for_target_parameter_binds_explicit_self_positionally() {
        // arguments.len() == parameters.len() means the authored call spells
        // self in the argument list, so the receiver expression is not used.
        let parameters = [parameter(1, true), parameter(2, false)];
        let arguments = [argument(10), argument(11)];
        assert_eq!(
            argument_for_target_parameter(
                &parameters,
                &arguments,
                Some(argument(99)),
                SymbolHandle::from_arena_index(1)
            ),
            Some(argument(10))
        );
        assert_eq!(
            argument_for_target_parameter(
                &parameters,
                &arguments,
                Some(argument(99)),
                SymbolHandle::from_arena_index(2)
            ),
            Some(argument(11))
        );
    }

    #[test]
    fn argument_for_target_parameter_fails_closed() {
        let parameters = [parameter(1, false)];
        let arguments = [argument(10)];
        assert_eq!(
            argument_for_target_parameter(
                &parameters,
                &arguments,
                None,
                SymbolHandle::from_arena_index(7)
            ),
            None,
            "a parameter the signature does not carry has no argument"
        );
        assert_eq!(
            argument_for_target_parameter(
                &parameters,
                &[],
                None,
                SymbolHandle::from_arena_index(1)
            ),
            None,
            "a parameter whose positional argument is missing has no argument"
        );
    }
}
