//! Check a record's declaration against the scalar values decoded from its backing.

use crate::substitution::clauses::{ClauseBindings, substitute_clause};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodedRecordError {
    Violated,
    Unsupported(&'static str),
}

impl DecodedRecordError {
    pub fn reason(self) -> &'static str {
        match self {
            Self::Violated => "the decoded fields violate the record requirement",
            Self::Unsupported(reason) => reason,
        }
    }
}

/// Both a byte-window read and an entry lender must discharge the same declaration.
/// Use the caller's decoded rows in place. A record with requirements orders its
/// fields by semantic ID before substitution; byte offsets stay attached to each row.
pub fn check_decoded_record_requirements<Row, Literal: FnMut(&Row) -> Option<ScalarTerm>>(
    requirements: &[Proposition],
    rows: &mut [Row],
    field_id: fn(&Row) -> StructuralFieldId,
    literal: Literal,
) -> Result<(), DecodedRecordError> {
    const LIMIT: DecodedRecordError = DecodedRecordError::Unsupported(
        "decoded record requirement evaluation exceeds the resource limit",
    );
    if requirements.is_empty() {
        return Ok(());
    }
    closed_predicates::check_predicate_evaluation_size(requirements).map_err(|_| LIMIT)?;
    rows.sort_unstable_by_key(field_id);
    if rows
        .windows(2)
        .any(|pair| field_id(&pair[0]) == field_id(&pair[1]))
    {
        return Err(DecodedRecordError::Unsupported(
            "decoded record repeats a field",
        ));
    }
    let mut fields = DecodedFields {
        rows,
        field_id,
        literal,
    };
    let mut integers = closed_predicates::ClosedIntegerEvaluator::default();
    for requirement in requirements {
        let holds = closed_predicates::closed_guard_with(requirement, |leaf| {
            let closed =
                substitute_clause(leaf, &mut fields).map_err(DecodedRecordError::Unsupported)?;
            closed_predicates::closed_guard(&closed, &mut integers).map_err(|_| LIMIT)
        })?;
        match holds {
            Some(true) => {}
            Some(false) => return Err(DecodedRecordError::Violated),
            None => {
                return Err(DecodedRecordError::Unsupported(
                    "the decoded fields do not establish the record requirement",
                ));
            }
        }
    }
    Ok(())
}

struct DecodedFields<'a, Row, Literal> {
    rows: &'a [Row],
    field_id: fn(&Row) -> StructuralFieldId,
    literal: Literal,
}

impl<Row, Literal: FnMut(&Row) -> Option<ScalarTerm>> ClauseBindings
    for DecodedFields<'_, Row, Literal>
{
    type Error = &'static str;

    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        ty: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let [CanonicalStructuralPathSegment::Field(field)] = path else {
            return Err("the decoded record requirement needs a direct scalar field");
        };
        // Data requirements use place 1 for self and observe its current contents.
        if version != ContentPlaceVersion::Current || root.get() != 1 {
            return Err("the decoded record requirement observes a different subject");
        }
        self.rows
            .binary_search_by_key(field, self.field_id)
            .ok()
            .and_then(|index| (self.literal)(&self.rows[index]))
            .filter(|value| value.scalar_type() == ty)
            .ok_or("the decoded record requirement names an absent or differently typed field")
    }

    fn value(&mut self, _: ValueId, _: ScalarType) -> Result<ScalarTerm, Self::Error> {
        Err("the decoded record requirement observes an external scalar")
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<bool, Self::Error> {
        Err("the decoded scalar record has no sum case")
    }
}
