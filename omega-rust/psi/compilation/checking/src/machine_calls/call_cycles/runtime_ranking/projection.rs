use crate::frozen_program::expression_table;
use crate::{RankingRangeMeasure, ScalarViewComputation, declared_scalar_view};
use language_semantics::RankingViewId;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Transient declaration-local projection. A measure's current typed carrier
/// has no populated symbol, so the unique table occurrence is its identity;
/// matching field spellings in another measure do not make it the same order.
#[derive(PartialEq, Eq)]
pub(super) enum RankOrder {
    Natural(PrimitiveType),
    IncreasingTo(PrimitiveType),
    /// `Nat::BoundedDistance`: the component's rank is the distance from the
    /// ranked (lower) subject up to the paired upper subject.
    BoundedDistance(PrimitiveType),
    /// `Slice::Length`: the component's rank is the ranked slice subject's
    /// length. Element types differ per member, but the produced natural
    /// coordinate is one shared order; each call edge still re-proves the
    /// actual's exact length arrival.
    SliceLength,
    /// A declared measure whose body forwards its parameter on the same
    /// unsigned carrier the subject has (validation's `declared_identity_view`).
    /// The produced rank is the subject itself, judged exactly as `Natural`,
    /// but the authored view stays private witness identity: members share
    /// this order only through the same measure, never with `Nat::Descending`.
    DeclaredIdentity {
        measure: SymbolHandle,
        primitive: PrimitiveType,
    },
    /// A declared scalar application compared by its produced value. Each
    /// occurrence independently forms the body and carrier; subject descent
    /// alone says nothing about this order. Members share the selected
    /// measure and carrier, never an inferred global monotonicity property.
    DeclaredComputation {
        measure: SymbolHandle,
        input_type: TypeReferenceHandle,
        result_primitive: PrimitiveType,
        computation: ScalarViewComputation,
    },
    Lexicographic {
        measure_index: usize,
        data: SymbolHandle,
        fields: Vec<SymbolHandle>,
    },
}

pub(super) struct RankProjection {
    pub(super) order: RankOrder,
    pub(super) argument_position: usize,
    pub(super) subject: ExpressionHandle,
    /// The upper subject of a two-subject view (`Nat::BoundedDistance` ranks
    /// `(subject, paired_subject)` by `paired_subject - subject`). Invalid for
    /// single-subject orders.
    pub(super) paired_subject: ExpressionHandle,
    pub(super) range: ExpressionHandle,
    /// The scalar measure the member's own rank-range judgment selected,
    /// rebuilt here so the call-component telescope can ask for the exact
    /// required-entry set the member's arrival proofs already cover.
    /// `None` for a lexicographic order, which has no scalar transport.
    pub(super) measure: Option<RankingRangeMeasure>,
}

