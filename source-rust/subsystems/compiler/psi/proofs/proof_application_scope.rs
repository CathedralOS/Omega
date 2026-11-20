use super::{CheckedMachineDeclaration, TerminalIdentityIndex};
use arena::Arena;
use proof_applications::{
    ProofApplicationPlan, ProofApplications, ProofCallGraph,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProofDeclarationId(pub u32);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofScopeDeclaration {
    #[default]
    Missing,
    Nominal(TerminalIdentityIndex),
    Callable(CheckedMachineDeclaration),
    Operator(super::BoundaryOperatorRequirement),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCallableHeader {
    pub declaration: ProofDeclarationId,
    pub callable: Result<TerminalIdentityIndex, String>,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub source_target: Option<String>,
    pub product_eligible: bool,
    pub supplies_checked_body: bool,
    pub type_parameter_count: usize,
    pub compiler_evaluation:
        Result<semantic_vocabulary::compiler_evaluation::CompilerEvaluationDeclaration, String>,
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
            type_parameter_count: 0,
            compiler_evaluation: Err("checked compiler-evaluation facts are absent".into()),
            synchronous_invocations: Err(
                "checked synchronous invocation facts are absent".into()
            ),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofApplicationScope {
    pub declarations: Arena<ProofScopeDeclaration>,
    pub callable_headers: Arena<ProofCallableHeader>,
    pub callables: Arena<ProofCallableAssociation>,
    pub plan: ProofApplicationPlan<ProofDeclarationId>,
    pub calls: ProofCallGraph<ProofDeclarationId>,
    pub recursive_components: Arena<ProofCheckedRecursiveComponent>,
    pub recursive_members: Arena<ProofDeclarationId>,
    pub recursive_calls: Arena<ProofCheckedRecursiveCall>,
    pub pending_evaluation_conditions: Arena<ProofDeclarationId>,
}

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
