//! A sum input of a theorem is one value of the sum's inductive declaration:
//! mathematical_bindings.md, "A sum input in a logical contract denotes a
//! single mathematical value of that sum's checked inductive declaration".
//! Its case tests and payload reads denote through the declaration's
//! recursor, and an Identity between two of them is the kernel's identity at
//! that type.

use super::{
    BoundedDenotationError, Denotation, ProofError, Proposition, ScalarTerm, ScalarType, TermHandle,
};
use bignum::BigInt;
use mathematical_kernel::{PlainData, PlainField};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ClauseCarrier, ClauseIdentity, ClauseOperand, IeeeFloatFormat,
    IeeeFloatValue, IntegerCarrier, PlaceId, ScalarReadProjection, StructuralCaseId,
    StructuralFieldId, StructuralTypeId,
};

/// One field of a case of a theorem's sum input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFieldShape {
    /// A payload of this closed-width integer or Boolean carrier.
    Scalar(ScalarType),
    /// The field holds the sum itself.
    Recursive,
}

/// The declaration of a theorem's sum input: each case, in declared order,
/// with its fields in declared order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSubjectShape {
    pub structural_type: StructuralTypeId,
    pub cases: Vec<(StructuralCaseId, Vec<(StructuralFieldId, DataFieldShape)>)>,
}

/// A theorem's sum input: the place its clauses name and its declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSubject {
    pub place: PlaceId,
    pub shape: DataSubjectShape,
}

/// One sum declaration in the denotation's signature.
pub(super) struct DeclaredData {
    shape: DataSubjectShape,
    data: PlainData,
    /// The entry of its inductive type.
    entry: u32,
}

/// A theorem has a handful of sum inputs, so these are short lists.
#[derive(Default)]
pub(super) struct DataSubjectTables {
    declarations: Vec<DeclaredData>,
    /// Each subject's place, its sum, and its own entry, an input of the
    /// sum's type.
    subjects: Vec<(PlaceId, StructuralTypeId, u32)>,
}

impl DataSubjectTables {
    fn declaration(&self, sum: StructuralTypeId) -> Option<&DeclaredData> {
        self.declarations
            .iter()
            .find(|declared| declared.shape.structural_type == sum)
    }

    fn subject(&self, place: PlaceId) -> Option<(StructuralTypeId, u32)> {
        self.subjects
            .iter()
            .find(|(at, ..)| *at == place)
            .map(|&(_, sum, entry)| (sum, entry))
    }
}

const UNSUPPORTED_PAYLOAD: &str =
    "a sum input's payload is not a closed-width integer, a Boolean, a float or the sum itself";

impl Denotation {
    /// Declare the subject's sum, once for each declaration, and the subject
    /// as an input of it. The returned entry is the input to bind.
    pub(super) fn declare_data_subject(
        &mut self,
        subject: &DataSubject,
    ) -> Result<u32, BoundedDenotationError> {
        if self.data.subject(subject.place).is_some() {
            return Err(BoundedDenotationError::Unsupported(
                "a theorem names one sum input twice",
            ));
        }
        let structural_type = subject.shape.structural_type;
        let entry = match self.data.declaration(structural_type) {
            Some(declared) if declared.shape == subject.shape => declared.entry,
            Some(_) => {
                return Err(BoundedDenotationError::Unsupported(
                    "a sum's inputs disagree about its declaration",
                ));
            }
            None => self.declare_data(&subject.shape)?,
        };
        let ty = self.constant(entry);
        let position = self.position()?;
        self.declarations
            .push(mathematical_kernel::Declaration::assumption(0, ty));
        self.data
            .subjects
            .push((subject.place, structural_type, position));
        Ok(position)
    }

    fn declare_data(&mut self, shape: &DataSubjectShape) -> Result<u32, BoundedDenotationError> {
        let mut cases = Vec::with_capacity(shape.cases.len());
        for (_, fields) in &shape.cases {
            let mut plain = Vec::with_capacity(fields.len());
            for &(_, field) in fields {
                plain.push(match field {
                    DataFieldShape::Recursive => PlainField::Recursive,
                    DataFieldShape::Scalar(scalar) => PlainField::Payload(self.payload(scalar)?),
                });
            }
            cases.push(plain);
        }
        let data = PlainData::new(cases).ok_or(BoundedDenotationError::Unsupported(
            "a sum input's declaration has no case",
        ))?;
        let entry = self.position()?;
        let declarations = data.declarations(&mut self.arena, entry);
        self.declarations.extend(declarations);
        self.data.declarations.push(DeclaredData {
            shape: shape.clone(),
            data,
            entry,
        });
        Ok(entry)
    }

    /// The carrier of a payload: the integers a fixed-width integer denotes
    /// into, as a scalar subject does, `Two`, or a float format's carrier,
    /// as a float subject has.
    fn payload(&mut self, scalar: ScalarType) -> Result<TermHandle, BoundedDenotationError> {
        match scalar {
            ScalarType::Boolean => Ok(self.two),
            ScalarType::IeeeFloat(_) => self.carrier(scalar),
            ScalarType::Integer(integer)
                if integer.carrier() == IntegerCarrier::Fixed
                    && !integer.is_address()
                    && integer.bits().is_some() =>
            {
                self.integer_constant()
            }
            _ => Err(BoundedDenotationError::Unsupported(UNSUPPORTED_PAYLOAD)),
        }
    }