impl RankProjection {
    pub(super) fn resolve(program: &dyn crate::CheckingInput, machine: &Machine) -> Option<Self> {
        let witness = machine.termination_plan.implementation_witness.as_ref()?;
        // Never rediscover the subject by scanning rendered expressions.
        let custody = program.ranking_expression_custody_for(machine.symbol)?;
        if witness.subjects.len() != custody.subjects.len()
            || witness.rank_range.is_some() != custody.rank_range.is_some()
            || custody.rank_range.is_some_and(|range| !range.is_valid())
        {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        if witness.ranking_view == RankingViewId::NAT_BOUNDED_DISTANCE
            && Some(witness.view_path.as_str()) == witness.ranking_view.canonical_path()
        {
            let [lower, upper] = custody.subjects.as_slice() else {
                return None;
            };
            if !witness.view_arguments.is_empty() || !custody.view_arguments.is_empty() {
                return None;
            }
            let (argument_position, parameter) = entry_parameter(program, entry, *lower)?;
            // The cursor has an exact storage carrier. Its bound is an
            // expression, not necessarily another formal: the call judgment
            // independently proves its meaning and formation after exact substitution.
            let primitive = integer_subject_carrier(program, machine, entry, *lower, parameter)?;
            return Some(Self {
                order: RankOrder::BoundedDistance(primitive),
                argument_position,
                subject: *lower,
                paired_subject: *upper,
                range: custody.rank_range.unwrap_or_default(),
                measure: Some(RankingRangeMeasure::Distance {
                    lower: *lower,
                    upper: *upper,
                }),
            });
        }
        let [subject] = custody.subjects.as_slice() else {
            return None;
        };
        let (argument_position, parameter) = entry_parameter(program, entry, *subject)?;
        if witness.ranking_view == RankingViewId::SLICE_LENGTH
            && Some(witness.view_path.as_str()) == witness.ranking_view.canonical_path()
        {
            // The ranked subject is the collection itself, never a scalar
            // carrier; the view produces its length coordinate. A member
            // chain reads the slice leaf its carrier formal's record stores,
            // so the leaf -- not the formal -- must carry the slice.
            if !witness.view_arguments.is_empty()
                || !custody.view_arguments.is_empty()
                || !slice_subject_carrier(program, machine, entry, *subject, parameter)
            {
                return None;
            }
            return Some(Self::for_subject(
                *subject,
                argument_position,
                RankOrder::SliceLength,
                RankingRangeMeasure::SliceLength(*subject),
                custody.rank_range.unwrap_or_default(),
            ));
        }
        if matches!(
            witness.ranking_view,
            RankingViewId::NAT_DESCENDING | RankingViewId::NAT_INCREASING_TO
        ) && Some(witness.view_path.as_str()) == witness.ranking_view.canonical_path()
        {
            let increasing = witness.ranking_view == RankingViewId::NAT_INCREASING_TO;
            let mut limit = ExpressionHandle::invalid();
            if increasing {
                let [bound] = custody.view_arguments.as_slice() else {
                    return None;
                };
                if witness.view_arguments.len() != 1 || !bound.is_valid() {
                    return None;
                }
                limit = *bound;
            } else if !witness.view_arguments.is_empty() || !custody.view_arguments.is_empty() {
                return None;
            }
            let primitive = integer_subject_carrier(program, machine, entry, *subject, parameter)?;
            return Some(Self::for_subject(
                *subject,
                argument_position,
                if increasing {
                    RankOrder::IncreasingTo(primitive)
                } else {
                    RankOrder::Natural(primitive)
                },
                if increasing {
                    RankingRangeMeasure::IncreasingTo {
                        subject: *subject,
                        limit,
                    }
                } else {
                    RankingRangeMeasure::Single(*subject)
                },
                custody.rank_range.unwrap_or_default(),
            ));
        }
        if witness.ranking_view.is_valid()
            || !witness.view_arguments.is_empty()
            || !custody.view_arguments.is_empty()
        {
            return None;
        }
        // A declared scalar view produces the subject's own natural rank
        // (identity) or its body over the subject (computation). The
        // classification is validation's, shared with the checked stage, so
        // the same admission covers both readers; an optional authored range
        // then transports through the scalar range judgment.
        if let Some(view) = declared_scalar_view(program, entry, *subject, &witness.view_path) {
            let order = match view.computation {
                None => RankOrder::DeclaredIdentity {
                    measure: view.measure,
                    primitive: integer_subject_carrier(
                        program, machine, entry, *subject, parameter,
                    )?,
                },
                Some(computation) => RankOrder::DeclaredComputation {
                    measure: view.measure,
                    input_type: view.input_type,
                    result_primitive: PrimitiveType::from_builtin_atom(view.result_carrier)?,
                    computation,
                },
            };
            let measure = match view.computation {
                None => RankingRangeMeasure::Single(*subject),
                Some(computation) => RankingRangeMeasure::Computed {
                    subject: *subject,
                    parameter: computation.parameter,
                    body: computation.body,
                    carrier: view.result_carrier,
                },
            };
            return Some(Self::for_subject(
                *subject,
                argument_position,
                order,
                measure,
                custody.rank_range.unwrap_or_default(),
            ));
        }
        if custody.rank_range.is_some() || member_subject(program, *subject) {
            return None;
        }
        let TypeReferenceNode::Named { symbol: data, .. } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        if !data.is_valid() {
            return None;
        }
        let definition =
            crate::declarations::data_queries::data_definition_by_symbol(program, *data)?;

        // The normalized view path is the existing measure-selection carrier.
        // Resolve it once, reject ambiguity, then use only this occurrence and
        // exact field declarations for rank equality/decrease comparisons.
        let mut measures = program
            .measures()
            .iter()
            .enumerate()
            .filter(|(_, measure)| {
                program
                    .measure_path_members(measure.name)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(witness.view_path.split("::"))
            });
        let (measure_index, measure) = measures.next()?;
        if measures.next().is_some() || !measure.lexicographic || measure.parameter.is_some() {
            return None;
        }
        let (owner, _) = witness.view_path.rsplit_once("::")?;
        if owner != program.symbols.display_path(*data, "::") {
            return None;
        }

        let components = expression_table(program).expression_handles(measure.body);
        if components.is_empty() {
            return None;
        }
        let mut fields = Vec::with_capacity(components.len());
        for component in components {
            // Lexicographic syntax declares unqualified projections in its
            // exact owner. No arbitrary member expression or global name
            // search can contribute a ranking component.
            let Some(component) = expression_table(program).name_path_node(*component) else {
                return None;
            };
            let [name] = expression_table(program).name_path_members(component.members) else {
                return None;
            };
            let mut declarations = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) if field.name.as_str() == name.as_str() => Some(field),
                    _ => None,
                });
            let field = declarations.next()?;
            if declarations.next().is_some()
                || !field.symbol.is_valid()
                || fields.contains(&field.symbol)
                || (component.symbol.is_valid() && component.symbol != field.symbol)
                || !program
                    .primitive_type_reference(field.type_reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
            {
                return None;
            }
            fields.push(field.symbol);
        }
        Some(Self {
            order: RankOrder::Lexicographic {
                measure_index,
                data: *data,
                fields,
            },
            argument_position,
            subject: *subject,
            paired_subject: ExpressionHandle::invalid(),
            range: ExpressionHandle::invalid(),
            measure: None,
        })
    }

    /// The shared single-subject projection shape every declared rank view
    /// fills: `order`/`measure` vary per view; the subject's carrier formal
    /// also names the record when the subject is a member chain.
    fn for_subject(
        subject: ExpressionHandle,
        argument_position: usize,
        order: RankOrder,
        measure: RankingRangeMeasure,
        range: ExpressionHandle,
    ) -> Self {
        Self {
            order,
            argument_position,
            subject,
            paired_subject: ExpressionHandle::invalid(),
            range,
            measure: Some(measure),
        }
    }

    pub(super) fn captured_components(
        &self,
        program: &dyn crate::CheckingInput,
    ) -> Option<Vec<crate::proof_contracts::contract_entailment::CapturedRankComponent>> {
        use crate::proof_contracts::contract_entailment::CapturedRankComponent;
        if let RankOrder::Lexicographic { data, fields, .. } = &self.order {
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *data)?;
            return fields
                .iter()
                .map(|symbol| {
                    let field =
                        program.data_members(definition).iter().find_map(
                            |member| match member {
                                DataMember::Field(field) if field.symbol == *symbol => Some(field),
                                _ => None,
                            },
                        )?;
                    let primitive = program.primitive_type_reference(field.type_reference)?;
                    Some(CapturedRankComponent {
                        subject: self.subject,
                        projection: vec![facts::PlaceSegment::Field { symbol: *symbol }],
                        primitive: Some(primitive),
                        minimum: numerics::bignum::BigInt::from_i128(primitive.integer_bounds()?.0),
                        measure: SymbolHandle::invalid(),
                    })
                })
                .collect();
        }
        let (primitive, result_primitive, measure) = match self.order {
            RankOrder::Natural(primitive) => (Some(primitive), primitive, SymbolHandle::invalid()),
            RankOrder::DeclaredIdentity { measure, primitive } => {
                (Some(primitive), primitive, measure)
            }
            RankOrder::DeclaredComputation {
                measure,
                input_type,
                result_primitive,
                ..
            } => (
                program.primitive_type_reference(input_type),
                result_primitive,
                measure,
            ),
            _ => return None,
        };
        Some(vec![CapturedRankComponent {
            subject: self.subject,
            projection: Vec::new(),
            primitive,
            minimum: if measure.is_valid() {
                numerics::bignum::BigInt::from_i128(result_primitive.integer_bounds()?.0)
            } else {
                numerics::bignum::BigInt::zero()
            },
            measure,
        }])
    }

    pub(super) fn same_order(&self, other: &Self) -> bool {
        self.order == other.order
    }
}

