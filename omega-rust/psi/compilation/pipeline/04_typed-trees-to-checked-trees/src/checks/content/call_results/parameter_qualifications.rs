//! Exact out-parameter subjects for one completed invocation.
//!
//! Flow publishes one promise per qualified child, including fixed-array
//! elements. Rebuilding the entire declaration and projecting every child for
//! each promise makes this join quadratic. Prepare that immutable roster once
//! and order its exact storage identities; no verdict or live premise is cached.
//! Checked bodies owe these obligations at every normal exit. A bodyless call
//! still needs the same readable-exclusive formal and exact entry membership:
//! an annotation, issuer name or prepared subject cannot introduce authority.

use checked_trees::{CheckFacts, FlowCallFact};
use facts::{FactPlace, PlaceRoot, PlaceSegment, ProgramPoint, QualificationEvidence};
use language_semantics::{QualificationEvidenceOrigin, SemanticDomainId};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use validation::{CallSite, CanonicalPlace, CheckingInput};

struct ParameterSubject {
    place: CanonicalPlace,
    domain: SymbolHandle,
    instance: SemanticDomainId,
    position: usize,
}

pub(super) struct ParameterQualificationJoin<'program> {
    program: &'program dyn CheckingInput,
    facts: &'program CheckFacts,
    target: SymbolHandle,
    parameters: &'program [StateParameter],
    call_flow: &'program FlowCallFact,
    checked_body: bool,
    subjects: Vec<ParameterSubject>,
}

impl<'program> ParameterQualificationJoin<'program> {
    pub(super) fn prepare(
        program: &'program dyn CheckingInput,
        facts: &'program CheckFacts,
        site: CallSite<'_>,
        point: ProgramPoint,
    ) -> Option<Self> {
        let ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = point
        else {
            return None;
        };
        let target = match site {
            CallSite::Expression { call, .. } => call.target_symbol,
            CallSite::Statement(call) => call.target_symbol,
            CallSite::TransitionNamed { path, .. } => path.symbol,
        };
        let selected = match site {
            CallSite::Expression { call, .. } => {
                program.requirement_call_signature(machine_symbol, call)
            }
            _ => None,
        };
        let parameters = selected
            .map(|signature| program.state_signature_parameters(signature))
            .or_else(|| validation::call_target_parameters(program, target))?;
        let arguments = validation::call_site_argument_expressions(program, &site);
        let state_flow = facts.flow.control.state(machine_symbol, state_symbol)?;
        let call_flow = facts
            .flow
            .control
            .calls
            .span(state_flow.calls)?
            .iter()
            .find(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == target
            })?;
        let operands = facts.flow.control.operands.span(call_flow.operands)?;
        let receiver = match site {
            CallSite::Expression { call, .. } if call.receiver.is_valid() => Some(call.receiver),
            _ => None,
        };
        if parameters.len() != arguments.len()
            || !operands
                .iter()
                .map(|operand| operand.expression)
                .eq(receiver.into_iter().chain(arguments.iter().copied()))
        {
            return None;
        }
        let checked_body = validation::call_target_has_checked_body_contract(program, target);
        let identities =
            crate::flow::call_parameter_qualification_identities(program, target, selected);
        let mut subjects = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            let mut promised = identities
                .iter()
                .filter(|(declared, _, _, from_declared_type)| {
                    *declared == position && (checked_body || !*from_declared_type)
                })
                .map(|(_, domain, instance, _)| (Vec::new(), *domain, *instance))
                .collect::<Vec<_>>();
            let referent = crate::checks::contracts::exclusive_referent_type(
                program,
                parameter.type_reference,
            );
            if referent.is_valid() {
                promised.extend(
                    validation::field_domains::declared_result_field_domain_paths(
                        program, referent,
                    ),
                );
            }
            if promised.is_empty() {
                continue;
            }
            let storage = if crate::flow::operand_binds_live_view(program, parameter.type_reference)
            {
                // These are the captured, completed operands used by promise
                // publication, not descriptor syntax replayed after later effects.
                let Some(operand) = operands.get(position + usize::from(receiver.is_some())) else {
                    continue;
                };
                let Some(referents) = facts.flow.control.operand_referents.span(operand.referents)
                else {
                    continue;
                };
                referents
                    .iter()
                    .filter_map(|referent| {
                        Some(CanonicalPlace {
                            root: referent.root,
                            segments: facts
                                .flow
                                .control
                                .operand_referent_segments
                                .span(referent.segments)?
                                .to_vec(),
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                validation::canonical_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    arguments[position],
                )
                .into_iter()
                .collect()
            };
            for storage in storage {
                for (path, domain, instance) in &promised {
                    if let Some(place) =
                        crate::flow::project_captured_view_place(storage.clone(), path)
                    {
                        subjects.push(ParameterSubject {
                            place,
                            domain: *domain,
                            instance: *instance,
                            position,
                        });
                    }
                }
            }
        }
        subjects.sort_unstable_by(|left, right| {
            left.place
                .compare_identity(&right.place)
                .then_with(|| left.position.cmp(&right.position))
        });
        Some(Self {
            program,
            facts,
            target,
            parameters,
            call_flow,
            checked_body,
            subjects,
        })
    }

    pub(super) fn check(
        &self,
        evidence: &QualificationEvidence,
        root: PlaceRoot,
        path: &[PlaceSegment],
        domain: SymbolHandle,
        instance: SemanticDomainId,
    ) -> Option<()> {
        if evidence.origin != QualificationEvidenceOrigin::Propagated
            || evidence.source_symbol != self.target
            || evidence.requirement_symbol.is_valid()
        {
            return None;
        }
        let subject = CanonicalPlace {
            root,
            segments: path.to_vec(),
        };
        let start = self
            .subjects
            .partition_point(|row| row.place.compare_identity(&subject).is_lt());
        // Position order preserves the original first matching formal, even
        // when several arguments name exactly the same captured storage.
        let position = self.subjects[start..]
            .iter()
            .take_while(|row| row.place == subject)
            .find(|row| row.domain == domain && row.instance == instance)?
            .position;
        if self.checked_body {
            return Some(());
        }
        let parameter = self.parameters.get(position)?;
        if !crate::checks::contracts::is_readable_mutable_reference(
            self.program,
            parameter.type_reference,
        ) {
            return None;
        }
        // A public ensures preserves this exact entry subject. Issuer
        // metadata is not an installation receipt or permission to mint it.
        self.facts
            .flow
            .semantic_constraint_contexts(self.call_flow.entry_constraints)
            .any(|context| {
                self.facts
                    .semantic
                    .context_view(self.facts.semantic.contexts.get(context))
                    .facts()
                    .any(|fact| {
                        fact.payload
                            .domain_membership()
                            .is_some_and(|(_, _, symbol, identity)| {
                                symbol == domain && identity == instance
                            })
                            && matches!(fact.place, FactPlace::Place(place)
                    if self.facts.semantic.places.get(place).root == root
                        && self.facts.semantic.place_segments.span_or_empty(
                            self.facts.semantic.places.get(place).segments) == path)
                    })
            })
            .then_some(())
    }
}
