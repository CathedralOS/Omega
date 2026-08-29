use super::calls::DependencyOccurrence;
use crate::declarations::dependencies::read::error::{
    DependencyPathProvenance, DependencyPathTaint, DependencyProjectionError,
};
use crate::declarations::dependencies::read::model::{
    ProjectedDependencies, TargetDependencyColumn,
};
use omega_target::TargetProfile;
use psi_source::SourceSpan;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_syntax_trees::item::{StateHandle, StateParameterHandle};
use psi_syntax_trees::statement::{
    StatementNode, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use psi_syntax_trees::types::TypeReferenceNode;
use std::collections::VecDeque;

#[derive(Clone, PartialEq, Eq)]
struct Reach {
    profile: Option<TargetProfile>,
    taint: Option<DependencyPathProvenance>,
    authorized_builders: Vec<bool>,
}

enum GuardProjection {
    Unconditional,
    Wildcard,
    ExactTarget { receiver: String, case_name: String },
    RuntimeSubject,
}

pub(super) fn project_state_graph(
    syntax_trees: &SyntaxTrees,
    tokens: &[psi_tokens::Token<'_>],
    state_span: psi_arena::HandleSpan<StateHandle>,
    entry: StateHandle,
    entry_builder: StateParameterHandle,
    occurrences: Vec<DependencyOccurrence>,
) -> Result<ProjectedDependencies, DependencyProjectionError> {
    let states = syntax_trees.items.state_handles(state_span).to_vec();
    let entry_index = state_index(&states, entry).expect("build entry belongs to build states");
    let entry_state = syntax_trees.items.state(entry);
    let entry_parameters = syntax_trees.items.state_parameters(entry_state.parameters);
    let entry_builder_index = entry_parameters
        .iter()
        .position(|parameter| *parameter == entry_builder)
        .expect("validated builder belongs to build entry");
    let mut entry_authority = vec![false; entry_parameters.len()];
    entry_authority[entry_builder_index] = true;

    let mut reaches = vec![Vec::<Reach>::new(); states.len()];
    let initial = Reach {
        profile: None,
        taint: None,
        authorized_builders: entry_authority,
    };
    reaches[entry_index].push(initial.clone());
    let mut work = VecDeque::from([(entry_index, initial)]);
    let mut referenced_profiles = Vec::new();

    while let Some((source_index, reach)) = work.pop_front() {
        let source_handle = states[source_index];
        let source_state = syntax_trees.items.state(source_handle);
        for statement_handle in syntax_trees.items.statements(source_state.statements) {
            let StatementNode::Transition(transition) =
                syntax_trees.statements.statement(*statement_handle)
            else {
                continue;
            };
            let Some((target_index, target_authority)) =
                transition_target(syntax_trees, &states, source_index, &reach, transition)?
            else {
                continue;
            };
            let mut next = Reach {
                profile: reach.profile,
                taint: reach.taint.clone(),
                authorized_builders: target_authority,
            };
            match project_guard(syntax_trees, tokens, transition) {
                GuardProjection::Unconditional => {}
                GuardProjection::Wildcard => taint_reach(
                    &mut next,
                    source_state.name.as_str(),
                    transition.source_span,
                    DependencyPathTaint::WildcardTargetArm,
                ),
                GuardProjection::RuntimeSubject => taint_reach(
                    &mut next,
                    source_state.name.as_str(),
                    transition.source_span,
                    DependencyPathTaint::RuntimeSubjectTransition,
                ),
                GuardProjection::ExactTarget {
                    receiver,
                    case_name,
                } => {
                    if !receiver_is_authorized(
                        syntax_trees,
                        source_handle,
                        &reach.authorized_builders,
                        &receiver,
                    ) {
                        taint_reach(
                            &mut next,
                            source_state.name.as_str(),
                            transition.source_span,
                            DependencyPathTaint::RuntimeSubjectTransition,
                        );
                    } else {
                        let profile =
                            TargetProfile::from_build_case_name(&case_name).ok_or_else(|| {
                                DependencyProjectionError::UnknownTargetProfile {
                                    case_name,
                                    arm: transition.source_span,
                                }
                            })?;
                        retain_profile(&mut referenced_profiles, profile);
                        match next.profile {
                            Some(existing) if existing != profile => continue,
                            Some(_) => {}
                            None => next.profile = Some(profile),
                        }
                    }
                }
            }
            if !reaches[target_index].contains(&next) {
                reaches[target_index].push(next.clone());
                work.push_back((target_index, next));
            }
        }
    }

    let mut requests = Vec::with_capacity(occurrences.len());
    let mut common_occurrence_indices = Vec::new();
    let mut profile_dependencies = TargetProfile::ALL
        .into_iter()
        .map(|profile| (profile, Vec::<usize>::new()))
        .collect::<Vec<_>>();

    for occurrence in occurrences {
        let occurrence_index = requests.len();
        let index = state_index(&states, occurrence.state).expect("occurrence is in build state");
        let state = syntax_trees.items.state(occurrence.state);
        let relevant = reaches[index]
            .iter()
            .filter(|reach| {
                receiver_is_authorized(
                    syntax_trees,
                    occurrence.state,
                    &reach.authorized_builders,
                    &occurrence.receiver,
                )
            })
            .collect::<Vec<_>>();
        if relevant.is_empty() {
            if reaches[index].is_empty() {
                return Err(DependencyProjectionError::UnreachableDependency {
                    state: state.name.as_str().to_owned(),
                    dependency: occurrence.source_span,
                });
            }
            return Err(DependencyProjectionError::WrongDependencyReceiver);
        }
        let clean = relevant
            .iter()
            .filter(|reach| reach.taint.is_none())
            .copied()
            .collect::<Vec<_>>();
        let tainted = relevant
            .iter()
            .find_map(|reach| reach.taint.as_ref())
            .cloned();
        if let Some(provenance) = tainted {
            let error = if clean.is_empty() {
                DependencyProjectionError::TaintedDependencyPath {
                    state: state.name.as_str().to_owned(),
                    dependency: occurrence.source_span,
                    provenance,
                }
            } else {
                DependencyProjectionError::MixedDependencyPaths {
                    state: state.name.as_str().to_owned(),
                    dependency: occurrence.source_span,
                    provenance,
                }
            };
            return Err(error);
        }
        if clean.iter().any(|reach| reach.profile.is_none()) {
            common_occurrence_indices.push(occurrence_index);
            requests.push(occurrence.request);
            continue;
        }
        for profile in TargetProfile::ALL {
            if clean.iter().any(|reach| reach.profile == Some(profile)) {
                profile_dependencies
                    .iter_mut()
                    .find(|(candidate, _)| *candidate == profile)
                    .expect("trusted catalog contains profile")
                    .1
                    .push(occurrence_index);
            }
        }
        requests.push(occurrence.request);
    }

    let by_profile = profile_dependencies
        .into_iter()
        .filter_map(|(profile, dependencies)| {
            (!dependencies.is_empty()).then(|| TargetDependencyColumn::new(profile, dependencies))
        })
        .collect();
    let referenced_profile_identities = TargetProfile::ALL
        .into_iter()
        .filter(|profile| referenced_profiles.contains(profile))
        .map(TargetProfile::identity)
        .collect();
    Ok(ProjectedDependencies::new(
        requests,
        common_occurrence_indices,
        by_profile,
        referenced_profile_identities,
    ))
}

fn transition_target(
    syntax_trees: &SyntaxTrees,
    states: &[StateHandle],
    source_index: usize,
    reach: &Reach,
    transition: &TableTransition,
) -> Result<Option<(usize, Vec<bool>)>, DependencyProjectionError> {
    if transition.continuation.is_valid()
        || !transition.proof_selectors.is_empty()
        || transition.exit != TransitionExit::Ordinary
    {
        return Ok(None);
    }
    match syntax_trees.statements.transition_target(transition.target) {
        TransitionTargetNode::SelfTarget => {
            Ok(Some((source_index, reach.authorized_builders.clone())))
        }
        TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => Ok(None),
        TransitionTargetNode::Named {
            path,
            path_starts_at_self,
            arguments,
            evidence_arguments,
            ..
        } => {
            if *path_starts_at_self || !evidence_arguments.is_empty() {
                return Ok(None);
            }
            let [target_name] = syntax_trees.statements.identifier_path_members(*path) else {
                return Ok(None);
            };
            let Some(target_index) = states.iter().position(|state| {
                syntax_trees.items.state(*state).name.as_str() == target_name.as_str()
            }) else {
                return Ok(None);
            };
            let target_state = syntax_trees.items.state(states[target_index]);
            let target_parameters = syntax_trees.items.state_parameters(target_state.parameters);
            let arguments = syntax_trees.statements.expression_handles(*arguments);
            if arguments.len() != target_parameters.len() {
                return Err(DependencyProjectionError::UnsupportedStateTransition {
                    state: syntax_trees
                        .items
                        .state(states[source_index])
                        .name
                        .as_str()
                        .to_owned(),
                    transition: transition.source_span,
                });
            }
            let source_state = syntax_trees.items.state(states[source_index]);
            let source_parameters = syntax_trees.items.state_parameters(source_state.parameters);
            let authorized_names = source_parameters
                .iter()
                .enumerate()
                .filter(|(index, _)| reach.authorized_builders.get(*index) == Some(&true))
                .map(|(_, parameter)| syntax_trees.items.state_parameter(*parameter).name.as_str())
                .collect::<Vec<_>>();
            let authority = target_parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    is_mutable_build_parameter(syntax_trees, *parameter)
                        && direct_name(syntax_trees, *argument)
                            .is_some_and(|name| authorized_names.contains(&name))
                })
                .collect();
            Ok(Some((target_index, authority)))
        }
    }
}

