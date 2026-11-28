//! Rebind a data predicate's reads of self to the same fields in a second
//! application of the declaration. The nested storage type is unchanged.
use crate::substitution::clauses::{CaseAnswer, ClauseBindings, substitute_clause};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentPlaceVersion, PlaceId, Proposition,
    ScalarTerm, ScalarType, StructuralCaseId, StructuralCaseSubject, ValueId,
};
use terminal_psi::StructuralFieldDeclaration;

pub(super) fn same(
    public: &[Proposition],
    actual: &[Proposition],
    fields: &[StructuralFieldDeclaration],
    supplied: &[StructuralFieldDeclaration],
) -> bool {
    if fields.iter().zip(supplied).all(|(a, b)| a.id == b.id) {
        return public == actual;
    }
    let mut bindings = MemberFields { fields, supplied };
    let mut remaining: Vec<_> = actual.iter().collect();
    for clause in public {
        let Ok(mapped) = substitute_clause(clause, &mut bindings) else {
            return false;
        };
        let Some(index) = remaining.iter().position(|p| **p == mapped) else {
            return false;
        };
        remaining.swap_remove(index);
    }
    remaining.is_empty()
}

struct MemberFields<'a> {
    fields: &'a [StructuralFieldDeclaration],
    supplied: &'a [StructuralFieldDeclaration],
}

impl MemberFields<'_> {
    fn path(&self, root: PlaceId, path: &[Segment]) -> Result<Vec<Segment>, &'static str> {
        let mut mapped = path.to_vec();
        if Some(root) == PlaceId::new(1)
            && let Some(Segment::Field(id)) = mapped.first_mut()
        {
            let index = self
                .fields
                .iter()
                .position(|field| field.id == *id)
                .ok_or("record requirement refers to an absent field")?;
            // The caller checked the complete field declarations in this order.
            *id = self
                .supplied
                .get(index)
                .ok_or("record requirement lost its field")?
                .id;
        }
        Ok(mapped)
    }
}

impl ClauseBindings for MemberFields<'_> {
    type Error = &'static str;

    fn value(&mut self, id: ValueId, scalar_type: ScalarType) -> Result<ScalarTerm, Self::Error> {
        Ok(ScalarTerm::Value { id, scalar_type })
    }

    fn field(
        &mut self,
        version: ContentPlaceVersion,
        root: PlaceId,
        path: &[Segment],
        ty: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        let path = self.path(root, path)?;
        Ok(match ty {
            ScalarType::Boolean => ScalarTerm::BooleanField {
                version,
                root,
                path,
            },
            ScalarType::Integer(scalar_type) => ScalarTerm::IntegerField {
                version,
                root,
                path,
                scalar_type,
            },
            ScalarType::IeeeFloat(format) => ScalarTerm::IeeeFloatField {
                version,
                root,
                path,
                format,
            },
            _ => return Err("record requirement has a nonnumeric structural read"),
        })
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[Segment],
        _: StructuralCaseId,
    ) -> Result<CaseAnswer, Self::Error> {
        Err("a field correspondence cannot select a case")
    }

    fn case_predicate(
        &mut self,
        root: PlaceId,
        path: &[Segment],
        case: StructuralCaseId,
    ) -> Result<Proposition, Self::Error> {
        Ok(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(root, self.path(root, path)?),
            case,
        })
    }
}
