//! Finite live predicates carried by exact state arguments and original
//! invocation referents. Domain memberships and normalized IEEE observations
//! use the same custody path, including when no executable binding survives.
//!
//! Origins alone supply no row. Every row is captured from an available fact;
//! incoming joins can only weaken claims, and root entry is never narrowed.
use super::TransitionTargetNode;
use crate::flow::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::ExpressionHandle;
use checked_trees::name::Identifier;
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::flow) struct QualifiedInput {
    subject: facts::PlaceRoot,
    segments: Vec<facts::PlaceSegment>,
    predicate: CarriedPredicate,
    evidence: QualificationEvidence,
}

/// The finite transportable predicate lattice: a declared domain membership or
/// a normalized IEEE observation. One implementation serves both the exact
/// state-argument carriers and the match-result predicate join; the two
/// variants compare and combine identically, and only the published row
/// differs in whether it names an evaluated value or an authored domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::flow) enum CarriedPredicate {
    Domain {
        domain: HandleSpan<Identifier>,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    },
    Float(facts::FloatPredicate),
}

impl CarriedPredicate {
    fn from_fact(fact: &Fact) -> Option<Self> {
        if matches!(fact.origin, FactOrigin::CallRequires) {
            return None;
        }
        match fact.payload {
            facts::domain_membership_fields_pattern!(domain, domain_symbol, semantic_domain)
                if domain_symbol.is_valid() =>
            {
                Some(Self::Domain {
                    domain,
                    domain_symbol,
                    semantic_domain,
                })
            }
            FactPayload::FloatPredicate { predicate } => Some(Self::Float(predicate)),
            _ => None,
        }
    }

    pub(in crate::flow) fn implies(self, other: Self) -> bool {
        match (self, other) {
            (
                Self::Domain {
                    domain_symbol,
                    semantic_domain,
                    ..
                },
                Self::Domain {
                    domain_symbol: other_symbol,
                    semantic_domain: other_domain,
                    ..
                },
            ) => domain_symbol == other_symbol && semantic_domain == other_domain,
            (Self::Float(left), Self::Float(right)) => left.implies(right),
            _ => false,
        }
    }

    pub(in crate::flow) fn common_consequence(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Float(left), Self::Float(right)) => {
                left.common_consequence(right).map(Self::Float)
            }
            _ => self.implies(other).then_some(self),
        }
    }

    /// The published fact row. `value` names the evaluated expression whose
    /// place is observed when the predicate was derived from a value; a
    /// predicate captured from a fact row keeps that row's authored domain
    /// spelling and publishes no value.
    pub(in crate::flow) fn payload(self, value: ExpressionHandle) -> FactPayload {
        match self {
            Self::Domain {
                domain,
                domain_symbol,
                semantic_domain,
            } => FactPayload::DomainMembership {
                value,
                domain,
                domain_symbol,
                semantic_domain,
            },
            Self::Float(predicate) => FactPayload::FloatPredicate { predicate },
        }
    }
}

impl QualifiedInput {
    fn same_subject(&self, other: &Self) -> bool {
        self.subject == other.subject
            && self.segments == other.segments
            && self.evidence == other.evidence
    }

    fn same_claim(&self, other: &Self) -> bool {
        self.same_subject(other)
            && self.predicate.implies(other.predicate)
            && other.predicate.implies(self.predicate)
    }
}

pub(in crate::flow) struct CapturedQualification {
    input: QualifiedInput,
    context: facts::FactContextHandle,
    isolated: bool,
}

impl CapturedQualification {
    /// An isolated claim stands on its own storage and never consults the
    /// caller-side binding-stability verdict -- the transition caller uses
    /// this to skip resolving stability when nothing could consume it.
    pub(in crate::flow) fn is_isolated(&self) -> bool {
        self.isolated
    }
}

pub(super) fn meet(previous: &mut Vec<QualifiedInput>, incoming: &[QualifiedInput]) -> bool {
    let before = previous.len();
    let mut changed = false;
    previous.retain_mut(|claim| {
        let mut common: Option<CarriedPredicate> = None;
        for next in incoming.iter().filter(|next| claim.same_subject(next)) {
            let Some(candidate) = claim.predicate.common_consequence(next.predicate) else {
                continue;
            };
            if common.is_none_or(|retained| candidate.implies(retained)) {
                common = Some(candidate);
            }
        }
        let Some(common) = common else {
            return false;
        };
        changed |= !claim.predicate.implies(common) || !common.implies(claim.predicate);
        claim.predicate = common;
        true
    });
    changed || previous.len() != before
}

