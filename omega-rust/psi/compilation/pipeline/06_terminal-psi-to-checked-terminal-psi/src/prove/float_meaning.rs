//! Select connected meaning equations, then ask the independent checker to
//! normalize them. Only selected facts expand into actual premise proofs.
//! IEEE comparisons and raw stored-value equalities are not meaning equations;
//! ordinary value-equality transport handles genuine scalar copies separately.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use proof_admission::{FloatMeaningNormalizationError, check_float_meaning_normalization};
use semantic_vocabulary::FloatMeaningClause;

use super::{ProofNode, ProofRule, Proposition, Prover, ScalarTerm, Unproven, node};

fn subjects(clause: &FloatMeaningClause) -> impl Iterator<Item = &ScalarTerm> {
    clause
        .sources()
        .filter(|source| !matches!(source, ScalarTerm::IeeeFloatLiteral(_)))
}

impl Prover<'_> {
    pub(super) fn float_meaning(&self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        let Proposition::FloatMeaningEqual(clause) = goal else {
            return Err("float meaning proof has a different goal form");
        };
        match check_float_meaning_normalization(goal, []) {
            Ok(()) => {
                return Ok(node(
                    goal.clone(),
                    ProofRule::FloatMeaningNormalization(Vec::new()),
                ));
            }
            Err(FloatMeaningNormalizationError::NotEstablished) => {}
            Err(
                FloatMeaningNormalizationError::WorkLimit
                | FloatMeaningNormalizationError::ValueLimit,
            ) => {
                return Err("float meaning proof exceeds the normalization budget");
            }
            Err(
                FloatMeaningNormalizationError::Malformed(_)
                | FloatMeaningNormalizationError::Semantic(_),
            ) => {
                return Err("float meaning goal has no valid catalog normalization");
            }
        }
        // Inventory once. Each subject and each selected equation is visited
        // once; a large disconnected set of facts does not become a closure
        // table or a scan of every fact at each reached endpoint.
        let mut indexed: BTreeMap<ScalarTerm, Vec<usize>> = BTreeMap::new();
        for (index, fact) in self.facts.iter().enumerate() {
            if let Proposition::FloatMeaningEqual(clause) = &fact.proposition {
                for source in subjects(clause) {
                    indexed.entry(source.clone()).or_default().push(index);
                }
            }
        }
        let mut pending: VecDeque<_> = subjects(clause).cloned().collect();
        let mut visited = BTreeSet::new();
        let mut selected = vec![false; self.facts.len()];
        let mut equations = Vec::new();
        while let Some(subject) = pending.pop_front() {
            if !visited.insert(subject.clone()) {
                continue;
            }
            let Some(candidates) = indexed.get(&subject) else {
                continue;
            };
            for &index in candidates {
                if std::mem::replace(&mut selected[index], true) {
                    continue;
                }
                equations.push(index);
                let Proposition::FloatMeaningEqual(clause) = &self.facts[index].proposition else {
                    unreachable!("indexed meaning equation")
                };
                pending.extend(subjects(clause).cloned());
            }
        }
        equations.sort_unstable();
        match check_float_meaning_normalization(
            goal,
            equations
                .iter()
                .map(|&index| &self.facts[index].proposition),
        ) {
            Ok(()) => Ok(node(
                goal.clone(),
                ProofRule::FloatMeaningNormalization(
                    equations
                        .into_iter()
                        .map(|index| self.facts[index].proof(&self.facts))
                        .collect(),
                ),
            )),
            Err(FloatMeaningNormalizationError::NotEstablished) => {
                Err("the live float meaning equations do not establish exact representability")
            }
            Err(
                FloatMeaningNormalizationError::WorkLimit
                | FloatMeaningNormalizationError::ValueLimit,
            ) => Err("float meaning proof exceeds the normalization budget"),
            Err(
                FloatMeaningNormalizationError::Malformed(_)
                | FloatMeaningNormalizationError::Semantic(_),
            ) => Err("float meaning premises have no valid catalog normalization"),
        }
    }
}
