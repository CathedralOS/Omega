//! The executed dependency preparations and checked call paths of one Build.
use psi_program_identity::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
use semantic_vocabulary::{MachineId, OperationId, PackageKeyIdentity};
use source_provenance::DependencyScope;
use wire::cursor::Cursor;

pub type BuildCallSite = (MachineId, OperationId);

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
pub enum BuildExecutionStep {
    Prepared {
        purpose: DependencyScope,
        request_ordinal: usize,
        package: PackageKeyIdentity,
        inputs: [u8; 32],
        child_build_source: Option<[u8; 32]>,
        handoff: Option<[u8; 32]>,
    },
    Resumed {
        previous: TerminalPsiIdentity,
        successor: TerminalPsiIdentity,
        calls: Vec<(BuildCallSite, BuildCallSite)>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, record_storage::Record)]
pub struct BuildExecutionHistory {
    steps: Vec<BuildExecutionStep>,
}

impl BuildExecutionHistory {
    pub fn steps(&self) -> &[BuildExecutionStep] {
        &self.steps
    }

    pub fn prepared(
        &mut self,
        purpose: DependencyScope,
        request_ordinal: usize,
        package: PackageKeyIdentity,
        inputs: [u8; 32],
        child_build_source: Option<[u8; 32]>,
        handoff: Option<[u8; 32]>,
    ) {
        self.steps.push(BuildExecutionStep::Prepared {
            purpose,
            request_ordinal,
            package,
            inputs,
            child_build_source,
            handoff,
        });
    }

    pub fn resumed(
        &mut self,
        previous: TerminalPsiIdentity,
        successor: TerminalPsiIdentity,
        calls: impl IntoIterator<Item = (BuildCallSite, BuildCallSite)>,
    ) -> Result<(), &'static str> {
        let calls: Vec<_> = calls.into_iter().collect();
        if calls.is_empty() || self.last_program().is_some_and(|code| code != previous) {
            return Err("Build resumption has no call path or follows a different checked program");
        }
        self.steps.push(BuildExecutionStep::Resumed {
            previous,
            successor,
            calls,
        });
        Ok(())
    }

    fn last_program(&self) -> Option<TerminalPsiIdentity> {
        self.steps.iter().rev().find_map(|step| match step {
            BuildExecutionStep::Resumed { successor, .. } => Some(*successor),
            BuildExecutionStep::Prepared { .. } => None,
        })
    }

    pub(crate) fn ends_with(&self, terminal: TerminalPsiIdentity) -> bool {
        let mut last = None;
        for step in &self.steps {
            if let BuildExecutionStep::Resumed {
                previous,
                successor,
                calls,
            } = step
            {
                if calls.is_empty() || last.is_some_and(|id| id != *previous) {
                    return false;
                }
                last = Some(*successor);
            }
        }
        last.is_none_or(|id| id == terminal)
    }

    /// Feed the same canonical fields to the receipt digest and the package wire format.
    pub fn write_bytes(&self, mut put: impl FnMut(&[u8])) {
        put(&(self.steps.len() as u64).to_le_bytes());
        for step in &self.steps {
            match step {
                BuildExecutionStep::Prepared {
                    purpose,
                    request_ordinal,
                    package,
                    inputs,
                    child_build_source,
                    handoff,
                } => {
                    put(&[
                        0,
                        match purpose {
                            DependencyScope::Product => 0,
                            DependencyScope::Build => 1,
                        },
                    ]);
                    put(&(*request_ordinal as u64).to_le_bytes());
                    put(&package.digest());
                    put(inputs);
                    for evidence in [child_build_source, handoff] {
                        match evidence {
                            Some(commitment) => {
                                put(&[1]);
                                put(commitment);
                            }
                            None => put(&[0]),
                        }
                    }
                }
                BuildExecutionStep::Resumed {
                    previous,
                    successor,
                    calls,
                } => {
                    put(&[1]);
                    for program in [previous, successor] {
                        put(&program.vocabulary_marker.get().to_le_bytes());
                        put(program.program_fingerprint.as_bytes());
                    }
                    put(&(calls.len() as u64).to_le_bytes());
                    for (old, new) in calls {
                        for (machine, operation) in [old, new] {
                            put(&machine.get().to_le_bytes());
                            put(&operation.get().to_le_bytes());
                        }
                    }
                }
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut input = Cursor::new(bytes);
        let count = usize::try_from(input.u64().ok()?).ok()?;
        // A preparation needs its two digests, ordinal, purpose and optional evidence tags.
        const MIN_EVENT_BYTES: usize = 1 + 1 + 8 + 32 + 32 + 1 + 1;
        if count > input.remaining() / MIN_EVENT_BYTES {
            return None;
        }
        let mut history = Self {
            steps: Vec::with_capacity(count),
        };
        for _ in 0..count {
            match input.byte().ok()? {
                0 => {
                    let purpose = match input.byte().ok()? {
                        0 => DependencyScope::Product,
                        1 => DependencyScope::Build,
                        _ => return None,
                    };
                    let ordinal = usize::try_from(input.u64().ok()?).ok()?;
                    let package = PackageKeyIdentity::from_digest(input.array().ok()?)?;
                    let inputs = input.array().ok()?;
                    let mut evidence = || match input.byte().ok()? {
                        0 => Some(None),
                        1 => Some(Some(input.array().ok()?)),
                        _ => None,
                    };
                    history.prepared(purpose, ordinal, package, inputs, evidence()?, evidence()?);
                }
                1 => {
                    let mut program = || {
                        Some(TerminalPsiIdentity {
                            vocabulary_marker: VocabularyMarker::new(input.u16().ok()?)?,
                            program_fingerprint: SemanticFingerprint::from_bytes(
                                input.array().ok()?,
                            ),
                        })
                    };
                    let previous = program()?;
                    let successor = program()?;
                    let depth = usize::try_from(input.u64().ok()?).ok()?;
                    if depth == 0 || depth > input.remaining() / 32 {
                        return None;
                    }
                    let mut site = || {
                        Some((
                            MachineId::new(input.u64().ok()?)?,
                            OperationId::new(input.u64().ok()?)?,
                        ))
                    };
                    let mut calls = Vec::with_capacity(depth);
                    for _ in 0..depth {
                        calls.push((site()?, site()?));
                    }
                    history.resumed(previous, successor, calls).ok()?;
                }
                _ => return None,
            }
        }
        (input.remaining() == 0).then_some(history)
    }
}
