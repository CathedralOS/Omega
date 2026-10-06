//! Normalized build-host requests derived from admitted filesystem custody.
//!
//! The package review protocol records logical roots, captured inventories and
//! resource ceilings, never host paths or live capabilities. The compiler
//! supplies the admitted execution mode before effects execute; projecting a
//! request does not grant that mode or authorize an execution.

use super::BuildCapturedSourceInventory;
use super::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY, BuildMachineFilesystemScope};
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineExecutionMode,
    BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
};

/// Retained restricted-request consent bound by a consumer to one activation.
/// Each projected request joins in issue order before its Build effect executes;
/// refusal returns the consumer's diagnostics unchanged. The worker owns the
/// guard across execution, so accepted meaning is owned rather than borrowed.
///
/// An absent guard is observation-only: requests remain in retained custody for
/// review, but execution consults no accepted meaning and issues no grants.
pub trait RestrictedBuildGrants: Send {
    fn admit(
        &mut self,
        request: &RestrictedBuildRequest,
    ) -> Result<(), Vec<diagnostics::Diagnostic>>;
}

/// The restricted host operation one admitted build activation requests.
///
/// Members are the normalized review vocabulary for restricted build-time
/// host reach (wiki/spec/packages/acceptance.md#restricted-build-acceptance).
/// A new restricted operation lands here so package review can surface it
/// beside — never inside — the package's product-authority rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildOperation {
    /// Execute the admitted build machine against scoped real filesystem
    /// roots: reads land under granted read roots and writes under granted
    /// write roots, and anything else is refused before the host is touched.
    ScopedFilesystemExecution,
    /// Execute against the real filesystem without path grants. Build
    /// admission does not currently select this mode; it is named so a
    /// review cannot silently drop it if an evaluator ever carries it.
    UnscopedFilesystemExecution,
}

/// The logical grant root one restricted build request names. Package
/// vocabulary only — never the host path the root maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestrictedBuildGrantRoot {
    /// The package's source inventory.
    SourceInventory,
    /// The activation's staged build-output tree.
    StagedOutput,
    /// A compiler-issued grant root identity this projection does not name.
    Other(u32),
}

/// One logical root grant inside a restricted build request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildGrant {
    root: RestrictedBuildGrantRoot,
    /// The granted root's canonical metadata index narrows every operation
    /// to captured membership and kind.
    narrowed: bool,
    /// The immutable captured inventory bound to the root, when one exists.
    /// Reads then hit a fresh private materialization of that inventory,
    /// never the live source root.
    captured: Option<BuildCapturedSourceInventory>,
}

impl RestrictedBuildGrant {
    /// Rebuild a grant from retained review evidence. The value was produced
    /// by the authority projection before it entered the record.
    #[doc(hidden)]
    pub const fn from_retained(
        root: RestrictedBuildGrantRoot,
        narrowed: bool,
        captured: Option<BuildCapturedSourceInventory>,
    ) -> Self {
        Self {
            root,
            narrowed,
            captured,
        }
    }

    pub const fn root(&self) -> RestrictedBuildGrantRoot {
        self.root
    }

    pub const fn narrowed(&self) -> bool {
        self.narrowed
    }

    pub const fn captured(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured
    }
}

/// The bounds admission attached to one restricted build request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildBounds {
    /// The shared filesystem sponsor account's staging ceilings, present
    /// when a caller-installed sponsor binds every granted operation.
    filesystem_sponsor_limits: Option<filesystem_authority::FilesystemSponsorLimits>,
    /// The evaluation sponsor's resource ceilings, present when a
    /// caller-installed sponsor caps this activation's evaluator.
    evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
    /// The declared sealed outputs that must complete in the staged-output
    /// tree before this activation's result may publish.
    required_outputs: Vec<Vec<u8>>,
    /// Whether this activation publishes retained outputs only.
    artifact_only: bool,
}

impl RestrictedBuildBounds {
    /// Rebuild bounds from retained review evidence. The value was produced
    /// by the authority projection before it entered the record.
    #[doc(hidden)]
    pub const fn from_retained(
        filesystem_sponsor_limits: Option<filesystem_authority::FilesystemSponsorLimits>,
        evaluation_sponsor_limits: Option<BuildEvaluationSponsorLimits>,
        required_outputs: Vec<Vec<u8>>,
        artifact_only: bool,
    ) -> Self {
        Self {
            filesystem_sponsor_limits,
            evaluation_sponsor_limits,
            required_outputs,
            artifact_only,
        }
    }

    pub const fn filesystem_sponsor_limits(
        &self,
    ) -> Option<filesystem_authority::FilesystemSponsorLimits> {
        self.filesystem_sponsor_limits
    }

    pub const fn evaluation_sponsor_limits(&self) -> Option<BuildEvaluationSponsorLimits> {
        self.evaluation_sponsor_limits
    }

    pub fn required_outputs(&self) -> &[Vec<u8>] {
        &self.required_outputs
    }

    pub const fn artifact_only(&self) -> bool {
        self.artifact_only
    }
}

