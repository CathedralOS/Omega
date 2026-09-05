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

## 1. Simplify the existing implementation

- [ ] **PACKAGE-ACCEPTANCE-SIMPLIFICATION.** In
  `omega-rust/omega/packages/manager` and its review interfaces, replace the
  certification/promotion prerequisites with compiler review plus explicit
  project decisions. Reuse source resolution, checked review, comparison, and
  decision handling. Remove wrappers and repeated reconstruction whose only
  purpose is promoting the same in-process result into supposed trust.
  Preserve checks that detect mismatched source, graph, target, review, or
  decisions. Acceptance: a checked supported closure can enter a transaction
  without native emission, a certificate-bearing package artifact, or a sealed
  `PackageInstance`. Existing compiler proof/artifact validators retain their
  actual guarantees.

## 2. Resolve and check the candidate

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

## 3. Record pins and accepted policy

- [ ] **LOCK-BASELINE-RECOVERY.** Connect locked recovery and fresh checking
  to command-level lock loading and explicit missing-baseline/unavailable-source
  outcomes. Locked use preserves exact pins and never silently updates a
  selector. Verify acquired content against the recorded resolution.
  A missing acceptance baseline triggers fresh review of the complete graph.
  Unavailable old source preserves a readable accepted baseline and produces
  standalone candidate review with an audit recommendation. Unsupported lock
  formats fail with recovery guidance; regeneration must not silently choose
  newer revisions. Recompute stale compiler analysis without treating an
  unchanged acceptance decision as newly certified.
  Acceptance: fresh checkout, offline cache reuse, missing old source,
  mismatched content, and stale/incompatible review data have explicit outcomes.

## 4. Review and publish the change

- [ ] **CAPABILITY-CONFLICT-TRANSACTION.** Join candidate review, baseline
  comparison, and project decisions into one recoverable install/update
  transaction. Capability changes block pending decisions for the exact changed
  rows, including removals; package-name/source replacement is explicit.
  Initial dangerous authority and accepted assumptions require review.
  Ordinary initial API rows have no previous compatibility contract.
  Wire the complete-policy review document into command-owned file loading and
  resume. Preserve explicit command replacement intent when
  both alias and source change; graph comparison must not infer pairing from
  package names or authored positions. Retain removed-package
  decisions without indexing them into the candidate-only lock graph.
  Recheck candidate identity and project-file versions before committing.
  Acceptance: unresolved/stale decisions and concurrent edits leave the
  previously accepted project state intact; interruption cannot leave
  `build.omg` and `omega.lock` describing different accepted graphs.
  Decision records identify accepted changes; they do not prove an audit.

- [ ] **AUDIT-RESULT-INTEGRATION.** Present compiler-rendered capability diffs,
  affected APIs, dependency paths, assumptions, and source changes in the
  install/update flow. Recommend audits for dangerous initial capabilities and
  retained filesystem/network or comparable authority on updates, even when
  the capability set is unchanged. Missing old source receives standalone
  candidate review. Optional LLM advice cannot suppress compiler findings or
  replace project decisions; no Y/N rubber-stamp prompt or proof-of-audit
  receipt. Acceptance: deterministic findings and blockers work without an
  advisory service; package prose cannot inject instructions into capability
  triage. External project policy may require stronger review.

## 5. Commands and integration tests

- [ ] **OMEGA-INSTALL.** Implement
  `omega install <source> [--rev <revision>] [--as <alias>]` in
  `manager/src/operations/`, with a thin CLI entrance. Acquire the repo,
  discover its declared package name, resolve/check its closure, handle review,
  and commit the dependency declaration and lock. Support the existing Git
  HTTPS/SSH and local adapters first; additional stores reuse the same flow.
  Acceptance: an actual remote package installs and imports through its
  default alias, with overrides optional and all failure paths recoverable.

- [ ] **OMEGA-UPDATE.** Implement
  `omega update [package-or-alias...] [--to <revision>]` over the same
  transaction. Acceptance: selected updates respect existing pins for
  unaffected packages, explain resolution conflicts, block capability changes
  pending decisions, recommend audit for retained dangerous authority, and
  commit the reviewed graph only.

- [ ] **OMEGA-AUDIT-PACKAGES.** Render the selected graph, exact pins, accepted
  baseline, freshly checked reach/API/assumption findings, and dependency paths.
  Clearly distinguish accepted policy from current compiler findings and
  unavailable analysis. Acceptance: users can identify which package and API
  introduces dangerous authority; the output makes no claim that lock authors
  performed an audit.

- [ ] **PACKAGE-MANAGER-RELEASE-AUDIT.** Exercise install/update through the
  real command and network adapters using pure, dangerous, capability-changing,
  same-name/different-source, transitive-authority, and generated-source
  fixtures. Cover missing baselines/old source, invalid proofs, spoofed
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