/// A membership can only disappear. A float predicate can additionally
/// weaken among the five positive comparisons at authored endpoints, then
/// non-NaN, then absence. Joins never manufacture new numeric endpoints.
pub(super) fn height(
    program: &dyn ::validation::CheckingInput,
    inputs: &[QualifiedInput],
) -> usize {
    let float_height = program
        .expression_table
        .expression_count()
        .saturating_mul(5)
        .saturating_add(2);
    inputs.iter().fold(0usize, |height, input| {
        height.saturating_add(match input.predicate {
            CarriedPredicate::Domain { .. } => 1,
            CarriedPredicate::Float(_) => float_height,
        })
    })
}

/// A field chain may pass through owned records, but never another reference.
/// The final qualified value must itself be isolated from referenced storage.
fn owned_projection(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    mut reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> bool {
    reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    reference = program
        .type_reference_table
        .dereferenced_type_reference(reference);
    for segment in segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return false;
        };
        let TypeReferenceNode::Named { symbol: owner, .. } =
            program.type_reference_table.type_reference(
                program
                    .type_reference_table
                    .unconstrained_type_reference(reference),
            )
        else {
            return false;
        };
        // An implicit receiver's referee names its machine, whereas ordinary
        // record arguments name the data declaration. Resolve that semantic
        // attachment before checking the field's owner; spelling is irrelevant.
        let owner = program
            .machine(*owner)
            .map_or(*owner, |machine| machine.attached_data_symbol);
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| owner.is_valid() && data.symbol == owner)
        else {
            return false;
        };
        if !symbol.is_valid() || program.symbols.get(*symbol).parent != owner {
            return false;
        }
        let Some(field) = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => {
                    Some(field)
                }
                _ => None,
            })
        else {
            return false;
        };
        reference = field.type_reference;
    }
    frames.proof_value_is_caller_isolated(reference)
}

/// Capture live predicates whose place is the argument's own denoted
/// place or a projection below it. `source_segments` names the argument value
/// itself, so a fact deeper in the same place describes a selected field of
/// the value the state parameter will carry; the captured claim republishes
/// only the part below the argument.
#[allow(clippy::too_many_arguments)]
fn capture_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    flow_context: &FlowBuildContext,
    source_root: facts::PlaceRoot,
    source_segments: &[facts::PlaceSegment],
    source_type: TypeReferenceHandle,
    destination: SymbolHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<CapturedQualification> {
    let Some(frames) = flow_context.call_frames else {
        return Vec::new();
    };
    let isolated = frames.proof_value_is_caller_isolated(source_type);
    let source_root = source_root;
    let mut captured = Vec::new();
    for context in flow_context
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
    {
        for fact in semantic
            .context_view(semantic.contexts.get(context.context))
            .facts()
        {
            let Some(predicate) = CarriedPredicate::from_fact(fact) else {
                continue;
            };
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let Some(place) = crate::flow::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(place),
            ) else {
                continue;
            };
            if place.root != source_root || !place.segments.starts_with(source_segments) {
                continue;
            }
            let segments = &place.segments[source_segments.len()..];
            if !owned_projection(program, frames, source_type, segments) {
                continue;
            }
            captured.push(CapturedQualification {
                input: QualifiedInput {
                    subject: facts::PlaceRoot::Symbol(destination),
                    segments: segments.to_vec(),
                    predicate,
                    evidence: fact.evidence,
                },
                context: context.context,
                isolated,
            });
        }
    }
    captured
}

/// Capture the predicates carried by one transition argument into its
/// destination parameter. The argument is read at its authored evaluation
/// point, so every denoted place counts — a renamed state parameter, a staged
/// local, a selected field, or an evaluated call result — rather than only a
/// bare parameter name. Facts must already be live in `contexts`; nothing is
/// rederived from the destination signature, and a missing place or value type
/// fails closed.
#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn capture_argument(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    flow_context: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    ordinal: usize,
    argument: ExpressionHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<CapturedQualification> {
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return Vec::new();
    };
    let Some(destination) = flow_context
        .state_index_in_machine(program, machine.symbol, path.symbol)
        .and_then(|index| program.machine_states(machine).get(index))
    else {
        return Vec::new();
    };
    let Some(parameter) = program.state_parameters(destination).iter().nth(ordinal) else {
        return Vec::new();
    };
    let Some(source) =
        flow_context.canonical_place_at(program, state.symbol, statement_index, argument)
    else {
        return Vec::new();
    };
    let Some(source_type) =
        flow_context.expression_result_type_at(program, machine, state, argument)
    else {
        return Vec::new();
    };
    capture_place(
        program,
        semantic,
        flow_context,
        source.root,
        &source.segments,
        source_type,
        parameter.symbol,
        contexts,
    )
}

