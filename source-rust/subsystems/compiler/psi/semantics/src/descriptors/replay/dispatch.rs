use super::engine::{EffectObservations, Summary, UnobservedWrite};
use super::*;
use std::sync::Arc;

pub(crate) fn join(mut joined: Summary, summaries: &[Arc<Summary>]) -> Summary {
    joined.invalidates_contents |= summaries.iter().any(|summary| summary.invalidates_contents);
    if let super::engine::ResultObservations::Observed {
        unknown,
        contents,
        slots,
    } = &mut joined.result
    {
        for path in summaries
            .iter()
            .flat_map(|summary| summary.result.unknown())
        {
            if !unknown.contains(path) {
                unknown.push(path.clone());
            }
        }
        for summary in summaries {
            let other = summary.result.contents();
            if other.len() != contents.len()
                || other
                    .iter()
                    .zip(contents.iter())
                    .any(|(left, right)| left.path != right.path)
            {
                *slots = None;
                contents.clear();
                break;
            }
            for (output, incoming) in contents.iter_mut().zip(other) {
                output.origins.extend(incoming.origins.iter().cloned());
            }
        }
    }
    let has_unobserved = std::iter::once(&joined)
        .chain(summaries.iter().map(Arc::as_ref))
        .any(|summary| matches!(summary.effects, EffectObservations::Incomplete { .. }));
    if has_unobserved {
        let mut writes = Vec::new();
        let mut located = true;
        for summary in std::iter::once(&joined).chain(summaries.iter().map(Arc::as_ref)) {
            if let EffectObservations::Incomplete { unobserved, .. } = &summary.effects {
                let Some(unknown) = unobserved else {
                    located = false;
                    break;
                };
                for write in unknown {
                    if !writes.contains(write) {
                        writes.push(write.clone());
                    }
                }
            }
            for effect in summary.effects.known_writes() {
                let write = UnobservedWrite {
                    parameter: effect.parameter,
                    steps: effect.steps.clone(),
                };
                if !writes.contains(&write) {
                    writes.push(write);
                }
            }
        }
        joined.effects = EffectObservations::Incomplete {
            observed: Vec::new(),
            unobserved: located.then_some(writes),
        };
        return joined;
    }
    let EffectObservations::Observed(effects) = &mut joined.effects else {
        unreachable!()
    };
    for summary in summaries {
        for effect in summary
            .effects
            .observed()
            .expect("unobserved effects joined above")
        {
            if !effects
                .iter()
                .any(|prior| prior.parameter == effect.parameter && prior.steps == effect.steps)
            {
                effects.push(DescriptorStoredEffect {
                    parameter: effect.parameter,
                    steps: effect.steps.clone(),
                    origins: Vec::new(),
                });
            }
        }
    }
    for effect in effects {
        for summary in summaries {
            if let Some(source) = summary
                .effects
                .observed()
                .expect("unobserved effects joined above")
                .iter()
                .find(|source| {
                    source.parameter == effect.parameter && source.steps == effect.steps
                })
            {
                effect.origins.extend(source.origins.iter().cloned());
            } else {
                effect
                    .origins
                    .push(DescriptorStoredOrigin::IncomingContents {
                        position: effect.parameter,
                        steps: effect.steps.clone(),
                        projection: Projection::new(&[]),
                    });
            }
        }
    }
    joined
}