/// One normalized restricted build-host request admitted on a build
/// activation (wiki/spec/packages/acceptance.md#restricted-build-acceptance).
///
/// The request records, in package vocabulary, what the package's build
/// machine asks of the host before that authority executes: the restricted
/// operation, the logical resource roots it reaches, and the bounds
/// admission attached. It carries no host paths, live handles, or ephemeral
/// capabilities, so an installer may retain it as accepted-request meaning —
/// for example in a lock — without storing machine state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedBuildRequest {
    operation: RestrictedBuildOperation,
    read_grants: Vec<RestrictedBuildGrant>,
    write_grants: Vec<RestrictedBuildGrant>,
    bounds: RestrictedBuildBounds,
    build_execution_profile: Option<target::TargetProfile>,
    selected_target_profile: Option<target::TargetProfile>,
}

impl RestrictedBuildRequest {
    pub const fn operation(&self) -> RestrictedBuildOperation {
        self.operation
    }

    /// Logical roots this request may read, in grant order.
    pub fn read_grants(&self) -> &[RestrictedBuildGrant] {
        &self.read_grants
    }

    /// Logical roots this request may write, in grant order.
    pub fn write_grants(&self) -> &[RestrictedBuildGrant] {
        &self.write_grants
    }

    pub const fn bounds(&self) -> &RestrictedBuildBounds {
        &self.bounds
    }

    /// The admitted build execution profile this request's build-scope
    /// declarations were checked against. `None` names an admitted host no
    /// catalogued profile describes.
    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
    }

    /// The product target the requesting compilation realizes with this
    /// request. The Build evaluates once for every target; each realized
    /// target's compilation records its own copy (`for_target`).
    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    /// Rebuild a request from retained review evidence. The value was
    /// produced by the authority projection before it entered the record.
    #[doc(hidden)]
    pub const fn from_retained(
        operation: RestrictedBuildOperation,
        read_grants: Vec<RestrictedBuildGrant>,
        write_grants: Vec<RestrictedBuildGrant>,
        bounds: RestrictedBuildBounds,
        build_execution_profile: Option<target::TargetProfile>,
        selected_target_profile: Option<target::TargetProfile>,
    ) -> Self {
        Self {
            operation,
            read_grants,
            write_grants,
            bounds,
            build_execution_profile,
            selected_target_profile,
        }
    }

    /// This request as recorded by one realized target's compilation.
    #[must_use]
    pub const fn for_target(mut self, profile: Option<target::TargetProfile>) -> Self {
        self.selected_target_profile = profile;
        self
    }
}

impl BuildMachineFilesystemScope {
    /// Project the admitted authority decision into normalized restricted
    /// build-host requests.
    ///
    /// Virtual execution and compiler-owned captured-input/private-staging work
    /// require no restricted-action decision. Other real host reach remains
    /// restricted, including scoped access without private staging custody.
    /// Logical roots are reported by package
    /// vocabulary identity — the host paths the roots map to never enter the
    /// projection.
    pub fn restricted_build_requests(
        &self,
        evaluation_sponsor: Option<&BuildEvaluationSponsor>,
        artifact_only: bool,
        execution_mode: &BuildMachineExecutionMode,
    ) -> Vec<RestrictedBuildRequest> {
        let BuildMachineExecutionMode::Granted { filesystem, .. } = execution_mode else {
            return Vec::new();
        };
        if self.is_private_snapshot_execution(filesystem) {
            return Vec::new();
        }
        let (operation, read_roots, write_roots, filesystem_sponsor) = match filesystem {
            BuildMachineFilesystemAccess::RealScoped(grants) => (
                RestrictedBuildOperation::ScopedFilesystemExecution,
                &grants.read_roots[..],
                &grants.write_roots[..],
                None,
            ),
            BuildMachineFilesystemAccess::RealScopedSponsored { grants, sponsor } => (
                RestrictedBuildOperation::ScopedFilesystemExecution,
                &grants.read_roots[..],
                &grants.write_roots[..],
                Some(sponsor),
            ),
            BuildMachineFilesystemAccess::RealUnscoped => (
                RestrictedBuildOperation::UnscopedFilesystemExecution,
                &[][..],
                &[][..],
                None,
            ),
            BuildMachineFilesystemAccess::Virtual => return Vec::new(),
        };
        let captured = self.captured_source_inventory();
        let grant = |grant_root: &BuildMachineFilesystemGrantRoot| {
            let identity = grant_root.identity();
            let (root, captured) = if identity == BUILD_SOURCE_ROOT_IDENTITY {
                (RestrictedBuildGrantRoot::SourceInventory, captured)
            } else if identity == BUILD_OUTPUT_ROOT_IDENTITY {
                (RestrictedBuildGrantRoot::StagedOutput, None)
            } else {
                (
                    RestrictedBuildGrantRoot::Other(identity.get()),
                    self.named_input_inventory(identity),
                )
            };
            RestrictedBuildGrant {
                root,
                narrowed: grant_root.canonical_metadata().is_some(),
                captured,
            }
        };
        vec![RestrictedBuildRequest {
            operation,
            read_grants: read_roots.iter().map(&grant).collect(),
            write_grants: write_roots.iter().map(&grant).collect(),
            bounds: RestrictedBuildBounds {
                filesystem_sponsor_limits: filesystem_sponsor
                    .and_then(|sponsor| sponsor.limits().ok()),
                evaluation_sponsor_limits: evaluation_sponsor.map(BuildEvaluationSponsor::limits),
                required_outputs: self.required_outputs().iter().cloned().collect(),
                artifact_only,
            },
            build_execution_profile: self.activation().build_execution_profile(),
            selected_target_profile: None,
        }]
    }
}