/// Resolve a retained ranked-subject expression to the unique non-self entry
/// parameter its projection is rooted at. A bare subject names its own
/// formal; a member-chain subject names the formal whose record the chain
/// starts at, with each hop's symbol validity checked against the declaration
/// the per-order leaf checks then judge. An ambiguous or non-name arrival
/// has no exact argument mapping.
fn entry_parameter<'program>(
    program: &'program dyn crate::CheckingInput,
    entry: &'program State,
    subject: ExpressionHandle,
) -> Option<(usize, &'program StateParameter)> {
    // This is requirement discovery, not a load judgment. The observation
    // owner independently proves selectors, access and value identity before
    // a coordinate can become runtime evidence.
    let place = crate::canonical_place_from_expression_in_state(program, entry.symbol, 0, subject)?;
    let symbol = place.root.as_symbol()?;
    let mut parameters = program
        .state_parameters(entry)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.symbol == symbol);
    let found = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    Some(found)
}

/// Whether the retained subject reads projected storage: a member chain
/// rooted at an entry formal rather than a bare formal name.
fn member_subject(program: &dyn crate::CheckingInput, subject: ExpressionHandle) -> bool {
    matches!(
        expression_table(program).expression(unwrapped(program, subject)),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    )
}

/// The slice carrier under any reference or constrained shells on the ranked
/// leaf. A projected subject judges its own declared leaf; a bare subject
/// judges its formal's carrier.
fn slice_subject_carrier(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    subject: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    let reference = if member_subject(program, subject) {
        let Some(reference) =
            crate::expression_result_type_reference(program, machine, entry, subject)
        else {
            return false;
        };
        reference
    } else {
        parameter.type_reference
    };
    crate::value_custody::type_shapes::has_slice_base(program, reference)
}

