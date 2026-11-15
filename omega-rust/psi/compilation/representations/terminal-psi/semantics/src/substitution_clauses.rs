//! Checked clause substitution shared by elaboration and proof checking.
//!
//! Bindings resolve names at the consumer's exact program point. Missing names
//! and unsupported observations return errors rather than retaining callee IDs.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, IntegerMathTerm, PlaceId, Proposition,
    ScalarTerm, ScalarType, StructuralCaseId, ValueId,
};

/// What each name of a clause stands for where the clause is used. A clause
/// is written over its owner's names: a callee's formals, or the fields of
/// the value a data type's `requires` is about.
pub trait ClauseBindings {
    type Error: From<&'static str>;
    /// The term a value of the clause's owner stands for.
    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error>;
    /// The value the scalar field at `path` beneath `root` holds. Entry
    /// observations keep the invocation input; current observations follow
    /// the consumer's cut and the formal's owned or borrowed meaning.
    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<ScalarTerm, Self::Error>;
    /// The current extent of this exact descriptor at the consumer's point.
    /// Consumers without its version-bound observation must refuse it.
    fn extent(
        &mut self,
        _root: PlaceId,
        _path: &[CanonicalStructuralPathSegment],
        _scalar_type: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, Self::Error> {
        Err(Self::Error::from(
            "clause extent has no descriptor observation",
        ))
    }
    /// Whether the sum at `path` beneath `root` is `case`, by structure.
    fn case(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<bool, Self::Error>;

    /// Resolve a case predicate, or retain its exact subject when only scalar
    /// names are being changed within the same machine.
    fn case_predicate(
        &mut self,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        case: StructuralCaseId,
    ) -> Result<Proposition, Self::Error> {
        self.case(root, path, case).map(|holds| {
            if holds {
                Proposition::Truth
            } else {
                Proposition::Falsehood
            }
        })
    }

    /// Actual call imports must rejoin a projection to the current owner's
    /// real scalar producer. Declaration-template bindings may explicitly keep
    /// symbolic sources; an unspecified consumer cannot invent correspondence.
    fn validate_float_meaning_source(&mut self, _source: &ScalarTerm) -> Result<(), Self::Error> {
        Err(Self::Error::from(
            "float meaning projection has no actual owner correspondence",
        ))
    }

    /// Declaration formation cannot skip a partial term because another
    /// branch already decides its enclosing condition. Ordinary substitution
    /// keeps its existing case-directed projection behavior.
    fn requires_complete_terms(&self) -> bool {
        false
    }

    /// Resolve one computation after its operands have been resolved. Ordinary
    /// call substitution retains the selected computation. Declaration
    /// formation can instead register it as a pending logical computation.
    fn computed(&mut self, term: ScalarTerm) -> Result<ScalarTerm, Self::Error> {
        Ok(term)
    }
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
        Proposition::FloatMeaningEqual(clause) => {
            let mapped = clause.map_sources(|source| substitute_clause_term(source, bindings))?;
            for source in mapped.projection_sources() {
                bindings.validate_float_meaning_source(source)?;
            }
            Proposition::FloatMeaningEqual(mapped)
        }
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        // An opaque proposition has no bindable scalar or storage name.
        // Its enclosing proposition scope owns its complete meaning; this
        // preserves that identity and introduces no premise or proof.
        Proposition::Atom(atom) => Proposition::Atom(*atom),
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
        Proposition::IntegerMathEqual(left, right) => {
            let mut left = bound_mathematical(left, bindings)?;
            let mut right = bound_mathematical(right, bindings)?;
            // Substituting a literal can change the canonical order of this
            // symmetric relation; keep its operands ordered after binding.
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::IntegerMathEqual(left, right)
        }
        Proposition::IntegerMathLessThan(left, right) => Proposition::IntegerMathLessThan(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::IntegerMathLessOrEqual(left, right) => Proposition::IntegerMathLessOrEqual(
            bound_mathematical(left, bindings)?,
            bound_mathematical(right, bindings)?,
        ),
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            // A comparison keeps its IEEE predicate while names change. In
            // particular, an ordered false result is not the opposite integer
            // relation: either operand may be NaN.
            let mut left = substitute_clause_term(left, bindings)?;
            let mut right = substitute_clause_term(right, bindings)?;
            if left.scalar_type() != ScalarType::IeeeFloat(*format)
                || right.scalar_type() != ScalarType::IeeeFloat(*format)
            {
                return Err(Error::from("float clause substitution changes its format"));
            }
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::ScalarIeeeFloatComparison {
                kind: *kind,
                format: *format,
                left,
                right,
            }
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            bindings.case_predicate(subject.root(), subject.path(), *case)?
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_clause(premise, bindings)?),
            conclusion: Box::new(substitute_clause(conclusion, bindings)?),
        },
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
    enum Pending {
        Visit(ScalarTerm),
        Complete(ScalarTerm, usize),
    }
    let mut pending = vec![Pending::Visit(term.clone())];
    let mut completed = Vec::new();
    while let Some(step) = pending.pop() {
        let resolved = match step {
            Pending::Complete(mut term, count) => {
                if completed.len() < count {
                    return Err(Error::from("clause computation lost a resolved operand"));
                }
                for operand in term.operands_mut().into_iter().rev() {
                    *operand = completed
                        .pop()
                        .ok_or(Error::from("clause computation lost a resolved operand"))?;
                }
                bindings.computed(term)?
            }
            Pending::Visit(term) => match term {
                ScalarTerm::Value { id, scalar_type } => bindings.value(id, scalar_type)?,
                ScalarTerm::IntegerField {
                    version,
                    root,
                    path,
                    scalar_type,
                } => bindings.field(version, root, &path, ScalarType::Integer(scalar_type))?,
                ScalarTerm::BooleanField {
                    version,
                    root,
                    path,
                } => bindings.field(version, root, &path, ScalarType::Boolean)?,
                ScalarTerm::IeeeFloatField {
                    version,
                    root,
                    path,
                    format,
                } => bindings.field(version, root, &path, ScalarType::IeeeFloat(format))?,
                ScalarTerm::ViewExtent {
                    root,
                    path,
                    scalar_type,
                } => bindings.extent(root, &path, scalar_type)?,
                ScalarTerm::Application { .. }
                | ScalarTerm::Integer { .. }
                | ScalarTerm::Boolean(_)
                | ScalarTerm::IeeeFloatLiteral(_) => term,
                ScalarTerm::StructuralRead { .. } => {
                    return Err(Error::from(
                        "clause structural read is outside the scalar subset",
                    ));
                }
                mut term => {
                    let first = pending.len();
                    let operands = term.operands_mut();
                    let count = operands.len();
                    if count == 0 {
                        return Err(Error::from("clause term outside the scalar subset"));
                    }
                    // Move each operand once. Every temporary is replaced
                    // before its parent reaches bindings; an error discards
                    // the unfinished tree.
                    for operand in operands.into_iter().rev() {
                        pending.push(Pending::Visit(std::mem::replace(
                            operand,
                            ScalarTerm::Boolean(false),
                        )));
                    }
                    pending.insert(first, Pending::Complete(term, count));
                    continue;
                }
            },
        };
        completed.push(resolved);
    }
    if completed.len() != 1 {
        return Err(Error::from("clause substitution did not produce one term"));
    }
    completed
        .pop()
        .ok_or(Error::from("clause substitution did not produce one term"))
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
    enum Pending {
        Visit(IntegerMathTerm),
        Complete(IntegerMathTerm, usize),
    }
    let mut pending = vec![Pending::Visit(term.clone())];
    let mut completed = Vec::new();
    while let Some(step) = pending.pop() {
        let resolved = match step {
            Pending::Complete(mut term, count) => {
                if completed.len() < count {
                    return Err(Error::from("mathematical clause lost a resolved operand"));
                }
                for operand in term.operands_mut().into_iter().rev() {
                    *operand = completed
                        .pop()
                        .ok_or(Error::from("mathematical clause lost a resolved operand"))?;
                }
                term
            }
            Pending::Visit(term) => match term {
                IntegerMathTerm::IntegerLiteral(_) => term,
                IntegerMathTerm::MathValue { source_type, value } => {
                    embedded(bindings.value(value, ScalarType::Integer(source_type))?)?
                }
                IntegerMathTerm::Embed(scalar) => {
                    embedded(substitute_clause_term(&scalar, bindings)?)?
                }
                mut term => {
                    // Clone the tree once, then move every owned operand once.
                    // Re-cloning each mathematical subtree grows with depth.
                    let first = pending.len();
                    let operands = term.operands_mut();
                    let count = operands.len();
                    for operand in operands.into_iter().rev() {
                        pending.push(Pending::Visit(std::mem::replace(
                            operand,
                            IntegerMathTerm::literal(semantic_vocabulary::IntegerValue::Unsigned(
                                0,
                            )),
                        )));
                    }
                    pending.insert(first, Pending::Complete(term, count));
                    continue;
                }
            },
        };
        completed.push(resolved);
    }
    if completed.len() != 1 {
        return Err(Error::from("mathematical clause did not produce one term"));
    }
    completed
        .pop()
        .ok_or(Error::from("mathematical clause did not produce one term"))
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
    let mut decided = false;
    for part in parts {
        match substitute_clause(part, bindings) {
            Ok(part) if part == deciding => {
                if !bindings.requires_complete_terms() {
                    return Ok(deciding);
                }
                decided = true;
            }
            Ok(part) if part == unit => {}
            Ok(part) => kept.push(part),
            Err(unbound) => unstated = unstated.or(Some(unbound)),
        }
    }
    if let Some(unbound) = unstated {
        return Err(unbound);
    }
    if decided {
        return Ok(deciding);
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
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralFieldId};

    #[test]
    fn ieee_clauses_keep_the_predicate_and_exact_actuals() {
        use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
        let format = IeeeFloatFormat::Binary32;
        let value =
            |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::IeeeFloat(format));
        for kind in [
            IeeeFloatComparisonKind::Equal,
            IeeeFloatComparisonKind::NotEqual,
            IeeeFloatComparisonKind::LessThan,
            IeeeFloatComparisonKind::NotLessThan,
            IeeeFloatComparisonKind::LessOrEqual,
            IeeeFloatComparisonKind::NotLessOrEqual,
        ] {
            let original = Proposition::ScalarIeeeFloatComparison {
                kind,
                format,
                left: value(1),
                right: value(2),
            };
            let expected = if kind.is_symmetric() {
                (value(1), value(2))
            } else {
                (value(2), value(1))
            };
            assert_eq!(
                substitute_clause(&original, &mut Inputs).unwrap(),
                Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format,
                    left: expected.0,
                    right: expected.1
                }
            );
        }
        for bits in [0x8000_0000, 0x7fc0_0042] {
            let literal = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(bits));
            assert_eq!(
                substitute_clause_term(&literal, &mut Inputs).unwrap(),
                literal
            );
        }
    }

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
            _: ContentPlaceVersion,
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
            version: ContentPlaceVersion::Current,
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

    #[test]
    fn mathematical_substitution_preserves_operand_order_and_simultaneous_names() {
        let integer = scalar_type().as_integer().unwrap();
        let value = |raw| IntegerMathTerm::MathValue {
            source_type: integer,
            value: ValueId::new(raw).unwrap(),
        };
        let expression = |left, right| {
            IntegerMathTerm::Subtract(
                Box::new(value(left)),
                Box::new(IntegerMathTerm::Embed(Box::new(
                    ScalarTerm::ExactIntegerAdd {
                        scalar_type: integer,
                        left: Box::new(ScalarTerm::value(
                            ValueId::new(right).unwrap(),
                            scalar_type(),
                        )),
                        right: Box::new(ScalarTerm::Integer {
                            scalar_type: integer,
                            value: IntegerValue::Unsigned(1),
                        }),
                    },
                ))),
            )
        };
        let clause = Proposition::IntegerMathEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
            expression(1, 2),
        );
        clause.validate().unwrap();
        assert_eq!(
            substitute_clause(&clause, &mut Inputs),
            Ok(Proposition::IntegerMathEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                expression(2, 1)
            ))
        );
        let foreign = Proposition::IntegerMathEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
            expression(1, 99),
        );
        assert_eq!(
            substitute_clause(&foreign, &mut Inputs),
            Err("unbound value")
        );
    }

    #[test]
    fn computed_bindings_receive_resolved_children_and_keep_their_failures() {
        struct Computations {
            pending: Vec<ScalarTerm>,
            refuse_child: bool,
        }

        impl ClauseBindings for Computations {
            type Error = &'static str;

            fn value(
                &mut self,
                id: ValueId,
                scalar: ScalarType,
            ) -> Result<ScalarTerm, Self::Error> {
                Inputs.value(id, scalar)
            }

            fn field(
                &mut self,
                version: ContentPlaceVersion,
                root: PlaceId,
                path: &[CanonicalStructuralPathSegment],
                scalar: ScalarType,
            ) -> Result<ScalarTerm, Self::Error> {
                Inputs.field(version, root, path, scalar)
            }

            fn case(
                &mut self,
                root: PlaceId,
                path: &[CanonicalStructuralPathSegment],
                case: StructuralCaseId,
            ) -> Result<bool, Self::Error> {
                Inputs.case(root, path, case)
            }

            fn computed(&mut self, term: ScalarTerm) -> Result<ScalarTerm, Self::Error> {
                if self.refuse_child {
                    return Err("child formation refused");
                }
                assert!(
                    term.operands()
                        .iter()
                        .all(|term| term.operands().is_empty())
                );
                let result = ScalarTerm::value(
                    ValueId::new(10 + self.pending.len() as u64).unwrap(),
                    term.scalar_type(),
                );
                self.pending.push(term);
                Ok(result)
            }
        }

        let integer = scalar_type().as_integer().unwrap();
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type());
        let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
        let field = ScalarTerm::IntegerField {
            version: ContentPlaceVersion::Current,
            root: PlaceId::new(1).unwrap(),
            path: vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(2).unwrap(),
            )],
            scalar_type: integer,
        };
        let child = ScalarTerm::exact_integer_subtract(integer, value(1), one.clone()).unwrap();
        let expression = ScalarTerm::exact_integer_add(integer, field, child).unwrap();
        let mut inputs = Computations {
            pending: Vec::new(),
            refuse_child: false,
        };
        assert_eq!(
            substitute_clause_term(&expression, &mut inputs),
            Ok(value(11))
        );
        assert_eq!(
            inputs.pending,
            [
                ScalarTerm::exact_integer_subtract(integer, value(2), one).unwrap(),
                ScalarTerm::exact_integer_add(integer, value(7), value(10)).unwrap(),
            ],
            "the parent keeps the resolved field and exact child correspondence",
        );
        let mut refused = Computations {
            pending: Vec::new(),
            refuse_child: true,
        };
        assert_eq!(
            substitute_clause_term(&expression, &mut refused),
            Err("child formation refused"),
        );
        assert!(
            refused.pending.is_empty(),
            "the parent must never be registered"
        );
    }

    #[test]
    fn implication_substitution_keeps_both_scoped_operands_and_refuses_unbound_names() {
        let predicate = |raw| {
            Proposition::Equal(
                ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Boolean),
                ScalarTerm::Boolean(true),
            )
        };
        let negated = Proposition::Implication {
            premise: Box::new(predicate(1)),
            conclusion: Box::new(Proposition::Falsehood),
        };
        assert_eq!(
            substitute_clause(&negated, &mut Inputs),
            Ok(Proposition::Implication {
                premise: Box::new(predicate(2)),
                conclusion: Box::new(Proposition::Falsehood),
            })
        );
        let both = Proposition::Implication {
            premise: Box::new(predicate(1)),
            conclusion: Box::new(predicate(2)),
        };
        assert_eq!(
            substitute_clause(&both, &mut Inputs),
            Ok(Proposition::Implication {
                premise: Box::new(predicate(2)),
                conclusion: Box::new(predicate(1)),
            })
        );
        let unknown_conclusion = Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(predicate(3)),
        };
        assert_eq!(
            substitute_clause(&unknown_conclusion, &mut Inputs),
            Err("unbound value")
        );
        let unbound = Proposition::Implication {
            premise: Box::new(predicate(3)),
            conclusion: Box::new(Proposition::Truth),
        };
        assert_eq!(
            substitute_clause(&unbound, &mut Inputs),
            Err("unbound value")
        );
    }
}

