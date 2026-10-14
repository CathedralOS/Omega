use optimization_core::OptimizationRollback;
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};

/// One target's native product request: the admission profile the Terminal
/// artifact is verified under, the receiving mechanism-classification policy,
/// the optional service-permission admission policy, and the optimization
/// rollback to settle. The source root belongs to the report settlement that
/// admitted this stop, not to the production request.
///
/// The terminal-authority policy is the receiving authority's explicit
/// mechanism table: the closed compiler-intrinsic inventory classifies itself,
/// while normalized-foreign, syscall, and checked-physical leaves may need
/// explicit rows. Ordinary production retains unclassified mechanisms; receiver
/// admission and physical exclusions require complete classification. This is an independent axis --
/// root grants, service permissions, and provider execution custody
/// never substitute for or widen its rows.
///
/// `terminal_authority_permission_policy` is the second, receiver-admission
/// axis. `None` means ordinary production makes no receiver-admission claim:
/// accepted package permissions are never projected into receiver rows, and
/// the emitted artifact cannot satisfy an explicit admission replay. `Some`
/// (including an explicit empty policy) adjudicates every closure leaf
/// against exact supplied rows.
pub struct NativeProductRequest {
    pub private_entry_ram: Vec<terminal_installation::PrivateEntryRamInitialization>,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub optimization_rollback: OptimizationRollback,
}
