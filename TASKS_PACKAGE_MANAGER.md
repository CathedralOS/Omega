# Tasks: Package Manager

Remaining work for repository install/update and compiler-derived capability
review. Design: [Build And Package Model](wiki/design_briefs/build_and_package_model.md).
Subsystem entrance: [packages/README.md](omega-rust/omega/packages/README.md).

The project trusts whoever lands its dependency and lock changes. `omega.lock`
records exact resolutions and the capabilities/assumptions the project accepted.
It does not certify packages, prove an audit occurred, or authenticate its own
acceptance. Compiler proof and reachability checking still apply to the selected
source. Native artifact verification belongs to compilation.

Deliver a complete transaction for the supported package surface. Reject a
candidate when its authority cannot be checked or represented; unrelated
backend work and future artifact classes do not block supported candidates.

Security work must name a concrete compiler or package invariant. Host
credential policy and audit seriousness belong to the operator, not Omega.
Escalate a genuinely unclear authority boundary to an owner question before
adding machinery; do not invent proof-of-review or host-security requirements.

## 1. Resolve and check the candidate

- [ ] **PACKAGE-REVIEW-PROJECTION.** Finish the compiler-to-manager report
  needed for supported source dependencies: package-qualified public APIs,
  declared and inferred reach, reachable implementation authority, selected
  providers, opaque external supplies, and explicit accepted assumptions.
  Include transitive dependency paths and relevant build/generated-source
  authority. Use the earliest checked compiler representation that establishes
  each fact; no additional IR or native binary is required.
  Acceptance: false reach ceilings, spoofed boundary identities, unresolved
  proof obligations, and omitted transitive authority reject; unsupported
  candidates produce a specific diagnostic. Generic effects must remain
  conservative until concrete substitutions are checked.

- [ ] **BUILD-REVIEW-INTEGRATION.** Reuse the existing scoped build execution
  and generated-source handoff during candidate checking. Resolve dependencies
  before running dependency build code; obtain required project decisions
  before supplying dangerous build capabilities. Include generated code in
  final review and detect relevant source/build drift before committing.
  Acceptance: a dependency cannot gain resolver credentials, alter its
  dependency graph during execution, write outside admitted output roots, or
  hide authority in generated code. Installing source does not publish a native
  executable.

## 2. Complete review integration

- [ ] **EXPLICIT-DEPENDENCY-REPLACEMENT.** Preserve command-authored replacement
  intent when both alias and source change, through planning, review resume,
  and publication in `manager/src/operations/package_commands/`. Graph
  comparison must not infer pairing from package names or authored positions.
  Acceptance: one explicit replacement is reported alongside its policy deltas;
  unrelated additions/removals are not paired, and stale decisions reject.

- [ ] **AUDIT-RESULT-INTEGRATION.** Connect source-code changes and optional
  audit advice to install/update review. Obtain exact old source when available;
  if unavailable, retain the accepted policy comparison and offer standalone
  candidate audit. Keep package-authored source/prose separate from compiler
  capability triage. Advisory service failure cannot suppress compiler findings
  or replace per-change decisions. Acceptance: a source upgrade exposes its
  available code diff or explicit unavailability, retained dangerous authority
  still recommends audit, and the flow works without an advisory service.
  No Y/N approval prompt, audit receipt, or proof-of-review requirement.

## 3. Commands and integration tests

- [ ] **FAILED-FETCH-RETRY.** Repair retry behavior in
  `sources/acquisition/src/git/`: a failed acquisition invalidates
  `source.identity` but leaves an entry that makes subsequent attempts fail on
  missing metadata before retrying the fetch. Acceptance: an interrupted or
  failed fetch can be retried without manual cache surgery; report the original
  transport failure, preserve accepted pins, and never reuse unverified content.

- [ ] **NAMED-WORKSPACE-INSTALL.** Expose the existing named Git workspace
  selection in `omega install` and its manager operation. Retain declared-name
  discovery and optional local alias overrides; do not ask callers for member
  filesystem paths. Acceptance: a named remote member installs and imports,
  undeclared/duplicate names reject, and updating one member reports all
  affected reachable members without refreshing unrelated repositories.

- [ ] **OFFLINE-COMMAND-SELECTION.** Expose existing offline exact-pin recovery
  through command options for locked compilation and package operations where
  applicable. Acceptance: cached accepted/proposed pins work without network;
  missing required content fails clearly without selector refresh or accepted
  file mutation. Do not add a new credential or host-isolation framework.

- [ ] **OMEGA-AUDIT-PACKAGES.** Render the selected graph, exact pins, accepted
  baseline, freshly checked reach/API/assumption findings, and dependency paths.
  Clearly distinguish accepted policy from current compiler findings and
  unavailable analysis. Acceptance: users can identify which package and API
  introduces dangerous authority; the output makes no claim that lock authors
  performed an audit.

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Exercise install/update through the
  real command and network adapters using pure, dangerous, capability-changing,
  same-name/different-source, transitive-authority, and generated-source
  fixtures. Refresh the remote fixture sources and exact pins that still use
  retired target declarations; then prove remote install, selected update, and
  import through the default alias. Run HTTPS coverage where the private
  fixtures' credentials are configured, independently of SSH coverage.
  Cover missing baselines/old source, invalid proofs, spoofed
  boundaries, concurrent edits, and interruption recovery.
  Run relevant package, resolver, compiler-handoff, and architecture checks.
  On supported Windows workers run process-tree cleanup and Job Object
  resource-limit canaries, reporting unavailable platforms explicitly.
  Acceptance: successful commands publish usable dependencies and every failed
  stage preserves or recovers the previously accepted state.

## Compiler integration ownership

`TASKS.md` owns independently compiled generic/representation composition,
opaque ABI and lifecycle, native publication, and ordinary std migration.
`TASKS_OPTIMIZER.md` owns allocation, frames, optimization, and physical replay.
Their results enter package review when relevant, but completing all of them
is not a prerequisite for source install/update. A lock decision cannot excuse
an invalid program or an unsupported native guarantee.