fn project_guard(
    syntax_trees: &SyntaxTrees,
    tokens: &[psi_tokens::Token<'_>],
    transition: &TableTransition,
) -> GuardProjection {
    match transition.guard {
        TransitionGuardNode::Always
            if transition.source_span == SourceSpan::default()
                || is_anonymous_unconditional_arm(tokens, transition.source_span) =>
        {
            GuardProjection::Unconditional
        }
        TransitionGuardNode::Always => GuardProjection::Wildcard,
        TransitionGuardNode::When(expression) => {
            exact_target_guard(syntax_trees, expression).unwrap_or(GuardProjection::RuntimeSubject)
        }
    }
}

fn is_anonymous_unconditional_arm(tokens: &[psi_tokens::Token<'_>], arm: SourceSpan) -> bool {
    let Some(arrow_index) = tokens.iter().position(|token| {
        token.span == arm.span && token.punctuation() == Some(psi_tokens::PunctuationKind::Arrow)
    }) else {
        return false;
    };
    let semantic = tokens[..arrow_index]
        .iter()
        .filter(|token| !token.is_non_semantic())
        .collect::<Vec<_>>();
    let Some(open_index) = semantic
        .iter()
        .rposition(|token| token.punctuation() == Some(psi_tokens::PunctuationKind::LeftBrace))
    else {
        return false;
    };
    open_index > 0
        && semantic[open_index - 1].keyword() == Some(psi_tokens::KeywordKind::Transition)
}

fn exact_target_guard(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<GuardProjection> {
    let ExpressionNode::Membership(membership) = syntax_trees.expressions.expression(expression)
    else {
        return None;
    };
    let [profile_type, profile_case] = syntax_trees
        .expressions
        .identifier_path_members(membership.domain)
    else {
        return None;
    };
    if profile_type.as_str() != "TargetProfile" {
        return None;
    }
    let ExpressionNode::Member(member) = syntax_trees.expressions.expression(membership.value)
    else {
        return None;
    };
    if member.member.as_str() != "target" || member.case_variant.is_some() {
        return None;
    }
    Some(GuardProjection::ExactTarget {
        receiver: direct_name(syntax_trees, member.receiver)?.to_owned(),
        case_name: profile_case.as_str().to_owned(),
    })
}

fn direct_name(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> Option<&str> {
    let ExpressionNode::Name(path) = syntax_trees.expressions.expression(expression) else {
        return None;
    };
    let [name] = syntax_trees.expressions.identifier_path_members(*path) else {
        return None;
    };
    Some(name.as_str())
}

fn receiver_is_authorized(
    syntax_trees: &SyntaxTrees,
    state: StateHandle,
    authority: &[bool],
    receiver: &str,
) -> bool {
    syntax_trees
        .items
        .state_parameters(syntax_trees.items.state(state).parameters)
        .iter()
        .enumerate()
        .any(|(index, parameter)| {
            authority.get(index) == Some(&true)
                && syntax_trees.items.state_parameter(*parameter).name.as_str() == receiver
        })
}

fn is_mutable_build_parameter(syntax_trees: &SyntaxTrees, parameter: StateParameterHandle) -> bool {
    let parameter = syntax_trees.items.state_parameter(parameter);
    let TypeReferenceNode::Reference {
        referee,
        access,
        lifetime,
    } = syntax_trees
        .type_references
        .type_reference(parameter.type_reference)
    else {
        return false;
    };
    lifetime.is_none()
        && access.is_exclusive()
        && access.is_readable()
        && matches!(
            syntax_trees.type_references.type_reference(*referee),
            TypeReferenceNode::Named(name) if name.as_str() == "Build"
        )
}

fn taint_reach(reach: &mut Reach, state: &str, transition: SourceSpan, taint: DependencyPathTaint) {
    if reach.taint.is_none() {
        reach.taint = Some(DependencyPathProvenance {
            state: state.to_owned(),
            transition,
            taint,
        });
    }
}

fn retain_profile(profiles: &mut Vec<TargetProfile>, profile: TargetProfile) {
    if !profiles.contains(&profile) {
        profiles.push(profile);
    }
}

fn state_index(states: &[StateHandle], sought: StateHandle) -> Option<usize> {
    states.iter().position(|state| *state == sought)
}
