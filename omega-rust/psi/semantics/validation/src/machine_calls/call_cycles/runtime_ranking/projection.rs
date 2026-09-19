use crate::proof_contracts::contract_entailment::{
    MeasureBodyShape, RankingRangeMeasure, ScalarViewComputation, declared_scalar_view,
    find_declared_measure, measure_body_shape, unwrap_constraint_shells,
};
use language_semantics::RankingViewId;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
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
    /// A declared measure whose `+`/`*` body validation's `declared_scalar_view`
    /// admitted as strictly increasing on the naturals with builtin meaning --
    /// the only reason the subject's descent stands for the produced rank's
    /// descent. The rank is the body over the subject, judged by the scalar
    /// range call judgment, which also proves its formation in the carrier.
    /// Members share this order only through the same measure and carrier.
    DeclaredComputation {
        measure: SymbolHandle,
        primitive: PrimitiveType,
        computation: ScalarViewComputation,
    },
    /// A declared measure whose body projects a `u64` field of its record
    /// parameter (`measure_body_shape`'s `FieldProjection`). The produced rank
    /// is that exact coordinate of the subject's record, reached through at
    /// most one reference boundary. Members share this order only through the
    /// same measure; each call edge re-resolves the chain against the exact
    /// formals' declarations and walks the actual's forward or literal.
    CustomStructView {
        measure: SymbolHandle,
    },
    Lexicographic {
        measure_index: usize,
        data: SymbolHandle,
        fields: Vec<SymbolHandle>,
    },
}

