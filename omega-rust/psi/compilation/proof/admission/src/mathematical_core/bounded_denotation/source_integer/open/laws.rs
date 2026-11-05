//! Instantiate only the universal laws supplied in the current premise
//! telescope. Matching chooses arguments; the returned application remains an
//! ordinary kernel term over that exact law's evidence.

use super::*;

#[derive(Clone)]
pub(super) enum LawArgument {
    Value(usize),
    Condition(usize),
}
#[derive(Clone)]
pub(super) struct SourceLaw {
    values: usize,
    conditions: Vec<TermHandle>,
    conclusion: TermHandle,
    pub evidence: TermHandle,
    order: Vec<LawArgument>,
}
pub(super) struct LawApplication {
    pub arguments: Vec<LawArgument>,
    pub values: Vec<TermHandle>,
    pub conditions: Vec<TermHandle>,
    pub evidence: TermHandle,
}

impl Denotation {
    pub(super) fn source_universal_law(
        &mut self,
        mut proposition: TermHandle,
        evidence: TermHandle,
    ) -> Result<Option<SourceLaw>, BoundedDenotationError> {
        let integer = self.integer_constant()?;
        let mut values = 0;
        let mut conditions = Vec::new();
        let mut order = Vec::new();
        loop {
            proposition = self.source_head(proposition)?;
            let Term::Pi { domain, codomain } = self.arena.get(proposition) else {
                break;
            };
            let domain = self.source_head(domain)?;
            if self.arena.structurally_equal(domain, integer) {
                order.push(LawArgument::Value(values));
                values += 1;
                proposition = codomain;
            } else {
                if self.occurs_free(codomain, 0) {
                    return Ok(None);
                }
                order.push(LawArgument::Condition(conditions.len()));
                conditions.push((domain, values));
                // The body does not use this evidence binder. Substitution
                // removes its index without inventing a proof of its domain.
                proposition = substitute(&mut self.arena, codomain, integer);
            }
        }
        if values == 0 {
            return Ok(None);
        }
        let conditions = conditions
            .into_iter()
            .map(|(condition, scope)| {
                crate::mathematical_core::shift(
                    &mut self.arena,
                    condition,
                    0,
                    (values - scope) as u32,
                )
            })
            .collect();
        Ok(Some(SourceLaw {
            values,
            conditions,
            conclusion: proposition,
            evidence,
            order,
        }))
    }

    fn source_law_matches(
        &mut self,
        pattern: TermHandle,
        actual: TermHandle,
        values: &mut [Option<TermHandle>],
        depth: usize,
    ) -> Result<bool, BoundedDenotationError> {
        if depth > 128 {
            return Err(BoundedDenotationError::DepthLimitExceeded);
        }
        let pattern = self.source_head(pattern)?;
        let actual = self.source_head(actual)?;
        if let Term::Variable(position) = self.arena.get(pattern) {
            let Some(index) = values.len().checked_sub(position as usize + 1) else {
                return Ok(false);
            };
            return Ok(match values[index] {
                Some(previous) => self.arena.structurally_equal(previous, actual),
                None => {
                    values[index] = Some(actual);
                    true
                }
            });
        }
        if self.arena.structurally_equal(pattern, actual) {
            return Ok(true);
        }
        let children = match (self.arena.get(pattern), self.arena.get(actual)) {
            (
                Term::Apply {
                    function: a,
                    argument: b,
                },
                Term::Apply {
                    function: c,
                    argument: d,
                },
            ) => vec![(a, c), (b, d)],
            (
                Term::Id {
                    ty: a,
                    left: b,
                    right: c,
                },
                Term::Id {
                    ty: d,
                    left: e,
                    right: f,
                },
            ) => vec![(a, d), (b, e), (c, f)],
            (Term::Fst { pair: a }, Term::Fst { pair: b })
            | (Term::Snd { pair: a }, Term::Snd { pair: b }) => vec![(a, b)],
            // Matching never invents a binder correspondence for an opaque
            // dependent function. An already equal term passed above.
            _ => return Ok(false),
        };
        for (pattern, actual) in children {
            if !self.source_law_matches(pattern, actual, values, depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn source_law_application_plan(
        &mut self,
        law: &SourceLaw,
        goal: TermHandle,
    ) -> Result<Option<LawApplication>, BoundedDenotationError> {
        let mut values = vec![None; law.values];
        if !self.source_law_matches(law.conclusion, goal, &mut values, 0)? {
            return Ok(None);
        }
        let Some(values) = values.into_iter().collect::<Option<Vec<_>>>() else {
            return Ok(None);
        };
        let conditions = law
            .conditions
            .iter()
            .map(|&condition| {
                values.iter().rev().fold(condition, |body, &argument| {
                    substitute(&mut self.arena, body, argument)
                })
            })
            .collect();
        Ok(Some(LawApplication {
            arguments: law.order.clone(),
            values,
            conditions,
            evidence: law.evidence,
        }))
    }
}
