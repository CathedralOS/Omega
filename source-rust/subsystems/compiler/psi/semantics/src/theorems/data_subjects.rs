//! A logical contract's subjects are fixed scalars. A plain record input is
//! one subject for each of its scalar fields, in its declared field order,
//! and a clause's field of that input names that subject.

use crate::substitution::clauses::{ClauseBindings, substitute_clause};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralCaseId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BoundaryParameterKind, MachineContract, StructuralFieldType, StructuralTypeShape,
    TerminalModule, ValueDeclaration,
};

/// One source input of a logical contract, in its header position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSubject {
    /// A scalar formal, named in clauses by `id`.
    Scalar {
        id: ValueId,
        scalar_type: ScalarType,
    },
    /// A plain record formal, named in clauses by its fields of `place`.
    /// Each leaf is one field's path and its scalar carrier.
    Record {
        place: PlaceId,
        leaves: Vec<(Vec<CanonicalStructuralPathSegment>, ScalarType)>,
    },
}

/// The fixed subjects of `sources`, numbered from one, and each clause
/// restated over them.
pub fn flatten(
    sources: &[SourceSubject],
    requires: &[Proposition],
    ensures: &[Proposition],
) -> Result<(Vec<ValueDeclaration>, Vec<Proposition>, Vec<Proposition>), &'static str> {
    let mut parameters = Vec::new();
    for source in sources {
        let carriers: Vec<_> = match source {
            SourceSubject::Scalar { scalar_type, .. } => vec![*scalar_type],
            SourceSubject::Record { leaves, .. } => {
                leaves.iter().map(|(_, scalar)| *scalar).collect()
            }
        };
        for scalar_type in carriers {
            let id = ValueId::new(parameters.len() as u64 + 1)
                .ok_or("logical subject identity overflow")?;
            parameters.push(ValueDeclaration {
                id,
                scalar_type,
                qualifications: Default::default(),
            });
        }
    }
    let mut bindings = Renamed { sources };
    let mut restate = |clauses: &[Proposition]| {
        clauses
            .iter()
            .map(|clause| substitute_clause(clause, &mut bindings))
            .collect::<Result<Vec<_>, &'static str>>()
    };
    let requires = restate(requires)?;
    let ensures = restate(ensures)?;
    Ok((parameters, requires, ensures))
}

/// The scalar fields of a plain record, in declared order. A record with a
/// data invariant, a field that is not a plain relevant scalar, or any other
/// shape is refused by name.
pub fn record_leaves(
    module: &TerminalModule,
    record: StructuralTypeId,
) -> Result<Vec<(Vec<CanonicalStructuralPathSegment>, ScalarType)>, &'static str> {
    let declaration = module
        .structural_type(record)
        .ok_or("logical data subject has no structural declaration")?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("logical data subject is not a plain record");
    };
    if !declaration.requires.is_empty()
        || declaration.quotient_carrier.is_some()
        || declaration.scalar_restriction.is_some()
    {
        return Err("logical data subject has a data invariant");
    }
    fields
        .iter()
        .map(|field| match field.field_type {
            StructuralFieldType::Scalar(scalar) if !field.relevance.is_erased() => Ok((
                vec![CanonicalStructuralPathSegment::Field(field.id)],
                scalar,
            )),
            StructuralFieldType::Structural(_) => {
                Err("logical data subject has a field that is itself data")
            }
            _ => Err("logical data subject has a field that is not a plain scalar"),
        })
        .collect()
}

/// The source inputs of a machine's contract, in header order.
pub fn machine_subjects(
    module: &TerminalModule,
    contract: &MachineContract,
) -> Result<Vec<SourceSubject>, &'static str> {
    contract.validate_parameter_order()?;
    let mut scalars = contract.parameters.iter();
    let mut records = contract.structural_parameters.iter();
    contract
        .parameter_order
        .iter()
        .map(|kind| match kind {
            BoundaryParameterKind::Scalar => {
                let formal = scalars
                    .next()
                    .ok_or("machine runtime parameter order does not cover its signature")?;
                if formal.qualifications != semantic_vocabulary::ScalarQualificationSetId::ZERO {
                    return Err("logical subject carries a qualification");
                }
                Ok(SourceSubject::Scalar {
                    id: formal.id,
                    scalar_type: formal.scalar_type,
                })
            }
            BoundaryParameterKind::Structural => {
                let formal = records
                    .next()
                    .ok_or("machine runtime parameter order does not cover its signature")?;
                if !formal.qualifications.is_empty()
                    || !formal.projected_qualifications.is_empty()
                    || !formal.additional_input_qualifications.is_empty()
                {
                    return Err("logical data subject carries a qualification");
                }
                Ok(SourceSubject::Record {
                    place: formal.place,
                    leaves: record_leaves(module, formal.structural_type)?,
                })
            }
        })
        .collect()
}