pub(super) struct RankProjection {
    pub(super) order: RankOrder,
    /// The entry formal the produced rank reads: the subject formal for a
    /// bare subject, or the carrier formal a member-chain subject's
    /// coordinate is rooted at.
    pub(super) parameter: SymbolHandle,
    /// The paired subject's entry parameter for a two-subject view. Invalid
    /// for single-subject orders.
    pub(super) paired_parameter: SymbolHandle,
    /// The record formal whose projected storage produces the rank: the
    /// subject formal of a field view, or the carrier formal of a
    /// member-chain subject. Invalid when the rank reads no record.
    pub(super) record_subject: SymbolHandle,
    /// The record formal whose projected storage produces the paired upper
    /// subject of a two-subject view. Invalid otherwise.
    pub(super) paired_record_subject: SymbolHandle,
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
    pub(super) fn resolve(program: &TypedTrees, machine: &Machine) -> Option<Self> {
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
            let (_, upper_parameter) = entry_parameter(program, entry, *upper)?;
            // A shared unsigned carrier keeps `same_order` meaningful across
            // members; a mixed-width distance is not a single order. A
            // member-chain subject's leaf, not its record formal's type,
            // names the carrier.
            let primitive = unsigned_subject_carrier(program, machine, entry, *lower, parameter)?;
            if unsigned_subject_carrier(program, machine, entry, *upper, upper_parameter)?
                != primitive
            {
                return None;
            }
            return Some(Self {
                order: RankOrder::BoundedDistance(primitive),
                parameter: parameter.symbol,
                paired_parameter: upper_parameter.symbol,
                record_subject: if member_subject(program, *lower) {
                    parameter.symbol
                } else {
                    Default::default()
                },
                paired_record_subject: if member_subject(program, *upper) {
                    upper_parameter.symbol
                } else {
                    Default::default()
                },
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
            return Some(Self {
                order: RankOrder::SliceLength,
                parameter: parameter.symbol,
                paired_parameter: SymbolHandle::default(),
                record_subject: if member_subject(program, *subject) {
                    parameter.symbol
                } else {
                    Default::default()
                },
                paired_record_subject: SymbolHandle::default(),
                argument_position,
                subject: *subject,
                paired_subject: ExpressionHandle::invalid(),
                range: custody.rank_range.unwrap_or_default(),
                measure: Some(RankingRangeMeasure::SliceLength(*subject)),
            });
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
            let primitive = unsigned_subject_carrier(program, machine, entry, *subject, parameter)?;
            return Some(Self {
                order: if increasing {
                    RankOrder::IncreasingTo(primitive)
                } else {
                    RankOrder::Natural(primitive)
                },
                parameter: parameter.symbol,
                paired_parameter: SymbolHandle::default(),
                record_subject: if member_subject(program, *subject) {
                    parameter.symbol
                } else {
                    Default::default()
                },
                paired_record_subject: SymbolHandle::default(),
                argument_position,
                subject: *subject,
                paired_subject: ExpressionHandle::invalid(),
                range: custody.rank_range.unwrap_or_default(),
                measure: Some(if increasing {
                    RankingRangeMeasure::IncreasingTo {
                        subject: *subject,
                        limit,
                    }
                } else {
                    RankingRangeMeasure::Single(*subject)
                }),
            });
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
            let primitive = unsigned_subject_carrier(program, machine, entry, *subject, parameter)?;
            let order = match view.computation {
                None => RankOrder::DeclaredIdentity {
                    measure: view.measure,
                    primitive,
                },
                Some(computation) => RankOrder::DeclaredComputation {
                    measure: view.measure,
                    primitive,
                    computation,
                },
            };
            let measure = match view.computation {
                None => RankingRangeMeasure::Single(*subject),
                Some(computation) => RankingRangeMeasure::Computed {
                    subject: *subject,
                    parameter: computation.parameter,
                    body: computation.body,
                    carrier: view.carrier,
                },
            };
            return Some(Self {
                order,
                parameter: parameter.symbol,
                paired_parameter: SymbolHandle::default(),
                record_subject: if member_subject(program, *subject) {
                    parameter.symbol
                } else {
                    Default::default()
                },
                paired_record_subject: SymbolHandle::default(),
                argument_position,
                subject: *subject,
                paired_subject: ExpressionHandle::invalid(),
                range: custody.rank_range.unwrap_or_default(),
                measure: Some(measure),
            });
        }
        // A declared field view produces the exact `u64` projection its
        // measure body declares. The subject must be the entry formal whose
        // (possibly referenced) record the chain starts at; the call judgment
        // re-resolves that chain against each formal's own declaration, so
        // this selection never supplies the coordinate itself. A member-chain
        // subject already names projected storage; it cannot also be the
        // record a field view reads.
        if member_subject(program, *subject) {
            return None;
        }
        let declared_path = witness
            .view_path
            .split("::")
            .filter(|member| !member.is_empty())
            .collect::<Vec<_>>();
        if let Some(measure) = find_declared_measure(program, &declared_path)
            && !measure.lexicographic
            && let Some(MeasureBodyShape::FieldProjection { path, owner, .. }) =
                measure_body_shape(program, measure)
        {
            let root = path.first().map_or(owner, |step| step.owner);
            let mut reference = unwrap_constraint_shells(program, parameter.type_reference);
            if let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(reference)
            {
                reference = unwrap_constraint_shells(program, *referee);
            }
            if !matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. } if *symbol == root
            ) {
                return None;
            }
            return Some(Self {
                order: RankOrder::CustomStructView {
                    measure: measure.symbol,
                },
                parameter: parameter.symbol,
                paired_parameter: SymbolHandle::default(),
                record_subject: parameter.symbol,
                paired_record_subject: SymbolHandle::default(),
                argument_position,
                subject: *subject,
                paired_subject: ExpressionHandle::invalid(),
                range: custody.rank_range.unwrap_or_default(),
                measure: Some(RankingRangeMeasure::Field {
                    subject: *subject,
                    measure: measure.symbol,
                }),
            });
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
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *data)?;

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