#[cfg(test)]
mod meaning_template_tests {
    use super::*;
    use numerics::float_semantics::{
        FloatSemanticOperation,
        FloatSemanticValueKind::{Format, Meaning},
    };
    use semantic_vocabulary::{FloatMeaningClause, IeeeFloatFormat, StructuralFieldId};

    fn equation(right: ScalarTerm) -> Proposition {
        let f = IeeeFloatFormat::Binary32;
        let scalar = ScalarType::IeeeFloat(f);
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), scalar);
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "subtract",
            &[Format, Meaning, Meaning],
            Meaning,
        )
        .unwrap();
        Proposition::FloatMeaningEqual(
            FloatMeaningClause::result_equation(
                value(3),
                operation.contract_identity(),
                f,
                [value(1), right],
            )
            .unwrap(),
        )
    }

    struct Actuals {
        wrong_format: bool,
        missing_result: bool,
    }
    impl ClauseBindings for Actuals {
        type Error = &'static str;
        fn value(&mut self, id: ValueId, scalar: ScalarType) -> Result<ScalarTerm, Self::Error> {
            let actual = match id.get() {
                1 => 2,
                2 => 1,
                3 if !self.missing_result => 30,
                _ => return Err("unmapped formal/result"),
            };
            Ok(ScalarTerm::value(
                ValueId::new(actual).unwrap(),
                if self.wrong_format {
                    ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
                } else {
                    scalar
                },
            ))
        }
        fn field(
            &mut self,
            version: ContentPlaceVersion,
            _: PlaceId,
            path: &[CanonicalStructuralPathSegment],
            scalar: ScalarType,
        ) -> Result<ScalarTerm, Self::Error> {
            let ScalarType::IeeeFloat(format) = scalar else {
                return Err("not a float field");
            };
            Ok(ScalarTerm::IeeeFloatField {
                version,
                root: PlaceId::new(5).unwrap(),
                path: path.to_vec(),
                format,
            })
        }
        fn case(
            &mut self,
            _: PlaceId,
            _: &[CanonicalStructuralPathSegment],
            _: StructuralCaseId,
        ) -> Result<bool, Self::Error> {
            Err("not a case")
        }
        fn validate_float_meaning_source(&mut self, _: &ScalarTerm) -> Result<(), Self::Error> {
            // This test checks template substitution, not concrete projection admission.
            Ok(())
        }
    }

    #[test]
    fn float_meaning_templates_substitute_actuals_results_and_saved_field_versions() {
        let scalar = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), scalar);
        let original = equation(value(2));
        let mapped = substitute_clause(
            &original,
            &mut Actuals {
                wrong_format: false,
                missing_result: false,
            },
        )
        .unwrap();
        let Proposition::FloatMeaningEqual(clause) = mapped else {
            unreachable!()
        };
        let ids = clause
            .sources()
            .map(|source| match source {
                ScalarTerm::Value { id, .. } => id.get(),
                _ => panic!("an actual/result lost its scalar identity"),
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![30, 2, 1]);
        assert_eq!(clause.left, semantic_vocabulary::FloatMeaningSlot(0));
        assert_eq!(clause.right, semantic_vocabulary::FloatMeaningSlot(3));
        let semantic_vocabulary::FloatMeaningClauseValue::Application {
            contract,
            format,
            operands,
        } = &clause.values[3]
        else {
            panic!("semantic application was lost")
        };
        assert_eq!(*format, IeeeFloatFormat::Binary32);
        let operation = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "subtract",
            &[Format, Meaning, Meaning],
            Meaning,
        )
        .unwrap();
        assert_eq!(*contract, operation.contract_identity());
        assert_eq!(
            operands,
            &vec![
                semantic_vocabulary::FloatMeaningClauseOperand::Format(IeeeFloatFormat::Binary32),
                semantic_vocabulary::FloatMeaningClauseOperand::Meaning(
                    semantic_vocabulary::FloatMeaningSlot(1)
                ),
                semantic_vocabulary::FloatMeaningClauseOperand::Meaning(
                    semantic_vocabulary::FloatMeaningSlot(2)
                ),
            ]
        );
        assert!(
            substitute_clause(
                &original,
                &mut Actuals {
                    wrong_format: true,
                    missing_result: false
                }
            )
            .is_err()
        );
        assert!(
            substitute_clause(
                &original,
                &mut Actuals {
                    wrong_format: false,
                    missing_result: true
                }
            )
            .is_err()
        );
        for version in [ContentPlaceVersion::Current, ContentPlaceVersion::Entry] {
            let field = ScalarTerm::IeeeFloatField {
                version,
                root: PlaceId::new(1).unwrap(),
                path: vec![CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(7).unwrap(),
                )],
                format: IeeeFloatFormat::Binary32,
            };
            let mapped = substitute_clause(
                &equation(field),
                &mut Actuals {
                    wrong_format: false,
                    missing_result: false,
                },
            )
            .unwrap();
            let Proposition::FloatMeaningEqual(clause) = mapped else {
                unreachable!()
            };
            assert!(clause.sources().any(|source| matches!(source, ScalarTerm::IeeeFloatField { version: actual, root, .. } if *actual==version && root.get()==5)));
        }
    }
}