    fn declared(&self, place: PlaceId) -> Option<(&DeclaredData, u32)> {
        let (structural_type, subject) = self.data.subject(place)?;
        Some((self.data.declaration(structural_type)?, subject))
    }

    fn case_position(
        declared: &DeclaredData,
        case: StructuralCaseId,
    ) -> Result<usize, BoundedDenotationError> {
        declared
            .shape
            .cases
            .iter()
            .position(|(id, _)| *id == case)
            .ok_or(BoundedDenotationError::Unsupported(
                "a sum input's clause names a case its declaration lacks",
            ))
    }

    /// `rec (λ _. motive) branches subject`, with the branch of each case
    /// built from the case's position.
    fn analyze(
        &mut self,
        place: PlaceId,
        motive: TermHandle,
        body: impl FnMut(&mut Self, &PlainData, usize) -> Result<TermHandle, BoundedDenotationError>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (declared, subject) = self.declared(place).expect("a declared sum input");
        let (data, entry) = (declared.data.clone(), declared.entry);
        let subject = self.constant(subject);
        self.analyze_at(&data, entry, motive, body, subject)
    }

    /// The same analysis of `scrutinee`, any term of the sum. Each branch
    /// body must be closed but for the case's own fields.
    fn analyze_at(
        &mut self,
        data: &PlainData,
        entry: u32,
        motive: TermHandle,
        mut body: impl FnMut(&mut Self, &PlainData, usize) -> Result<TermHandle, BoundedDenotationError>,
        scrutinee: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let ty = self.constant(entry);
        let motive = self.lam(ty, motive);
        let mut branches = Vec::with_capacity(data.cases());
        for case in 0..data.cases() {
            let value = body(self, data, case)?;
            branches.push(data.branch(&mut self.arena, entry, motive, case, value));
        }
        Ok(data.induct(&mut self.arena, entry, motive, &branches, scrutinee))
    }

    /// The test that `scrutinee` is in the case at `tested`: the identity
    /// of its recursor, `0` exactly at that case, with `0`.
    fn case_test_at(
        &mut self,
        data: &PlainData,
        entry: u32,
        tested: usize,
        scrutinee: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let two = self.two;
        let test = self.analyze_at(
            data,
            entry,
            two,
            |s, _, case| {
                Ok(if case == tested {
                    s.two_zero
                } else {
                    s.two_one
                })
            },
            scrutinee,
        )?;
        Ok(self.id(self.two, test, self.two_zero))
    }

    /// A case test of a whole sum input: its recursor is `0` exactly at the
    /// tested case, so the test is the identity of that with `0`.
    pub(super) fn data_case_test(
        &mut self,
        prop: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::StructuralCaseMembership { subject, case } = prop else {
            return Ok(None);
        };
        let Some((declared, input)) = self.declared(subject.root()) else {
            return Ok(None);
        };
        if !subject.path().is_empty() {
            return Err(BoundedDenotationError::Unsupported(
                "a case test of a field of a sum input",
            ));
        }
        let tested = Self::case_position(declared, *case)?;
        let (data, entry) = (declared.data.clone(), declared.entry);
        let input = self.constant(input);
        self.case_test_at(&data, entry, tested, input).map(Some)
    }

    /// `c in S::A || c in S::B || ...`, each case of a sum input once and in
    /// any order, by induction over `c`. The motive is that disjunction of
    /// the tests at the bound value; at the value of each case, its own test
    /// computes to `0 = 0` and the branch injects `refl` there. Nothing is
    /// assumed: the recursor and the computing tests are the proof.
    pub(super) fn data_case_exhaustion(
        &mut self,
        prop: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (place, cases) = crate::proof::case_exhaustion_cases(prop)
            .map_err(BoundedDenotationError::Certificate)?;
        let Some((declared, subject)) = self.declared(place) else {
            return Err(BoundedDenotationError::Unsupported(
                "a case exhaustion of other than a theorem's sum input",
            ));
        };
        let tested = cases
            .iter()
            .map(|case| Self::case_position(declared, *case))
            .collect::<Result<Vec<_>, _>>()?;
        let (data, entry) = (declared.data.clone(), declared.entry);
        if tested.len() != data.cases() {
            return Err(BoundedDenotationError::Certificate(
                ProofError::RuleConclusionMismatch("a case exhaustion that omits a case"),
            ));
        }
        // Right-nested tagged sums bind one value each: the disjunct at `j`
        // sits under `j + 1` of them, and the last beside the one before it.
        let count = tested.len();
        let mut leaves = Vec::with_capacity(count);
        for (j, &case) in tested.iter().enumerate() {
            let depth = if count == 1 {
                0
            } else {
                (j + 1).min(count - 1)
            };
            let scrutinee = self.var(depth as u32);
            leaves.push(self.case_test_at(&data, entry, case, scrutinee)?);
        }
        let family = if count == 1 {
            leaves[0]
        } else {
            self.disjunction_terms(leaves)
        };
        let ty = self.constant(entry);
        let motive = self.lam(ty, family);
        let mut branches = Vec::with_capacity(count);
        for case in 0..count {
            let index = tested
                .iter()
                .position(|&at| at == case)
                .expect("every case is tested once");
            let mut proof = self.refl(self.two, self.two_zero);
            proof = self.inject(count, index, proof);
            let value = data.case_value(&mut self.arena, entry, case, 0);
            let at = self.app(motive, value);
            let body = self.ascribe(at, proof);
            branches.push(data.branch(&mut self.arena, entry, motive, case, body));
        }
        let subject = self.constant(subject);
        Ok(data.induct(&mut self.arena, entry, motive, &branches, subject))
    }