/// The exact integer storage carrier of the selected subject. Signed storage
/// does not make a natural rank invalid: source admission separately proves
/// nonnegative formation and descent. Unsigned native projection is narrower
/// and must not define the source admission language.
fn integer_subject_carrier(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    subject: ExpressionHandle,
    parameter: &StateParameter,
) -> Option<PrimitiveType> {
    let reference = if member_subject(program, subject) {
        crate::expression_result_type_reference(program, machine, entry, subject)?
    } else {
        parameter.type_reference
    };
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    crate::value_custody::recasts::exact_primitive_type(program, reference)
        .filter(|primitive| primitive.is_fixed_integer())
}

pub(super) use crate::expression_traversal::unwrapped;

/// Publish only coordinates already selected by the component's filtered
/// telescope. Unsupported ranking orders keep their ordinary admission but
/// do not acquire an unsigned scalar projection.
pub(super) fn natural_component(
    program: &dyn crate::CheckingInput,
    component: &[usize],
    ranks: &[RankProjection],
    mappings: &[Vec<crate::RankStateBindings>],
    relevance: &super::super::RankRelevance,
) -> Option<super::ValidatedRuntimeNaturalComponent> {
    let mut states = Vec::new();
    for ((machine_position, rank), mappings) in component.iter().zip(ranks).zip(mappings) {
        let inputs = match &rank.order {
            RankOrder::Natural(primitive) => {
                let coordinates = crate::ranking_range_required_coordinates(
                    program,
                    &program.machines()[*machine_position],
                    rank.range,
                    rank.measure?,
                    crate::RankingRangePremises::RankInvariant,
                )?;
                let [coordinate] = coordinates.as_slice() else {
                    return None;
                };
                vec![(coordinate.clone(), *primitive)]
            }
            RankOrder::Lexicographic { .. } => rank
                .captured_components(program)?
                .into_iter()
                .map(|component| {
                    let mut place =
                        crate::canonical_place_from_expression(program, component.subject)?;
                    place.segments.extend(component.projection);
                    Some((
                        crate::RankSubjectCoordinate {
                            parameter: place.root.as_symbol()?,
                            path: place.segments,
                        },
                        component.primitive?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        for (state, roles) in program
            .machine_states(&program.machines()[*machine_position])
            .iter()
            .zip(mappings)
        {
            if !relevance.requires_rank(state.symbol) {
                continue;
            }
            let values = inputs
                .iter()
                .map(|(entry, primitive_type)| {
                    let coordinate = roles.destination(entry)?;
                    let (position, parameter) = program
                        .state_parameters(state)
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.symbol == coordinate.parameter)?;
                    let reference = crate::ranking_coordinate_type(program, state, &coordinate)?;
                    if crate::value_custody::recasts::exact_primitive_type(program, reference)
                        != Some(*primitive_type)
                    {
                        return None;
                    }
                    Some(super::ValidatedRuntimeIntegerRank {
                        application: None,
                        application_bindings: Vec::new(),
                        input_type: reference,
                        measure: SymbolHandle::invalid(),
                        parameter: parameter.symbol,
                        coordinate,
                        entry_indexes: Vec::new(),
                        parameter_position: u32::try_from(position).ok()?,
                        primitive_type: *primitive_type,
                        observation: crate::RankPremiseObservation::Value.into(),
                        function: checked_trees::CheckedNaturalRankFunction::Identity,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            states.push(super::ValidatedRuntimeNaturalState {
                state: state.symbol,
                values,
            });
        }
    }
    Some(super::ValidatedRuntimeNaturalComponent {
        states,
        edges: Vec::new(),
    })
}

/// A subordinate-state hop may rename or decrease its rank formal. Retain its
/// exact source-admitted comparison rather than recognizing the actual's shape
/// again. Local descent contributes to complete state-and-call cycles.
pub(super) fn retain_internal_transfer(
    program: &dyn crate::CheckingInput,
    component: &mut super::ValidatedRuntimeNaturalComponent,
    component_states: &[SymbolHandle],
    relevance: &super::super::RankRelevance,
    source: SymbolHandle,
    target: SymbolHandle,
    target_handle: typed_trees::statement::TransitionTargetHandle,
    statement_ordinal: usize,
    judgments: &[checked_trees::CheckedRankArrivalEdge],
) -> bool {
    let target = program
        .machine(target)
        .and_then(|machine| program.machine_states(machine).first())
        .map_or(target, |state| state.symbol);
    if !relevance.requires_rank(target) {
        return true;
    }
    if !component_states.contains(&source) || !component_states.contains(&target) {
        return false;
    }
    let mut matching = judgments.iter().filter(|edge| {
        edge.source == source
            && edge.destination == target
            && edge.target == target_handle
            && usize::try_from(edge.statement_ordinal).ok() == Some(statement_ordinal)
    });
    let Some(edge) = matching.next() else {
        return false;
    };
    if matching.next().is_some() || !edge.non_increasing {
        return false;
    }
    component.edges.push(super::ValidatedRuntimeNaturalEdge {
        source,
        target,
        statement_ordinal: edge.statement_ordinal,
        strict: edge.strictly_decreases,
    });
    true
}
