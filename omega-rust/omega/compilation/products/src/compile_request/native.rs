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
#[derive(Clone)]
pub struct NativeProductRequest {
    /// Save the original checked inputs for later source-free native compilation.
    /// Ordinary compilation leaves this off and performs no serialization.
    pub retain_native_inputs: bool,
    /// Explicit caller assumptions for exact installation intervals.
    pub installation_assumptions: Vec<crate::InstallationEnvironmentAssumption>,
    pub image: NativeImageDemand,
    pub private_entry_ram: Vec<terminal_installation::PrivateEntryRamInitialization>,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub optimization_rollback: OptimizationRollback,
}

/// Where native production stops. `RuntimeAdmissionOnly` serves Terminal-only
/// execution (`omega run --engine terminal`): the request passes the same
/// Build, package, provider and entry admission as an image, then settles as
/// a Terminal product carrying its admitted runtime provisions, with no
/// object or image emitted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeImageDemand {
    #[default]
    Emit,
    RuntimeAdmissionOnly,
}

/// Check the complete caller roster before projecting target-local requests.
/// A row for an unrequested target is an error, never silently discarded.
pub fn validate_installation_assumption_targets(
    rows: &[crate::InstallationEnvironmentAssumption],
    targets: &[target::TargetProfile],
) -> Result<(), String> {
    for row in rows {
        row.validate_description()?;
        if !targets.contains(&row.profile()) {
            return Err(format!(
                "installation assumption names unrequested target {}",
                row.profile().target_name()
            ));
        }
    }
    Ok(())
}

/// Partition the already typed request once, retaining caller order within each
/// selected target. Exact artifact, entry and operation joins remain native work.
pub fn partition_installation_assumptions(
    rows: Vec<crate::InstallationEnvironmentAssumption>,
    targets: &[target::TargetProfile],
) -> Result<Vec<Vec<crate::InstallationEnvironmentAssumption>>, String> {
    validate_installation_assumption_targets(&rows, targets)?;
    let mut groups = vec![Vec::new(); targets.len()];
    for row in rows {
        let position = targets
            .iter()
            .position(|target| *target == row.profile())
            .ok_or("installation assumption lost its validated target")?;
        groups[position].push(row);
    }
    Ok(groups)
}
