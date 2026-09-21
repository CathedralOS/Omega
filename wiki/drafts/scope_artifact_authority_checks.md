# Scope: ARTIFACT-AUTHORITY-CHECKS

Scope-verification record for the dispatched name `ARTIFACT-AUTHORITY-CHECKS`
(planner item NEW-SV-ARTIFACT-AUTHORITY-CHECKS-SCOPE). Audited at
`f2c1762c2c3`; re-audited at `c924529921d` on linux x86-64 (z157).

## Resolution

The name carries only a mined stub row (`TASKS.md` ~:8439,
"mined candidate; verify scope then implement"), minted after the first
audit. The surface it names — artifact/install
authority checks — is owned by **WIRE-RUNTIME-AND-INSTALLATION**
(TASKS.md, the admitted executable installation contract from
[executable_installation.md](../spec/build/executable_installation.md)):
reusable artifact validation, consumed placement authority, W^X and
instruction-fetch coherence, physical invocation, uninstall and replacement
joins.

## Landed substrate (authority checks already on main)

Crate `executable-installation`
(`omega-rust/omega/backend/runtime/executable-installation/`):

- The linear state model exists — admit, materialize, freeze, validate,
  install, retire, quarantine — each failed transition returning its inputs.
- `authority_digests.rs` owns the domain-separated authority digests and
  normalized identities (ArtifactId, PlacementPlanId, InstalledCodeId,
  AdmissionReceiptId, …) plus report-only fingerprints.
- `InstallAuthority` names the provider-canonical `InstallationFactDigest`
  set the operation must establish; `install_validated` rejects a receipt
  that omits or renames a demanded fact (an established superset installs),
  returning every input — the required-facts gate.
- `entry_references.rs` owns the control-flow-integrity gate:
  `InstalledCode::seal_entry_reference` turns an admitted-entry selection
  into a sealed `InstalledEntryReference` retaining the satisfier and the
  demanded `EntryContractDigest`, gated on provider-established requirement
  compatibility, instruction-fetch visibility, and demanded
  `EntryReferenceFactDigest` facts; the reference borrows `InstalledCode`,
  so the realization cannot retire while an entry remains possible.
- `uninstall.rs` owns the drain-or-quarantine join;
  `replacement.rs`/`replacement_quarantine.rs` own the patch-then-drain
  replacement join over declared entries carrying bound admitted fragments.
- The final image carries placed-executable and initialized-data
  inventories bound by `image-emission/src/installed_artifact.rs` to an
  `InstalledCode` occurrence, rejecting unclassified gaps, a truncated
  compiler prefix, and resolver-claimed uninstalled thunk addresses.

Verified substrate health at audit time: the owning row's re-verification
records the crate at 130/130 (`cargo nextest run -p
executable-installation`, linux x86-64 at `8570ba9ae8`); re-witnessed
130/130 green at `c924529921d`.

## Open legs (all live on the owning row, none unowned)

- **Provider write-to-execute operation**: no provider performs the
  write-to-execute transition, the target cache and ordering work, or
  instruction-fetch visibility — `InstallationReceipt` still only records
  what a provider would report.
- **Physical invocation**: no caller invokes installed code through an
  `InstalledEntryReference`; entry references are produced but never
  exercised by a live call path.
- **Provider patching operation**: the replacement join's receipts likewise
  only record what a provider would report.
- **Omega source route**: no `.omg` file names an admitted artifact, a
  placement, or installed code, so no canary reaches the surface.

## Slice verdict

No independent slice exists under this name. The authority-check
substrate is landed and exercised; every open leg is enumerated on
WIRE-RUNTIME-AND-INSTALLATION's row and is provider/runtime work rather
than a doc or check slice. The name should be retired from the dispatch
list in favor of the owning row (or dispatched as an explicit
WIRE-RUNTIME-AND-INSTALLATION leg).

## Fences observed at audit time

No live claim fences `executable-installation/` or this file (159 live
claims at `c924529921d`; TOPOLOGY-PRIVATE-PIPE-INSTALLATION is held only
by item name, no path overlap). Provider and
physical-invocation neighbors were claim-held in earlier wave dumps
(UEFI-PHYSICAL-SEMANTIC-ENTRY on native-realization surfaces); re-check
`tools/claims.py status` before starting a code leg.

> Field note (e1dc35c92948..97be15c1b592 review): the dispatched name has no board row; either mint one resolved row pointing at WIRE-RUNTIME-AND-INSTALLATION (so the planner stops re-dispatching it) or delete this draft once the landed-substrate table is folded into that row.

> Field note (z157, `c924529921d`): a mined stub row now exists at ~:8439 — stamp it with this verdict (resolved-by-delegation to WIRE-RUNTIME-AND-INSTALLATION's enumerated legs) at the next board-touching leg; the slice verdict is unchanged.
