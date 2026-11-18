//! Produce an interval certificate from ordinary integer proof questions.
//! The independent kernel replays every endpoint group and normalized set;
//! this search never publishes a guessed order or a successful Boolean.

use proof_admission::content_intervals::{endpoint_requirements, projected_endpoints};
use semantic_vocabulary::IntegerMathTerm;

use super::{ProofNode, ProofRule, Proposition, Prover, Unproven, node};

impl Prover<'_> {
    pub(super) fn content_intervals(&mut self, goal: &Proposition) -> Result<ProofNode, Unproven> {
        let Proposition::ContentConservation(equation) = goal else {
            return Err("interval proof has a different goal form");
        };
        let endpoints = projected_endpoints(self.context, equation)?;
        let mut groups: Vec<Vec<IntegerMathTerm>> = Vec::new();
        for endpoint in endpoints {
            let mut low = 0;
            let mut high = groups.len();
            let mut equal = None;
            while low < high {
                let position = low + (high - low) / 2;
                let representative = &groups[position][0];
                let below = self
                    .prove(&Proposition::IntegerMathLessOrEqual(
                        endpoint.clone(),
                        representative.clone(),
                    ))
                    .is_ok();
                let above = self
                    .prove(&Proposition::IntegerMathLessOrEqual(
                        representative.clone(),
                        endpoint.clone(),
                    ))
                    .is_ok();
                if below && above {
                    equal = Some(position);
                    break;
                }
                if below {
                    high = position;
                } else if above {
                    low = position + 1;
                } else {
                    return Err("live integer facts do not order the content endpoints");
                }
            }
            if let Some(position) = equal {
                groups[position].push(endpoint);
                groups[position].sort();
            } else {
                groups.insert(low, vec![endpoint]);
            }
        }
        let premises = endpoint_requirements(&groups)?
            .iter()
            .map(|requirement| self.prove(requirement))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(node(
            goal.clone(),
            ProofRule::ContentIntervalNormalization {
                endpoint_groups: groups,
                premises,
            },
        ))
    }
}
