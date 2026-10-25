//! Join executable post-state effects independently of public result contracts.
use super::replay::Summary;
use super::*;

/// Stored effects are a may-union over every normal realization. An alternative
/// that leaves a slot untouched contributes its incoming contents. Result
/// predicates and geometry remain those of the public requirement.
pub(super) fn join(mut joined: Summary, summaries: &[Summary]) -> Summary {
    joined.invalidates_contents = summaries.iter().any(|summary| summary.invalidates_contents);
    for summary in summaries {
        for effect in &summary.effects {
            if !joined
                .effects
                .iter()
                .any(|prior| prior.parameter == effect.parameter && prior.steps == effect.steps)
            {
                joined.effects.push(DescriptorStoredEffect {
                    parameter: effect.parameter,
                    steps: effect.steps.clone(),
                    origins: Vec::new(),
                });
            }
        }
    }
    for effect in &mut joined.effects {
        for summary in summaries {
            if let Some(source) = summary
                .effects
                .iter()
                .find(|source| source.parameter == effect.parameter && source.steps == effect.steps)
            {
                for origin in &source.origins {
                    effect.origins.push(origin.clone());
                }
            } else {
                let incoming = DescriptorStoredOrigin::IncomingContents {
                    position: effect.parameter,
                    steps: effect.steps.clone(),
                    projection: Projection::new(&[]),
                };
                effect.origins.push(incoming);
            }
        }
    }
    joined
}
