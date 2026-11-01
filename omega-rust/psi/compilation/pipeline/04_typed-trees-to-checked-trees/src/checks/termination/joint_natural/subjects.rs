//! A declared input is one typed root and its complete demanded read roster.
//! Selecting leaves independently could splice unrelated records into a rank.
//! Scalar builtins retain their scalar coordinate; records have no pretend
//! integer carrier, integer atom, or selected-field identity.

use super::{applications::Application, coordinates::Coordinate};
use crate::checks::current_scalar_values::CurrentScalarValues;
use typed_trees::{state::State, types::TypeReferenceHandle};
use validation::{RankSubjectCoordinate, ScopedArithmeticBinding, ScopedArithmeticValue};

#[derive(Clone)]
pub(super) enum Subject {
    Scalar(Coordinate),
    Record {
        root: RankSubjectCoordinate,
        position: usize,
        input_type: TypeReferenceHandle,
        reads: Vec<Coordinate>,
    },
}

impl Subject {
    pub(super) fn declared(
        program: &dyn validation::CheckingInput,
        state: &State,
        root: &RankSubjectCoordinate,
        application: Application,
        available: &[Coordinate],
        values: &CurrentScalarValues<'_>,
    ) -> Option<Self> {
        let parameters = program.state_parameters(state);
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == root.parameter)?;
        let input_type = validation::project_type_reference_from_segments(
            program,
            parameters[position].type_reference,
            &root.path,
        )?;
        if !validation::declared_input_type_matches(program, input_type, application.input_type) {
            return None;
        }
        if program
            .primitive_type_reference(application.input_type)
            .is_some()
        {
            let coordinate = available.iter().find(|coordinate| {
                coordinate.parameter == root.parameter
                    && coordinate.observation == validation::RankPremiseObservation::Value
                    && Some(coordinate.primitive)
                        == program.primitive_type_reference(application.input_type)
                    && values.same_scalar_projection(&coordinate.path, &root.path)
            })?;
            return Some(Self::Scalar(coordinate.clone()));
        }
        let shape = application.shape(program)?;
        let reads = shape
            .reads
            .iter()
            .map(|read| {
                let path = root
                    .path
                    .iter()
                    .chain(&read.coordinate.path)
                    .cloned()
                    .collect::<Vec<_>>();
                available
                    .iter()
                    .find(|coordinate| {
                        coordinate.parameter == root.parameter
                            && coordinate.primitive == read.primitive_type
                            && coordinate.observation == validation::RankPremiseObservation::Value
                            && values.same_scalar_projection(&coordinate.path, &path)
                    })
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self::Record {
            root: root.clone(),
            position,
            input_type,
            reads,
        })
    }

    pub(super) fn root(&self) -> RankSubjectCoordinate {
        match self {
            Self::Scalar(coordinate) => RankSubjectCoordinate {
                parameter: coordinate.parameter,
                path: coordinate.path.clone(),
            },
            Self::Record { root, .. } => root.clone(),
        }
    }

    pub(super) fn position(&self) -> usize {
        match self {
            Self::Scalar(coordinate) => coordinate.position,
            Self::Record { position, .. } => *position,
        }
    }

    pub(super) fn scalar(&self) -> Option<&Coordinate> {
        match self {
            Self::Scalar(coordinate) => Some(coordinate),
            Self::Record { .. } => None,
        }
    }

    pub(super) fn input_type(&self, _application: Application) -> TypeReferenceHandle {
        match self {
            Self::Scalar(coordinate) => coordinate.type_reference,
            Self::Record { input_type, .. } => *input_type,
        }
    }

    pub(super) fn retained_bindings(
        &self,
        program: &dyn validation::CheckingInput,
        application: Application,
    ) -> Option<Vec<validation::RuntimeRankApplicationBinding>> {
        let shape = application.shape(program)?;
        match self {
            Self::Scalar(coordinate) => Some(vec![validation::RuntimeRankApplicationBinding {
                formal: RankSubjectCoordinate {
                    parameter: application.parameter,
                    path: Vec::new(),
                },
                actual: self.root(),
                entry_indexes: coordinate.entry_indexes.clone(),
            }]),
            Self::Record { reads, .. } => {
                if reads.len() != shape.reads.len() {
                    return None;
                }
                Some(
                    shape
                        .reads
                        .iter()
                        .zip(reads)
                        .map(
                            |(read, coordinate)| validation::RuntimeRankApplicationBinding {
                                formal: read.coordinate.clone(),
                                actual: RankSubjectCoordinate {
                                    parameter: coordinate.parameter,
                                    path: coordinate.path.clone(),
                                },
                                entry_indexes: coordinate.entry_indexes.clone(),
                            },
                        )
                        .collect(),
                )
            }
        }
    }

    /// Read each destination coordinate at the completed argument cut. Entry
    /// captures are reused only for an entry, never reread after a write.
    pub(super) fn bindings(
        &self,
        program: &dyn validation::CheckingInput,
        application: Application,
        mut value: impl FnMut(&Coordinate) -> Option<ScopedArithmeticValue>,
    ) -> Option<(TypeReferenceHandle, Vec<ScopedArithmeticBinding>)> {
        use validation::ScopedArithmeticBinder as Binder;
        match self {
            Self::Scalar(coordinate) => {
                // Selection checked the actual scalar carrier. Formation
                // proves the formal qualification on this captured value.
                let input_type = coordinate.type_reference;
                Some((
                    input_type,
                    vec![ScopedArithmeticBinding {
                        binder: Binder::Symbol(application.parameter),
                        value: coordinate.bounded_value(value(coordinate)?)?,
                    }],
                ))
            }
            Self::Record {
                input_type, reads, ..
            } => {
                let shape = application.shape(program)?;
                if shape.reads.len() != reads.len() {
                    return None;
                }
                let mut bindings = Vec::new();
                for (read, coordinate) in shape.reads.iter().zip(reads) {
                    let captured = coordinate.bounded_value(value(coordinate)?)?;
                    bindings.extend(read.occurrences.iter().map(|source| {
                        ScopedArithmeticBinding {
                            binder: Binder::Occurrence(*source),
                            value: captured.clone(),
                        }
                    }));
                }
                Some((*input_type, bindings))
            }
        }
    }
}
