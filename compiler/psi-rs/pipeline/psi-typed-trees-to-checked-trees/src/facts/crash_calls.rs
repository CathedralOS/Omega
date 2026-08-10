use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

use super::{encode_contract_fact_canonical, is_true_crash_route};

/// Temporary source-independent form of a crash predicate. Private body
/// summaries need more than the final predicate fingerprint: every enclosing
/// call must still be able to replace the callee's positional parameters with
/// its own arguments. This tree uses the same tags as the canonical contract
/// encoder and is discarded after checked call rows have been materialized.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CrashPredicateExpression {
    Invalid,
    Binary {
        operator: u8,
        left: Box<Self>,
        right: Box<Self>,
    },
    Unary {
        operator: u8,
        operand: Box<Self>,
    },
    Integer(String),
    Boolean(bool),
    Name(Vec<String>),
    Member {
        receiver: Box<Self>,
        member: String,
    },
    Call {
        target: String,
        receiver: Box<Self>,
        arguments: Vec<Self>,
    },
    Opaque(String),
    Parameter(u32),
    ContentConservation(Vec<u8>),
}

impl CrashPredicateExpression {
    fn from_expression(
        program: &TypedTrees,
        expression: psi_typed_trees::expression::ExpressionHandle,
        parameter_names: &[String],
        content_conservation: Option<&[psi_validation::ContentConservationSourcePlan]>,
    ) -> Self {
        use psi_typed_trees::expression::ExpressionNode;

        if let Some(conservation) = content_conservation.and_then(|plans| {
            plans
                .iter()
                .find(|candidate| candidate.source_expression == expression)
        }) {
            return Self::ContentConservation(
                psi_language_semantics::content::content_conservation_plan_bytes(
                    &conservation.plan,
                ),
            );
        }
        if !expression.is_valid() {
            return Self::Invalid;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => Self::Binary {
                operator: binary.operator as u8,
                left: Box::new(Self::from_expression(
                    program,
                    binary.left,
                    parameter_names,
                    content_conservation,
                )),
                right: Box::new(Self::from_expression(
                    program,
                    binary.right,
                    parameter_names,
                    content_conservation,
                )),
            },
            ExpressionNode::Unary(unary) => Self::Unary {
                operator: unary.operator as u8,
                operand: Box::new(Self::from_expression(
                    program,
                    unary.operand,
                    parameter_names,
                    content_conservation,
                )),
            },
            ExpressionNode::Integer(value) => Self::Integer(value.text().to_owned()),
            ExpressionNode::Boolean(value) => Self::Boolean(*value),
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if let [single] = members
                    && let Some(index) = parameter_names
                        .iter()
                        .position(|name| name == single.as_str())
                {
                    return Self::Parameter(
                        u32::try_from(index).expect("parameter index fits u32"),
                    );
                }
                Self::Name(
                    members
                        .iter()
                        .map(|member| member.as_str().to_owned())
                        .collect(),
                )
            }
            ExpressionNode::Member(member) => Self::Member {
                receiver: Box::new(Self::from_expression(
                    program,
                    member.receiver,
                    parameter_names,
                    content_conservation,
                )),
                member: member.member.as_str().to_owned(),
            },
            ExpressionNode::Call(call) => Self::Call {
                target: call.target.as_str().to_owned(),
                receiver: Box::new(Self::from_expression(
                    program,
                    call.receiver,
                    parameter_names,
                    content_conservation,
                )),
                arguments: program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| {
                        Self::from_expression(
                            program,
                            *argument,
                            parameter_names,
                            content_conservation,
                        )
                    })
                    .collect(),
            },
            other => {
                let _ = other;
                Self::Opaque(program.expression_table.display_name(expression))
            }
        }
    }

    fn substitute(&self, arguments: &[Option<Self>]) -> Self {
        match self {
            Self::Parameter(index) => arguments
                .get(*index as usize)
                .and_then(Clone::clone)
                .unwrap_or_else(|| self.clone()),
            Self::Binary {
                operator,
                left,
                right,
            } => Self::Binary {
                operator: *operator,
                left: Box::new(left.substitute(arguments)),
                right: Box::new(right.substitute(arguments)),
            },
            Self::Unary { operator, operand } => Self::Unary {
                operator: *operator,
                operand: Box::new(operand.substitute(arguments)),
            },
            Self::Member { receiver, member } => Self::Member {
                receiver: Box::new(receiver.substitute(arguments)),
                member: member.clone(),
            },
            Self::Call {
                target,
                receiver,
                arguments: nested,
            } => Self::Call {
                target: target.clone(),
                receiver: Box::new(receiver.substitute(arguments)),
                arguments: nested
                    .iter()
                    .map(|argument| argument.substitute(arguments))
                    .collect(),
            },
            _ => self.clone(),
        }
    }

    fn boolean_value(&self) -> Option<bool> {
        use psi_typed_trees::expression::{BinaryOperator, UnaryOperator};

        match self {
            Self::Boolean(value) => Some(*value),
            Self::Unary { operator, operand } if *operator == UnaryOperator::LogicalNot as u8 => {
                operand.boolean_value().map(|value| !value)
            }
            Self::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::And as u8 => {
                Some(left.boolean_value()? && right.boolean_value()?)
            }
            Self::Binary {
                operator,
                left,
                right,
            } if *operator == BinaryOperator::Or as u8 => {
                Some(left.boolean_value()? || right.boolean_value()?)
            }
            _ => None,
        }
    }

    fn write_canonical(&self, out: &mut Vec<u8>) {
        match self {
            Self::Invalid => out.push(0),
            Self::Binary {
                operator,
                left,
                right,
            } => {
                out.push(1);
                out.push(*operator);
                left.write_canonical(out);
                right.write_canonical(out);
            }
            Self::Unary { operator, operand } => {
                out.push(2);
                out.push(*operator);
                operand.write_canonical(out);
            }
            Self::Integer(value) => {
                out.push(3);
                out.extend(value.as_bytes());
                out.push(0);
            }
            Self::Boolean(value) => {
                out.push(4);
                out.push(u8::from(*value));
            }
            Self::Name(members) => {
                out.push(5);
                for member in members {
                    out.extend(member.as_bytes());
                    out.push(b'.');
                }
                out.push(0);
            }
            Self::Member { receiver, member } => {
                out.push(6);
                receiver.write_canonical(out);
                out.extend(member.as_bytes());
                out.push(0);
            }
            Self::Call {
                target,
                receiver,
                arguments,
            } => {
                out.push(7);
                out.extend(target.as_bytes());
                out.push(0);
                receiver.write_canonical(out);
                for argument in arguments {
                    argument.write_canonical(out);
                }
                out.push(0xfe);
            }
            Self::Opaque(display) => {
                out.push(8);
                out.extend(display.as_bytes());
                out.push(0);
            }
            Self::Parameter(index) => {
                out.push(9);
                out.extend(index.to_le_bytes());
            }
            Self::ContentConservation(bytes) => {
                out.push(0xcc);
                out.extend(bytes);
            }
        }
    }

    fn identity(&self) -> psi_checked_trees::CrashPredicateIdentity {
        let mut bytes = vec![1]; // ProofFact::Expression
        self.write_canonical(&mut bytes);
        psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SummaryCrashRouteGuard {
    Truth,
    Predicate(CrashPredicateExpression),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SummaryCrashBucket {
    cause: psi_checked_trees::CrashCause,
    alternative_guards: Vec<SummaryCrashRouteGuard>,
}

impl SummaryCrashBucket {
    fn unconditional(cause: psi_checked_trees::CrashCause) -> Self {
        Self {
            cause,
            alternative_guards: vec![SummaryCrashRouteGuard::Truth],
        }
    }

    fn substitute(&self, arguments: &[Option<CrashPredicateExpression>]) -> Self {
        let mut guards = self
            .alternative_guards
            .iter()
            .filter_map(|guard| match guard {
                SummaryCrashRouteGuard::Truth => Some(SummaryCrashRouteGuard::Truth),
                SummaryCrashRouteGuard::Predicate(predicate) => {
                    let predicate = predicate.substitute(arguments);
                    match predicate.boolean_value() {
                        Some(false) => None,
                        Some(true) => Some(SummaryCrashRouteGuard::Truth),
                        None => Some(SummaryCrashRouteGuard::Predicate(predicate)),
                    }
                }
            })
            .collect::<Vec<_>>();
        normalize_summary_guards(&mut guards);
        Self {
            cause: self.cause,
            alternative_guards: guards,
        }
    }

    fn into_checked(self) -> Option<psi_checked_trees::CrashRouteBucket> {
        psi_checked_trees::CrashRouteBucket::new(
            self.cause,
            self.alternative_guards
                .into_iter()
                .map(|guard| match guard {
                    SummaryCrashRouteGuard::Truth => psi_checked_trees::CrashRouteGuard::Truth,
                    SummaryCrashRouteGuard::Predicate(predicate) => {
                        psi_checked_trees::CrashRouteGuard::Predicate(predicate.identity())
                    }
                })
                .collect(),
        )
    }
}

fn normalize_summary_guards(guards: &mut Vec<SummaryCrashRouteGuard>) {
    guards.sort();
    guards.dedup();
    if guards.contains(&SummaryCrashRouteGuard::Truth) {
        guards.clear();
        guards.push(SummaryCrashRouteGuard::Truth);
    }
}

fn normalize_summary_buckets(buckets: Vec<SummaryCrashBucket>) -> Vec<SummaryCrashBucket> {
    let mut grouped = std::collections::BTreeMap::<
        psi_checked_trees::CrashCause,
        Vec<SummaryCrashRouteGuard>,
    >::new();
    for bucket in buckets {
        grouped
            .entry(bucket.cause)
            .or_default()
            .extend(bucket.alternative_guards);
    }
    grouped
        .into_iter()
        .filter_map(|(cause, mut alternative_guards)| {
            normalize_summary_guards(&mut alternative_guards);
            (!alternative_guards.is_empty()).then_some(SummaryCrashBucket {
                cause,
                alternative_guards,
            })
        })
        .collect()
}

enum SelectedTargetCrashRoutes<'a> {
    Published {
        buckets: &'a [psi_checked_trees::CrashRouteBucket],
        contracts: &'a [psi_typed_trees::signature::SignatureContract],
    },
    Private(&'a [SummaryCrashBucket]),
    Empty,
}

fn call_argument_substitution(
    program: &TypedTrees,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    caller_parameter_names: &[String],
) -> Vec<Option<CrashPredicateExpression>> {
    let mut argument_index = 0usize;
    target_parameters
        .iter()
        .map(|parameter| {
            if parameter.is_self {
                // Direct crash-route instantiation has historically retained
                // `self` as an ordinary named expression rather than treating
                // the receiver as a positional call argument.
                return Some(CrashPredicateExpression::Name(vec![
                    parameter.name.as_str().to_owned(),
                ]));
            }
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            Some(argument.map_or_else(
                || CrashPredicateExpression::Name(vec![parameter.name.as_str().to_owned()]),
                |argument| {
                    // This matches the existing direct-call encoder: content
                    // theorem substitution belongs to the callee route, while an
                    // ordinary argument uses the caller expression encoding.
                    CrashPredicateExpression::from_expression(
                        program,
                        argument,
                        caller_parameter_names,
                        None,
                    )
                },
            ))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn refine_published_crash_routes(
    program: &TypedTrees,
    state_flow: &psi_checked_trees::FlowStateFact,
    call_flow: &psi_checked_trees::FlowCallFact,
    call_site: &crate::CallSite<'_>,
    target_state_symbol: SymbolHandle,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    target_parameter_names: &[String],
    buckets: &[psi_checked_trees::CrashRouteBucket],
    contracts: &[psi_typed_trees::signature::SignatureContract],
    caller_parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
) -> Vec<SummaryCrashBucket> {
    let route_expressions = crash_route_expressions_by_identity(
        program,
        contracts,
        target_parameter_names,
        content_conservation,
    );
    let arguments = crate::call_site_argument_expressions(program, call_site);
    let substitution = call_argument_substitution(
        program,
        target_parameters,
        arguments,
        caller_parameter_names,
    );
    let mut surviving = Vec::new();
    for bucket in buckets {
        let mut guards = Vec::new();
        for guard in bucket.alternative_guards() {
            match guard {
                psi_checked_trees::CrashRouteGuard::Truth => {
                    guards.push(SummaryCrashRouteGuard::Truth);
                }
                psi_checked_trees::CrashRouteGuard::Predicate(identity) => {
                    let expression = *route_expressions.get(identity).expect(
                        "a canonical published crash route retains its typed producer expression",
                    );
                    match crate::checks::contracts::call_site_boolean_contract_expression_value(
                        program,
                        state_flow,
                        call_flow,
                        call_site,
                        target_state_symbol,
                        target_parameters,
                        expression,
                    ) {
                        Some(false) => {}
                        Some(true) => guards.push(SummaryCrashRouteGuard::Truth),
                        None => {
                            let predicate = CrashPredicateExpression::from_expression(
                                program,
                                expression,
                                target_parameter_names,
                                Some(content_conservation),
                            )
                            .substitute(&substitution);
                            match predicate.boolean_value() {
                                Some(false) => {}
                                Some(true) => guards.push(SummaryCrashRouteGuard::Truth),
                                None => guards.push(SummaryCrashRouteGuard::Predicate(predicate)),
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

/// Materialize direct invocation-specific crash refinement while the typed
/// expressions are still available. Selection uses a published ceiling when
/// one exists and a conservative monotone checked-body summary for same-unit
/// private machines. Recursive components close over their finite cause/scope
/// buckets; a dependency outside the recognized local/capsule graph keeps its
/// caller unexamined. The retained rows are entirely checked data: downstream
/// propagation can distinguish a proved-crash-free call from an unexamined call
/// without reopening source trees.
pub(super) fn attach_checked_crash_calls(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    crash_capsules: &[psi_checked_trees::CrashContractCapsule],
    plans: &mut [psi_checked_trees::MachineContractPlan],
) {
    let inferred_body_summaries =
        infer_private_body_summaries(program, flow, content_conservation, crash_capsules, plans);
    let mut calls_by_caller =
        Vec::<(SymbolHandle, Vec<psi_checked_trees::CheckedCrashCallSite>)>::new();
    for (_, state_flow) in flow.control.states.iter() {
        let caller_parameter_names = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
            .and_then(|machine| program.machine_states(machine).first())
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for call_flow in flow.control.calls.span_or_empty(state_flow.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                crate::contract_target_from_state_symbol(program, call_flow.target_symbol)
            else {
                continue;
            };
            let local_target = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == target_machine_symbol);
            let local_plan = plans
                .iter()
                .find(|plan| plan.machine == target_machine_symbol);
            let (
                target_parameters,
                target_parameter_names,
                target_routes,
                target_contract_fingerprint,
            ) = if let (Some(target_machine), Some(target_plan)) = (local_target, local_plan) {
                let Some(target_state) = program
                    .machine_states(target_machine)
                    .iter()
                    .find(|state| state.symbol == target_state_symbol)
                else {
                    continue;
                };
                let target_routes = if !target_plan.crash.published().is_empty() {
                    SelectedTargetCrashRoutes::Published {
                        buckets: target_plan.crash.published(),
                        contracts: program.machine_contracts(target_machine),
                    }
                } else if target_machine.supply_mode
                    == psi_language_semantics::MachineSupplyMode::CheckedBody
                {
                    let Some((_, summary)) = inferred_body_summaries
                        .iter()
                        .find(|(machine, _)| *machine == target_machine_symbol)
                    else {
                        // Dependency-unresolved private bodies remain
                        // unexamined rather than erasing a nested crash.
                        continue;
                    };
                    SelectedTargetCrashRoutes::Private(summary)
                } else {
                    // Omission on a requirement/boundary/exported interface is
                    // the published negative guarantee, so retain an empty row
                    // as positive crash-free evidence.
                    SelectedTargetCrashRoutes::Empty
                };
                (
                    program.state_parameters(target_state),
                    program
                        .machine_states(target_machine)
                        .first()
                        .map(|entry| {
                            program
                                .state_parameters(entry)
                                .iter()
                                .map(|parameter| parameter.name.as_str().to_owned())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                    target_routes,
                    target_plan.fingerprint,
                )
            } else {
                let Some(capsule) = crash_capsules.iter().find(|capsule| {
                    capsule.target_machine() == target_machine_symbol
                        && capsule.target_state() == target_state_symbol
                }) else {
                    continue;
                };
                let Some(signature) =
                    requirement_signature(program, target_machine_symbol, target_state_symbol)
                else {
                    continue;
                };
                let parameters = program.state_signature_parameters(signature);
                (
                    parameters,
                    parameters
                        .iter()
                        .map(|parameter| parameter.name.as_str().to_owned())
                        .collect(),
                    if capsule.published_buckets().is_empty() {
                        SelectedTargetCrashRoutes::Empty
                    } else {
                        SelectedTargetCrashRoutes::Published {
                            buckets: capsule.published_buckets(),
                            contracts: program.state_signature_contracts(signature),
                        }
                    },
                    capsule.target_contract_fingerprint(),
                )
            };
            let Some(call_site) = crate::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call_flow.statement_index,
                call_flow.call_ordinal,
            ) else {
                continue;
            };
            if matches!(call_site, crate::CallSite::TransitionNamed(_)) {
                // A named transition transfers within the current machine; it
                // is not an invocation of that machine's public crash ceiling.
                continue;
            }
            let arguments = crate::call_site_argument_expressions(program, &call_site);
            let surviving_summary = match target_routes {
                SelectedTargetCrashRoutes::Published { buckets, contracts } => {
                    refine_published_crash_routes(
                        program,
                        state_flow,
                        call_flow,
                        &call_site,
                        target_state_symbol,
                        target_parameters,
                        &target_parameter_names,
                        buckets,
                        contracts,
                        &caller_parameter_names,
                        content_conservation,
                    )
                }
                SelectedTargetCrashRoutes::Private(summary) => {
                    let substitution = call_argument_substitution(
                        program,
                        target_parameters,
                        arguments,
                        &caller_parameter_names,
                    );
                    normalize_summary_buckets(
                        summary
                            .iter()
                            .map(|bucket| bucket.substitute(&substitution))
                            .collect(),
                    )
                }
                SelectedTargetCrashRoutes::Empty => Vec::new(),
            };
            let surviving_buckets = surviving_summary
                .into_iter()
                .filter_map(SummaryCrashBucket::into_checked)
                .collect::<Vec<_>>();
            let caller_index = calls_by_caller
                .iter()
                .position(|(machine, _)| *machine == state_flow.machine_symbol)
                .unwrap_or_else(|| {
                    calls_by_caller.push((state_flow.machine_symbol, Vec::new()));
                    calls_by_caller.len() - 1
                });
            calls_by_caller[caller_index]
                .1
                .push(psi_checked_trees::CheckedCrashCallSite::new(
                    psi_checked_trees::CrashCallSiteLocation::new(
                        state_flow.state_symbol,
                        u32::try_from(call_flow.statement_index)
                            .expect("statement ordinal exceeds checked crash-call identity range"),
                        u32::try_from(call_flow.call_ordinal)
                            .expect("call ordinal exceeds checked crash-call identity range"),
                    ),
                    target_machine_symbol,
                    target_state_symbol,
                    target_contract_fingerprint,
                    surviving_buckets,
                ));
        }
    }

    for plan in plans {
        let checked_calls = calls_by_caller
            .iter_mut()
            .find(|(machine, _)| *machine == plan.machine)
            .map(|(_, calls)| std::mem::take(calls))
            .unwrap_or_default();
        plan.crash = plan
            .crash
            .clone()
            .with_checked_calls(checked_calls)
            .expect("one checked crash-call record occupies each invocation coordinate");
    }
}

fn infer_private_body_summaries(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    crash_capsules: &[psi_checked_trees::CrashContractCapsule],
    plans: &[psi_checked_trees::MachineContractPlan],
) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
    let mut nodes = plans
        .iter()
        .filter(|target| {
            target.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
                && target.crash.published().is_empty()
        })
        .filter_map(|target| {
            Some(SummaryNode {
                machine: target.machine,
                direct: inferred_direct_body_crash_buckets(target),
                invocations: machine_non_transition_invocation_sites(
                    program,
                    flow,
                    target.machine,
                )?,
            })
        })
        .collect::<Vec<_>>();

    // Compute the greatest viable local subgraph. A recursive SCC is viable
    // when all of its outgoing targets are known local plans or pinned
    // requirement capsules. Any unresolved dependency removes its caller and
    // then every private caller that depended on it.
    loop {
        let viable_machines = nodes.iter().map(|node| node.machine).collect::<Vec<_>>();
        let before = nodes.len();
        nodes.retain(|node| {
            node.invocations.iter().all(|invocation| {
                if let Some(plan) = plans
                    .iter()
                    .find(|plan| plan.machine == invocation.target_machine)
                {
                    plan.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
                        || !plan.crash.published().is_empty()
                        || viable_machines.contains(&invocation.target_machine)
                } else {
                    crash_capsules.iter().any(|capsule| {
                        capsule.target_machine() == invocation.target_machine
                            && capsule.target_state() == invocation.target_state
                    })
                }
            })
        });
        if nodes.len() == before {
            break;
        }
    }

    let mut equations = Vec::new();
    for node in &nodes {
        let caller_parameter_names = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == node.machine)
            .and_then(|machine| program.machine_states(machine).first())
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut private_dependencies = Vec::new();
        let mut published_dependencies = Vec::new();
        for invocation in &node.invocations {
            let state_flow = flow
                .control
                .states
                .iter()
                .find_map(|(_, state)| {
                    (state.machine_symbol == node.machine
                        && state.state_symbol == invocation.caller_state)
                        .then_some(state)
                })
                .expect("a retained summary invocation has its flow state");
            let call_flow = flow
                .control
                .calls
                .span_or_empty(state_flow.calls)
                .iter()
                .find(|call| {
                    call.statement_index == invocation.statement_index
                        && call.call_ordinal == invocation.call_ordinal
                })
                .expect("a retained summary invocation has its flow call");
            let call_site = crate::find_call_site(
                program,
                node.machine,
                invocation.caller_state,
                invocation.statement_index,
                invocation.call_ordinal,
            )
            .expect("a retained summary invocation has its typed call site");
            let arguments = crate::call_site_argument_expressions(program, &call_site);

            if let Some(target_plan) = plans
                .iter()
                .find(|plan| plan.machine == invocation.target_machine)
            {
                let target_machine = program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == invocation.target_machine)
                    .expect("a local crash plan has its typed machine");
                let target_state = program
                    .machine_states(target_machine)
                    .iter()
                    .find(|state| state.symbol == invocation.target_state)
                    .expect("a local crash invocation has its typed state");
                let target_parameters = program.state_parameters(target_state);
                let target_parameter_names = program
                    .machine_states(target_machine)
                    .first()
                    .map(|entry| {
                        program
                            .state_parameters(entry)
                            .iter()
                            .map(|parameter| parameter.name.as_str().to_owned())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if target_plan.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
                    && target_plan.crash.published().is_empty()
                {
                    private_dependencies.push(PrivateSummaryDependency {
                        machine: invocation.target_machine,
                        substitution: call_argument_substitution(
                            program,
                            target_parameters,
                            arguments,
                            &caller_parameter_names,
                        ),
                        recursive: private_dependency_reaches(
                            &nodes,
                            plans,
                            invocation.target_machine,
                            node.machine,
                        ),
                    });
                } else if !target_plan.crash.published().is_empty() {
                    published_dependencies.extend(refine_published_crash_routes(
                        program,
                        state_flow,
                        call_flow,
                        &call_site,
                        invocation.target_state,
                        target_parameters,
                        &target_parameter_names,
                        target_plan.crash.published(),
                        program.machine_contracts(target_machine),
                        &caller_parameter_names,
                        content_conservation,
                    ));
                }
            } else {
                let capsule = crash_capsules
                    .iter()
                    .find(|capsule| {
                        capsule.target_machine() == invocation.target_machine
                            && capsule.target_state() == invocation.target_state
                    })
                    .expect("the viability pass retained only pinned requirement targets");
                if capsule.published_buckets().is_empty() {
                    continue;
                }
                let signature = requirement_signature(
                    program,
                    invocation.target_machine,
                    invocation.target_state,
                )
                .expect("a pinned requirement capsule has its typed signature");
                let target_parameters = program.state_signature_parameters(signature);
                let target_parameter_names = target_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>();
                published_dependencies.extend(refine_published_crash_routes(
                    program,
                    state_flow,
                    call_flow,
                    &call_site,
                    invocation.target_state,
                    target_parameters,
                    &target_parameter_names,
                    capsule.published_buckets(),
                    program.state_signature_contracts(signature),
                    &caller_parameter_names,
                    content_conservation,
                ));
            }
        }
        equations.push(PrivateSummaryEquation {
            machine: node.machine,
            direct: node.direct.clone(),
            private_dependencies,
            published_dependencies: normalize_summary_buckets(published_dependencies),
        });
    }
    solve_private_summary_fixed_point(&equations)
}

struct SummaryNode {
    machine: SymbolHandle,
    direct: Vec<SummaryCrashBucket>,
    invocations: Vec<SummaryInvocationSite>,
}

#[derive(Debug, Clone, Copy)]
struct SummaryInvocationSite {
    caller_state: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
}

struct PrivateSummaryEquation {
    machine: SymbolHandle,
    direct: Vec<SummaryCrashBucket>,
    private_dependencies: Vec<PrivateSummaryDependency>,
    published_dependencies: Vec<SummaryCrashBucket>,
}

struct PrivateSummaryDependency {
    machine: SymbolHandle,
    substitution: Vec<Option<CrashPredicateExpression>>,
    recursive: bool,
}

fn solve_private_summary_fixed_point(
    equations: &[PrivateSummaryEquation],
) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
    let mut resolved = equations
        .iter()
        .map(|equation| (equation.machine, equation.direct.clone()))
        .collect::<Vec<_>>();
    for _ in 0..=equations.len() {
        let mut changed = false;
        for equation in equations {
            let mut buckets = equation.direct.clone();
            buckets.extend(equation.published_dependencies.clone());
            for dependency in &equation.private_dependencies {
                let selected = &resolved
                    .iter()
                    .find(|(machine, _)| *machine == dependency.machine)
                    .expect("every private dependency belongs to the viable fixed point")
                    .1;
                buckets.extend(selected.iter().map(|bucket| {
                    if dependency.recursive {
                        // Substitution around a recursive cycle can create an
                        // unbounded family such as p(n), p(n - 1), ... . The
                        // finite conservative lattice widens exactly those SCC
                        // edges to their cause bucket.
                        SummaryCrashBucket::unconditional(bucket.cause)
                    } else {
                        bucket.substitute(&dependency.substitution)
                    }
                }));
            }
            let buckets = normalize_summary_buckets(buckets);
            let (_, current) = resolved
                .iter_mut()
                .find(|(machine, _)| *machine == equation.machine)
                .expect("every viable node starts with a direct summary");
            if *current != buckets {
                *current = buckets;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    resolved
}

fn machine_non_transition_invocation_sites(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    machine: SymbolHandle,
) -> Option<Vec<SummaryInvocationSite>> {
    let mut sites = Vec::new();
    for (_, state) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine)
    {
        for call in flow.control.calls.span_or_empty(state.calls) {
            let site = crate::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            if matches!(site, crate::CallSite::TransitionNamed(_)) {
                continue;
            }
            let (target_machine, target_state) =
                crate::contract_target_from_state_symbol(program, call.target_symbol)?;
            sites.push(SummaryInvocationSite {
                caller_state: state.state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine,
                target_state,
            });
        }
    }
    Some(sites)
}

fn inferred_direct_body_crash_buckets(
    target: &psi_checked_trees::MachineContractPlan,
) -> Vec<SummaryCrashBucket> {
    let mut buckets = target
        .crash
        .checked_sites()
        .iter()
        .map(|site| SummaryCrashBucket::unconditional(site.cause()))
        .collect::<Vec<_>>();
    normalize_summary_buckets(std::mem::take(&mut buckets))
}

fn private_dependency_reaches(
    nodes: &[SummaryNode],
    plans: &[psi_checked_trees::MachineContractPlan],
    start: SymbolHandle,
    goal: SymbolHandle,
) -> bool {
    let mut pending = vec![start];
    let mut visited = Vec::new();
    while let Some(machine) = pending.pop() {
        if machine == goal {
            return true;
        }
        if visited.contains(&machine) {
            continue;
        }
        visited.push(machine);
        let Some(node) = nodes.iter().find(|node| node.machine == machine) else {
            continue;
        };
        pending.extend(node.invocations.iter().filter_map(|invocation| {
            plans
                .iter()
                .find(|plan| plan.machine == invocation.target_machine)
                .filter(|plan| {
                    plan.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
                        && plan.crash.published().is_empty()
                })
                .map(|_| invocation.target_machine)
        }));
    }
    false
}

fn requirement_signature(
    program: &TypedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Option<&psi_typed_trees::signature::StateSignature> {
    if target_machine == target_state {
        return program
            .machine_parameter_signature(target_state)
            .map(|(_, signature)| signature);
    }
    program
        .traits()
        .iter()
        .find(|definition| definition.symbol == target_machine)
        .and_then(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.symbol == target_state)
        })
}

fn crash_route_expressions_by_identity(
    program: &TypedTrees,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
) -> std::collections::BTreeMap<
    psi_checked_trees::CrashPredicateIdentity,
    psi_typed_trees::expression::ExpressionHandle,
> {
    let mut expressions = std::collections::BTreeMap::new();
    for contract in contracts {
        if !matches!(
            contract.kind,
            psi_typed_trees::signature::SignatureContractKind::Crashes { .. }
        ) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if is_true_crash_route(program, fact) {
                continue;
            }
            let mut bytes = Vec::new();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                false,
                &mut bytes,
            );
            expressions.insert(
                psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes),
                *expression,
            );
        }
    }
    expressions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_summary_fixed_point_closes_recursive_components() {
        let first = SymbolHandle::from_arena_index(1);
        let second = SymbolHandle::from_arena_index(2);
        let abort = SummaryCrashBucket::unconditional(psi_checked_trees::CrashCause::Abort);
        let guarded_abort = SummaryCrashBucket {
            cause: psi_checked_trees::CrashCause::Abort,
            alternative_guards: vec![SummaryCrashRouteGuard::Predicate(
                CrashPredicateExpression::Parameter(0),
            )],
        };
        let trap = SummaryCrashBucket::unconditional(psi_checked_trees::CrashCause::Trap);
        let equations = vec![
            PrivateSummaryEquation {
                machine: first,
                direct: Vec::new(),
                private_dependencies: vec![PrivateSummaryDependency {
                    machine: second,
                    substitution: Vec::new(),
                    recursive: true,
                }],
                published_dependencies: vec![guarded_abort],
            },
            PrivateSummaryEquation {
                machine: second,
                direct: vec![trap.clone()],
                private_dependencies: vec![PrivateSummaryDependency {
                    machine: first,
                    substitution: Vec::new(),
                    recursive: true,
                }],
                published_dependencies: Vec::new(),
            },
        ];

        let summaries = solve_private_summary_fixed_point(&equations);
        for machine in [first, second] {
            let buckets = summaries
                .iter()
                .find_map(|(candidate, buckets)| (*candidate == machine).then_some(buckets))
                .expect("each recursive member has a summary");
            assert!(buckets.contains(&abort));
            assert!(buckets.contains(&trap));
        }
    }

    #[test]
    fn private_summary_preserves_acyclic_guard_substitution() {
        let leaf = SymbolHandle::from_arena_index(1);
        let wrapper = SymbolHandle::from_arena_index(2);
        let route = SummaryCrashBucket {
            cause: psi_checked_trees::CrashCause::Trap,
            alternative_guards: vec![SummaryCrashRouteGuard::Predicate(
                CrashPredicateExpression::Parameter(0),
            )],
        };
        let equations = vec![
            PrivateSummaryEquation {
                machine: leaf,
                direct: Vec::new(),
                private_dependencies: Vec::new(),
                published_dependencies: vec![route],
            },
            PrivateSummaryEquation {
                machine: wrapper,
                direct: Vec::new(),
                private_dependencies: vec![PrivateSummaryDependency {
                    machine: leaf,
                    substitution: vec![Some(CrashPredicateExpression::Parameter(1))],
                    recursive: false,
                }],
                published_dependencies: Vec::new(),
            },
        ];

        let summaries = solve_private_summary_fixed_point(&equations);
        let [bucket] = summaries
            .iter()
            .find_map(|(machine, buckets)| (*machine == wrapper).then_some(buckets.as_slice()))
            .expect("wrapper summary")
        else {
            panic!("wrapper should retain one guarded bucket")
        };
        assert_eq!(
            bucket.alternative_guards,
            vec![SummaryCrashRouteGuard::Predicate(
                CrashPredicateExpression::Parameter(1)
            )]
        );
    }
}
