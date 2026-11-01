//! Retain the canonical roots selected from Psi's declaration inventory.
//!
//! One shared declaration table retains exact identities in occurrence order.
//! Missing declarations stay deferred: checking-only products need not lower
//! a body. Composition admission consumes each result lazily, before optimization.

use crate::{CheckedMachineDeclaration, TerminalIdentityTable, TerminalModule};
use arena::Arena;
use semantic_vocabulary::MachineId;
use source::SourceSpan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TerminalRootDeclaration {
    #[default]
    Missing,
    Available(CheckedMachineDeclaration),
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalRootRequest {
    pub declaration: TerminalRootDeclaration,
    pub name: Option<String>,
    pub span: Option<SourceSpan>,
}

/// An invocation companion, not an execution grant or a portable-use roster.
/// No source handles survive preparation. Each declaration retains the same
/// owner, signature, contract and specialization as the ordinary body producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTerminalRoots {
    identities: Arc<TerminalIdentityTable>,
    roots: Arena<TerminalRootRequest>,
}

pub struct RetainedTerminalRoot<'a> {
    pub machine: MachineId,
    pub name: Option<&'a str>,
    pub span: Option<SourceSpan>,
}

impl PreparedTerminalRoots {
    /// Freeze requested roots without granting execution authority. Invocation-local
    /// owners cannot establish a portable selected-root declaration. Missing and
    /// unavailable declarations remain deferred until the root is consumed.
    pub fn new(
        identities: Arc<TerminalIdentityTable>,
        requests: impl IntoIterator<Item = TerminalRootRequest>,
    ) -> Self {
        let mut roots = Arena::new();
        for mut request in requests {
            if let TerminalRootDeclaration::Available(declaration) = &request.declaration {
                let has_portable_owner = !identities
                    .is_invocation_local(declaration.machine.identity())
                    && !identities.is_invocation_local(declaration.state.identity())
                    && !identities.is_invocation_local(declaration.semantic_declaration)
                    && declaration
                        .specialization
                        .as_ref()
                        .is_none_or(|specialization| {
                            !identities.is_invocation_local(specialization.template.identity())
                                && !identities.is_invocation_local(
                                    specialization.template_semantic_declaration,
                                )
                        });
                if !has_portable_owner {
                    request.declaration = TerminalRootDeclaration::Unavailable(
                        "checked declaration has no exact package or toolchain owner".into(),
                    );
                }
            }
            roots.insert(request);
        }
        Self { identities, roots }
    }

    /// Resolve each exact declaration against the unoptimized composition.
    /// Neither a nominal name alone nor a matching ABI may select another body.
    pub fn retained_roots<'a>(
        &'a self,
        module: &'a TerminalModule,
    ) -> impl Iterator<Item = Result<RetainedTerminalRoot<'a>, String>> + 'a {
        self.roots.iter().map(|(_, root)| {
            let declaration = match &root.declaration {
                TerminalRootDeclaration::Available(declaration) => declaration,
                TerminalRootDeclaration::Unavailable(reason) => return Err(reason.clone()),
                TerminalRootDeclaration::Missing => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CheckedMachineSpecialization, NominalDeclarationIdentity, TerminalIdentityTableBuilder,
    };

    fn declaration(table: &mut TerminalIdentityTableBuilder) -> CheckedMachineDeclaration {
        CheckedMachineDeclaration {
            machine: NominalDeclarationIdentity::new(table.intern("package::Machine")),
            state: NominalDeclarationIdentity::new(table.intern("package::Machine::entry")),
            semantic_declaration: table.intern("entry-signature"),
            parameter_types: vec![table.intern("qualified-input")],
            callable: table.intern("qualified-callable"),
            result_type: table.intern("qualified-result"),
            contract: [17; 32],
            specialization: Some(CheckedMachineSpecialization {
                capture: None,
                template: NominalDeclarationIdentity::new(table.intern("package::Template")),
                template_callable: table.intern("template-callable"),
                template_semantic_declaration: table.intern("template-signature"),
                commitment: [29; 32],
            }),
        }
    }

    #[test]
    fn preparation_retains_complete_declarations_and_deferred_occurrences() {
        let mut table = TerminalIdentityTableBuilder::default();
        let declaration = declaration(&mut table);
        let requests = [
            TerminalRootRequest::default(),
            TerminalRootRequest {
                declaration: TerminalRootDeclaration::Unavailable(
                    "unselected declaration unavailable".into(),
                ),
                ..Default::default()
            },
            TerminalRootRequest {
                declaration: TerminalRootDeclaration::Available(declaration.clone()),
                name: Some("selected".into()),
                span: None,
            },
        ];
        let prepared = PreparedTerminalRoots::new(Arc::new(table.build()), requests.clone());
        let retained: Vec<_> = prepared
            .roots
            .iter()
            .map(|(_, request)| request.clone())
            .collect();
        assert_eq!(retained, requests);
    }

    #[test]
    fn every_invocation_local_declaration_owner_is_refused_at_the_boundary() {
        for owner in 0..5 {
            let mut table = TerminalIdentityTableBuilder::for_invocation();
            let mut declaration = declaration(&mut table);
            let local = table.intern_invocation_local("private-coordinate").unwrap();
            match owner {
                0 => declaration.machine = NominalDeclarationIdentity::new(local),
                1 => declaration.state = NominalDeclarationIdentity::new(local),
                2 => declaration.semantic_declaration = local,
                3 => {
                    declaration.specialization.as_mut().unwrap().template =
                        NominalDeclarationIdentity::new(local)
                }
                4 => {
                    declaration
                        .specialization
                        .as_mut()
                        .unwrap()
                        .template_semantic_declaration = local
                }
                _ => unreachable!(),
            }
            let prepared = PreparedTerminalRoots::new(
                Arc::new(table.build()),
                [TerminalRootRequest {
                    declaration: TerminalRootDeclaration::Available(declaration),
                    ..Default::default()
                }],
            );
            let (_, request) = prepared.roots.iter().next().unwrap();
            assert_eq!(
                request.declaration,
                TerminalRootDeclaration::Unavailable(
                    "checked declaration has no exact package or toolchain owner".into()
                ),
                "owner {owner}"
            );
        }
    }
}
