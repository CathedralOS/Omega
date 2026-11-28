use super::*;
use crate::source_state::expression::{
    CompletedValueCapture, MatchFieldAction, MatchPattern, TableMatchExpression, TableSavedSubject,
    ValueCaptureOwner,
};

pub(in crate::read) struct ClauseMatchSubject {
    capture: arena::Handle<CompletedValueCapture>,
    owner: SymbolHandle,
    root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    value_type: ValueType,
}

impl Program<'_> {
    pub(super) fn contract_match_projection(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        saved: &TableSavedSubject,
    ) -> Checked<Option<(PlaceId, Vec<ScalarReadProjection>, ValueType)>> {
        let subject = self
            .clause_match_subjects
            .iter()
            .rev()
            .find(|subject| subject.capture == saved.capture && subject.owner == scope.owner);
        let Some(subject) =
            subject.filter(|_| saved.component == language_core::PatternComponent::Fields)
        else {
            return Err(refuse_at(
                id,
                Unserved::Contract("match payload outside its selected subject"),
            ));
        };
        Ok(Some((
            subject.root,
            subject.path.iter().copied().map(Into::into).collect(),
            subject.value_type,
        )))
    }

    /// An exhaustive match of unguarded sum cases is the conjunction of its
    /// case implications. The captured paths still name the original value,
    /// so arm fields retain their case, carrier, permissions and owner.
    pub(super) fn contract_case_match(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        matched: &TableMatchExpression,
    ) -> Checked<Proposition> {
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        let table = &self.typed.expression_table;
        if !table.contains_value_capture(matched.capture) {
            return Err(unserved("match without its contract subject"));
        }
        let capture = table.value_capture(matched.capture);
        if !matches!(capture.owner, ValueCaptureOwner::Match) || capture.subject != matched.subject
        {
            return Err(unserved("match without its contract subject"));
        }
        let Some((root, path, value_type)) = self.projected(id, scope, matched.subject)? else {
            return Err(unserved("match on a value without a structural path"));
        };
        let record = self
            .opened(id, Some(value_type), Access::Read)?
            .ok_or_else(|| unserved("match on a non-structural value"))?;
        let mut covered = vec![false; self.data_type(id, record.record())?.cases().len()];
        if covered.is_empty() {
            return Err(unserved("contract match on a value without sum cases"));
        }
        let mut clauses = Vec::new();
        self.clause_match_subjects.push(ClauseMatchSubject {
            capture: matched.capture,
            owner: scope.owner,
            root,
            path: path.clone(),
            value_type,
        });
        let result = (|| {
            for arm in table.match_arms(matched.arms) {
                let MatchPattern::Destructure(pattern) = &arm.pattern else {
                    return Err(unserved(
                        "contract match with a pattern other than a sum case",
                    ));
                };
                if table
                    .match_fields(pattern.fields)
                    .iter()
                    .any(|field| matches!(field.action, MatchFieldAction::Equal(_)))
                {
                    return Err(unserved("contract match with a payload test"));
                }
                let case =
                    self.case_pattern(id, record, pattern, SourceOrigin::of(arm.source_span))?;
                if std::mem::replace(&mut covered[case], true) {
                    return Err(unserved("contract match with overlapping cases"));
                }
                let membership = Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(root, path.clone()),
                    case: self.data_type(id, record.record())?.cases()[case].id,
                };
                let expected =
                    canonical::conjunction(vec![membership.clone(), Proposition::Truth], id)?;
                if self.clause(id, scope, pattern.condition)? != expected {
                    return Err(unserved(
                        "contract match whose arm condition differs from its case",
                    ));
                }
                clauses.push(Proposition::Implication {
                    premise: Box::new(membership),
                    conclusion: Box::new(self.clause(id, scope, arm.value)?),
                });
            }
            if covered.contains(&false) {
                return Err(unserved("contract match that omits a case"));
            }
            canonical::conjunction(clauses, id)
        })();
        self.clause_match_subjects.pop();
        result
    }
}
