//! Source-declaration joins and human descriptions for exclusion evidence.
//!
//! The producing join reads exact typed declarations; receiver replay continues
//! to reconstruct ownership independently from canonical module identities.
//! Descriptions label possible behavior and missing evidence, never witnessed
//! runtime execution, and neither names nor diagnostic text grant authority.

use super::{BehaviorExclusion, BoundaryServiceOwners, EvidenceGapKind, ProhibitedSite};
use std::collections::BTreeMap;

impl BoundaryServiceOwners {
    /// Join normalized boundary requirements to the services their typed traits
    /// declare. The caller supplies the checked program used for this lowering;
    /// loaded declarations alone do not establish execution or receiver authority.
    pub fn from_declared_traits(
        program: &typed_trees::TypedTrees,
        module: &terminal_psi::TerminalModule,
    ) -> Self {
        let mut owning_service: BTreeMap<String, String> = BTreeMap::new();
        for definition in program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
        {
            for signature in program.trait_machine_signatures(definition) {
                owning_service.insert(
                    program.trait_requirement_callable_identity_text(definition, signature),
                    definition.name.as_str().to_owned(),
                );
            }
        }
        Self::from_module(module, &|boundary_identity| {
            owning_service.get(boundary_identity).cloned()
        })
    }
}

/// Map a checked boundary-trait symbol to the identity Terminal service
/// declarations carry for it: the trait's declared name.
pub fn boundary_trait_identity(
    program: &typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<String> {
    program
        .traits()
        .iter()
        .find(|definition| definition.symbol == symbol && definition.is_boundary)
        .map(|definition| definition.name.as_str().to_owned())
}

impl BehaviorExclusion {
    /// Describe this exclusion, resolving service identities in the exact module.
    pub fn describe(self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            Self::CrashCause(cause) => {
                format!("crash cause {cause:?}")
            }
            Self::Service(service) => {
                match module
                    .services
                    .iter()
                    .find(|declaration| declaration.id == service)
                {
                    Some(declaration) => format!("service `{}`", declaration.identity),
                    None => format!("service {service}"),
                }
            }
            // A physical-class selection is a demand on the mechanism closure:
            // this walk never yields a prohibited site for it, but the arm keeps
            // the description total if one is ever named.
            Self::PhysicalAuthorityClass(class) => {
                format!("physical authority class {class:?}")
            }
        }
    }
}

impl ProhibitedSite {
    /// Describe a retained possible behavior site, not an observed execution.
    pub fn describe(&self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            ProhibitedSite::CrashTerminator { block } => {
                format!("crash terminator in block {block}")
            }
            ProhibitedSite::BoundaryCall {
                block,
                operation,
                boundary,
            } => {
                let identity = module
                    .boundary_machines
                    .iter()
                    .find(|declaration| declaration.id == *boundary)
                    .map(|declaration| declaration.identity.as_str())
                    .unwrap_or("<undeclared boundary>");
                format!("call to boundary `{identity}` (block {block}, operation {operation})")
            }
            ProhibitedSite::PortWrite { block, operation } => {
                format!("port write (block {block}, operation {operation})")
            }
        }
    }
}

impl EvidenceGapKind {
    /// Describe why the retained closure cannot establish absence.
    pub fn describe(&self) -> String {
        match self {
            EvidenceGapKind::UnknownCallee(callee) => {
                format!("call target machine {callee} is not retained in the module")
            }
            EvidenceGapKind::UnknownBoundary(boundary) => {
                format!("boundary {boundary} has no retained declaration")
            }
            EvidenceGapKind::UnknownEntry => {
                "the selected entry is not retained in the module".to_owned()
            }
        }
    }
}