    /// The injection of `proof` at disjunct `index` of `count`.
    fn inject(&mut self, count: usize, index: usize, proof: TermHandle) -> TermHandle {
        if count == 1 {
            return proof;
        }
        // `index` ones lead to the disjunct, and a zero picks it unless it
        // is the last.
        let mut tags = vec![self.two_one; index];
        if index + 1 < count {
            tags.push(self.two_zero);
        }
        tags.into_iter()
            .rev()
            .fold(proof, |inner, tag| self.pair(tag, inner))
    }

    /// A payload read `Case(c), Field(f)` of a sum input: the field in its
    /// case's branch. Every other branch gives the carrier's zero; a contract
    /// reads a payload only under knowledge of its case.
    pub(super) fn data_payload_read(
        &mut self,
        term: &ScalarTerm,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } = term
        else {
            return Ok(None);
        };
        let Some((declared, _)) = self.declared(*root) else {
            return Ok(None);
        };
        let [
            ScalarReadProjection::Static(CanonicalStructuralPathSegment::Case(case)),
            ScalarReadProjection::Static(CanonicalStructuralPathSegment::Field(field)),
        ] = path.as_slice()
        else {
            return Err(BoundedDenotationError::Unsupported(
                "a sum input's read is not one payload field of one case",
            ));
        };
        let read = Self::case_position(declared, *case)?;
        let fields = &declared.shape.cases[read].1;
        let Some(position) = fields.iter().position(|(id, _)| id == field) else {
            return Err(BoundedDenotationError::Unsupported(
                "a sum input's read names a field its case lacks",
            ));
        };
        if fields[position].1 != DataFieldShape::Scalar(*scalar_type) {
            return Err(BoundedDenotationError::Unsupported(
                "a sum input's read is not at its field's scalar carrier",
            ));
        }
        let carrier = self.payload(*scalar_type)?;
        let zero = match scalar_type {
            ScalarType::Boolean => self.two_zero,
            // Positive zero, the literal a float subject's `0.0` denotes.
            ScalarType::IeeeFloat(format) => {
                let zero = match format {
                    IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                    IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
                };
                self.scalar_term(&ScalarTerm::IeeeFloatLiteral(zero))?
            }
            _ => self.math_literal(BigInt::from(0))?,
        };
        let value = self.analyze(*root, carrier, |s, data, case| {
            Ok(if case == read {
                data.field(&mut s.arena, case, position as u32, 0)
            } else {
                zero
            })
        })?;
        Ok(Some(value))
    }

    /// An Identity of two whole sum inputs of one declaration.
    pub(super) fn data_identity(
        &mut self,
        prop: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::DeclarationIdentity(identity) = prop else {
            return Ok(None);
        };
        let Some((left, right)) = Self::identity_inputs(identity) else {
            return Ok(None);
        };
        let (Some((left_type, left_entry)), Some((right_type, right_entry))) =
            (self.data.subject(left.0), self.data.subject(right.0))
        else {
            return Ok(None);
        };
        let (Some(declared), true) = (
            self.data.declaration(left_type),
            left_type == right_type && left.1 == left_type && right.1 == right_type,
        ) else {
            return Err(BoundedDenotationError::Unsupported(
                "an Identity of two sum inputs at other than their one declaration",
            ));
        };
        let entry = declared.entry;
        let ty = self.constant(entry);
        let (left, right) = (self.constant(left_entry), self.constant(right_entry));
        Ok(Some(self.id(ty, left, right)))
    }

    /// The two sides of an Identity whose values are both whole inputs at
    /// their structural type, each with that type.
    fn identity_inputs(
        identity: &ClauseIdentity,
    ) -> Option<((PlaceId, StructuralTypeId), (PlaceId, StructuralTypeId))> {
        if identity.values.len() != 2 {
            return None;
        }
        let side = |index: u32| {
            let value = identity.values.get(index as usize)?;
            match (&value.carrier, &value.operand) {
                (ClauseCarrier::Structural(ty), ClauseOperand::Input(subject))
                    if subject.path().is_empty() =>
                {
                    Some((subject.root(), *ty))
                }
                _ => None,
            }
        };
        Some((side(identity.left)?, side(identity.right)?))
    }
}
