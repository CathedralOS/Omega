//! Close selected source roots before handing application custody to Omega.
//!
//! One declaration table retains exact checked identities in occurrence order.
//! Missing declarations stay deferred: checking-only products need not lower
//! a body. Composition admission consumes each result lazily, before optimization.

use arena::Arena;
use checked_trees::CheckedTrees;
use semantic_vocabulary::MachineId;
use source::SourceSpan;
use terminal_psi::{CheckedMachineDeclaration, TerminalIdentityTable, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum RootDeclaration {
    #[default]
    Missing,
    Available(CheckedMachineDeclaration),
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PreparedRoot {
    declaration: RootDeclaration,
    name: Option<String>,
    span: Option<SourceSpan>,
}

/// An invocation companion, not an execution grant or a portable-use roster.
/// No source handles survive preparation. Each declaration retains the same
/// owner, signature, contract and specialization as the ordinary body producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTerminalRoots {
    identities: TerminalIdentityTable,
    roots: Arena<PreparedRoot>,
}

pub struct RetainedTerminalRoot<'a> {
    pub machine: MachineId,
    pub name: Option<&'a str>,
    pub span: Option<SourceSpan>,
}

impl PreparedTerminalRoots {
    /// Source policy has selected these roots already. Declaration projection
    /// neither selects providers nor repeats calling-plan admission.
    pub fn prepare(
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        machines: impl IntoIterator<Item = symbols::SymbolHandle>,
    ) -> Self {
        // Preserve the existing portable-owner requirement. A fresh private
        // identity table cannot confer the lowering's invocation-local custody.
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let mut roots = Arena::new();
        for machine in machines {
            let declaration = match crate::machine_lowering::declarations::identity(
                &mut identities,
                checked,
                sources,
                machine,
            ) {
                Ok(declaration) => RootDeclaration::Available(declaration),
                Err(reason) => RootDeclaration::Unavailable(reason),
            };
            roots.insert(PreparedRoot {
                declaration,
                name: checked
                    .typed
                    .machine(machine)
                    .map(|body| body.name.as_str().to_owned()),
                span: checked.typed.symbols.symbol_source_span(machine),
            });
        }
        Self {
            identities: identities.build(),
            roots,
        }
    }

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
