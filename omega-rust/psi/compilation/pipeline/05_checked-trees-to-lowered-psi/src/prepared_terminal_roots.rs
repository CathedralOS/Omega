//! Retain the canonical roots selected from Psi's declaration inventory.
//!
//! One shared declaration table retains exact identities in occurrence order.
//! Missing declarations stay deferred: checking-only products need not lower
//! a body. Composition admission consumes each result lazily, before optimization.

use arena::Arena;
use semantic_vocabulary::MachineId;
use source::SourceSpan;
use std::sync::Arc;
use terminal_psi::{CheckedMachineDeclaration, TerminalIdentityTable, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum RootDeclaration {
    #[default]
    Missing,
    Available(CheckedMachineDeclaration),
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PreparedRoot {
    pub declaration: RootDeclaration,
    pub name: Option<String>,
    pub span: Option<SourceSpan>,
}

/// An invocation companion, not an execution grant or a portable-use roster.
/// No source handles survive preparation. Each declaration retains the same
/// owner, signature, contract and specialization as the ordinary body producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTerminalRoots {
    pub(crate) identities: Arc<TerminalIdentityTable>,
    pub(crate) roots: Arena<PreparedRoot>,
}

pub struct RetainedTerminalRoot<'a> {
    pub machine: MachineId,
    pub name: Option<&'a str>,
    pub span: Option<SourceSpan>,
}

impl PreparedTerminalRoots {
    /// Resolve each exact declaration against the unoptimized composition.
    /// Neither a nominal name alone nor a matching ABI may select another body.
    pub fn retained_roots<'a>(
        &'a self,
        module: &'a TerminalModule,
    ) -> impl Iterator<Item = Result<RetainedTerminalRoot<'a>, String>> + 'a {
        self.roots.iter().map(|(_, root)| {
            let declaration = match &root.declaration {
                RootDeclaration::Available(declaration) => declaration,
                RootDeclaration::Unavailable(reason) => return Err(reason.clone()),
                RootDeclaration::Missing => {
                    return Err("selected root lost its checked declaration".into());
                }
            };
            let mut matches = module.machines.iter().filter(|machine| {
                machine.declaration.as_ref().is_some_and(|candidate| {
                    declaration.same_identity_as(&self.identities, candidate, &module.identities)
                })
            });
            let machine = matches
                .next()
                .ok_or("selected root has no exact retained declaration")?;
            if matches.next().is_some() {
                return Err("selected root has multiple exact retained declarations".into());
            }
            Ok(RetainedTerminalRoot {
                machine: machine.id,
                name: root.name.as_deref(),
                span: root.span,
            })
        })
    }
}