struct Renamed<'a> {
    sources: &'a [SourceSubject],
}

impl Renamed<'_> {
    fn subject(&self, found: impl Fn(&SourceSubject) -> Option<usize>) -> Option<ValueId> {
        let mut next = 1u64;
        for source in self.sources {
            if let Some(offset) = found(source) {
                return ValueId::new(next + offset as u64);
            }
            next += match source {
                SourceSubject::Scalar { .. } => 1,
                SourceSubject::Record { leaves, .. } => leaves.len() as u64,
            };
        }
        None
    }
}

impl ClauseBindings for Renamed<'_> {
    type Error = &'static str;

    fn value(&mut self, id: ValueId, scalar: ScalarType) -> Result<ScalarTerm, Self::Error> {
        let renamed = self
            .subject(|source| {
                matches!(source, SourceSubject::Scalar { id: own, scalar_type }
                    if *own == id && *scalar_type == scalar)
                .then_some(0)
            })
            .ok_or("logical clause names another subject or carrier")?;
        Ok(ScalarTerm::Value {
            id: renamed,
            scalar_type: scalar,
        })
    }

    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        if version != ContentPlaceVersion::Current {
            return Err("logical clause observes a data subject at another version");
        }
        let renamed = self
            .subject(|source| match source {
                SourceSubject::Record { place, leaves } if *place == root => leaves
                    .iter()
                    .position(|(own, carrier)| own.as_slice() == path && *carrier == scalar),
                _ => None,
            })
            .ok_or("logical clause names a field that is no scalar subject")?;
        Ok(ScalarTerm::Value {
            id: renamed,
            scalar_type: scalar,
        })
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<crate::substitution::clauses::CaseAnswer, Self::Error> {
        Err("logical clause tests a case of a data subject")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralFieldId};

    fn byte() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
    }

    fn field(place: u64, id: u64) -> ScalarTerm {
        ScalarTerm::IntegerField {
            version: ContentPlaceVersion::Current,
            root: PlaceId::new(place).unwrap(),
            path: vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(id).unwrap(),
            )],
            scalar_type: byte(),
        }
    }

    fn one() -> ScalarTerm {
        ScalarTerm::Integer {
            scalar_type: byte(),
            value: IntegerValue::Unsigned(1),
        }
    }

    /// `(n: u8, p: Pair)`: `n` is subject 1 and `p.x`, `p.y` are 2 and 3.
    fn sources() -> Vec<SourceSubject> {
        let leaf = |id| {
            (
                vec![CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(id).unwrap(),
                )],
                ScalarType::Integer(byte()),
            )
        };
        vec![
            SourceSubject::Scalar {
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(byte()),
            },
            SourceSubject::Record {
                place: PlaceId::new(1).unwrap(),
                leaves: vec![leaf(1), leaf(2)],
            },
        ]
    }

    #[test]
    fn a_record_field_names_its_own_subject() {
        let clause = Proposition::Equal(field(1, 2), one());
        let (parameters, requires, _) = flatten(&sources(), &[clause], &[]).unwrap();
        assert_eq!(parameters.len(), 3);
        assert_eq!(
            requires,
            [Proposition::Equal(
                ScalarTerm::Value {
                    id: ValueId::new(3).unwrap(),
                    scalar_type: ScalarType::Integer(byte()),
                },
                one(),
            )]
        );
    }

    #[test]
    fn a_field_of_another_place_names_no_subject() {
        let clause = Proposition::Equal(field(2, 1), one());
        assert_eq!(
            flatten(&sources(), &[clause], &[]),
            Err("logical clause names a field that is no scalar subject")
        );
    }
}
