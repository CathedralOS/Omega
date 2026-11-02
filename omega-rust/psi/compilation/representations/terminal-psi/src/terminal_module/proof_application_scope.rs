//! Declaration scope of retained proof debt, not executable reachability.
//!
//! These edges survive source disposal and use the same membership walk as
//! checked-source closure. Target admission supplies settled provider edges.
//! Keeping a declaration here never grants an executable body or provider.

use super::{CheckedMachineDeclaration, TerminalIdentityIndex};
use arena::Arena;
use semantic_vocabulary::proof_applications::{
    ProofApplicationPlan, ProofApplications, ProofCallGraph,
};

/// One-based, module-local declaration coordinate. Zero is absence; source
/// arena positions and generations never enter this namespace.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProofDeclarationId(pub u32);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofScopeDeclaration {
    #[default]
    Missing,
    Nominal(TerminalIdentityIndex),
    Callable(CheckedMachineDeclaration),
    Operator(super::BoundaryOperatorRequirement),
    /// Dormant symbolic metadata may lack a closed portable description.
    /// Keep its refusal without making it an executable or published supplier.
    Unavailable(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofCallableAssociation {
    #[default]
    Missing,
    Callable {
        declaration: ProofDeclarationId,
        callable: super::TargetMachineCallable,
    },
}

/// Lightweight checked-source facts tied to this scope's declaration roster.
/// Full signature/contract availability is independent. These facts never grant
/// invocation, provider satisfaction, foreign backing or execution permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCallableHeader {
    pub declaration: ProofDeclarationId,
    pub callable: Result<TerminalIdentityIndex, String>,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub source_target: Option<String>,
    pub product_eligible: bool,
    pub supplies_checked_body: bool,
    pub compiler_evaluation:
        Result<semantic_vocabulary::compiler_evaluation::CompilerEvaluationDeclaration, String>,
    /// Checker-normalized realized calls, including explicit abstract edges.
    /// Neither public permission ceilings nor emitted calls replace this list.
    pub synchronous_invocations: Result<Vec<DeclaredSynchronousBoundaryTarget>, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredSynchronousBoundaryTarget {
    pub declaration: TerminalIdentityIndex,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub application: Option<TerminalIdentityIndex>,
}

impl Default for ProofCallableHeader {
    fn default() -> Self {
        Self {
            declaration: ProofDeclarationId::default(),
            callable: Err("checked callable header is absent".into()),
            package: None,
            source_target: None,
            product_eligible: false,
            supplies_checked_body: false,
            compiler_evaluation: Err("checked compiler-evaluation facts are absent".into()),
            synchronous_invocations: Err("checked synchronous invocation facts are absent".into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofApplicationScope {
    pub declarations: Arena<ProofScopeDeclaration>,
    pub callable_headers: Arena<ProofCallableHeader>,
    /// Exact producer associations, not identities reconstructed from names.
    /// Declarations without emitted code remain in the proof plan only.
    pub callables: Arena<ProofCallableAssociation>,
    pub plan: ProofApplicationPlan<ProofDeclarationId>,
    pub calls: ProofCallGraph<ProofDeclarationId>,
    pub recursive_components: Arena<ProofCheckedRecursiveComponent>,
    pub recursive_members: Arena<ProofDeclarationId>,
    pub recursive_calls: Arena<ProofCheckedRecursiveCall>,
    /// Still-partial invocation formation cannot execute to justify itself.
    /// Zero records an unknown owner and therefore fences every invocation.
    pub pending_evaluation_conditions: Arena<ProofDeclarationId>,
}

/// A complete checked body component. Members are exact published bodies and
/// call ordinals refer to their retained operational summaries, not emitted
/// instructions or a name-based call graph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofCheckedRecursiveComponent {
    pub members: arena::HandleSpan<ProofDeclarationId>,
    pub calls: arena::HandleSpan<ProofCheckedRecursiveCall>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofCheckedRecursiveCall {
    pub caller: ProofDeclarationId,
    pub call_ordinal: u32,
    pub callee: ProofDeclarationId,
}

impl ProofApplicationScope {
    pub fn callable_declaration(
        &self,
        callable: super::TargetMachineCallable,
    ) -> Option<ProofDeclarationId> {
        self.callables.iter().find_map(|(_, row)| match row {
            ProofCallableAssociation::Callable {
                declaration,
                callable: emitted,
            } if *emitted == callable => Some(*declaration),
            _ => None,
        })
    }

    pub fn applications(
        &self,
    ) -> ProofApplications<'_, ProofDeclarationId, ProofCallGraph<ProofDeclarationId>> {
        ProofApplications::new(&self.plan, &self.calls)
    }

    pub fn declaration(&self, identity: ProofDeclarationId) -> Option<&ProofScopeDeclaration> {
        let position = usize::try_from(identity.0.checked_sub(1)?).ok()?;
        self.declarations.storage_slice().get(position)
    }
}
