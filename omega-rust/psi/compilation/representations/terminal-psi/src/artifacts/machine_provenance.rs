//! Source-completion diagnostic attribution, never semantic or admission evidence.
//!
//! Psi projects source names and spans while it owns the checked source. A
//! policy can attribute a rejected Terminal site without retaining that source
//! or depending on a lowering implementation. Source-completion storage retains
//! this companion; it is not part of canonical Terminal semantics.

use semantic_vocabulary::MachineId;
use source::SourceSpan;
use std::collections::BTreeMap;

#[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalMachineProvenance {
    machines: BTreeMap<MachineId, (String, Option<SourceSpan>)>,
    callers: BTreeMap<MachineId, MachineId>,
}

#[cfg(test)]
mod tests {
    use super::TerminalMachineProvenance;
    use crate::{TerminalModule, UnoptimizedBehaviorOperation, UnoptimizedMachineBehavior};
    use semantic_vocabulary::{BlockId, InvocationSite, MachineId, OperationId};

    #[test]
    fn root_attribution_uses_retained_edges_without_source_or_live_bodies() {
        let entry = MachineId::new(1).unwrap();
        let helper = MachineId::new(2).unwrap();
        let cleanup = MachineId::new(3).unwrap();
        let other = MachineId::new(4).unwrap();
        let block = BlockId::new(1).unwrap();
        let site = InvocationSite::Operation(OperationId::new(1).unwrap());
        let mut module = TerminalModule::declarations();
        module.unoptimized_behavior = vec![
            UnoptimizedMachineBehavior {
                machine: entry,
                operations: vec![UnoptimizedBehaviorOperation::Invocation {
                    block,
                    site,
                    callee: helper,
                }],
            },
            UnoptimizedMachineBehavior {
                machine: helper,
                operations: vec![
                    UnoptimizedBehaviorOperation::Invocation {
                        block,
                        site,
                        callee: entry,
                    },
                    UnoptimizedBehaviorOperation::NominalCleanup {
                        block,
                        callee: cleanup,
                    },
                ],
            },
            UnoptimizedMachineBehavior {
                machine: other,
                operations: Vec::new(),
            },
        ];
        let mut names = TerminalMachineProvenance::default();
        names.name(entry, "generic declaration label".into(), None);
        let rooted = names.for_root(&module, entry, Some(("selected application", None)));
        assert!(rooted.label(cleanup).contains("selected application"));
        assert!(!rooted.label(other).contains("selected application"));
        let another = names.for_root(&module, other, Some(("other application", None)));
        assert!(!another.label(cleanup).contains("selected application"));
        assert!(!another.label(cleanup).contains("other application"));
        assert!(
            names.label(entry).contains("generic declaration label"),
            "rooting mutated shared names"
        );
    }
}

impl TerminalMachineProvenance {
    /// Attribute one root using retained pre-optimization invocation edges.
    /// The shared names are diagnostic payload; neither they nor this walk
    /// establish reach, provider selection or execution authority.
    pub fn for_root(
        &self,
        module: &crate::TerminalModule,
        entry: MachineId,
        name: Option<(&str, Option<SourceSpan>)>,
    ) -> Self {
        let mut provenance = Self {
            machines: self.machines.clone(),
            callers: BTreeMap::new(),
        };
        if let Some((name, span)) = name {
            provenance.machines.insert(entry, (name.to_owned(), span));
        }
        let mut pending = std::collections::VecDeque::from([entry]);
        let mut visited = std::collections::BTreeSet::from([entry]);
        while let Some(current) = pending.pop_front() {
            let Some(body) = module
                .unoptimized_behavior
                .iter()
                .find(|body| body.machine == current)
            else {
                continue;
            };
            for operation in &body.operations {
                let mut visit = |callee| {
                    if visited.insert(callee) {
                        provenance.record_caller(callee, current);
                        pending.push_back(callee);
                    }
                };
                match operation {
                    crate::UnoptimizedBehaviorOperation::Invocation { callee, .. }
                    | crate::UnoptimizedBehaviorOperation::NominalCleanup { callee, .. } => {
                        visit(*callee)
                    }
                    crate::UnoptimizedBehaviorOperation::BoundaryInvocation {
                        boundary, ..
                    } => {
                        for candidate in module
                            .provider_candidates
                            .iter()
                            .filter(|candidate| candidate.boundary == *boundary)
                        {
                            visit(candidate.candidate);
                        }
                    }
                    _ => {}
                }
            }
        }
        provenance
    }

    pub fn name(&mut self, machine: MachineId, name: String, span: Option<SourceSpan>) {
        self.machines.entry(machine).or_insert((name, span));
    }

    pub fn record_caller(&mut self, machine: MachineId, caller: MachineId) {
        self.callers.entry(machine).or_insert(caller);
    }

    fn named_ancestor(
        &self,
        machine: MachineId,
    ) -> Option<(MachineId, &(String, Option<SourceSpan>))> {
        let mut current = machine;
        for _ in 0..=self.callers.len() {
            if let Some(named) = self.machines.get(&current) {
                return Some((current, named));
            }
            current = *self.callers.get(&current)?;
        }
        None
    }

    pub fn label(&self, machine: MachineId) -> String {
        match self.named_ancestor(machine) {
            Some((named, (name, _))) if named == machine => {
                format!("machine `{name}` (Terminal machine {machine})")
            }
            Some((named, (name, _))) => format!(
                "Terminal machine {machine}, reached through machine `{name}` (Terminal machine {named})"
            ),
            None => format!("Terminal machine {machine}"),
        }
    }

    pub fn source_span(&self, machine: MachineId) -> Option<SourceSpan> {
        let (_, (_, span)) = self.named_ancestor(machine)?;
        *span
    }
}
