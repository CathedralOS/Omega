//! The executable-installation ladder: the six lifecycle transitions and
//! the one-call drives that compose a provider operation with the transition
//! consuming its receipt.
//!
//! No operation converts arbitrary bytes into executable memory. Immutable
//! artifacts are admitted once and reused; each installation instead consumes
//! one exact destination authority through frozen and validated states. The
//! ladder, in lifecycle order:
//!
//! 1. [`admit_executable`] — a canonical [`Artifact`] plus validator evidence
//!    becomes an [`AdmittedArtifact`]; container-v1 candidates without strong
//!    authority commitments are refused here.
//! 2. [`materialize_and_freeze`] — the admitted artifact, one claimed
//!    [`CodePlacement`], the materializer's [`MaterializedArtifactBytes`] and
//!    the provider's [`MaterializationReceipt`] become a [`FrozenPlacement`];
//!    every mismatch between placement, output, plan and byte length refuses.
//! 3. [`ValidatedPlacement`] — the final-byte producer's retained placement
//!    feeds installation without a compiler-owned certificate replay.
//! 4. [`install_validated`] — the provider's [`InstallationReceipt`] under an
//!    [`InstallAuthority`] becomes [`InstalledCode`]; fetch visibility and
//!    W^X support are required facts.
//! 5. `InstalledCode::seal_entry_reference` (in `entry_references`) — the
//!    control-flow-integrity gate that seals one admitted entry into an
//!    [`InstalledEntryReference`]; nothing in [`InstalledCode`] is callable
//!    before it.
//! 6. [`retire_installed`] — a [`RetirementReceipt`] under a
//!    [`RetirementAuthority`] returns the placement as a
//!    [`RetiredInstallation`]; `retirement` owns the uninstall, replacement
//!    and quarantine joins over this ending.
//!
//! Each transition consumes provider evidence and returns every input on
//! refusal. The `impl OwnedImageProvider` block below is the one-call form
//! for the provider this crate ships: `install_code`, `seal_code_entry`,
//! `call_code`, `retire_code`, `quarantine_code`, `uninstall_code` and
//! `replace_code` each perform the contracted provider operation, mint the
//! receipt, and run it through the matching transition, so a caller need not
//! hand-sequence provider call, receipt custody and gate. A refused drive
//! returns the receiver-side custody, the authority, and whichever receipts
//! were minted, so nothing is partially consumed.
//!
//! The drain-or-quarantine join composes the same way: a caller that knows
//! residual authority still holds the realization supplies the attributed
//! `MappingQuarantineCause`; the provider parks the mapping, reports the
//! drain as incomplete, and the join routes the custody to quarantine. A
//! caller with no residual authority drives the plain retirement ending.
//!
//! The domains beside this file: `authority_digests` derives the
//! domain-separated digests, `artifacts` carries artifacts, their container
//! and admission, `placement` the placement claims, materialization and
//! final validation, `installation` installed code and its registry,
//! `entry_references` the control-flow-integrity gate, `retirement` the
//! retirement, uninstall, replacement and quarantine endings,
//! `post_handoff_writer` the separate installed-entry writer protocol, and
//! `owned_image_provider` the provider that performs the operations over
//! resident image buffers it owns.

