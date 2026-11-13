//! Checked clause substitution shared by elaboration and proof checking.
//!
//! Bindings resolve names at the consumer's exact program point. Missing names
//! and unsupported observations return errors rather than retaining callee IDs.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralCaseId, ValueId,
};

/// What each name of a clause stands for where the clause is used. A clause
/// is written over its owner's names: a callee's formals, or the fields of
/// the value a data type's `requires` is about.
pub trait ClauseBindings {
    type Error: From<&'static str>;
    /// The term a value of the clause's owner stands for.
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error>;
    /// The value the scalar field at `path` beneath `root` holds.
    fn field(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error>;
    /// Whether the sum at `path` beneath `root` is `case`, by structure.
    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, Self::Error>;
}

/// States a clause over the consumer's values by
/// replacing each of its names with what `bindings` says it stands for. The
/// consumer owns the resolution of storage observations and value identities.
///
/// A case test is not a value: where the clause is used the case is known by
/// structure or not at all, so the test becomes `Truth` or `Falsehood` and
/// the connective around it is simplified. A conjunction with a false
/// conjunct is false and a disjunction with a true disjunct is true whatever
/// their other parts are, which is what lets a clause project a payload
/// under the test of its case (data_and_literals.md: "inactive payloads
/// cannot be projected"): the projection is only resolved when the test did
/// not already decide its connective.
///
/// Any other form is refused: substituting under a construct this function
/// does not know would be a silent change of meaning.
pub fn substitute_clause<Error: From<&'static str>>(
    proposition: &Proposition,
    bindings: &mut dyn ClauseBindings<Error = Error>,
) -> Result<Proposition, Error> {
    Ok(match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_clause_term(left, bindings)?,
            substitute_clause_term(right, bindings)?,
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_clause_term(left, bindings)?,
            substitute_clause_term(right, bindings)?,
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_clause_term(left, bindings)?,
            substitute_clause_term(right, bindings)?,
        ),
        Proposition::IntegerMathEqual(left, right) => Proposition::IntegerMathEqual(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::IntegerMathLessThan(left, right) => Proposition::IntegerMathLessThan(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::IntegerMathLessOrEqual(left, right) => Proposition::IntegerMathLessOrEqual(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::StructuralCaseMembership { subject, case } => {
            if bindings.case(subject.root(), subject.path(), *case)? {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            }
        }
        Proposition::Conjunction(parts) => connective(parts, bindings, true)?,
        Proposition::Disjunction(parts) => connective(parts, bindings, false)?,
        _ => {
            return Err(Error::from("clause form outside the scalar subset"));
        }
    })
}

/// One term of a clause under substitution.
pub fn substitute_clause_term<Error: From<&'static str>>(
    term: &ScalarTerm,
    bindings: &mut dyn ClauseBindings<Error = Error>,
) -> Result<ScalarTerm, Error> {
    match term {
        ScalarTerm::Value { id, scalar_type } => bindings.value(*id, *scalar_type),
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => bindings.field(*root, path, ScalarType::Integer(*scalar_type)),
        ScalarTerm::BooleanField { root, path } => bindings.field(*root, path, ScalarType::Boolean),
        ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) => Ok(term.clone()),
        ScalarTerm::StructuralRead { .. } => Err(Error::from(
            "clause structural read is outside the scalar subset",
        )),
        _ if !term.operands().is_empty() => {
            let mut bound_term = term.clone();
            for operand in bound_term.operands_mut() {
                *operand = substitute_clause_term(operand, bindings)?;
            }
            Ok(bound_term)
        }
        _ => Err(Error::from("clause term outside the scalar subset")),
    }
}

