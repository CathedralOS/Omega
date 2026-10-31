//! The selected natural ordering and its executable normalization.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Order {
    Descending,
    BoundedDistance,
    IncreasingTo,
}

impl Order {
    pub(super) fn function(self) -> checked_trees::CheckedNaturalRankFunction {
        match self {
            Self::Descending => checked_trees::CheckedNaturalRankFunction::Identity,
            Self::BoundedDistance | Self::IncreasingTo => {
                checked_trees::CheckedNaturalRankFunction::Complement
            }
        }
    }
}

/// Source selection and produced numeric observation are separate. A declared
/// identity or field projection may normalize to its exact value coordinate;
/// selecting it still retains the declaration's identity through admission.
#[derive(Clone, Copy)]
pub(super) struct Application {
    pub(super) measure: symbols::SymbolHandle,
    pub(super) parameter: symbols::SymbolHandle,
    pub(super) body: typed_trees::expression::ExpressionHandle,
    pub(super) primitive: typed_trees::types::PrimitiveType,
}

impl Application {
    pub(super) fn expression(
        self,
        input: validation::ScopedArithmeticValue,
    ) -> validation::ScopedArithmeticExpression {
        validation::ScopedArithmeticExpression {
            owner: self.measure,
            expression: self.body,
            bindings: vec![validation::ScopedArithmeticBinding {
                binder: validation::ScopedArithmeticBinder::Symbol(self.parameter),
                value: input,
            }],
        }
    }
}

pub(super) struct SelectedView {
    pub(super) application: Option<Application>,
    pub(super) order: Order,
    pub(super) declaration: Option<symbols::SymbolHandle>,
    pub(super) observation: validation::RankPremiseObservation,
    pub(super) coordinate: validation::RankSubjectCoordinate,
    pub(super) upper: Option<typed_trees::expression::ExpressionHandle>,
    pub(super) measure: validation::RankingRangeMeasure,
}

pub(super) fn select(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    entry: &typed_trees::state::State,
) -> Option<SelectedView> {
    use crate::checks::termination::order::{OrderResolution, RankingOrder};
    use validation::{RankPremiseObservation, RankingRangeMeasure};

    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    let subjects = typed_trees::ranking::resolve_machine_witness_subjects(program, machine)?;
    let arguments = typed_trees::ranking::resolve_machine_witness_view_arguments(program, machine)?;
    let OrderResolution::Resolved(selected) =
        RankingOrder::resolve_witness(program, entry, witness, &subjects, &arguments)
    else {
        return None;
    };
    let subject = *subjects.first()?;
    let mut place =
        validation::canonical_place_from_expression_in_state(program, entry.symbol, 0, subject)?;
    let mut application = None;
    let (order, declaration, observation, upper, measure) = match selected {
        RankingOrder::NatDescending if subjects.len() == 1 => (
            Order::Descending,
            None,
            RankPremiseObservation::Value,
            None,
            RankingRangeMeasure::Single(subject),
        ),
        RankingOrder::SliceLength if subjects.len() == 1 => (
            Order::Descending,
            None,
            RankPremiseObservation::Length,
            None,
            RankingRangeMeasure::SliceLength(subject),
        ),
        RankingOrder::BoundedDistance if subjects.len() == 2 => (
            Order::BoundedDistance,
            None,
            RankPremiseObservation::Value,
            Some(subjects[1]),
            RankingRangeMeasure::Distance {
                lower: subject,
                upper: subjects[1],
            },
        ),
        RankingOrder::IncreasingTo(upper) if subjects.len() == 1 => (
            Order::IncreasingTo,
            None,
            RankPremiseObservation::Value,
            Some(upper),
            RankingRangeMeasure::IncreasingTo {
                subject,
                limit: upper,
            },
        ),
        RankingOrder::CustomScalarView {
            measure,
            parameter,
            body,
            carrier,
        } if subjects.len() == 1 => {
            application = Some(Application {
                measure,
                parameter,
                body,
                primitive: typed_trees::types::PrimitiveType::from_builtin_atom(carrier)?,
            });
            (
                Order::Descending,
                Some(measure),
                RankPremiseObservation::Value,
                None,
                RankingRangeMeasure::Computed {
                    subject,
                    parameter,
                    body,
                    carrier,
                },
            )
        }
        RankingOrder::CustomNatDescending { measure } if subjects.len() == 1 => (
            Order::Descending,
            Some(measure),
            RankPremiseObservation::Value,
            None,
            RankingRangeMeasure::Single(subject),
        ),
        RankingOrder::CustomStructView {
            measure,
            path,
            field_symbol,
            ..
        } if subjects.len() == 1 => {
            // These identities come from the validated measure body. Compose
            // its produced coordinate with the actual subject; spelling and
            // another same-typed field supply no observation.
            place
                .segments
                .extend(path.iter().map(|step| facts::PlaceSegment::Field {
                    symbol: step.field_symbol,
                }));
            place.segments.push(facts::PlaceSegment::Field {
                symbol: field_symbol,
            });
            validation::structural_scalar_coordinate(
                program,
                program.state_parameters(entry),
                place.root.as_symbol()?,
                &place.segments,
            )?;
            (
                Order::Descending,
                Some(measure),
                RankPremiseObservation::Value,
                None,
                RankingRangeMeasure::Field { subject, measure },
            )
        }
        // Other view families retain their ordinary formation/comparison owner.
        // A computation above keeps its application; its input is not the rank.
        _ => return None,
    };
    Some(SelectedView {
        application,
        order,
        declaration,
        observation,
        coordinate: validation::RankSubjectCoordinate {
            parameter: place.root.as_symbol()?,
            path: place.segments,
        },
        upper,
        measure,
    })
}