pub(in crate::flow) fn finish(
    flow_context: &FlowBuildContext,
    contexts: HandleSpan<FlowSemanticContextRef>,
    captured: Vec<CapturedQualification>,
    bindings_stable: bool,
) -> Vec<QualifiedInput> {
    let live = flow_context
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts);
    let mut inputs: Vec<QualifiedInput> = Vec::new();
    for captured in captured {
        if (captured.isolated
            || (bindings_stable
                && live
                    .iter()
                    .any(|context| context.context == captured.context)))
            && !inputs.iter().any(|input| input.same_claim(&captured.input))
        {
            inputs.push(captured.input);
        }
    }
    inputs
}

/// Capture original input storage independently of the next state's runtime
/// argument roster. These are current live observations, not a replay of the
/// machine requires. The same meet as explicit arguments withdraws a claim
/// when any reachable predecessor lacks it.
pub(super) fn capture_invocation_referents(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    contexts: HandleSpan<FlowSemanticContextRef>,
) -> Vec<QualifiedInput> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    if !program.state_parameters(entry).iter().any(|parameter| {
        crate::flow::entry_origins::invocation_referent(program, machine, parameter.symbol)
            .is_some()
    }) {
        return Vec::new();
    }
    let context_refs = build
        .contexts
        .semantic_context_refs
        .span_or_empty(contexts)
        .to_vec();
    let mut inputs = Vec::new();
    for context in context_refs {
        for fact in semantic
            .context_view(semantic.contexts.get(context.context))
            .facts()
        {
            let Some(predicate) = CarriedPredicate::from_fact(fact) else {
                continue;
            };
            let Some(place) = fact.place.as_place().and_then(|place| {
                crate::flow::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(place),
                )
            }) else {
                continue;
            };
            let Some(projected) = crate::flow::entry_origins::project_invocation_place(
                program,
                build,
                machine,
                state,
                statement_index,
                &place,
                true,
            ) else {
                continue;
            };
            let [projected] = projected.as_slice() else {
                continue;
            };
            let facts::PlaceRoot::InvocationReferent { type_reference, .. } = projected.root else {
                continue;
            };
            let Some(frames) = build.call_frames else {
                continue;
            };
            if !owned_projection(program, frames, type_reference, &projected.segments) {
                continue;
            }
            let same_carrier = || {
                let source = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    &place,
                )?;
                let destination = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement_index,
                    projected,
                )?;
                let carrier = |reference| {
                    let reference = program
                        .type_reference_table
                        .unconstrained_type_reference(reference);
                    let reference = program
                        .type_reference_table
                        .dereferenced_type_reference(reference);
                    program.normalized_type_identity(
                        program
                            .type_reference_table
                            .unconstrained_type_reference(reference),
                    )
                };
                Some(carrier(source) == carrier(destination))
            };
            if same_carrier() != Some(true) {
                continue;
            }
            let input = QualifiedInput {
                subject: projected.root,
                segments: projected.segments.clone(),
                predicate,
                evidence: fact.evidence,
            };
            if !inputs
                .iter()
                .any(|previous: &QualifiedInput| previous.same_claim(&input))
            {
                inputs.push(input);
            }
        }
    }
    inputs
}

pub(super) fn append(semantic: &mut FactPlan, inputs: &[QualifiedInput], point: ProgramPoint) {
    for input in inputs {
        let place = semantic.places.append(facts::Place {
            root: input.subject,
            segments: HandleSpan::empty(),
        });
        for segment in &input.segments {
            semantic.push_place_segment(place, *segment);
        }
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: input.evidence,
            payload: input.predicate.payload(ExpressionHandle::invalid()),
        });
        let mut references = HandleSpan::empty();
        semantic.append_ref(&mut references, fact);
        semantic.append_context(point, references);
    }
}