        let components = program.expression_table.expression_handles(measure.body);
        if components.is_empty() {
            return None;
        }
        let mut fields = Vec::with_capacity(components.len());
        for component in components {
            // Lexicographic syntax declares unqualified projections in its
            // exact owner. No arbitrary member expression or global name
            // search can contribute a ranking component.
            let ExpressionNode::Name(component) = program.expression_table.expression(*component)
            else {
                return None;
            };
            let [name] = program
                .expression_table
                .name_path_members(component.members)
            else {
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
                || !matches!(
                    program
                        .type_reference_table
                        .primitive_type(field.type_reference),
                    Some(
                        PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                    )
                )
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
            parameter: parameter.symbol,
            paired_parameter: SymbolHandle::default(),
            record_subject: SymbolHandle::default(),
            paired_record_subject: SymbolHandle::default(),
            argument_position,
            subject: *subject,
            paired_subject: ExpressionHandle::invalid(),
            range: ExpressionHandle::invalid(),
            measure: None,
        })
    }

    pub(super) fn same_order(&self, other: &Self) -> bool {
        self.order == other.order
    }

    /// Whether the expression spells the retained ranked subject. `role`
    /// translates a spelled symbol to the entry parameter it carries at the
    /// issuing site -- a subordinate state's formal answers to its
    /// discovered role while every other symbol stands for itself, so an
    /// entry-state call reads the same identity mapping it always had.
    pub(super) fn is_subject(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        role: &dyn Fn(SymbolHandle) -> SymbolHandle,
    ) -> bool {
        matches!(program.expression_table.expression(unwrapped(program, expression)),
            ExpressionNode::Name(path) if role(path.symbol) == self.parameter)
    }

    pub(super) fn is_component(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        field: SymbolHandle,
        role: &dyn Fn(SymbolHandle) -> SymbolHandle,
    ) -> bool {
        let RankOrder::Lexicographic { data: owner, .. } = self.order else {
            return false;
        };
        let ExpressionNode::Member(member) = program
            .expression_table
            .expression(unwrapped(program, expression))
        else {
            return false;
        };
        if !self.is_subject(program, member.receiver, role) || member.case_variant.is_some() {
            return false;
        }
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner)
        else {
            return false;
        };
        // Ordinary typed member expressions can still lack their field
        // symbol. Resolve that one hop under the exact retained parameter's
        // nominal owner, using the same unique-member judgment as proof SCCs.
        super::super::exact_data_member_field(
            program,
            data,
            member.member_symbol,
            member.member.as_str(),
            None,
        )
        .is_some_and(|selected| selected.symbol == field)
    }
}

/// Resolve a retained ranked-subject expression to the unique non-self entry
/// parameter its projection is rooted at. A bare subject names its own
/// formal; a member-chain subject names the formal whose record the chain
/// starts at, with each hop's symbol validity checked against the declaration
/// the per-order leaf checks then judge. An ambiguous or non-name arrival
/// has no exact argument mapping.
fn entry_parameter<'program>(
    program: &'program TypedTrees,
    entry: &'program State,
    subject: ExpressionHandle,
) -> Option<(usize, &'program StateParameter)> {
    let mut cursor = unwrapped(program, subject);
    let mut visited = Vec::new();
    while let ExpressionNode::Member(member) = program.expression_table.expression(cursor) {
        if !member.member_symbol.is_valid()
            || member.case_variant.is_some()
            || visited.contains(&cursor)
            || visited.len() >= 128
        {
            return None;
        }
        visited.push(cursor);
        cursor = unwrapped(program, member.receiver);
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(cursor) else {
        return None;
    };
    if !path.symbol.is_valid() {
        return None;
    }
    let mut parameters = program
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .filter(|(_, parameter)| parameter.symbol == path.symbol);
    let found = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    Some(found)
}

/// Whether the retained subject reads projected storage: a member chain
/// rooted at an entry formal rather than a bare formal name.
fn member_subject(program: &TypedTrees, subject: ExpressionHandle) -> bool {
    matches!(
        program
            .expression_table
            .expression(unwrapped(program, subject)),
        ExpressionNode::Member(_)
    )
}

/// The slice carrier under any reference or constrained shells on the ranked
/// leaf. A projected subject judges its own declared leaf; a bare subject
/// judges its formal's carrier.
fn slice_subject_carrier(
    program: &TypedTrees,
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
    slice_carrier_type(program, reference)
}

/// The unsigned primitive carrier under any constrained shells on the ranked
/// leaf. A projected subject's leaf -- not its record formal's type -- must
/// carry the exact unsigned base a natural-order projection requires.
fn unsigned_subject_carrier(
    program: &TypedTrees,
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
    unsigned_carrier_type(program, reference)
}

/// The slice carrier under any reference or constrained shells. A ranked
/// subject without an exact slice base cannot carry a length projection.
fn slice_carrier_type(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return false,
        }
    }
    false
}

/// The unsigned primitive carrier under any constrained shells. A ranked
/// subject or endpoint without an exact unsigned base cannot carry a
/// natural-order projection.
fn unsigned_carrier_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    let primitive = crate::value_custody::recasts::exact_primitive_type(program, reference)?;
    matches!(
        primitive,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
    )
    .then_some(primitive)
}

pub(super) fn unwrapped(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    while let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) {
        expression = atomic.value;
    }
    expression
}
