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
        /// The data invariants of the record and of each record nested in
        /// it: a clause over `self` (place 1) and the path of the record it
        /// constrains. Every value of the type holds them, so each is a
        /// premise of the contract over these subjects.
        invariants: Vec<(Vec<CanonicalStructuralPathSegment>, Proposition)>,
    },
}

/// A plain record's scalar leaves and data invariants (`SourceSubject::Record`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSubject {
    pub leaves: Vec<(Vec<CanonicalStructuralPathSegment>, ScalarType)>,
    pub invariants: Vec<(Vec<CanonicalStructuralPathSegment>, Proposition)>,
}

/// The place a data invariant names the value it constrains by.
pub fn invariant_self() -> PlaceId {
    PlaceId::new(1).expect("place one is a valid place")
}

/// The fixed subjects of `sources`, numbered from one, and each clause
/// restated over them. Each record's data invariants come first among the
/// premises, in header order.
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
    let mut premises = Vec::new();
    for source in sources {
        let SourceSubject::Record {
            place, invariants, ..
        } = source
        else {
            continue;
        };
        for (prefix, clause) in invariants {
            premises.push(substitute_clause(
                clause,
                &mut Invariant {
                    renamed: &bindings,
                    place: *place,
                    prefix,
                },
            )?);
        }
    }
    let mut restate = |clauses: &[Proposition]| {
        clauses
            .iter()
            .map(|clause| substitute_clause(clause, &mut bindings))
            .collect::<Result<Vec<_>, &'static str>>()
    };
    premises.extend(restate(requires)?);
    let ensures = restate(ensures)?;
    Ok((parameters, premises, ensures))
}

/// The scalar fields of a plain record, in declared order, with the fields
/// of a nested plain record in its place, and the data invariants of the
/// record and of each nested record. A quotient or restricted scalar, a
/// field that is neither a plain relevant scalar nor a plain record, or any
/// other shape is refused by name.
pub fn record_subject(
    module: &TerminalModule,
    record: StructuralTypeId,
) -> Result<RecordSubject, &'static str> {
    let declaration = module
        .structural_type(record)
        .ok_or("logical data subject has no structural declaration")?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("logical data subject is not a plain record");
    };
    if declaration.quotient_carrier.is_some() || declaration.scalar_restriction.is_some() {
        return Err("logical data subject is a quotient or a restricted scalar");
    }
    let mut subject = RecordSubject {
        leaves: Vec::new(),
        invariants: declaration
            .requires
            .iter()
            .map(|clause| (Vec::new(), clause.clone()))
            .collect(),
    };
    for field in fields {
        if field.relevance.is_erased() {
            return Err("logical data subject has a field that is not a plain scalar");
        }
        let at = CanonicalStructuralPathSegment::Field(field.id);
        match field.field_type {
            StructuralFieldType::Scalar(scalar) => subject.leaves.push((vec![at], scalar)),
            // A field that is itself a plain record contributes its own
            // scalar fields and invariants in place, under this field's path.
            StructuralFieldType::Structural(child) => {
                let nested = record_subject(module, child)?;
                for (mut path, scalar) in nested.leaves {
                    path.insert(0, at);
                    subject.leaves.push((path, scalar));
                }
                for (mut path, clause) in nested.invariants {
                    path.insert(0, at);
                    subject.invariants.push((path, clause));
                }
            }
            _ => return Err("logical data subject has a field that is not a plain scalar"),
        }
    }
    Ok(subject)
}

/// The declaration of a sum that a logical contract takes as one value
/// (mathematical_bindings.md, "A sum input in a logical contract denotes a
/// single mathematical value of that sum's checked inductive declaration"):
/// each case with its fields, each a relevant scalar payload or the sum
/// itself. A sum with an invariant, a quotient, a mixed record and sum, or any
/// other field is refused by name.
pub fn sum_shape(
    module: &TerminalModule,
    sum: StructuralTypeId,
) -> Result<proof_admission::DataSubjectShape, &'static str> {
    let declaration = module
        .structural_type(sum)
        .ok_or("logical sum subject has no structural declaration")?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err("logical sum subject is not a sum");
    };
    if declaration.quotient_carrier.is_some()
        || declaration.scalar_restriction.is_some()
        || !declaration.requires.is_empty()
    {
        return Err("logical sum subject has an invariant, a quotient or a restriction");
    }
    let mut shape = proof_admission::DataSubjectShape {
        structural_type: sum,
        cases: Vec::with_capacity(cases.len()),
    };
    for case in cases {
        let mut fields = Vec::with_capacity(case.fields.len());
        for field in &case.fields {
            if field.relevance.is_erased() {
                return Err("logical sum subject has an erased payload");
            }
            let kind = match field.field_type {
                StructuralFieldType::Scalar(scalar) => {
                    proof_admission::DataFieldShape::Scalar(scalar)
                }
                StructuralFieldType::Structural(child) if child == sum => {
                    proof_admission::DataFieldShape::Recursive
                }
                _ => {
                    return Err("logical sum subject has a payload that is not a scalar or itself");
                }
            };
            fields.push((field.id, kind));
        }
        shape.cases.push((case.id, fields));
    }
    Ok(shape)
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
                let RecordSubject { leaves, invariants } =
                    record_subject(module, formal.structural_type)?;
                Ok(SourceSubject::Record {
                    place: formal.place,
                    leaves,
                    invariants,
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

/// A data invariant of the record at `prefix` beneath the subject `place`:
/// its `self` fields are that record's fields among the subject's leaves.
struct Invariant<'a> {
    renamed: &'a Renamed<'a>,
    place: PlaceId,
    prefix: &'a [CanonicalStructuralPathSegment],
}

impl ClauseBindings for Invariant<'_> {
    type Error = &'static str;

    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, Self::Error> {
        Err("logical data subject's invariant names a value that is no field")
    }

    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        if version != ContentPlaceVersion::Current || root != invariant_self() {
            return Err("logical data subject's invariant observes another place");
        }
        let renamed = self
            .renamed
            .subject(|source| match source {
                SourceSubject::Record { place, leaves, .. } if *place == self.place => {
                    leaves.iter().position(|(own, carrier)| {
                        *carrier == scalar
                            && own.len() == self.prefix.len() + path.len()
                            && own.starts_with(self.prefix)
                            && own.ends_with(path)
                    })
                }
                _ => None,
            })
            .ok_or("logical data subject's invariant names a field that is no scalar subject")?;
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
        Err("logical data subject's invariant tests a case")
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
                SourceSubject::Record { place, leaves, .. } if *place == root => leaves
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
                invariants: Vec::new(),
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