/// One mathematical-integer term of a clause under substitution. An embedded
/// name is replaced by what `bindings` says it stands for, embedded again:
/// the value keeps its carrier, and a literal actual is the mathematical
/// integer it denotes (`IntegerMathTerm::embed`). The arithmetic around it is
/// over unbounded integers and is kept as written
/// (mathematical_values.md: "These terms denote unbounded mathematical
/// integers and cannot overflow").
fn bound_mathematical<Error: From<&'static str>>(
    term: &IntegerMathTerm,
    bindings: &mut dyn ClauseBindings<Error = Error>,
) -> Result<IntegerMathTerm, Error> {
    let embedded = |term: ScalarTerm| {
        IntegerMathTerm::embed(term)
            .ok_or(Error::from("clause embeds a term that is not an integer"))
    };
    match term {
        IntegerMathTerm::IntegerLiteral(_) => Ok(term.clone()),
        IntegerMathTerm::MathValue { source_type, value } => {
            embedded(bindings.value(*value, ScalarType::Integer(*source_type))?)
        }
        IntegerMathTerm::Embed(scalar) => embedded(substitute_clause_term(scalar, bindings)?),
        IntegerMathTerm::Add(..)
        | IntegerMathTerm::Subtract(..)
        | IntegerMathTerm::Multiply(..)
        | IntegerMathTerm::ShiftLeft { .. } => {
            let mut bound_term = term.clone();
            for operand in bound_term.operands_mut() {
                *operand = bound_mathematical(operand, bindings)?;
            }
            Ok(bound_term)
        }
    }
}

/// A conjunction (`conjunction`) or disjunction under substitution. The
/// deciding constant is `Falsehood` for a conjunction and `Truth` for a
/// disjunction: one part equal to it decides the whole, and then a part
/// that could not be stated does not matter. The other constant is the
/// connective's unit and is dropped.
fn connective<Error: From<&'static str>>(
    parts: &[Proposition],
    bindings: &mut dyn ClauseBindings<Error = Error>,
    conjunction: bool,
) -> Result<Proposition, Error> {
    let (deciding, unit) = if conjunction {
        (Proposition::Falsehood, Proposition::Truth)
    } else {
        (Proposition::Truth, Proposition::Falsehood)
    };
    let mut kept = Vec::with_capacity(parts.len());
    let mut unstated = None;
    for part in parts {
        match substitute_clause(part, bindings) {
            Ok(part) if part == deciding => return Ok(deciding),
            Ok(part) if part == unit => {}
            Ok(part) => kept.push(part),
            Err(unbound) => unstated = unstated.or(Some(unbound)),
        }
    }
    if let Some(unbound) = unstated {
        return Err(unbound);
    }
    Ok(match kept.len() {
        0 => unit,
        1 => kept.remove(0),
        _ if conjunction => Proposition::Conjunction(kept),
        _ => Proposition::Disjunction(kept),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, StructuralFieldId};

    struct Inputs;

    fn scalar_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    impl ClauseBindings for Inputs {
        type Error = &'static str;

        fn value(
            &mut self,
            formal: ValueId,
            carrier: ScalarType,
        ) -> Result<ScalarTerm, Self::Error> {
            match formal.get() {
                1 | 2 => Ok(ScalarTerm::value(
                    ValueId::new(3 - formal.get()).unwrap(),
                    carrier,
                )),
                _ => Err("unbound value"),
            }
        }

        fn field(
            &mut self,
            root: PlaceId,
            path: &[CanonicalStructuralPathSegment],
            carrier: ScalarType,
        ) -> Result<ScalarTerm, Self::Error> {
            if root.get() != 1 {
                return Err("unbound root");
            }
            match path {
                [CanonicalStructuralPathSegment::Field(field)] if field.get() == 2 => {
                    Ok(ScalarTerm::value(ValueId::new(7).unwrap(), carrier))
                }
                _ => Err("unbound field"),
            }
        }

        fn case(
            &mut self,
            _: PlaceId,
            _: &[CanonicalStructuralPathSegment],
            _: StructuralCaseId,
        ) -> Result<bool, Self::Error> {
            Err("unbound case")
        }
    }

    #[test]
    fn checked_clause_substitution_preserves_selected_fields_and_simultaneous_values() {
        let field = |selected| ScalarTerm::IntegerField {
            root: PlaceId::new(1).unwrap(),
            path: vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(selected).unwrap(),
            )],
            scalar_type: scalar_type().as_integer().unwrap(),
        };
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type());
        let clause = Proposition::LessThan(field(2), value(1));
        assert_eq!(
            substitute_clause(&clause, &mut Inputs),
            Ok(Proposition::LessThan(value(7), value(2)))
        );
        assert_eq!(
            substitute_clause(&Proposition::LessThan(field(1), value(1)), &mut Inputs),
            Err("unbound field")
        );
        assert_eq!(
            substitute_clause(&Proposition::LessThan(field(2), value(3)), &mut Inputs),
            Err("unbound value")
        );
        assert_eq!(
            substitute_clause(&Proposition::LessThan(value(1), value(2)), &mut Inputs),
            Ok(Proposition::LessThan(value(2), value(1)))
        );
    }
}
