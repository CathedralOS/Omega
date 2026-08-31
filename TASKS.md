# Tasks

Last pruned: 2026-08-29.

This file is the current execution queue, not a changelog. Git retains completed
implementation history; architecture pages and design briefs describe the
current model and only the implementation state needed to explain remaining
work. A task belongs here only when it names:

- the remaining work;
- the owning design and code area;
- a real blocker, if one exists; and
- a concrete acceptance condition.

Before taking a task, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping another active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Unresolved owner decisions live
in `OWNER_QUESTIONS.md`; deliberately deferred research stays at the end of this
file.

## Ownership firewall

Psi operates on Omega files and owns parsing plus all target-neutral semantics
through terminal Psi. Omega consumes terminal Psi and owns provider
installation, optimization, ABI/storage realization, native emission, and
general execution machinery. Target backends own unavoidable ISA, ABI,
object-format, and relocation encoding. Cathedral owns OS data structures,
policies, protocols, and lifecycle.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement page tables, descriptor
tables, schedulers, process tables, timer queues, or drivers as compiler-owned
Rust models.

Compiler validation and code generation may consume general plans. They must
not acquire customer-shaped semantic types, lifecycle states, writers,
scanners, or receipts.

## Trusted-core simplification

- [x] **KEEP-TERMINAL-VERIFICATION-NONSEARCHING.** Separate deterministic
  reconstruction of the complete Terminal-Psi obligation set from discovery
  of proof routes. The producer may search and must serialize the selected
  derivation. The verifier checks that explicit derivation against the
  independently reconstructed obligation; it must not enumerate alternate
  candidates merely to discover whether some proof exists.

  Audit the mirrored producer `nonzero_divisor_certificate` and verifier
  `verification/reconstruction` trees. Share canonical rule definitions where
  that reduces drift without sharing producer conclusions, and retain mutation
  teeth proving that omitted obligations and malformed witness edges reject.

  The complete proposition vocabulary is settled. Nonzero-divisor,
  defined-exact-division, and exact-shift-count questions already come directly
  from Terminal semantic schemas. Exact cast, exact shift-left, and exact
  add/subtract/multiply use a proof-only total `IntegerMathTerm` plus parallel
  mathematical equality/order relations. Carrier representability expands to
  its canonical lower and upper bounds; operation identity remains in
  `CanonicalScalarGoal` rather than multiplying proposition families.

  Exact cast now projects the canonical mathematical carrier bounds, uses a
  producer certificate checked through the existing equality/order calculus,
  and has no production sufficient-form reducer or mirrored verifier search.
  The existing affine- and cast-bound witnesses now replay ordered literal
  arithmetic, exact divide/remainder and shifts, partial cast words, and strict
  fixed-native widening edges. Carrier-determined total images and exact
  cast-intersection endpoints may use only a kernel-checked `Truth` child; the
  checker derives and validates the exact endpoint itself. This extends no
  proof tag or verifier search route.
  Exact shift-left now retains the independent canonical count question before
  the mathematical shifted-value carrier bounds. Its untrusted producer may
  search prior facts and recursively compose checked cast/affine/shift endpoint
  certificates, while the verifier replays only the serialized route. The
  direct endpoint checker binds the proof-only shift expression without citing
  the operation's own result equation, and replays explicit count endpoints;
  sign-crossing bounds choose the sound minimum or maximum count independently.
  Exact addition now projects the canonical mathematical sum bounds and uses
  checked `IntegerAffineBound` certificates. Independent-range certificates
  serialize one ordered endpoint child per operand. Embedded literals use a
  checked `Truth` child whose endpoint is intrinsic; bare carrier endpoints use
  `Truth` with the direction fixed by the other ordered child. Correlated guards
  cite the earlier exact complement equation (`MAX - right` or `MIN - right`), its
  ordered endpoint-literal landing when the endpoint is a value, and the
  authored comparison to that bound. The producer
  may select bounds and cast-compatible candidate literals, while verification
  only replays the serialized route and recomputes the sum. Missing, reordered,
  redirected, mixed-orientation, overflowing, or wrong-complement evidence
  rejects, and the operation's own result equation remains unavailable.
  Exact subtraction now projects its canonical mathematical difference bounds:
  signed subtraction retains both carrier endpoints, while unsigned subtraction
  retains its nonvacuous zero lower bound. Independent-range certificates use
  opposite endpoint orientations for the minuend and subtrahend. The unsigned
  joint guard serializes the exact authored `right <= left` comparison.
  Correlated signed guards cite the earlier exact complement equation
  (`MIN + right` or `MAX + right`), its ordered endpoint-literal landing when
  needed, and the authored comparison to `left`. The existing
  `IntegerAffineBound` checker replays those coordinates and recomputes the
  mathematical difference without verifier search or result self-citation;
  reordered, redirected, missing, or wrong-orientation evidence rejects.
  Exact multiplication now projects its canonical mathematical product bounds
  and uses the unchanged checked `IntegerAffineBound` wire. Direct certificates
  serialize four ordered operand endpoints and the checker recomputes all four
  mathematical corners. Correlated certificates serialize the authored factor
  sign and `MIN / factor` or `MAX / factor` quotient comparison, including an
  earlier endpoint-literal landing when required. Target-bounded producer
  custody replays affine/shift suffixes and existing cast or remainder range
  certificates without widening the global search frontier. Embedded zero and
  one, closed products, and the signed syntactic `-1` carrier implication fold
  schema-locally; runtime `-1` retains both checked bounds. Endpoint order,
  sign orientation, quotient identity, landing order/value/type, and redirected
  definitions have mutation coverage. `kernel_proposition` is now total rather
  than optional, and production reconstruction no longer selects any legacy
  sufficient proposition. The correlated affine exact-divide/remainder route
  now serializes `IntegerCorrelatedForbiddenRoots` tag 14. Its witness names
  two disjoint ordered definition words over one signed machine parameter plus
  the exact lower/upper requirement coordinates after the semantic boundary.
  Admission replays those coordinates, solves the zero and `-1` forbidden
  roots, requires the safe empty-root result, and reconstructs the unchanged
  canonical `ExactDivisionDefined` proposition. Proof-bundle format 21 and the
  canonical proof-calculus trust root 21 retain that rule; terminal semantic
  format 39, vocabulary 42, and proof-system marker 1 are unchanged. Missing
  parameter custody, redirected definitions or literals, bound drift, partial
  safety, and forged conclusions reject. The strict broad corpus is green
  after the exact-add candidate-selection latency was bounded.
  Deterministic schema-local normalization may fold closed mathematical
  expressions, bare carrier inclusion, and vacuous bounds.
  Symbolic interval propagation, affine reduction, aliases, and other
  fact-dependent proof discovery remain untrusted producer work.

  Acceptance: verifier complexity scales with the supplied certificate and
  reconstructed obligations rather than a proof-search frontier; deleting the
  producer-selected route from a certificate rejects even when the verifier
  could otherwise rediscover another route.

- [x] **CLASSIFY-AND-HARDEN-AUTHORITATIVE-IDENTITIES.** Inventory every compact
  FNV or other `u64` fingerprint in Psi, native realization, image emission,
  installation, provider planning, external roots, and component machinery.
  Classify each as either a non-authoritative local index/cache discriminator
  or an identity used for evidence, compatibility, replay, installation, or
  admission.

  Local discriminators may remain compact. Authoritative identities must bind
  canonical bytes or use a domain-separated collision-resistant digest; no
  authority decision may depend solely on FNV equality. Preserve structural
  byte replay where it already supplies the real check, and rename residual
  fingerprints so their non-authoritative role is unmistakable.

  The executable-installation-local slice is live. Artifact content, retained
  proof payloads, and materialized final bytes now use separate domain-framed
  SHA-256 digest types with no public constructors; the canonical artifact
  constructor derives content internally. Container v1 retains its exact
  64-bit content field only as an explicitly non-authoritative compatibility
  fingerprint, and its encoder remains byte-for-byte stable. Materializer
  output retains the exact admitted artifact and Extent-backed placement
  evidence; final validation retains that placement evidence plus the strong
  final-byte digest and exact bytes. Installed writer contexts retain the full
  installed realization, so collision-equal report IDs cannot transplant a
  context. The remaining FNV uses are named non-authoritative container/
  informational compatibility fingerprints and writer replay fingerprints,
  with a local architecture test policing that inventory. Retirement facts now
  use provider-canonical SHA-256 commitments; quarantine and stale-entry faults
  retain exact installed/provider evidence. Component-era receipts and leases
  retain complete candidates, while activated deployment journals rejoin exact
  installed contexts plus canonical installation bytes and label serialized
  installed/artifact values as report identities. Reclaimable opaque-callback
  capacity receipts now retain private occurrence provenance through both
  registration joins.

  The first image/publication slice is also live. Encoded compiler text, final
  compiler text, canonical relocation envelopes, and their derivation use
  separate strong digest fields; installation format 42 serializes them and
  rejects drift. Compiler publication uses strong certificate, publication,
  container, compiler-text, and destination-installation digest types rather
  than FNV-only receipt identities. Native final images now retain a
  domain-separated SHA-256 commitment to the exact target, entry handle, and
  every function/data symbol row; native replay recomputes it before
  publication, whose certificate also binds it, so same-handle symbol
  substitutions reject. Final compiler text partitions now bind every region
  and gap, exact
  structural state-footprint evidence, ordered executable inventory, entry
  binding, coverage, placement, and final-footprint certificate with distinct
  domain-separated SHA-256 types; replay rejects compact-collision
  substitutions. Selected-provider facts and boundary calling realizations
  now retain and rejoin the complete plan behind their compact report IDs, so
  same-ID structural substitutes reject. Provider plans now additionally own a
  domain-separated SHA-256 identity over the complete normalized plan, retained
  through selection and external-root execution; compact-ID-equal structural
  substitutions reject at the sealed bridge. The format-specific image audit is
  also live: Mach-O and PE introduce no local compact hash, while every ELF FNV
  value is retained only beside the exact owned planning carrier, independently
  replayed, and named as a non-authoritative compatibility fingerprint. An
  architecture inventory rejects a newly exported format-specific compact
  identity. Compiler-function validation now exposes a domain-separated SHA-256
  commitment to its complete normalized summary. Final-footprint identity and
  publication receipts carry that commitment, while the historical aggregate
  FNV value remains report compatibility only; publication replay rejects a
  digest substitution even when the compact report value is unchanged. The
  standalone component candidate now independently rejoins a domain-separated
  SHA-256 commitment to the complete selected-provider closure; native
  artifacts retain the historical FNV value only as a compatibility report
  coordinate. Exact plans, execution scope, opaque admissions, and
  installation-reach resolutions enter the commitment, and an
  architecture inventory plus adversarial test reject a compact-equal
  structural substitution. Provider-execution coordinates projected through
  target operations, machine code, installation encoding, and retained native
  artifacts are now likewise named report identities/fingerprints. Lowering
  still requires borrowed non-constructible admitted evidence; native custody
  retains the strong selected-closure digest plus exact requirement strings
  and plan requirement catalogs, and exact-requirement substitution rejects
  even when every compact execution coordinate is held equal. The producing
  external-root ledger now applies the same classification: normalized root,
  provider execution, opaque exit, stack, fuel, boundary-contract, and closure
  summaries are report coordinates beside the full validated root, boundary,
  resource columns, and exit assurance. Installed root reports additionally
  retain the strong selected-provider-closure digest; compact-equal root-policy
  substitution still rejects through exact structural replay. External-root
  producer-schema cohorts now use a
  domain-separated SHA-256 commitment over the complete resolved schema for
  prebinding uniqueness, aggregation, lifecycle ownership, and epoch grouping;
  the historical schema FNV is report compatibility only. Required-slot
  closures and snapshots already retain their exact members and evidence.
  Psi access, placement, and resource-profile FNV values are now explicitly
  non-authoritative compatibility fingerprints beside exact retained plans.
  Provider-existing-content grants additionally bind a domain-separated digest
  over canonical layout, access policy, and reach, and build-time schema FNV
  keys are collision-checked local discriminators. Component-era leases now
  bind a domain-separated digest of the complete installed occurrence and
  replay it through publication, deployment journals, and program-local root
  cohorts; the former installed-artifact `u64` is report compatibility only.
  Checked machine contracts now retain a domain-separated SHA-256 commitment to
  the complete canonical public contract beside their historical compact report
  coordinate. Checked plans, crash capsules/call sites, realized envelopes,
  Terminal handoff, nominal refinement, and provider callback selection replay
  that commitment. Every exported checked contract coordinate is now named as
  a report fingerprint. Contract-plan replay rejects an empty strong
  commitment, and boundary lowering refuses to treat a boundary row's own
  stored digest as authority when its canonical checked plan or crash capsule
  is absent. The default executable container is now v2: a required
  authority section carries independent imported-contract-set, declared-
  footprint, machine-regime, and installation-scope commitments, all four enter
  executable content identity, and admission rejects v1 candidates. Container
  v1 remains an explicit byte-stable tooling compatibility path. Closed
  conformance applications now retain domain-separated commitments through
  typed, checked, and Terminal Psi; dispatch selects by owner plus strong
  commitment and then replays the exact row, while compact values are report/
  index coordinates only. Terminal vocabulary 38 serializes and verifies those
  commitments. Typed, checked, and Terminal carriers now name every one of
  those compact coordinates as a report fingerprint without changing wire
  order. Content-projection and conservation FNV values are now named
  report fingerprints throughout semantic, codec, verifier, optimizer, image,
  runtime, and visualization consumers; authority remains the exact owner
  definition, algebra, structural places, substitution, producer call, and
  verifier replay. Component-progress manifests now bind both a strong selected-
  provider-closure digest and their own domain-separated digest. External-root
  receipt admission and component sealing replay both, so a compact-equal
  selected closure cannot substitute. Durable deployment-journal bytes and
  normalized may-write frames likewise expose their FNV values only as report
  coordinates beside exact bytes/records or completeness/paths. The target-
  owned UEFI system-table layout now names its FNV value as a non-authoritative
  report fingerprint, while lifecycle admission replays the complete target
  slot and every exact field/aggregate layout row; a compact-equal structural
  substitute rejects. Backend callback placements likewise name inbound and
  registrar calling-plan FNV values as report coordinates; thunk binding
  identity retains the exact inbound plan, and schedule replay rejects an
  exact-plan substitution even when the compact report value is held equal.
  Target closure now requires the pre-closure strong commitment before
  rewriting a checked callback placement, and callback admission rejoins the
  published requirement report plus commitment to its exact checked capsule.
  Fixed-fuel, ordinary stack, and epoch-stack composition carriers likewise
  name their FNV values as non-authoritative report/cache fingerprints while
  retaining complete provider graphs, nesting relations, arithmetic inputs,
  and admitted evidence; compact-equal graph substitutions remain structurally
  distinct.
  Fixed-record, conventional-sum, and first direct record-with-sum
  `ConstMaterializable` carriers now also name their layout and materialization
  FNV values as non-authoritative report fingerprints. Replay retains and
  compares the exact typed value, complete outer and applicable nested layout
  reports, target byte order, selected sum case, and staged bytes, so a compact-
  equal layout substitution rejects before materialization or copying.
  Build evaluation now binds target-owned physical-entry package provenance
  with a domain-separated SHA-256 commitment to the exact package identity,
  canonical source path, and source bytes. Its former package-source FNV is an
  explicitly non-authoritative report fingerprint, and compact-equal source
  substitution rejects through the retained strong commitment. The physical
  entry calling-plan FNV is likewise a report coordinate; build evaluation
  replays the realization's strong commitment and crate-sealed exact plan.
  Checked nominal-machine-use facts now name contract-envelope, refinement,
  resource-anchor, and boundary-calling-plan compact values as report
  fingerprints. Contract/refinement joins retain normalized machine-contract
  commitments, while callback placement now carries a domain-separated strong
  commitment to the complete canonical boundary plan; target planning
  recomputes it and replays exact plan custody, rejecting compact-equal plan
  substitution. The repository compact-field ceiling shrinks with this slice.
  Package-review provider projections likewise label plan FNV values as report
  coordinates beside the exact package owners, schema, target, rows, and
  declaration coordinates retained in canonical review evidence.
  Native image region bytes, final text, and placed-region inventory now expose
  their compact values only as report fingerprints beside domain-separated
  byte/text/inventory digests. Native publication certificates and flat/bundle
  replay retain the strong inventory digest, so compact-equal inventory-digest
  substitution rejects. Residual native-image callback placement, boundary,
  fixed/body mechanics, composed-footprint, final-region, validation, and text
  coordinates are explicitly named report fingerprints across output,
  certificate, installation-codec, and publication consumers. Exact relocated
  text, relocation envelopes, placed-region rows, entry-region custody, and
  state footprints retain their domain-separated commitments; the compact
  summary fields do not recreate those authorities. Trust tooling now applies
  the same split: provider-plan and selected-closure compact values are report
  fingerprints beside their domain-separated digests; generic template and
  specialization coordinates are report-only beside exact arguments, strong
  selected machine/conformance commitments, and the exact instance-contract
  commitment. Rendered trust output labels every compact value as a report
  coordinate rather than authority. Accepted-machine rows now also label
  contract/template compact values as reports and retain the checked machine-
  contract commitment. Provider `ServiceMethod` schemas likewise
  retain the typed boundary calling-plan commitment beside an explicitly named
  report fingerprint; provider-plan and selected-closure digests include it,
  and native/program-entry replay compares it to the exact evaluated plan.
  A repository-wide architecture test now scans private and exported direct,
  optional, and collection `u64` fingerprint declarations: explicit report/
  cache/compatibility names pass, while every unclassified declaration
  rejects; the former legacy ceiling is empty. Task
  activation specialization now follows the same
  rule: its historical FNV value is a report coordinate, while a
  domain-separated SHA-256 commitment binds the exact checked TaskRuntime
  requirement, operation, target/entry signature, and target machine-contract
  commitment through provider planning and runtime receipt validation.
  Compact-equal exact-target substitution changes the
  authoritative runtime binding. Generic machine specializations now retain the exact
  pre-substitution template encoding, normalized template and instance
  identities, exact type/const arguments, selected machine-contract
  commitments, closed-conformance commitments, and any accepted-template
  grant. Checked-to-Terminal lowering independently replays a domain-separated
  SHA-256 specialization commitment and places all 32 bytes in proof-producer
  identity; the aggregate FNV is report-only, and compact-equal structural or
  stored-commitment substitution rejects. Universal-template trust identity
  now likewise retains a domain-separated SHA-256 commitment to the canonical
  pre-substitution contract beside its report FNV. Provider grants retain and
  replay the exact selected plan plus its strong digest instead of rejoining by
  compact identity. Persisted owner admissions bind subject kind, human
  commitment, and the underlying provider-plan, machine-template, or checked
  machine-contract digest; `omega.lock` stores the resulting 32-byte digest,
  and legacy 16-hex compact rows fail closed. Compiler-issued access field keys
  now retain a domain-separated commitment to the exact canonical layout that
  issued them, so compact-equal foreign keys reject during mutation, lookup,
  authorization, and projection. Checked resource envelopes and callback
  receipts retain the selected machine-contract commitment beside report-only
  axis and roster FNVs. External-root installed-entry, opaque-arrival, and stack
  evidence similarly retain the exact boundary-plan commitment beside compact
  plan, target-rule, domain, and realization reports. Opaque same-stack WCSU
  admission likewise rejoins the selected provider plan's strong commitment;
  its compact plan value is report-only, and a compact-equal foreign plan or
  absent commitment rejects before its byte/alignment claim is sealed.
  Terminal program-local-root producer schemas now label their serialized FNV
  as compatibility report identity while verifier and installation replay
  retain exact schema/projection authority. Compiler wire reports likewise
  label schema, codec, encode, and plan FNVs as report identities, and wire
  compatibility always compares retained exact fields/cases even when compact
  schema reports match. Build-time fixed-record and conventional-sum carriers
  have removed their remaining ambiguous compact accessor aliases.
  Provider-execution, external-root, target-locator, relocation,
  executable-fragment, layout-plan, typed-boundary, and generic-test compact
  accessors now likewise use explicit report or compatibility vocabulary.
  Closed conformance application SHA-256 authority now binds the complete
  canonical state-signature bytes instead of an FNV compression. The
  architecture inventory scans compact-returning fingerprint accessors and
  rejects every ambiguous compact accessor; executable-fragment,
  calling-plan, state-footprint-evidence, and provider-plan accessors have now
  cleared the migration ceiling. Selected-provider closure/application/coverage,
  component-progress demand, executable-TCB, and ELF dynamic-import compact
  identity fields now use explicit report vocabulary. Component-progress and
  executable-TCB admission also retain and replay `ProviderPlanDigest`; a
  compact-equal candidate with the wrong plan digest rejects. The residual
  identity-named audit is complete. Psi schema, indexed-provider application/
  coverage/closure, task-plan, UEFI layout, foreign-locator, and callback-
  catalog compact values now use explicit report/compatibility/discriminator
  vocabulary. Checked operator dispatch retains and rejoins the strong
  `ProviderPlanDigest`; callback requirements and native parameters collision-
  check their exact retained catalogs. Remaining raw `u64` identities are
  exact authored schema numbers, compiler-generated graph coordinates, or
  runtime-issued lifecycle tokens rather than hashes. An architecture test
  guards the residual compact aliases in addition to the generic fingerprint
  field/accessor scanners.

  Acceptance: an automated architecture test rejects new authoritative
  `u64`-only identities, every retained FNV use has a local non-authoritative
  owner, and adversarial collision tests cannot substitute an artifact,
  certificate, provider plan, or installed image.

- [x] **QUARANTINE-SPECULATIVE-COMPONENT-RUNTIME.** Keep component deployment,
  executable installation, external-root, and era experiments
  outside the ordinary compiler and package-manager production dependency
  graph until one real checked Omega provider is independently emitted,
  verified, installed, invoked through its boundary contract, replaced, and
  safely retired or retained.

  The compilation report no longer carries component-deployment custody; keep
  that separation. Experimental crates may retain focused direct tests, but
  ordinary compilation reports contain only checked/Terminal/native products
  actually produced by the compiler route.

  The compilation-report, artifact-reporting, and default image-emission
  closures now exclude executable installation, external roots, and component
  runtime crates; installation support is an explicit image-emission feature
  selected only by component publication/deployment. Ordinary compiler and
  package-manager closures also exclude component candidate, publication, and
  deployment owners. Default program-entry planning and Terminal-native
  realization now exclude executable installation and external roots; only the
  explicit provider-planning `installed-writer` feature composes the
  post-handoff runtime. Provider planning now keeps that bridge behind the
  same opt-in feature, so default provider-planning, build-evaluation,
  compiler, and package-manager closures do not carry executable installation
  or external roots transitively.

  Acceptance: the normal compiler/package closure imports none of the
  speculative runtime deployment crates, and reintegration is driven by the
  end-to-end provider customer rather than by placeholder report states.

Optimizer architecture is specified in
[`optimizer_architecture.md`](wiki/design_briefs/optimizer_architecture.md), and
its detailed execution queue lives in
[`TASKS_OPTIMIZER.md`](TASKS_OPTIMIZER.md). Keep the product-compiler ownership
task here; do not duplicate optimizer pass milestones in both queues.

## Omega-written product compiler

Remaining:

- **OMEGA-PRODUCT-COMPILER-SOURCE.** Establish the production compiler as two
  sibling Omega packages: target-neutral phases under `source/psi/` and the
  Terminal-Psi-consuming product under `source/omega/`, with hosted entrypoints
  at `source/omega/{build.omg,main.omg}`. Preserve the Psi/Omega ownership
  firewall: Psi owns parsing and target-neutral semantics through terminal Psi;
  Omega owns provider installation, optimization, target realization, and
  artifact emission. The maintained implementation under
  `source/omega-rust/` is a differential producer, not the
  source tree for this task.

  Acceptance: the exact Omega source closure builds a compiler that implements
  the full Omega specification, including the production optimizer and lowering
  pipeline, passes the applicable product compiler and language suites, and
  contains no Rust implementation under either reserved product owner.
  "Full" governs accepted Omega and emitted-artifact
  meaning; it does not require standalone interpreters, REPLs, proof explorers,
  viewers, debuggers, or other adjacent tools unless the compiler executable
  imports them. Publish a deterministic manifest of every transitive compiler
  module, library, generated/compile-time source, build input, and tool imported
  by that build.

  The lattice-built Delta compiler builds the separate Delta-written closure
  `D` into `omega₀`; `omega₀` then builds this Omega-written closure `C` into
  production `omega`. `D` and `C` implement the same complete Omega language
  but are different source closures in different implementation languages.
  This task owns `C`; `TASKS_BOOTSTRAP.md` owns `D` and both checked top edges.

  Author this source against the working compiler-source policy in
  `wiki/architecture/bootstrap_lattice/compiler_source_profile.md`; this task
  does not wait for Delta v1 or a frozen feature census. The deliberately
  conservative feature usage restricts only the compiler's own ordinary-Omega
  source, never the full Omega language that the resulting compiler implements.
  Prefer a small regular implementation
  footprint, but do not replace useful general facilities with brittle
  monomorphic duplication merely to reduce the feature count. The working
  authoring bias is to avoid proof-program mathematics and dependent or proof-
  indexed typing, including linear-dependent forms, in the compiler's own
  source. Retain ordinary ownership/multiplicity, records, payload sums, and
  other regular compiler facilities when they improve the implementation.
  Basic generics and concrete domains are ordinary compiler facilities: use
  them when they keep real source regular, and provisionally retain their
  general compositional forms once a checkpoint demonstrates that use. Remove
  them only when a concrete refactor measurably lowers total bridge and
  assurance cost without creating monomorphic duplication. Domain polymorphism
  and advanced generic constraints remain separate cost questions.
  Treat numeric schema tags, mixed field-plus-case declarations, and aggregate
  transition payloads as measured simplification candidates; do not conflate
  numeric tags with ordinary named fields, or force a split representation when
  it makes the source worse.
  Keep adjacent tools outside the closure unless the compiler executable
  imports them.

  Deriving the exact surface used by `C`, authoring `D`, and validating
  `D → omega₀` and `C → omega` remain in `TASKS_BOOTSTRAP.md`; do not duplicate
  product Psi/Omega implementation tasks there. The exact manifest is closure
  evidence, not permission for either compiler to recognize particular files
  or AST permutations. Terminal-Psi representation and lowering modules linked
  into the compiler remain ordinary source dependencies; standalone
  interpreters, viewers, REPLs, proof explorers, and debuggers do not.

  Deliver this incrementally through coherent live source slices, each of which
  passes the applicable product suites. Historical `checkpoint-000001`
  manifests and profiles were retired with `source/omega/source-checkpoints/`;
  they are not current closure evidence and must not be recreated as a bridge
  dialect or file allowlist. The live authored slice under
  `source/psi/` plus the product entrypoint currently reaches source/span
  custody, tokens, Unicode tables, lexing, and fail-closed whole-file parsing
  for ordinary
  `use path::member;` roots plus basic `[pub] data` declarations with optional
  `[copy]`, bare named field types, and payload-free `case Name;` members. The
  parser retains exact mixed field/case order and source spans in owner-local
  bounded syntax and type-reference tables; retired inline discriminants, case
  payloads, other properties, richer types, and every unsupported root reject
  fail closed. Remaining declaration grammar, resolution, checking, terminal
  Psi, and all later Psi/Omega phases remain open. Extend this as live
  grammar/semantic slices, not checkpoint dialects, private bridge IRs, or
  file-shape allowlists.
  The first parser state machine is now split into owner/facade, scalar token
  access, root sequencing, and data-declaration modules under
  `source/psi/parse/`. One flat parser owner retains the only token and
  syntax tables; the hosted AArch64 backend cannot yet address the resulting
  1.76 MiB nested aggregate or lower a second arena ownership transfer, so the
  physical tables deliberately remain flat while behavior is modular. The 45
  black-box parser cases passed against the last emitted product artifact. A
  fresh current compiler now reaches checked product source in about 24 seconds,
  but explicit-target artifact emission rejects because `Main::main` has no
  checked transitive machine plan. The parser gate requires both the exact fresh
  CLI and an explicit target profile; it selects neither a cached CLI nor an
  ambient host target. Restore the lowering path and rerun all 45 cases against
  that same explicitly selected fresh CLI before extending the parser.
  This is not a missing lookup or artifact-report switch. `Main::main` has two
  real backedges, while the current structural-control producer, Terminal
  verifier, ownership-frontier replay, and fixed-fuel path accept only acyclic
  control. Close that general resource-bounded cyclic-control slice first with
  retained rank/invariant evidence through codec, verifier, reconstruction,
  interpreter, fuel, Omega lowering, native execution, and artifact custody.
  The first vertical acceptance case is a structural Unit machine with one
  unsigned scalar countdown parameter, a `remaining > 0` guard, and a backedge
  carrying `remaining - 1`. Extend the checked control plan with only the
  scalar expression forms that case requires; retain a canonical ranked-SCC
  component naming the header, rank parameter, bounds, and covered cyclic
  edges; and reconstruct invariant establishment/preservation, strict descent,
  and subtraction safety from the actual operation and successor-argument
  graph. Cyclic ownership-frontier replay must converge rather than depend on a
  topological order, and fixed fuel must derive the all-input ceiling from the
  verified rank bound and exact exit/cycle costs. Preserve the same identity
  through Terminal encoding, abstract/target operations, selection, allocation,
  both supported ISA encoders, backward-edge relocation, object/image checks,
  installation, and native artifact custody. Interpreter execution at an exact
  initial value must agree with native execution and the derived schedule. A
  mutation that forwards the original rank instead of the subtraction result
  must reject as a non-decreasing ranked edge; removing rank evidence must
  retain the existing unranked-cycle rejection. Do not broaden this first slice
  to the product `Main` receiver, mixed operations, or boundary calls.
  The checked-plan substep is complete: the existing Nat-ranking owner now
  exports the exact direct unsigned-countdown component, and checked structural
  control retains its header, rank carrier/bounds, covered edge coordinate,
  positive guard, and parameter-minus-one successor form without a second
  syntax recognizer. The source-handle-free Terminal representation substep is
  also complete: lowering emits an explicit entry preheader, ranked header,
  positive guard, decrement block, and covered backedge; Terminal format 34 /
  vocabulary 37 retains that identity canonically; and representation replay
  reconstructs the zero-to-rank entry, actual guard, exact subtraction, and
  successor argument while ordinary execution authority rejects the cyclic
  machine. Missing rank evidence still reaches the existing unranked-cycle
  rejection, and forwarding the original rank rejects as invalid rank evidence.
  Execution-grade cyclic ownership-frontier convergence is now complete for
  that exact representation slice. Frontier replay computes the header's
  establishment candidate over the acyclic skeleton, computes the covered
  backedge exit after one complete cycle body, and requires exact equality of
  live claims, owned places, and partial-custody paths. That equality is the
  separately checked invariant-preservation fixed point for the one admitted
  backedge; the edge is no longer silently omitted from custody validation. A
  countdown carrying a nonempty affine frontier passes, while discarding that
  custody only on the cycle path rejects at the header. Interpreter execution
  is now complete for the same exact slice. Proof reconstruction schedules the
  acyclic skeleton by removing only the validated covered backedge, retains the
  taken positive-guard fact as the exact `1 <= remaining` subtraction premise,
  and requires producer evidence for that reconstructed site. Canonical
  semantic/proof sections execute ranks 0, 1, and 3 with exact measured costs
  5, 11, and 23 respectively. A distinct opaque
  interpreter verifier carrier grants no fixed-fuel, Omega, native, provider-
  installation, or mixed-operation authority; ordinary execution validation
  still returns `NonExecutableRankedScc`. Derived fixed fuel is now complete
  through its own opaque verifier carrier for the same exact slice. The checker
  recomputes actual preheader, header, decrement, exit, and return costs under
  the current schedule and proves `entry + maximum_iterations * cycle + exit`,
  with checked overflow into the certificate's `u64` denomination. The exact
  `u32` all-input ceiling is `25_769_803_775` units for concrete cost
  `5 + 6 * remaining`; the canonical codec round trip replays the certificate,
  while a `u64` carrier fails closed because its ceiling cannot be represented.
  `omega inspect-terminal` now selects that distinct fixed-fuel verifier for a
  lowered closure carrying ranked metadata, derives and validates the ranked
  whole-entry certificate, and renders the same canonical terminal/fuel
  summary. Acyclic inspection keeps its ordinary verifier path, while wider or
  otherwise unsupported ranked shapes fail closed instead of falling back.
  Existing acyclic segment authority remains unchanged. Native admission and
  custody are now complete through assigned target operations for the same
  exact first slice. A third opaque verifier carrier admits only the canonical
  structural Unit / `u32` countdown machine. Its lower projection retains
  canonical Terminal semantics and proof bytes, the exact fixed-fuel fields,
  the two relevant complete structural-frontier snapshots, and the concrete
  ranked graph as replay data rather than embedding semantic-layer Rust
  authority in a representation crate. The object boundary independently
  decodes the proof, reruns native and fixed-fuel verification, derives the
  certificate again, and compares every retained frontier row. Abstract
  lowering constructs that projection directly from canonical semantic/proof
  sections; ordinary acyclic lowering still rejects the cycle. Target lowering
  replays every ranked operation and edge,
  validates the affine-owned structural frontier and exact exit cleanup, and
  preserves the fixed-fuel identity. Assignment accepts only the ABI-prescribed
  rank register (`rdi` on Linux x86-64, `x0` on Linux AArch64) and rejects
  stack, cross-target, or call-plan drift. The ordinary selected-instruction
  path remains fail-closed. A disjoint unoptimized route now carries the exact
  ranked authority through assignment and emits the fixed relocation-free
  countdown body for both Linux ISAs, including the one-time preheader and the
  header-targeting backward branch. Machine code retains semantic custody,
  complete ABI/structural inputs, and exact semantic-code attribution. Target-
  owned x86-64 and AArch64 validators independently decode the rank register,
  opcodes, immediates, and all branch destinations. Object, final-image,
  installation, and native-artifact replay retain the ordinary unchanged body.
  The fixed-work theorem remains separate non-authorizing analysis evidence.
  The resulting native artifact validates for both Linux ISAs.
  The first product-following extension is also complete for an operation-free
  ranked countdown carrying one ordinary affine `&mut self`. The checked plan
  retains the implicit receiver as `is_self` plus `MutableBorrow`, synthesizes
  its exact transfer across every edge without inventing an authored argument,
  and never treats it as an affine discard. Terminal ownership-frontier replay
  now distinguishes by-value `Owned` roots from borrowed parameters, so the
  receiver remains present in the semantic signature while the header and
  backedge agree on the empty by-value frontier. Native lowering classifies
  that borrow as one pointer-sized ABI value rather than the attached aggregate
  by value. Target, assignment, machine-code, object, and final-image owners
  join their physical place/type/multiplicity/access and placement back to the
  authoritative semantic replay instead of adding a parallel `is_self` bit.
  Both Linux ISAs accept the exact slice; semantic receiver, physical access,
  and ABI-placement mutations reject. The prior affine-owned token countdown
  remains accepted unchanged.
  This operation-free case does not claim product publication or subplace
  custody. Later product-required slices must add a real projected receiver
  subplace transfer, mixed operations in multi-state blocks, structural-result
  boundary calls and payload cases, nested field/index reads and writes, and
  Darwin realizations for `read_byte`, `write_byte`, and `exit_process`. Do not
  bypass Terminal Psi, revive the deleted backend, or route around the failure
  in report/artifact policy.

  The token/entrypoint cleanup below was completed early because retaining its
  parallel truth was negative value even while native publication remained
  blocked. Runtime acceptance still precedes any grammar expansion:

  1. Produce acceptance evidence from the current product source with one
     freshly built CLI and one explicitly selected target, then run all 45
     parser cases against the resulting fresh native artifact. Record the CLI,
     target, and artifact identity. The canonical script has no cached-artifact
     path and prints SHA-256 identities for the selected CLI and fresh artifact.
     It is currently **DEPENDENCY-BLOCKED**: both the product closure and
     gate-owned harness pass checked-source compilation, but native publication
     fails closed because the attached Unit closure lacks a checked transitive
     machine plan.
  2. **SOURCE-COMPLETE.** The lexer now transfers its one canonical mixed
     `Token` array to the parser as a whole ownership move. `TokenObservation`,
     the lexer ordinal mapper, the per-token handoff, four parser token arrays,
     raw ordinal comparisons, and scalar current-token projections were deleted
     atomically. Only a bounded current index remains; every classification
     reads the typed token.
  3. **SOURCE-COMPLETE.** Lex/parse diagnostic serialization moved from the
     exact product compiler entrypoint into `source/psi/parse/harness.omg`,
     imported only by `source/psi/gates/parser/`. The product entrypoint is now
     66 lines of phase driving and exit diagnostics. Numeric protocol projection
     exists only as gate-owned ephemeral state. The same 45 accepted, rejected,
     capacity-edge, lexical-handoff, and determinism cases remain mandatory once
     native publication is restored; the Python decoder remains semantic-free.

  Do not recover speed by duplicating token access, generating state
  permutations, or enabling unconsumed viewers/debug output.
  Freeze the exact manifest and feature census only for the complete compiler
  closure at the Delta-to-Omega join.

  **SOURCE-COMPLETE.** The standalone `source-snapshot` / feature-census
  command, compiler inspection route, `omega-source-profile` schemas/catalog,
  and bespoke gates are deleted. They resolved with
  `omega-source-inspection-v1` while product compilation resolves with
  `omega-local-project-v1`, stopped before generated and build-tool inputs
  existed, and never joined an emitted artifact. Their second fingerprint and
  diagnostic closure were parallel non-production truth, not reusable `C`
  scaffolding. Architecture coverage rejects their return.

  **SOURCE-COMPLETE.** Production compilation now derives one ordered typed
  consumed-unit projection from the final `CheckedCompilation` after generated
  source admission. One canonical `CompileReport` manifest joins that exact
  package/dependency/source subject to owner-derived build-observation,
  selected build-machine, target, Terminal/native-artifact custody. Native
  identity is owned and replayed by `NativeArtifact`; build-observation
  identity is owned by build evaluation rather than package review. Package
  projects with a `build.omg` enter the production package resolver even with
  zero dependencies, and publication preserves the same manifest while its
  receipt binds the native-artifact identity. There is no inspection request,
  second resolver domain, census, JSON authority, or standalone command.

  The product build directly selects the complete `ConsoleNativeProvider`
  through the normative
  `Build::select_provider<Console, ConsoleNativeProvider>` surface rather than
  the Rust-recognized `Owner::provider_defaults` suffix convention.
  **IMMUTABLE-TARGET-ACTIVATION-AND-REACH-CLOSURE** must now implement the
  settled build model before deleting `source/omega/build.omg`'s four legacy
  `target ... {}` blocks: resolve an optional CLI `Host` convenience to one
  exact profile before evaluation; inject that immutable request as
  `Build.target`; validate the role-specific target closure; retain the exact
  target identity; and add one explicit complete runtime-reach ceiling whose
  transitive-demand failures carry a provenance path. Migrate legacy host rows
  into target-owned nominal provider realizations and the complete reach
  ceiling, then retire `target ... {}` and source assignment to `builder.target`
  across the corpus. Normalize the legacy x86 `windows_x64`, `linux_x64`, and
  `uefi_x64` machine-prefix/item spellings to the canonical `*_x86_64` profile
  identities. Input-only CLI aliases may remain temporarily but must normalize
  immediately and never enter locks or semantic identity. Do not remove the live
  declarations before the activation, provider, and denial destinations exist.
  Keep canonical Omega case names, semantic profile identities, CLI names and
  aliases, and validated target-package declarations in one target-catalog
  mapping rather than argument-parsing tables; the temporary Rust enum must not
  become the permanent profile vocabulary.
  Target-package defaults outside this explicitly selected product closure
  still require either that same ordinary build surface or exact declaration,
  receiver, target-scope, and duplicate/default precedence rules;
  do not generalize that compatibility suffix into the live closure.

  The activation carrier is now source-real: native-artifact request admission
  resolves an omitted target to one exact host profile, while checking and
  Terminal requests remain targetless. Exact-target build evaluation injects
  one closed, toolchain-owned `TargetProfile` value through ordinary
  `Build.target`; the evaluator admits only the exact nominal prelude
  vocabulary and rejects assignment, transient overwrite-and-restore,
  whole-value replacement, construction, storage, and exclusive lending before
  execution. The request-owned profile remains the single retained authority,
  and final evaluated equality is only a corruption check. Targetless checking
  receives no synthetic field or fake `Host` value. Remaining work is the
  role-specific closure, explicit complete runtime-reach ceiling with
  provenance, target-owned provider migration, canonical CLI spelling, and
  deletion of the corpus's legacy target blocks/assignments; this task is not
  complete until those displaced surfaces are deleted.

  Canonical x86-64 target spelling is now closed for the target catalog,
  semantic identities, locks and package-review recovery, compiler admission,
  provider/artifact consumers, and the real `source/` target packages.
  `linux_x64`, `windows_x64`, and `uefi_x64` remain input-only CLI aliases and
  normalize immediately to `linux_x86_64`, `windows_x86_64`, and
  `uefi_x86_64`; source declarations and recovered canonical evidence reject
  the aliases. The authored `tests/omega`, `tests/fixtures`, and `samples`
  corpora now use only canonical profile spellings too. The native CLI canary
  passes target discovery and reaches its independently open checked-plan
  closure. The broader role-specific closure, runtime-reach ceiling, provider
  migration, and eventual deletion of transitional target blocks remain open.

  **BOUNDARY-OPERATOR-FAMILY-SELECTION** must extend the typed build selection
  subject from one boundary-trait type to either an exact boundary trait or an
  exact package-qualified boundary-operator family. Select every overload
  coordinate atomically; retain a deduplicated set canonically ordered by
  coordinate identity, the nominal provider, per-coordinate plan, selected
  target, and generic or exact-application coverage. Partial coverage, reach-
  selected subsets, display signatures, ordinals, declaration order, and
  ambiguous coordinates reject. Adding a public overload must invalidate an
  incomplete existing override with a diagnostic naming the new coordinate.
  Permit composite providers to call explicitly public exact realization
  machines without redispatch, while an operator spelling inside its own
  provider remains recursive.

  Typed build harvesting and provider planning now accept a source-resolved
  boundary-operator representative, reconstruct the complete same-package,
  same-path boundary family, canonicalize its exact requirement coordinates,
  and select every coordinate through one nominal provider atomically. Unknown,
  ambiguous, or provider-missing coordinates reject the whole selection.
  Package review now rejoins that complete declaration-family selection to the
  exact provider, target, selection authority, and coordinate-to-plan mapping.
  D35 retires the indexed-provider arity/string closure carrier, its selected-
  plan attachment and fingerprints, and its package-review `NonGeneric` /
  `ExactApplications` encoding. Provider assertions are claims, not coverage;
  equality with compiler demand cannot establish a realization. D29 fixes the
  production path: checked uses retain tagged
  type/const demands, const values normalize in their declared carrier,
  generic artifacts export only symbolic demand, and final specialization
  issues coverage after the selected checked-body, intrinsic, or exact
  external semantic realization recheck. D32 then binds every surviving
  optimized boundary-operation occurrence to one exact native physical child
  carrying the target-lowering, assignment, relocation, and emitted-span
  joins. Empty telescopes use one canonical empty application. Bootstrap
  lowering cannot issue authoritative coverage. Producing those exact rows
  remains work; genuinely universal realization coverage stays deliberately
  unimplemented under D28, alongside
  public-leaf delegation/recursive redispatch and the external-leaf
  source-form migration below.

  In the same migration, make external provider leaves bodyless `boundary
  machine ... satisfies ...` declarations. Infer compiler-intrinsic supply from
  exact package, machine, signature, and target identity; remove the payload-
  free `via Binding::CompilerIntrinsic` source/IR form across target libraries.
  Retain `via` only for bindings carrying an undiscoverable payload such as a
  DLL locator, syscall number, or validated foreign-table field. Retire authored
  numeric `Binding::VtableSlot`. The parser now rejects that case before
  consuming its payload with a stable migration diagnostic, genuine
  firmware/native-table source uses `Binding::VtableField`, and the downstream
  ordinal enums/codecs remain only for artifact compatibility. Add pass/fail
  canaries for complete and partial family overrides, canonical reorder
  stability, generic/exact-family coverage,
  exact-leaf delegation versus recursive redispatch, missing intrinsic catalog
  entries, payload-bearing bindings, and retired numeric slots.

  **TOP-LEVEL-BOUNDARY-REQUIREMENTS** must add the explicit
  `pub boundary requirement Package::operation(...);` declaration and retain
  its package-qualified operation, visibility, static telescope, signature,
  complete contract, and optional installation-bound reach row through parsed,
  resolved, typed, checked, Terminal Psi, package-review, and artifact identity.
  Replace the undifferentiated semantic `Boundary` supply mode with explicit
  requirement, external-realization, or admission-claim modes; a claim-free
  bodyless free machine must reject once the temporary core migrations finish.
  Rename/normalize the existing `Accepted` semantic variant as an admission
  claim whose later owner-policy receipt is separate from declaration identity.
  Extend `satisfies` to target that exact declaration and extend the typed
  `select_provider` subject with `BoundaryRequirement`; build policy selects
  only an already-declared candidate and never creates the satisfier edge.
  Missing, private, ambiguous, foreign, duplicate, wrong-signature, and
  wrong-target candidates reject. Provider/operation identity stays separate
  from reach, and invocation of a carrier-owned operation must replay the exact
  installed execution and era retained by the linear token rather than ambient
  redispatch or row equality.

  Explicit top-level requirements now retain their distinct supply identity
  through syntax, resolved/typed/checked trees, Terminal Psi, package review,
  and canonical provider identity. Exact checked `satisfies` edges validate the
  carrier-aware signature, contracts, effects, and installation reach ceiling;
  build harvesting derives only declared public requirements and selects their
  checked provider plans without creating a satisfier edge. Installation reach
  settlement now rejoins the selected exact requirement/provider row. The core
  task claim and public outcome carriers are visible in preparation for their
  exported requirement signatures. Nongeneric payload-bearing bodyless
  external satisfiers now derive selectable provider candidates from their
  exact typed requirement edge and binding. Candidate validation and selected
  provenance replay reject binding, realization-symbol, or requirement-symbol
  substitution. Package review now emits a distinct opaque-blocking external-
  supply row keyed by the exact normalized top-level requirement overload and
  independently replays selected requirement, realization, provider-type, and
  binding identity; canonical review v95/row v53 and recovery canaries retain
  that association. Selected payload-bearing external plans now also extract
  the exact calling-convention row from that same retained plan. Compatibility
  ABI planning resolves the top-level declaration by normalized overload
  identity and preserves its semantic `self` carrier as the satisfier's first
  explicit foreign parameter; trait-receiver erasure cannot drop it.
  Remaining work is selected invocation replay of the installed execution and
  era, exact generic-provider coverage, the remaining compiler-intrinsic
  catalog entries, and final removal of the transitional undifferentiated
  bodyless-machine cases.

  The first ordinary boundary-trait compiler-intrinsic catalog entry is now
  physically closed through provider selection and native emission for Linux
  `Console::exit_process(i32) -> Unit`. Checked
  settlement rejoins the exact toolchain-owned requirement and realization
  symbols, their normalized signatures and conformance, the payloadless
  `CompilerIntrinsic` binding, and the independently selected canonical Linux
  profile; targetless, non-Linux, wrong-symbol, wrong-signature, and sibling
  Console rows remain outside the catalog. Package review v97/canonical row
  v55 retains and rederives the closed execution identity. Terminal planning
  preserves a selected bodyless intrinsic as its exact boundary requirement
  without granting execution authority, and the compiler projects only
  boundary identities actually called by the canonical Terminal artifact into
  exact selected-plan provider evidence. Both Linux x86-64 and AArch64 then
  reuse the existing settlement-gated `exit_group` realization and retain one
  provider execution, one boundary settlement, and an ELF NativeArtifact in a
  product-source canary. D39 deliberately withholds semantic external-
  termination authority from this migration shape: Unit plus a backend-known
  nonreturning syscall does not distinguish successful termination from crash
  or divergence. Carry the same explicit checked terminal-effect completion
  identity through the boundary contract, checked trees, Terminal, and target
  realization before using this path in `TerminalTraceV1`. Compiler-function
  publication certification remains a later engineering rung; read/write
  Console leaves, Darwin/Windows exits,
  source-form inference, and removal of `via Binding::CompilerIntrinsic`
  remain open.

  Migrate `InterruptMaskGuard::restore`,
  `InterruptAcknowledgement::complete`, `Task::request_cancel`, and
  `Task::finish` to this form. Change `complete` to
  `reaches <= MachineControl + PortIo` only when its satisfier and selection
  route land, and remove vacuous `ensures true` clauses. Keep
  `PlacedField::read/take/write` as external realizations: they already carry
  exact `satisfies` edges. Complete the `no_wrap` proposition migration under
  **TARGET-SEMANTIC-APPLICATIONS**; remove the fake bodyless `embed` declaration in favor of the
  canonical compiler-owned fact-position term former; and do not turn N5's
  temporary claim-free `Real` symbols into a language category. N6/N8 must
  replace those symbols with constructed checked operations, while current
  bodyless Real laws remain disclosed axioms until checked proof bodies replace
  them. Add pass/fail canaries for explicit declaration kind, checked and
  external satisfiers, same-reach alternate providers, cross-package
  visibility, exact build selection, unresolved installation rows, token-era
  drift, absence of inference from `reaches <=`/bodylessness/catalog lookup,
  and the complete declaration-classification migration.

  **LEXICAL-PROFILE-V1 is source-complete.** Both maintained lexers now enforce
  the closed Chapter 1 contract: valid UTF-8 framing, ASCII identifiers, and
  exactly space/tab/CR/LF as syntactic whitespace, while non-ASCII bytes remain
  exact inside comments and quoted literal bodies. Unicode-XID classification,
  codepoint-to-UTF-8 escapes, raw-string decoding, the generated product-source
  Unicode table, and the comparator's direct `unicode-ident` dependency are
  gone. One `OutsideLexicalProfile` diagnostic replaces the retired Unicode,
  raw-string, and unsupported-punctuation vocabulary. Canonical lexical
  observation format 2 and a gate-only lex mode compare 31 accepted/rejected
  byte streams exactly across the Omega and Rust implementations, including
  byte-preserving `"café"`, the closed whitespace set, invalid UTF-8, Unicode
  identifier boundaries, codepoint/raw spellings, and quoted newlines. The
  existing domain canary independently rejects establishing `AsciiOnly` for
  `"café"`. Byte coordinates and lexer counts remain `u64`; the UTF-8 framing
  decoder retains `u32` scalar scratch. Future Unicode identifiers or
  an Omega-native raw-payload form require a new normative contract and do not
  preserve the retired spellings by default. Executing the complete fresh
  native product gate remains dependency-blocked on the already-recorded
  attached-Unit transitive machine-plan boundary, not on lexical design or
  source implementation.

  The evaluation-order ruling is closed. Every eager child evaluates exactly
  once in the language's authored left-to-right schedule: attached receiver
  before arguments, strict operands left to right, collection before index,
  range start before end, fixed arrays by increasing index, and record/case
  fields in authored literal order. `&&`, `||`, and transition dispatch are the
  closed selective forms. Ordinary abandonment cleans partially staged values
  in reverse establishment order; completed aggregates retain recursive reverse
  declaration cleanup, and trap/nuclear-abort edges clean nothing. Audit the
  Omega-written compiler, comparator, checked interpreter, Terminal lowering,
  optimizer, and native backends together. Add effect, trap, move, partial-call,
  partial-aggregate, source/declaration-order divergence, short-circuit, and
  mutation canaries. Any incomplete lowering lane must keep its order-sensitive
  compound forms fenced rather than choose a local schedule.

  Inline `case Name = integer;` discriminants are retired and must remain a
  loud parse rejection rather than a bootstrap-private extension. Foreign
  integer sets cross boundaries in typed scalar carriers and map to nominal
  Omega sums through ordinary checked machines with an explicit unknown-value
  path. A future zero-copy optimization must prove complete byte-equivalence;
  it does not revive declaration-level integer tags.

- **OMEGA-RUST-COMPARATOR.** Maintain the current Rust Psi/Omega compiler under
  `source/omega-rust/` as a parallel differential implementation
  while its bug-finding value justifies its cost. It may compare diagnostics,
  normalized semantics, artifacts, and execution observations against the
  Omega-written product compiler, but it grants no authority and must be
  omittable from bootstrap and release builds once the hosted compiler closes.

  Acceptance: shared product suites can exercise both implementations without
  making Rust agreement or availability a correctness, bootstrap, or release
  condition. Rust-specific maintenance stays in the explicit
  `source/omega-rust/` owner and never
  moves into `source/omega/`.

## Execution order

The numbered groups express dependency order, not an exclusive assignment.
Independent compiler lanes may proceed in parallel when their files and
semantic owners do not overlap.

### P1 — Authority, content, and admitted roots

Owners:

- `wiki/design_briefs/authority_values_and_boundary_evidence.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/language_guide/chapter_8_domains.md`

Remaining:

- **ENTRY-CONTENT-ROOTS.** Complete the generated native entry bridge and
  explicit-entry corpus migration. The production-facing installation carrier
  now joins the exact selected root-provider plan and invocation, semantic
  arrival requirement, and both roots before committing either semantic
  occurrence. UEFI selection separately retains the target-fixed physical
  requirement, its two firmware parameter identities, result identity, and
  calling-plan fingerprint as a planned-but-not-invoked contract; it does not
  yet claim a physical shell, bootstrap invocation, or physical root issuance.
  The semantic bridge path maps and zeroes the exact receiver reservation
  and returns one exclusive activation loan; the separate local seam rejects
  provider-issued roots. Connect an emitted target entry stub to that carrier,
  consume the activation loan while invoking the selected source continuation,
  and retain the resulting generated-bridge evidence. Pure language/checker
  fixtures stop at
  checked artifacts; deployable/provider/artifact/ABI/layout/native fixtures
  select an exact target-owned `ProgramEntry`; temporary ABI probes name their
  explicit fixture seam. Sample refresh and native execution must use authored
  roots and never invent one, while targetless checking selects none.

  The function spine now retains a sealed compiler-private identity from
  abstract operations through target/assigned operations, machine
  instructions, encoded bytes, and object-entry selection. `Source(StateKey)`
  and `ProgramStorageEntryWrapper(continuation StateKey)` are distinct: object
  planning selects one exact symbol/identity pair, and synthetic wrapper gates
  retain the source continuation's symbol and text interval separately rather
  than relabeling it. Duplicate, redirected, missing-continuation, and
  interval-drift claims reject. Object planning now publishes every encoded
  function through a compiler-private identity-to-text-symbol linkage table;
  duplicate identities, canonical link-name collisions, overlapping or out-of-
  bounds intervals, and tampered table bindings fail closed. A future wrapper
  call relocation can therefore target the exact retained source continuation
  without rediscovering it by name. For the currently admitted
  `ProgramStorageApplication`/`ImageAndInitialStorage` semantic schema, the
  bridge also retains an address-free wrapper transfer plan that maps both
  semantic root ordinals to their exact source-visible parameter, frame byte
  range, and disjoint capture-instruction rows, plus free-versus-borrowed activation-loan
  receiver behavior. It deliberately does not call the physical arrival plan a
  source-call ABI. The retained bridge plan also owns a sealed platform
  executor gate: only the exact selected physical-provider
  installation and mapped, zeroed receiver activation can construct its
  borrowed continuation handoff, and the executor runs before that activation
  is finished. This gate intentionally does not claim that native bytes
  executed. The main backend now also owns a generic compiler-private direct-
  call operation from abstract operations through target/assigned operations,
  machine instructions, real x86-64 `call rel32` / AArch64 `bl imm26`
  placeholders, exact-identity object relocations, and final-image byte,
  relocation, and footprint replay. Missing, duplicate, redirected, wrong-
  width, and opcode-tampered call claims fail closed. No production builder
  emits that operation yet, and it deliberately carries no invented argument
  or receiver placement. Production builds still emit and select
  `Source(entry StateKey)`. The native bridge now retains an address-free
  outbound continuation ABI, distinct from the physical arrival
  `BoundaryEntryPlan`. The bridge separately retains the exact checked
  source declaration signature captured before typed ownership moves into the
  backend: target slot, machine/state symbols and names, canonical normalized
  callable identity, Unit result, free-versus-mutable receiver identity, and
  ordered receiver-excluded visible parameter type/mode/shape rows. Those facts
  are rechecked against the exact lowered continuation, selected slot, arrival
  parameter identities, and checked receiver layout. For the currently
  admitted UEFI target's semantic continuation shape only, the compiler-private
  Microsoft x64 policy now derives
  one complete `CallPlan` over the optional receiver followed by Image and
  InitialStorage, with Unit result, and validates every placement against the
  sealed declaration shape. A future SysV/AAPCS schema remains fenced until the
  structural classification graph is retained. The production executor gate
  binds the attached receiver placement to the exact mapped address and live
  activation loan; identity, shape, alignment, and loan-length drift reject.
  The free form has a complete layout but no production executor traversal
  because the current gate requires receiver activation. The ABI plan carries
  no runtime root value, `Extent`, root authority, wrapper body, emitted call,
  or callee inbound realization. After activation ends, installed roots can now
  move into a sealed authority-disposition carrier that revalidates the exact
  initial-storage geometry, lineage, rights, provenance, mapping era, origin,
  receiver selection, and complete partition coverage. A receiver-free
  disposition may release Image and whole InitialStorage as two owned root
  authorities. An attached disposition keeps the receiver's
  `OwnedExtentPartition` intact, exposes its potentially noncontiguous before
  and after residuals only by borrow, and fails closed while returning the
  intact carrier if asked for two whole roots. This is not an outbound source
  argument: it cannot move residual authority during the live receiver loan or
  make two separated remainders satisfy one `Extent in Granted` formal. The
  receiver-free whole-root form can now bind to the exact emitted bridge's
  retained free continuation ABI. That sealed carrier owns both `Extent`
  authorities and retains ordered Image/InitialStorage declaration indices,
  call indices, nominal identities, shapes, and address-free placements;
  bridge-binding, target-slot, source-continuation, callable, role/order,
  type/shape/placement, receiver, and Unit-result drift reject while returning
  the intact authority. It does not materialize operand bytes, populate
  registers or stack, emit the call edge, or claim native execution. Attached
  and zero-sized-receiver forms remain deliberately excluded. The
  receiver-free path now has one sealed transition from the recorded
  production installation through validated root disposition and whole-root
  authority into that argument carrier. A borrowed preflight rejects binding,
  source/ABI, receiver, role/order, type/shape, or placement drift without
  consuming the recording; fail-closed errors after ownership starts moving
  retain the highest successfully constructed authority carrier for recovery.
  This linkage still creates no runtime operand bytes, wrapper body, call edge,
  or native-execution evidence. The selected source signature now also retains
  the exact checked `Extent` record graph—`base: addr` at byte 0 and
  `length: u64` at byte 8—and replays its data/field symbols, names, primitive
  types, offsets, aggregate size/alignment, and absence of alternate storage
  encodings against the backend layout. The receiver-free argument carrier can
  move into one sealed non-clone logical-value carrier that keeps Image and
  InitialStorage authority intact while binding their exact base/length
  observations to those declaration and call rows. Structural, role/index,
  target-layout, or wrapping-geometry drift returns the intact prior carrier.
  These are logical values only: no bytes, registers, stack locations, wrapper
  body, call, or execution evidence is produced.
  Those receiver-free logical values can now move into a sealed indirect-
  operand image carrier for the exact semantic continuation ABI currently
  selected by the UEFI target. It
  retains little-endian `{base,length}` bytes beside each immutable
  `ValuePlacement`, requires Image through RCX with caller copy `32..48` and
  InitialStorage through RDX with caller copy `48..64`, and rejects role,
  index, field-layout, shape, pointer, range, size, alignment, overlap, or
  target drift while returning the intact authority-bearing logical carrier.
  The byte images are geometry, not authority. This slice deliberately does
  not allocate or write the caller-copy stack area, populate RCX/RDX, emit a
  wrapper body or call edge, realize the callee inbound ABI, claim native
  execution, or admit attached/zero-sized-receiver entries.
  The operand carrier can now move into a sealed planning-only wrapper caller-
  frame plan. For that same exact ABI it retains the balanced 72-byte outgoing
  reservation/release, the 32-byte shadow area, four ordered eight-byte recipe
  writes at `rsp+32/+40/+48/+56`, and subsequent address-binding recipe rows
  for `RCX=&rsp+32` and `RDX=&rsp+48`. Reservation, release, alignment,
  role/index, operand bytes, field offsets, copy ranges, pointer registers, and
  step ordering are revalidated; rejection returns the intact operand and
  authority chain. These are immutable planning rows only: no machine
  operation allocates or writes stack storage, changes RCX/RDX, inserts a
  wrapper, emits a call or relocation, selects a new object entry, or proves
  native execution.
  The main backend now also owns a compiler-private RSP-relative outgoing-stack
  address-load operation across abstract, target, assigned, machine-
  instruction, encoded-byte, and final-image replay. For x86-64 it retains
  exact `lea register,[rsp+disp32]` bytes, rejects non-positional registers,
  offsets outside nonnegative `disp32`, and non-x86 targets, carries no
  relocation, and derives the exact selected-register plus stack-pointer
  footprint. Synthetic gates pin `RCX=&rsp+32` and `RDX=&rsp+48` and reject
  opcode, ModRM, displacement, metadata, and footprint drift. No production
  builder emits this operation, the authority-bearing caller-frame plan
  remains unconsumed, and no stack reservation/write, wrapper insertion, call
  edge, object-entry switch, or native-execution evidence is claimed.
  The main backend now also owns paired compiler-private outgoing-stack frame
  reserve/release operations through abstract, target, assigned, machine-
  instruction, encoded-byte, and final-image replay. For x86-64, frame size is
  validated to cover Microsoft shadow space, fit positive `disp32`, and
  preserve pre-call alignment; reserve/release bytes and
  X86Rsp/Flags/StackPointer footprints replay with no relocation. Independent
  lowering and final-image scans reject orphan, nested, mismatched, unreleased,
  and out-of-range address-use rows. Synthetic gates pin the balanced 72-byte
  frame around RCX/RDX caller-copy address recipes. No production builder emits
  these operations; no stack stores, wrapper insertion, call edge, object-entry
  switch, or native-execution evidence is claimed.
  The receiver-free wrapper caller-frame plan can now move into a sealed,
  non-clone reserved-outgoing-frame planning authority. It retains the intact
  authority-bearing operand chain and authorizes only four ordered eight-byte
  writes: Image base/length at `rsp+32/+40` and InitialStorage base/length at
  `rsp+48/+56`; shadow `[0,32)` and alignment padding `[64,72)` remain
  unwritable, and rejection returns the intact prior caller-frame plan. The
  main backend also owns `WriteOutgoingStackU64` through abstract, target,
  assigned, machine-instruction, encoded-byte, and final-image replay. X86-64
  emits canonical full-width `mov rax,imm64; mov [rsp+disp32],rax`, reusing the
  host-call qword-store mechanic, with exact RAX/StackPointer footprint,
  untouched flags, and no relocation. Independent assigned and final scans
  require the exact four writes under a live 72-byte reservation before RCX/RDX
  caller-copy address bindings and reject shadow, padding, range, order,
  metadata, byte, footprint, incomplete-sequence, or AArch64 drift. No
  production builder consumes this authority; no physical stack mutation,
  wrapper insertion, call edge, object-entry switch, or native execution is
  claimed.
  The main backend now also retains a compiler-private launch-value copy
  operation across abstract, target, assigned, machine, encoded-byte, and
  final-image replay. For the receiver-free semantic continuation leg currently
  selected by the UEFI target, exact indirect `{base,length}` fields arriving
  through RCX/RDX can be copied into
  the live 72-byte outgoing frame at `rsp+32/+40/+48/+56` before the retained
  address loads. Canonical x86 bytes, RAX/StackPointer footprint, zero
  relocation, exact tuple ordering, and immediate-versus-dynamic write-mode
  separation fail closed. No production builder emits this sequence yet; no
  generated wrapper body, source-continuation call, object-entry selection, or
  native execution is claimed.
  The receiver-free semantic continuation wrapper now also retains sealed
  source-continuation inbound-realization evidence. It joins the independently
  derived free Unit `CallPlan` to the exact encoded `Source(StateKey)` function,
  symbol/text interval, and two immediately-post-`FunctionEnter`
  Image/InitialStorage captures: 16-byte indirect values through RCX/RDX into
  their exact retained frame destinations. Role, declaration/call index,
  normalized type, physical-versus-internal placement, capture order/count,
  pointer, destination, instruction, byte-range, identity, and interval drift
  fail closed; final-image validation independently replays the capture bytes
  and static-storage relocations. Attached entries retain no receiver-free
  realization. This emits no wrapper body or call, consumes no installation-
  derived values, does not switch the object entry, and does not claim native
  execution.
  The emitted receiver-free semantic continuation wrapper now also retains a
  sealed post-encoding phase-alignment template for the generated wrapper body.
  It binds the canonical generated-wrapper identity and symbol to the exact
  retained Source identity, symbol, and text interval, then pins eleven ordered
  compiler-private steps: function entry, a balanced 72-byte outgoing
  reservation, four launch-time indirect Image/InitialStorage field copies
  from RCX/RDX into `rsp+32/+40/+48/+56`, RCX/RDX caller-copy address loads,
  one exact Source-identity call, balanced release, and Unit return. Receiver,
  role/index, shape/placement, identity, interval, call-target, frame, and
  sequence drift fail closed. Installation-owned operand bytes and authority
  are deliberately not routed backward into compilation. A transactional
  second backend pass now consumes this template: it privately relabels the
  retained `Source(StateKey)`, appends the exact generated wrapper operations,
  and rebuilds target, assigned, machine, encoded, object, and relocation plans
  as candidates before publishing any mutation. Independent replay pins the
  wrapper bytes and validation rows, retained Source interval, object entry,
  and exact single `call rel32` relocation to Source. The final bridge names
  `ProgramStorageEntryWrapper(StateKey)` as entry while retaining Source as its
  continuation, and the rebuilt plan proceeds through checked final-image
  validation. Candidate failure leaves the original backend unchanged.
  Written receiver-free builds now bind one sealed emitted-wrapper evidence
  carrier after checked relocation but before any executable, bundle, or final-
  image artifact is published. It joins exact wrapper and Source object
  identities to their placed executable regions, retains offsets, addresses,
  sizes, byte fingerprints, compiler text/function validation, and executable-
  inventory identity, and independently replays the single final `call rel32`
  bytes against the Source interval. The compile report and optional manifest
  retain that carrier; non-writing builds retain none. This proves final image
  content only, not firmware invocation, installed roots, or native execution.
  That carrier now also retains independently replayed semantic-continuation
  evidence for the exact UEFI-target receiver-free semantic leg: the checked
  Image/InitialStorage continuation-plan placements must be the same indirect
  RCX/RDX placements consumed by the generated wrapper, and the four ordered
  launch-value copy rows must occupy exact in-wrapper byte ranges, match their
  canonical 15-byte encodings in both encoded and final text, and own no
  relocation. Placement, role/index, wrapper identity/interval, row inventory,
  byte, or relocation drift fails closed. This remains final-image evidence;
  it does not connect installed authority to firmware launch or prove that the
  platform invoked the wrapper.
  The receiver-free whole-root argument carrier can now move into a sealed,
  non-cloneable emitted-wrapper binding. This transition keeps both installed
  `Extent` authorities intact while requiring final wrapper evidence from the
  same bridge, then replays wrapper/Source identities and intervals, executable
  fingerprints, the physical calling-plan fingerprint, Image/InitialStorage
  roles and indices, exact indirect placements, and all four canonical
  launch-value copy rows. An unwritten bridge or any identity, placement,
  interval, byte, or row drift rejects while returning the intact authority
  carrier. This proves that those installed ordinary values match one exact
  final wrapper certificate; it does not prove firmware supplied them to that
  wrapper, invocation, or native execution.
  Production builds still lack a source-compatible attached-root
  value/authority carrier (or separate hidden supply), final firmware
  composition, and native-execution evidence; those remain before this slice
  is complete. The next receiver-free production boundary is now exact: the
  target profile must fix a physical UEFI requirement receiving `ImageHandle`
  and `SystemTable` and returning `EfiStatus`, then join its generated ABI shell
  to an exact target-authored bootstrap adapter. That adapter installs scoped
  firmware providers, obtains image geometry/correspondence evidence, acquires
  independent initial storage, proves the selected stack/resource plan, and
  crosses the separate
  `ProgramStorageEntry::enter` semantic installation edge before invoking the
  build-bound continuation. The first profile is a returning UEFI application;
  the fallible linear map/exit protocol belongs to the distinct OS-handoff
  lifecycle below. Until that producer exists, no compiler carrier may
  claim that installed Image/InitialStorage authority occupied RCX/RDX at this
  semantic-wrapper invocation. Native-invocation evidence belongs after that
  adapter;
  another compiler-side authority row would duplicate facts without closing
  the boundary.

  The CLI corpus is rooted on all hosted targets except the four GUI samples,
  which currently select Windows x64 and macOS arm64. Linux needs an ordinary
  source-level `Gui`/`Input` provider plus its general call/result realization;
  that is engineering work, not a language-design blocker. It first needs an
  authored Linux GUI protocol/provider contract and executable binding path:
  no X11/Wayland provider exists, and the current ELF direct-image emitter
  cannot bind shared-library imports. ELF final-image emission now derives one
  canonical referenced-import request catalog joining retained provider-
  authored library/symbol identities to exact relocation sites and rejects
  invalid, duplicate, unqualified, or unowned rows before the loader boundary.
  Actual binding remains fenced on interpreter/dynamic-section, PLT/GOT,
  symbol-version, and exact-library/interposition policy. Provider substitution
  must not use a headless or fake-handle shim in place of the samples' real-
  window contract.
  Proof-only and
  deliberately trapping fixtures remain targetless. Final firmware composition
  of `ImageHandle`/`SystemTable` inputs with semantic roots is specified below;
  the remaining physical bridge and corpus work is engineering. The native
  differential RUN corpus now routes every host-authored fixture through
  production entry selection (including bounded outer-job/single-worker native
  compiles) instead of silently retaining the legacy test-entry seam. Eight
  result-as-process-exit probes now keep their value-returning logic in ordinary
  helpers while target-rooted Unit entries consume those results through the
  explicit exit provider; that migration also closed named unsigned-conversion
  signedness and logical-NOT helper-result lowering gaps. Six additional
  residual scalar-result and host-deployable probes now use the same authored
  four-host root and Unit-entry discipline. Eight indexed-array and slice-loop
  native probes now also route their existing Unit entries through authored
  four-host roots without weakening their bounds, mutation, or conversion
  regression shapes. Nine further indexed-access, mutable-slice, subslice, and
  two-pointer native probes retain their exact regression programs while using
  the same authored production roots. Ten direct/dispatched slice reads,
  element copies, frame aliases, and bounded or dynamic subslice probes now
  likewise compile and run only through authored production roots. The tracked
  nested-window, parameter-subslice, runtime-end, and descriptor-pointer probes
  add ten more unchanged Unit-entry programs to that rooted native cohort.
  Eight linear ownership handoff, transfer, and transparent-record frontier
  fixtures now preserve their ownership and transition programs in direct Unit
  entries with explicit exit providers. Ten named float provider and conversion
  matrices now also use authored roots for native and cross-target differential
  execution. Ten indexed string-concat, bounded-carrier, slice-alias, and guard
  probes now consume the same checked-in four-host roots in their native and
  cross-target artifact tests. Ten further fixed-index, pointee, mutable-
  parameter, copied-struct, and lookup-driven text probes now use authored
  roots in their native and cross-target artifact coverage. Ten array
  reduction, indexed-write, indexed-guard, and stack-algorithm probes now also
  run their unchanged Unit entries through authored four-host roots. Four
  nested-loop/index probes and six dependent range, ordering, subtraction, and
  product-index probes likewise retain their exact programs under authored
  four-host roots. Ten dungeon reentry, Boolean/ordered dispatch, and
  string-field lookup regressions now also run their unchanged Unit entries
  through authored roots. Eight atomic operation probes and two structural
  dispatch/nested-field probes now share authored roots across native execution
  and the existing AArch64 opcode checks. Ten aggregate construction, nested-
  field, and value-copy probes now likewise run unchanged Unit entries through
  authored roots. Ten call-result, machine-owned storage, sum-payload, and
  subslice-window probes now likewise use authored production roots. Ten text-
  storage, string-reference, room-dispatch, and tuple-matrix probes now also
  use authored production roots. Ten domain-membership, address-value, finite-
  matrix, and static generic-dispatch probes now likewise consume authored
  roots. Nine proof/runtime, dependent-call, saturating-arithmetic, storage-
  alias, and case-membership probes now also consume authored roots. Ten
  indexed-write, target-selection, stdin, host-result, room-dispatch, and
  accepted-proof probes now likewise consume authored roots. Ten trapping-
  conversion, trapping-float, and portable filesystem probes now also consume
  authored roots. Ten portable filesystem wrapper probes now likewise consume
  authored roots. Ten value-call, indexed-collection, and result-domain probes
  now likewise consume authored roots. Fifteen typed-dispatch, fixed-integer,
  saturating-time, wire-policy, portable-filesystem, and console-byte probes now
  likewise consume authored roots. Fifteen Option/record value calls,
  computed-transition arguments, dispatched result deliveries, and distinct-
  receiver call chains now likewise consume authored roots. Fifteen further
  nested/parameter receiver, text-guard forwarding, dispatched terminal,
  partition, and slice-result probes now likewise consume authored roots. The
  next fifteen borrow/view, alias-transition, nested-terminal, builder/time,
  host-output, state-loop, reference-field, and dungeon-guard probes now retain
  the same direct Unit entries under authored four-host roots; their three
  target-specific footprint consumers use those checked-in roots as well. The
  final broad portable cohort adds twenty-four expression, slice/index, Result,
  text-domain, storage, and closed trait-dispatch fixtures without changing
  their direct Unit programs; the frame-indexed footprint consumer now uses
  that checked-in root as well. The tuple-transition and referenced-local
  sibling-guard result probes now keep their value-returning dispatch in
  ordinary helpers while rooted Unit entries route the exact results through
  the console exit provider. The referenced-local migration exposed and fixed
  native nested-splice ordering: a deferred branch prelude, straight-line arm,
  and leaf expansion now fire as one inner-first bundle after every local,
  host-call, or mutation effect in the contiguous callee splice, matching the
  interpreter instead of letting the parent entry mutation overwrite a nested
  leaf mutation. That ordering now distinguishes the newly deferred nested
  statement-call prelude from assignment-value, transition-result, and host-
  argument preludes that must still run at the call site: their declaration-
  time local capture is preserved while their straight-line and leaf result
  selection waits for the contiguous splice. The bounded-product index probe
  now retains its exact runtime
  coupling under an authored root: the contract widens each u32 factor to u64
  so the proposition is total without citing itself as overflow evidence, and
  both typed validation and resolved hoist synthesis project only independently
  checked value-preserving unsigned widenings back to the original field
  identities. The i64-backed interval lattice's missing u64 endpoint is closed
  by a structural width proof limited to two unsigned widening operands whose
  source-width sum fits their common target. The local-named dynamic probe also
  now has an authored root. Raw Windows and GUI fixtures remain platform-bound.
  The final three non-GUI gaps—User32 key-state and the two raw-filesystem
  Windows probes—now retain authored Windows entry selection. Their exact
  Windows roots are structurally cross-compiled on every development host,
  while native execution remains Windows-gated; this does not imply Linux
  `Gui`, `Input`, or raw-filesystem lowering. An additional bounded-carrier
  regression now projects one runtime-indexed `u8` value and explicitly widens
  it to `i32`, proving unsigned extension agrees between checked interpretation
  and native execution rather than existing only in guard comparison. A
  registry-derived inventory now pins 891 `RUN_CANARIES`, 887 with authored
  roots, and exactly the four excluded GUI fixtures rootless. The tracked
  non-GUI authored-root backlog is zero. The earlier reported
  backlog of 18 was incorrect: its baseline parser omitted 39 multiline-form
  RUN rows, then
  the migration ledger subtracted 34 authored roots outside `RUN_CANARIES` as
  if they belonged to the differential corpus.
  The non-GUI entry migration is complete. Keep exactly the four GUI exclusions
  rootless until real platform providers exist; when they migrate, use ordinary
  Unit entries and explicit exit providers rather than restoring the legacy
  entry seam or inventing fake GUI authority.
- **UEFI-PHYSICAL-SEMANTIC-ENTRY — implement the settled two-surface bridge.**
  The target-package and contract-retention slices are complete. The UEFI
  physical types, `UefiPhysicalEntry::enter`, calling policy, and boundary
  schema now live in the exact toolchain-owned `uefi_x64` target package rather
  than in application fixtures. `omega-target` selects that package through a
  closed identity; compilation checks the exact target-package declaration
  identity plus canonical package-relative source membership, then retains the
  package fingerprint in
  the physical plan and manifest. Application-authored lookalikes reject.
  Normalized target and artifact identity keep the physical requirement, its
  exact two input types, `EfiStatus` result, and evaluated Microsoft-x64 plan
  distinct from the semantic `ProgramStorageEntry::enter` continuation.
  Physical and semantic plan conflation, missing/duplicate plans, wrong calling
  policy, and missing physical result reject. Manifests still report this
  contract as `planned_not_invoked`; receiver-bound semantic bridges may omit
  receiver-free wrapper evidence only under their exact checked receiver/
  activation-loan predicate.

  The target-owned UEFI x64 layout rung now seals the complete known 120-byte
  `EFI_SYSTEM_TABLE` prefix beneath that exact package, entry slot, and
  Microsoft-x64 target selection. Eighteen ordered rows retain the flattened
  24-byte table header, explicit four-byte ABI padding, every console/service
  handle or pointer, native entry count, and configuration-table pointer.
  Independent replay checks target/package/entry identity, row order and
  cardinality, offsets, widths, alignments, exact prefix coverage, and
  deterministic layout identity. This proves `ConOut` at byte 64 and
  `BootServices` at byte 96 but itself inspects no occurrence and grants no
  pointer, provider, lifecycle, root, shell, or execution authority. A
  separate non-authorizing occurrence gate now consumes that layout and joins
  it to borrowed bytes only after validating the exact system-table signature,
  a runtime `HeaderSize` covering the known prefix and no larger than the
  supplied occurrence, zero `Reserved`, and the UEFI CRC32 over every table
  byte in the runtime `HeaderSize` extent with the stored CRC field zeroed. It
  retains revision for later capability-specific policy, accepts CRC-covered
  forward-compatible suffixes, and deliberately projects no pointer field. A
  later lifecycle-scoped provider must still join this header-integrity
  evidence to occurrence provenance and firmware phase before projecting
  services.

  The returning-profile lifecycle composition gate now lives in
  `omega-external-roots`. One invocation-owned firmware ledger mints a
  non-clone exact-range occurrence provenance carrier and the sole current
  Boot-Services-live phase lease. The consuming join replays the exact UEFI
  x64 entry/layout identity, requires integrity and provenance to retain the
  same allocation and range, rejects copied report identities from a foreign
  ledger, and returns all three inputs on rejection. Success is a non-clone,
  metadata-only lifecycle carrier: it exposes no table bytes, raw address,
  service-field value, provider, `Extent`, root, or execution authority.
  Releasing it consumes the retained inputs and returns only a report
  observation; a failed release returns the complete scoped carrier. The
  returning ledger will not begin firmware return while its phase lease is
  live.

  The physical-input custody rung is now complete. The invocation ledger mints
  the opaque image-handle occurrence provenance once, and a recoverable join
  consumes it with the lifecycle-scoped system table and retained physical
  contract only when all three name the same private ledger, firmware session,
  and invocation. The plan owner independently reconstructs the target-authored
  UEFI x64 contract, including its integer-only volatile-register ceiling, and
  replays the exact package-source commitment, normalized requirement,
  input/result types, report fingerprint, and complete boundary plan; a
  compiler canary checks this against the contract produced from the real
  target package. The resulting
  non-clone carrier remains pre-provider and pre-installation and exposes no
  raw handle, address, table bytes, `Extent`, storage root, shell, or execution
  authority.

  The first provider-specific correspondence rung is now live without yet
  creating a callable provider. A consuming projection replays the exact live
  ledger, physical contract, complete UEFI x64 system-table layout, and the
  target-owned BootServices pointer row at ordinal 15 / byte 96. It retains the
  nonzero field privately under the complete physical-arrival and phase-lease
  custody. Rejection and ledger mismatch return that custody intact; successful
  owner release consumes the projection before firmware return. Its public
  surface exposes only report coordinates—never the field value, table bytes,
  an address, `Extent`, service operation, provider execution, shell, or root
  authority.

  The next bounded provider prerequisite now retains the complete UEFI x64
  `EFI_BOOT_SERVICES` layout as forty-nine flattened rows over the 376-byte
  aggregate and independently validates one runtime occurrence's signature,
  header coverage, reserved-zero field, and CRC. The lifecycle join requires
  that occurrence's admitted address to equal the still-private System Table
  `BootServices` field and pins the nonzero `HandleProtocol` function row at
  ordinal 21 / byte 152 without exposing or invoking its address. One consumed,
  closed provider outcome for the exact Loaded Image GUID can establish only
  non-null, nonempty, non-wrapping image base/size correspondence for the same
  opaque image-handle occurrence. Rejection returns complete provider custody
  for retry or release, and firmware return remains blocked until release.
  This is not an `Extent`, a `Granted` root, a physical shell, the target
  adapter, semantic installation, or native execution.

  That lifecycle provider can now move into one exact address-free
  `HandleProtocol` invocation-plan carrier before any outcome is admitted. The
  plan independently replays the service identity and row, Loaded Image GUID,
  three pointer-shaped input types, `EFI_STATUS` result, Microsoft-x64
  RCX/RDX/R8 and RAX placements, shadow/alignment/state plan, collision-
  resistant calling-plan commitment, and the closed Success /
  InvalidParameter / Unsupported status table. Outcome admission now consumes
  this non-clone plan rather than the unplanned provider. Failure returns the
  complete planned custody for retry or release. This adds no runtime argument
  values, interface destination, service-pointer exposure, call operation,
  emitted bytes, firmware execution, image root, or adapter/shell claim.

  The first adapter-composition typestate now consumes the complete physical-
  arrival custody before `BootServices` provider projection. It independently
  replays the exact target-owned UEFI x64 physical contract and retains the
  collision-resistant calling-plan commitment; foreign-ledger and commitment-
  substitution failures return the whole readiness carrier or arrival for
  owner retry. Provider projection and every later `HandleProtocol` rung now
  remain structurally beneath that non-clone readiness value. This is not
  environment or entry-stack admission, `UefiPhysicalEntry::enter`
  satisfaction, a generated shell, native adapter execution, semantic root
  installation, or publication. Those claims still require target-owned
  arrival/stack evidence and the generated invocation/install producers rather
  than a caller-chosen receipt identity.

  The symbolic target-semantics rung is now retained explicitly. The closed
  compiler-owned application
  `TargetSemantics::guaranteed_entry_stack<UefiX86_64>()` records its
  projection, subject, semantics version, and selected `UefiX64` deployment
  profile in exact compatibility evidence and a domain-separated commitment;
  its compact compatibility value remains report-only. Cross-target
  application and profile/report/commitment substitution reject, and the
  physical-entry contract now retains and replays that exact application. This
  Target closure now also seals the UEFI x64 boot-services minimum specified
  by UEFI 2.11 section 2.3.4: 131072 available bytes with 16-byte alignment.
  The physical-entry contract retains and independently replays the complete
  numeric closure beside the symbolic application. A planning-only same-stack
  relation binds one exact adapter-readiness invocation to four explicit,
  nonzero coordinates—generated shell WCSU, live adapter frames, maximum
  nested continuation/provider WCSU, and target reserve—and rejects omission,
  arithmetic overflow, or a sum above the target guarantee. The plan retains
  the readiness ledger's private authority plus both physical occurrence and
  live phase-lease coordinates, so equal public report IDs from another ledger
  cannot substitute; exact equality with the 131072-byte guarantee is admitted.
  These coordinates are not yet derivation evidence: exact emitted-shell,
  checked-adapter, continuation/provider WCSU producers and physical-arrival firmware-
  conformance admission remain before the plan can authorize invocation. No
  runtime stack address, stack storage, environment admission, or private-stack
  authority is granted.

  The next provider/adapter composition edge is design-settled. Implement one
  exact target-runtime bootstrap adapter satisfying
  `UefiPhysicalEntry::enter`; the generated shell invokes it, while `build.omg`
  binds only the semantic
  continuation. Keep `EfiSystemTable` private beneath lifecycle-scoped
  providers and `EfiImageHandle` as an opaque provenance-bearing input; neither
  is an `Extent`. Have the adapter consume the landed integrity/provenance/phase
  join before exposing any provider projection.

  Land the returning `UefiApplication` profile first. Its checked adapter uses
  exact admitted provider postconditions: physical arrival supplies a valid
  system-table occurrence, selected initial regime, and conforming entry stack;
  `HandleProtocol` yields Loaded Image correspondence for the supplied handle;
  allocation/free operations transfer and return exact page custody. Derive
  interval geometry and separation above those claims. Obtain InitialStorage
  from a separately owned reserved region in the final image plan that is
  proved disjoint from the installed image root, or from an explicit
  allocation, never from the active provider-selected entry stack.

  Feed the closed
  `TargetSemantics::guaranteed_entry_stack<UefiX86_64>()` relation with exact
  derivation evidence for generated shell WCSU, live adapter frames, maximum
  nested continuation/provider WCSU, and one explicit reserve, or emit and
  verify a private-stack switch that preserves the physical return state. The
  current four-coordinate planning carrier deliberately does not authenticate
  those producers. When stack and source storage share a parent allocation,
  conserve exact active-stack, retained-bootstrap, and disjoint contiguous
  InitialStorage partitions.

  Cross `ProgramStorageEntry::enter` once, invoke the build-bound continuation,
  reclaim adapter-owned resources, map normal Unit return to the fixed success
  status, and map each recoverable bootstrap rejection through the closed
  target status table. Crash, trap, and abort remain non-returning routes and
  never synthesize `EfiStatus`. Join physical invocation, exact provider
  postconditions and arrival admissions, stack plan, semantic root positions,
  continuation, and one installation receipt. Until that producer exists, no
  carrier may claim a physical shell, bootstrap invocation, installed roots at
  the semantic wrapper, native execution, or publication.

  Add canaries for physical/semantic ABI conflation, hidden firmware
  parameters, handle-to-extent fabrication, broad firmware admission, treating
  entry stack as InitialStorage, omitted WCSU contributors, early target-value
  folding, overlapping partitions, incorrect recoverable status mapping, and a
  fabricated status on a non-returning route.

- **UEFI-OS-HANDOFF — implement the separate custody-transferring lifecycle.**
  Do not extend the returning Unit profile implicitly. Define the successful
  OS-loader contract over a private surviving stack and a target-bounded
  `GetMemoryMap`/`ExitBootServices` state cycle whose decreasing attempt count
  and every non-copy capability, allocation, snapshot, and key are explicit
  arrival terms. Stale-key rejection returns all live custody before map
  reacquisition and retry; exhaustion returns the authored EFI error while Boot
  Services remain live; success consumes boot-scoped services, transfers
  existing allocation lineage, permits only policy-qualified final-map
  introductions, and does not return to firmware. Add canaries for unmeasured
  retry, lost custody, provider use after exit, duplicated storage lineage, and
  a successful handoff that retains the firmware stack.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Take one real
  content-bearing source program through terminal Psi. Add sealed introduction
  and custody-exit frontiers, derive residual geometry at partial bodyless
  boundaries, and admit only provider custody. Infer identity-preserving
  reshuffles; partition changes require an authored theorem. Before emitting an
  introduction or exit, checked facts must bind the exact content subject and
  geometry to the selected provider plan, invocation receipt, backing/root
  lineage, installed occurrence, and route; a generic established-claim identity
  is insufficient. Checked source already derives exact identity-reshuffle and
  authored-partition composition rows, and terminal Psi independently validates
  their canonical replay. Terminal vocabulary 29 now additionally binds every
  partition row to its exact emitted call operation, retains canonical authored
  boundary guarantees, checks internal or boundary target correspondence and
  exact structural arguments, and schedules the derived theorem only after
  that call succeeds. Missing/non-call producers, guarantee drift, argument
  drift, pre-call use, and fingerprint-only replay reject. The first internal
  structural-result call slice now carries one explicit operation-result place,
  structural signature and qualification set, caller result-claim binding, and
  exact callee-to-caller returned-claim map. Checked production accepts only a
  final direct call that moves one whole linear qualified root into an exact
  one-parameter checked callee and immediately returns the same occurrence.
  Canonical format 27/vocabulary 29, independent verification, interpretation,
  and fixed-fuel derivation preserve that transfer; the result and its claims
  become live only after successful callee return, and crash produces neither.
  Bodyless results, projections, and local staging remain fenced. Checked
  source production still emits the bounded one-claim form. Canonical Terminal
  Psi, independent verification, and interpretation now admit one whole-root
  structural argument carrying an exact nonempty finite claim map: caller
  transfers, callee entry claims, every successful callee return, returned
  transfers, and result bindings must form path-preserving bijections. Missing,
  duplicate, swapped, overlapping, or path-mismatched rows reject; suspension
  replays no transfer and crash abandons only the exact live frontier. The
  verifier now uses one
  multiplicity-independent partial-custody frontier: a projected owned move
  blocks whole-root use and return, overlapping moves reject, and the bounded
  dense linear-array slice closes only after every sibling has transferred.
  This closes the sibling-debt hole without admitting projected
  `CallStructural`; reconstruction and wider aggregate shapes remain fenced.
  The exact
  root-only source passthrough now produces a
  structural result/return carrier with claim transfer, exit-time content
  replay, interpretation, and fuel. Omega preserves that carrier through the
  exact direct native ABI path and all artifact/install layers, with claim
  identity retained as zero-runtime metadata. The native path now covers one
  8-byte fragment and direct 9--16-byte integer-class aggregates split across
  two canonical registers on System V AMD64 and AAPCS64. Microsoft x64's
  indirect aggregate plan, wider or non-integer-class shapes, multiple roots or
  claims, projections, staging, and bodyless calls remain fenced. Exact
  whole-parameter content
  custody now also lowers from a real qualified source declaration through both
  Unit and primitive-result bodyless exits. The source-derived terminal entry
  row retains the checked claim, callable-entry-revision subject, owner-unique
  projection,
  and content algebra; verification rejects structural/content rebinding,
  provider rejection leaves custody live, and successful completion consumes
  it. This slice remains deliberately whole-root: projected bodyless exits still
  require authored partition/residual geometry. Bodyless boundary completion
  without content now has a separate projected installed-provider replay
  slice. Omega resolves each relevant-record or in-range fixed-array path to
  the exact provider leaf type, rebases the provider's whole-root entry claim
  onto that caller path, requires the matching completion receipt and source,
  and preserves unrelated sibling claim sources. Cross-path, receipt, provider,
  access, multiplicity, or type substitution rejects. Projected content remains
  fenced by the residual-geometry requirement above.
  Content-bearing bodyless boundary completion sources also retain one
  canonical combined whole-claim/content row: exact
  claim, optional structural entry path, full callable-entry-revision content
  subject,
  and owner-unique projection/algebra identity. Omega preserves and replays the
  catalog beside exact provider execution through native evidence and canonical
  installation format 30; missing, duplicate, malformed, or whole/content-
  mismatched custody rejects. Each successful receipt now additionally retains
  an exact structural binding to its complete caller source and enclosing
  admitted provider execution. Machine emission derives the ordered catalog,
  object and installation validation independently rederive it, and canonical
  installation format 31 preserves it; source, receipt, or provider substitution
  rejects. One real content-bearing source canary now closes that same row
  through verified Terminal Psi, an admitted native provider, target assignment,
  machine/object/image emission, and canonical installation replay. Qualified
  or linear scalar-function structural inputs enter that native lane only when
  the exact boundary argument, entry source, and completion receipt carry their
  claim into provider custody; missing content, receipt substitution, and
  provider substitution reject before object publication. One private
  completion-custody facade now owns argument-path
  canonicality, receipt bounds, exact source/receipt replay, and provider-
  custody replay in their load-bearing order; object and installation callers
  only map its closed failures to unchanged public errors. The format-31
  completion-source codec now likewise lives in a private 189-line child,
  retaining exact tags, reserved bytes, structural paths, content segments,
  projection/algebra rows, count guards, bytes, and decode errors. Its format-
  31 structural argument place/path codec now lives in a further 83-line child,
  retaining field/index tags, reserved bytes, UTF-8/nonempty-field validation,
  count/end guards, exact bytes, and established errors. A 51-line scalar-
  result codec additionally owns the exact six-byte Boolean/integer/address
  type grammar and reserved-byte/invalid-result errors. A 57-line call-site
  owner codec likewise owns operation and cleanup tags, exact identities,
  canonical cleanup zero, reserved bytes, and the established invalid-tag and
  nonzero-cleanup errors. A 33-line provider-execution codec now owns the
  ordered five-identity grammar and nonzero decoding shared by enclosing
  settlements and nested completion-custody bindings, removing the final
  duplicate byte spelling while leaving admission and closure checks in the
  parent. Canonical format 31 now also isolates each installed function's
  optional Unit/scalar stack envelopes and ordered Unit/scalar call-site rows
  in a private 164-line stack-facts codec. The parent retains function order
  and admission validation; the child preserves exact owner tags, target
  identities, offsets, stack-byte facts, count guards, reserved bytes, and
  established decode errors. Canonical value shapes, machine-register tags,
  structural single-location placements, and direct multi-location/indirect
  placements now live in a private 310-line codec. Function-home, internal-
  call, structural-return, and boundary-result rows retain their exact order;
  canonical bytes, reserved fields, representability guards, error precedence,
  and admission replay are unchanged. Installed function parameter and
  parameter-home rows now share a private 149-line codec. Unit and scalar rows
  retain their original positions around affine cleanup evidence, exact bytes,
  reserved fields, direct placements, count guards, and distinct literal zero-
  identity diagnostics; admission replay is unchanged. Unit affine-cleanup
  records and scalar-control cleanup lists now share a private 203-line codec.
  Function-row presence fields and order, structural/local/action bytes,
  literal identity diagnostics, count/capacity guards, cleanup canonicality,
  fuel/call validation, and admission replay remain unchanged. Each installed
  structural-return row now lives in a private 158-line codec. Machine/edge
  identity, ordered parameters and placements, source/result declarations,
  returned claims, trivial affine locals and discards, offsets, literal
  diagnostics, representability errors, exact bytes, and precedence remain
  unchanged; the parent retains function association, validation, and
  admission replay. Each installed internal Unit-call custody row now lives in
  a private 296-line codec. Owner/result tags, structural arguments, exact
  source/destination placements, fixed-array facts, emitted bytes, claim
  transfers, reserved fields, count guards, literal diagnostics, and offset
  errors preserve exact bytes and precedence; the parent retains call order,
  stack composition, validation, and admission replay. These structural-return
  and internal Unit-call codecs now also own their complete ordered collections,
  including count bytes and decode allocation guards. The parent retains upfront
  count conversion and global collection order; exact row bytes, literal
  diagnostics, the internal-call minimum-row capacity guard, validation, and
  admission replay remain unchanged. The ordered semantic-code-attribution
  collection now lives in a private codec. Operation/edge site identities,
  ordinals, text/code spans, reserved fields, count/capacity guards, literal
  diagnostics, and offset errors preserve exact bytes and precedence; the
  parent retains upfront count conversion, canonicality, function association,
  and admission replay. The ordered privileged port-effect collection now lives in a private
  98-line codec. Machine/operation/service identities, port/value facts,
  ordinals, text/code spans, reserved bytes, count/capacity guards, literal
  diagnostics, and offset errors preserve exact bytes and precedence; the
  parent retains upfront count conversion, effect/settlement order, byte
  validation, function association, and admission replay. The ordered boundary-
  settlement collection now lives in a private 270-line composition codec.
  Provider execution, realization, structural arguments, completion sources,
  receipts and provider custody, optional native result, identities, spans,
  reserved bytes, count/capacity guards, literal diagnostics, and offset errors
  preserve exact bytes and precedence; the parent retains upfront count
  conversion, settlement order, validation, and admission replay. The
  format-32 successor adds ordered scalar-argument custody and an exact
  Linux-only `exit_group(i32)` realization. Object, image, and installation
  validation independently replay the consumed literal, ABI destination,
  nonempty code interval, syscall bytes, and trap-on-return tail; Darwin and
  Windows reject this realization before emission. Whole-entry stack-demand
  composition recognizes only that validated full-body nonreturning leaf as
  an exact zero-stack contribution; other missing stack evidence still
  rejects. All format-31 custody rows
  retain their canonical spelling under the new marker. The ordered installed-
  function collection now lives in a private 172-line
  composition codec. Function identity, attachment and spans, stack facts,
  Unit/scalar parameter-home carriers, cleanup presence fields, nested row
  order, reserved bytes, count/capacity guards, literal diagnostics, and offset
  errors preserve exact bytes and precedence; the parent retains upfront count
  conversion, cross-function order, canonicality, association, and admission
  replay. The selected-provider-plan codec now owns its complete ordered
  catalog: count bytes, exact nonzero identities, the eight-byte minimum-row
  capacity guard, and strict increasing-order replay. The parent retains upfront
  count conversion and global record order; canonical bytes, literal errors,
  validation, and admission replay remain unchanged. The fixed installation-
  header codec now owns magic/version, Terminal-Psi identity, target/subsystem
  facts, profile/image identity, and compiler-text validation evidence. The
  parent retains validation and every preflight count conversion in original
  order; canonical bytes, literal errors, target admission, and installation
  replay remain unchanged. Every header and row codec now routes through one
  bounds-checked private wire layer for cursor advancement, little-endian u16/
  u32/u64 writes, and boolean-tag decoding. Facade APIs, byte order,
  truncation/error precedence, canonical re-encoding, validation, and admission
  replay remain unchanged. One private fingerprint codec now owns the domain-
  separated installed-image and installation-record SHA-256 digests, including
  length framing and canonical hexadecimal rendering. Digest domains and bytes,
  public identity types, validation comparisons, literal errors, and admission
  replay remain unchanged. One structural scalar codec now owns identity
  strings, multiplicity tags, and qualification-domain catalogs shared across
  function, return, completion, and structural-type rows. Exact bytes, UTF-8/
  nonempty and identity diagnostics, domain/multiplicity error order, public
  APIs, validation, and admission replay remain unchanged. A structural-
  signature codec now owns exact source-parameter and result-declaration rows,
  including place/type identities, multiplicity, reserved fields, and
  qualification catalogs. Structural-return order, row bytes, literal
  diagnostics and precedence, public APIs, validation, and admission replay
  remain unchanged. One trivial-affine-local codec now owns local-place rows and
  their paired empty-record type declarations across structural returns and
  affine cleanup. Exact bytes, reserved fields, shape/identity diagnostics and
  precedence, public APIs, validation, and admission replay remain unchanged.
  The structural-field codec now owns every field-row encoder and the sum-case
  field decoder across Boolean, integer, float, byte-sequence, nested, and
  erased shapes. Exact tags, reserved bytes, identity/shape diagnostics and
  precedence, public APIs, validation, and admission replay remain unchanged;
  record decoding remains separate for a dedicated consolidation. The
  structural-case codec now owns sum-case counts, ordered identities, and
  nested field catalogs. Exact shape tags, case/field bytes, capacity guards,
  literal diagnostics and precedence, public APIs, validation, and admission
  replay remain unchanged. The structural-record codec now likewise owns
  record field counts, capacity guards, ordered allocation, and exact field-row
  replay. The prior byte-for-byte duplicate record decoder shares the
  established field codec; tags, bytes, literal diagnostics and precedence,
  public APIs, validation, and admission replay remain unchanged. The
  installation parent is now 2,640
  lines. This is
  custody, not
  authorization. The remaining
  work is real
  authorized introduction, custody exit, residual geometry, and provider binding—not
  another passthrough representation.
- **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION.** Implement the settled
  domain-route/installation model without a provision declaration. A
  content-bearing domain remains the sole source-level authority for one exact
  requirement. Its statically enumerable installed parameter positions may
  introduce fresh program-local lineages; ordinary calls and result routes with
  no parent lineage reject. Reconstruct exact per-occurrence capacity from the
  requirement instance, qualification, and owner-unique `Content<A>` projection,
  including owner-constrained const families. Join that schema to finite slot
  cardinality and lifecycle epoch during installation verification and derive,
  rather than trust, the aggregate for one installed artifact instance.

  Preserve provider issuance as a distinct admitted origin and keep
  `MappingEraId` separate from lifecycle epoch. Terminal Psi must
  first provide the canonical requirement-position, qualification, projection/
  algebra, capacity/family-instance, and artifact-scope schema; installation can
  then join exact slot occurrences, cardinality, artifact instance, and epoch.
  The Rust product implementation now publishes and independently replays the first portable
  producer-schema slice: one exact static boundary requirement and authored
  parameter position, its exact qualified carrier and normalized domain
  identity, the owner-unique content projection and closed algebra, normalized
  per-occurrence capacity, and one canonical schema identity that excludes
  module-local dense IDs. Terminal codec fixtures and source/verifier tamper
  canaries cover this description. Terminal vocabulary 27 now also retains the
  owner domain's normalized content projection independently from every route
  schema. The verifier replays that owner definition first and requires each
  producer schema, entry claim, reshuffle, and partition proposition to cite
  the exact same projection identity and algebra; coherently lowering both a
  producer's capacity and its self-authored fingerprint therefore still
  rejects against the unchanged owner definition. It is intentionally not an introduction
  event and mints no claim. The portable side now has one owned producer
  catalog constructible only from a successfully verified Terminal module. It
  retains the exact Terminal identity and entry plus each resolved requirement,
  qualification, carrier, and producer schema, while deliberately carrying no
  occurrence, cardinality, lifecycle, lineage, or grant state. The Rust product implementation
  accepts only that catalog for non-authoritative installation prebinding, then
  replays the exact native object bytes/architecture/entry and binds the
  admitted provider execution, installed code/artifact, root, slot, owner, and
  admission. Prebinding identities are exact typed tuples rather than trusted
  compact hashes, their records are private, and distinct occupied slots derive
  only a reportable snapshot count from ledger state.

  The component-era side issues an opaque non-clonable epoch lease only for the
  exact current open era and entry contract. Each lease is bound to one ledger,
  installed artifact occurrence, entry plan, and plan admission receipt;
  quiescence and retirement reject while a lease remains, cross-ledger
  substitution and identity replay reject, and failed release returns the lease
  intact. Installation can now join one canonical prebinding to the exact root
  and lease. The resulting non-clonable occurrence borrows the root/code and
  owns the lifecycle hold. Joining revalidates that the leased era is still the
  exact current open era; root, slot, owner, code, artifact, admission, provider,
  requirement, artifact occurrence, ledger, and epoch substitution reject
  transactionally. One full prebinding-plus-ledger-plus-era key joins at most
  once, successful retirement never re-enables it, and a later epoch is a
  deliberately fresh occurrence.

  The installation prerequisite now has one canonical root registry per exact
  installed-code occurrence. `InstalledCode` burns a non-clonable registry
  authority once; the resulting ledger retains the complete installed-code
  evidence and installation scope, rejects replay and cross-installation use,
  and shares target `ProgramEntry` slot/owner identity derivation with compiler
  selection. An opaque target-derived required-slot closure also rejects an
  omitted, duplicate, extra, or cross-profile selection. This closure is
  descriptive and cloneable, not authority. The installed root ledger now
  replays and retains it against the exact code, artifact, scope, owner, entry,
  requirement, and installed-root evidence; required members remain frozen
  while unrelated runtime-open roots may still retire.

  That exact installation burns issuance of one non-clonable program-local
  cohort verifier. There is no public fresh-ledger constructor. Prebinding is
  restricted to the retained required roots, and one atomic epoch seal accepts
  exactly every eligible prebinding with leases from one current lifecycle
  ledger and epoch. Omitted, duplicate, extra, substituted, stale, replayed,
  and later same-epoch members reject transactionally with every lease returned.
  The resulting non-clonable cohort retains the exact closure and occurrences
  and derives ordered aggregate schemas plus finite cardinality from the closed
  set. Per-occurrence expressions remain separate rather than being blindly
  multiplied across subject-dependent or interval content. It still mints no
  lineage.

  The sealed cohort now consumes into one non-clonable epoch runtime rather
  than releasing loose occurrence grants. A generated installed-entry subject
  binds the exact installed root, ABI and semantic parameter positions,
  qualification, carrier, invocation, and runtime place. Establishment matches
  one still-dormant occurrence, requires the exact verified scalar-observation
  set, evaluates `SubjectField`/runtime-embedding/natural arithmetic into a
  canonical interval set or counted quantity, and only then commits one exact
  lineage account carrying the lifecycle lease. A finite batch validates every
  subject, lease, scalar roster, evaluated capacity, and algebra before it
  commits any member, so duplicate or malformed establishment returns every
  subject in source order and leaves the whole cohort pending. Missing or extra scalars,
  root/position/type substitution, stale lifecycle, ambiguous membership, and
  replay return the subject and leave the occurrence pending. Exact natural
  subtraction rejects underflow rather than silently using monus. The account
  retains the full occurrence and evaluated capacity; its copyable lineage ID
  is report-only, and failed retirement reconstructs the complete account.

  This closes the generic runtime subject/capacity establishment gate. Closed
  indexed domain applications retain their exact semantic identity from the
  qualified subject through owner-unique content projection, checked Unit
  planning, Terminal structural-domain/schema identity, and representation
  verification; one family application cannot substitute another. Contract-
  only indexed membership fails closed until proof facts retain that same exact
  identity. The legacy `ExtentCompilerProvisioning`/`sealed_declaration`
  carrier has now been removed. `psi-extents` retains only a passive exact
  program-local origin tuple (installed code/root/slot/schema, lifecycle
  ledger/epoch, entry invocation, and runtime subject place), while
  `omega-external-roots` owns the non-copyable account registry that retains
  the installed occurrence and lifecycle lease. Materialization accepts only
  one exact nonempty `u64` interval whose geometry, carrier, qualification, and
  algebra match the established account; counted and separated capacities
  reject transactionally. Split, loan, mapping, and merge retain the passive
  origin, and retirement releases the account only for the exact recombined
  root. Provider existing-content issuance remains unavailable to local roots.
  Artifact audit rows now report exact program-local occurrence fields instead
  of a fictitious provision declaration.

  The existing Terminal unit-call conservation check already rejects an ordinary
  call whose callee would acquire a content claim absent from the caller; the
  routed source canary likewise emits producer schema only on the exact
  boundary requirement. A checked-source `ProgramStorageEntry` satisfier
  canary now also requires an ordinary direct caller to supply both existing
  `Granted` inputs; the call cannot reinterpret itself as the installed entry
  event. An explicit verifier regression rejects a claimed result lineage with
  no bound entry parent. The generated program-entry handoff
  now preflights both exact positions before cohort establishment and owns the
  resulting account registry beside the installed image/storage Extents. Its
  failure carriers retain the exact subjects, established accounts, or installed
  roots plus registry appropriate to the failed phase, and record retry cannot
  release raw roots without their account owner. The binding now retains the
  bare `Extent` carrier independently from the qualified interface type, so a
  verified producer schema is never compared with `Extent in Granted` as if
  those were the same identity. A real installed-entry canary seals one UEFI
  root with the two exact producer positions and covers atomic subject
  preflight, returned-account materialization retry, record retry, the
  two-account installation aggregate, and exact audit origins. The
  canary now obtains those positions from imported Omega source lowered to
  Terminal Psi and an opaque verifier-produced catalog rather than a
  hand-authored Terminal module. Routed provider claims retain the canonical
  carrier independently from their complete qualified parameter type, so the
  compiler entry binding joins `named(name(Extent))` exactly instead of
  substituting the display spelling `Extent`. The
  transitional passive-grant installer and its synthetic audit helper are now
  removed. One sealed program-local custody carrier keeps the non-copyable
  registry beside the recorded installation through receiver rejection/retry,
  exact activation and finish, receiver-free ABI binding and recovery, emitted
  wrapper binding, logical values, operand images, caller-frame planning, and
  outgoing-frame reservation. Provider-issued stages remain separate. The
  target vocabulary now exposes a closed, ordered, enumerable required-root
  catalog. Build selection validates every named row against its owning target,
  requires the selected profile's complete catalog, and fails closed before
  ProgramEntry lowering on an unsupported future schema. Root authority and
  installation closure derive their expected set from the same catalog and
  reject missing, duplicate, extra, cross-profile, and compact-ID-colliding
  members. `ProgramEntry` remains the sole authentic member; a finite
  multi-member-slot canary waits for a real target-owned second slot rather
  than synthesizing test-only authority. The sealed cohort and runtime now
  expose one private-construction, cloneable aggregate snapshot retaining the
  exact required-slot closure, lifecycle-qualified cohort identity, and every
  unreduced schema row. Live-era composition takes the authoritative
  component-era roster and requires exactly one snapshot for every live epoch;
  stale, missing, duplicate, cross-ledger, cross-artifact, and cross-cohort
  rows reject. The report preserves each algebra, symbolic per-occurrence
  expression, occurrence roster, and cardinality separately rather than
  fabricating one scalar total. A two-era external-root regression and the
  real source-to-Terminal-to-installation storage handoff cover this
  coexistence-reporting seam. The source-derived two-root handoff also covers
  exact lifecycle-ledger substitution and complete batch retry custody, then
  composes two live eras into the four exact ordered epoch/schema rows.
  The same real source-derived handoff now covers stale installation-epoch
  replay end to end. It acquires the exact epoch-10 cohort leases, publishes
  epoch 20, proves the stale seal rejects atomically with every prebinding,
  root borrow, and lease returned, releases those old holds, and retries with
  fresh epoch-20 leases. The successful installation retains exactly the two
  original producer schemas, mints only its two expected lineages, and reports
  epoch 20 on both artifact audit origins. A distinct one-root source canary now
  carries exactly one verified producer schema through prebinding, lifecycle
  cohort, aggregate/snapshot/coexistence reporting, runtime establishment,
  materialization, and audit origin. Terminal-artifact, lifecycle, and
  materialization-plan substitutions reject transactionally and exact retry
  mints only one lineage. That same installed source root now rejects
  recomposition with a provider-issued extent even when numeric lineage,
  geometry, rights, provenance, mapping era, and address space coincide; both
  origins and their custody return unchanged. A finite multi-instance canary
  now lowers one real source-owned producer schema into one verified Terminal
  catalog, installs the same artifact as two distinct `InstalledCode`
  occurrences, and composes their exact installation-derived snapshots across
  two live eras. Both rows retain the common artifact, Terminal schema,
  symbolic per-occurrence capacity, and cardinality one while preserving their
  distinct installed-code and lifecycle identities; omission and same-instance
  substitution reject. This covers source, Terminal, artifact, and
  installation identity without fabricating a second target slot or reducing
  the two rows to an authored total. The former
  `unbounded installation shape` request does not yet name a representable
  threat: every current source projection, Terminal catalog, target slot
  roster, cohort, and lifecycle ceiling is explicitly finite. Specify whether
  the intended rejection frontier is an unresolved source family, a dynamic
  target-slot roster, or an artifact-authored occurrence count before adding a
  canary; do not invent an `Unbounded` production variant solely to reject it.
  Source, canonical-codec, and verifier canaries now close
  coherent understatement of a producer schema against its independent owner
  projection. Downstream artifact prebinding now also cross-pairs two
  independently valid but semantically distinct Terminal artifacts and proves
  that neither producer catalog can describe the other's artifact identity;
  rejection commits no prebinding and exact retry succeeds. The real
  source-to-installation handoff likewise rejects a materialization plan that
  understates the owner-derived interval capacity, returns the exact account,
  and then accepts the full verified geometry. Retain those invariants through
  the remaining installation work rather than adding a second authored total.
  A shared cap is one aggregate parent root divided among children; another
  child without supply rejects. Cross-epoch limits require persistent authority.
- **BOUNDARY-ISSUANCE** (after conservation): derive invocation geometry from
  parameters, entry places, and results. Keep ownership, issuance, custody,
  aliasing, and partition succession distinct. Providers may attest custody,
  never computable interval arithmetic.
- Under **TR3-TR8**, finish routed task claims, stack authority, cancellation,
  and transactional custody. Deferred acknowledgements lease the interrupt root
  and controller configuration; reconfiguration drains them.

Acceptance: reconstructed carriers mint no authority; every introduced content
claim traces to a verifier-reconstructed installed program-local occurrence or
admitted provider issuance; artifact-instance aggregates are derived for an
exact epoch and Cathedral can compose coexistence peaks; external effects have
an exact root-to-provider backing chain;
partition and residual arithmetic are compiler-derived; overlapping children,
gaps without a custody exit, algebra drift, receipt replay, and cross-root
recomposition reject.

### P2 — Source-visible materialization and placed access

Owners:

- `wiki/design_briefs/programmable_layouts.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_20_memory_layout_abi.md`

#### L4/L5 — plan-laid views

- Finish source-visible materialization over owned storage, including
  non-scalar tiling and mutable views beyond current record/array/slice checks.
  Raw bytes establish no typed fact without a selected validated plan and exact
  field identities. The accepted fixed subset reflects primitive arrays and
  recursively fixed arrays/records as whole `Repeated` or `Nested` fields, and
  one plan may place multiple independently keyed aggregate fields. View paths
  retain one whole `At` extent; owned materialization also admits an outer
  fixed array tiled by exactly one compiler-sized element `At` at one validated
  constant destination stride. Compiler-derived strides and offsets drive the
  interpreter and all three native target paths. Mutable fact-free byte views
  write and reread through those same extents, including two runtime indices
  through a gapped outer fixed array of recursively fixed arrays while
  retaining the plan-derived outer stride and compiler-derived inner stride,
  and through a gapped outer fixed array of fixed records whose interior fixed
  array retains the compiler-derived member offset between those indices.
  Typed owned materialization
  derives complete bytes from the exact schema (or a checked zero-argument Psi
  evaluator) while Omega supplies byte order, zeroes padding, and validates
  completely before mutation. Fully specialized generic records now participate
  through their synthesized concrete `CheckedShape` symbol and substituted
  member types, including specializations nested under fixed-array wrappers;
  spelling is never layout authority, and distinct specializations retain
  distinct symbols and widths. Retained layouts for numbered aggregate fields
  now rejoin the current typed schema by stable member identity rather than
  presentation spelling; a field rename preserves materialization, while
  missing or drifted identities reject before destination mutation. Numbered
  ordinary scalar materialization and decoding now use the same identity join
  across whole, stored-integer, and fragmented entries; decoded values retain
  the current schema spelling, while identity drift or collision rejects
  transactionally. Every fixed materialization and scalar-decoding entry point
  now preflights the retained layout identity set: one stable identity under
  multiple names, or one name under multiple identities, rejects before
  destination mutation, value exposure, or symbolic resolution. Retained access
  plans now retain complete source layout geometry and authorize replay only by
  a hash-free exact structural relation over schema identity, placements, size,
  alignment, offsets, and canonical identity sets. Authored entry order and
  numbered-member presentation renames are nonsemantic; compact fingerprints
  remain report/cache identity and cannot hide geometry drift.
  The layout-plan foundation's 1,965-line unit corpus now lives in a private
  test child rather than sharing its production coordinator. Source-owned fixed
  materialization now delegates structured-field inventory and recursive
  primitive/fixed-array/checked-record encoding to a focused 264-line private
  owner. Byte order, carrier/range checks, erased fields, alignment/padding,
  recursion rejection, error order, transactional mutation, and public APIs
  remain unchanged. Closed build-time Schema ABI construction now also lives in
  a focused 228-line private owner, retaining stable nonzero record/case/payload
  keys, Optional identities, fixed-capacity padding, tombstones, payload
  reflection, and exact capacity diagnostics. Evaluated Plan decoding,
  structural validation, and canonical report normalization now live in a
  focused 525-line owner, preserving u64 carriers, placement-family legality,
  repeated/fragmented tiling, alignment/overflow, overlap/bounds, stored-
  integer total decode, and diagnostic order. The 107-function production
  inventory and all 33 layout tests remain intact. Exact typed-schema identity
  and fixed runtime-geometry reflection now live in a focused 411-line owner,
  including quotient/unsupported-shape rejection, primitive/range geometry,
  fixed-array/nested-record recursion, erased-runtime handling, alignment,
  capacity, and key-collision checks. Plan-laid pre-resolution elaboration now
  lives in a focused 213-line private owner, including policy/schema indexing,
  application validation, synthetic checked-record construction, and exact
  type-reference rewriting. Public APIs, diagnostic order, synthesized
  identities, and the 107-function production inventory remain unchanged.
  Post-typing plan-laid
  layout installation now lives in a focused 376-line private owner, including
  policy evaluation, exact producer/schema/data identity capture, host-sized
  geometry projection, stored-integer total-write derivation, and transactional
  typed-layout publication. Public APIs, diagnostic order, identity custody,
  and the 107-function production inventory remain unchanged; the 45-line root
  is now the natural public two-phase facade. The 316-line layout-plan
  root has reached its natural
  orchestration/public-materialization-entry boundary.
  Type-reference indexed-domain admission and open-index normalization now live
  in a focused 550-line private owner. Exact const-binder/range diagnostics,
  recursive expression validation order, selected public operator/provider/
  algebra identities, normalizer bytes, crate APIs, and the exact 33-function
  inventory remain unchanged. Type-reference constraint-chain admission now
  lives in a focused 522-line private owner. Declared-domain matching, indexed-
  argument handoff, semantic-role conflict ordering, carry/value/arithmetic
  diagnostics, OmegaLayout carrier checks, dependent-range admission, crate
  APIs, and that same inventory remain unchanged. Generic type-reference
  argument admission now lives in a focused 401-line private owner. Exact
  static-machine contract selection, symbolic array-length checks, canonical
  structural const-index eligibility, forwarded const identity, integer-range
  diagnostics, property-bound ordering, crate APIs, and the 33-function
  inventory remain unchanged; the 662-line parent retains recursive type-shape
  dispatch and owner/scope orchestration.
  Erased terms remain semantically mandatory but add no bytes, including nested
  records and fixed arrays whose entire runtime shape is erased. Scalar
  placement/access semantics remain fenced for aggregates. Continue beyond
  this fixed subset. Sum materialization is design-blocked on the unsettled
  tagged-case placement vocabulary.

#### L6b — `AccessPlan` and `Placed<P, T>`

- The first source-vocabulary milestone is live. Core now publishes opaque
  `Placed<P, T>`; `Extent::Vacant`; invariant indexed
  `Extent::Resident<P, T>`; canonical `PlacementOutcome` and
  `PlacementReturn` carriers; and the empty ordinary
  `PlacementCustody<P, T>` trait. Structural and malformed-use canaries pin
  exact arity, declaration order, opacity, fixed-carrier type-index identity,
  generic substitution, and mismatch rejection. This adds no placement
  operation, authority route, admission intermediate, occurrence identity, or
  runtime domain carrier. Operations and broader custody agreement remain open
  below.

  The first compiler-checked custody-agreement rung is now live when one
  concrete named `PlacementCustody<P, T>` conformance names the exact concrete
  policy/schema pair of an already retained source-derived `Placed<P, T>`
  plan. Every direct erased record field omitted from that normalized physical
  plan must occur once in the custody record under the same canonical field
  path, exact normalized type, and multiplicity; represented fields must be
  absent. The next bounded recursion rung also accepts one acyclic,
  non-generic, case-free checked-record field that is itself represented while
  direct erased leaves below it travel through an authored projection-record
  spine. Those leaves retain their complete root-to-leaf canonical paths, and
  represented siblings remain forbidden with the enclosing plan entry cited.
  The following bounded rung also accepts exactly one further represented
  acyclic, non-generic, case-free record on that spine when its canonical fixed
  representation is nonzero. The authored custody projection preserves both
  enclosing field identities before the same direct erased leaves; missing,
  cross-sibling, represented-leaf, type, and multiplicity drift reject under
  the original root plan decision.
  Third, fourth, and fifth bounded represented-record levels are now live under
  the same nonzero, acyclic, non-generic, case-free rules. Their custody
  projections preserve every enclosing field identity and must completely
  cover every erased descendant admitted by the bounded classifier; a direct
  erased leaf cannot conceal an unsupported deeper descendant.
  Revalidation cites the exact `Policy::plan` machine and its retained
  offset/width decision, and only the toolchain `core/layout.omg` trait receives
  this meaning. The conformance remains ordinary evidence and grants no
  storage, content, domain, provider, or establishment authority. A sixth
  represented record level and broader
  recursion, structurally zero-layout wrappers, arrays, generic or case-
  dependent custody, planless agreement checking, generic placement operation
  selection, and outcome dispositions remain open.

- Implement the settled borrowed/owned `Placed<P, T>` establishment and
  retirement model from `Extent in Granted`, using ordinary subrange borrows
  and no source-visible admission intermediate. Declare opaque core
  `Placed<P, T>`, `Vacant`, and invariant `Resident<P, T>` identities; ordinary
  generic `PlacementOutcome<View, Returned, Reason>` and
  `PlacementReturn<Source, Custody>` data; operation-specific rejection sums;
  and distinct `view_borrowed`/`view_owned`,
  `initialize_borrowed`/`initialize_owned`, and
  `validate_borrowed`/`validate_owned` operations. Provider-specific adopt/open
  wrappers establish their external domains before the appropriate view.

  Custody identity is authored, not synthesized. Add the compiler-checked
  `PlacementCustody<P, T>` relationship: a named conformance proves that one
  ordinary custody data declaration agrees exactly, by canonical field path,
  type, and multiplicity, with the unconditional non-runtime Type fields
  selected by the evaluated placement plan. Generic operations carry both the
  custody type and exact selected conformance; concrete calls explicitly name
  that evidence, including all of its owned arguments. Retain the ordinary
  conformance through closure, package review, canonical encoding, and replay;
  add no placement-specific evidence category.

  Borrowed rejection releases the source loan and returns the authored custody
  value. Owned rejection returns `PlacementReturn<Extent in Granted, C>` so the
  source extent and custody cannot disappear. Proof inputs and outputs remain
  in the `;` lanes. Every moved Type input has an explicit disposition on every
  outcome: embedded in the successful view, returned in the authored carrier,
  or consumed by one exact named operation. Embedded inputs become retirement
  debt; absence never proves consumption. Finish owned destruction/move-out
  evidence before returning `Granted & Vacant`. Validator-specific error sums
  remain ordinary generic parameters rather than erased codes.

  Diagnostics for a failed custody conformance must cite the exact evaluated
  `Placement::plan` identity and normalized field decision—for example, that a
  declared custody field is represented at one offset and width—not merely
  report a field-set difference. Pin acceptance, missing/extra/wrong-type and
  multiplicity failures, explicit conformance selection, policy-change drift,
  borrowed rejection, and owned rejection returning both extent and custody.
  Existing Rust admission and occurrence identifiers remain implementation
  scaffolding and must not be promoted into a source ABI.
  A source-to-foundation evidence canary now takes the exact Stable placement
  plan retained by checked `Placed<P, T>` derivation through owned Extent
  admission and provider-content adoption. Exact interpretation mismatch
  returns the complete Extent and content grant, and withdrawal/re-admission
  retries successfully under the correct retained plan without minting a
  placed occurrence or choosing the open source result schema.
  A second source-to-foundation canary takes an exact source-retained External
  plan through provider schema/device correspondence. Wrong-plan and wrong-
  profile receipt joins return the exact loan and correspondence for retry;
  the exact join then withdraws both unchanged. It likewise stops before a
  placed view or occurrence identity and therefore does not choose that ABI.
  Source-retained provider-profile admission also covers two exact operation
  families transactionally: Atomic `load + fetch_add` rejects load-only supply,
  and External destructive `Take` rejects Repeatable supply. Both failures
  return the exact loan for corrected-profile retry and unchanged withdrawal;
  neither path creates placement occurrence or installation authority.
- Derive readable, destructive-read, writable, and atomic field accessors while
  keeping logical extents distinct from whole-transfer footprints. Enforce
  total decode/encode, exact provider width/alignment, and operation-specific
  atomic laws. Continue rejecting External initialization, multi-transfer
  reads, and synthesized RMW. Retained numbered layouts now rejoin the current
  reflected schema and source-authored access decisions by stable member
  identity rather than presentation spelling; positional renames, identity
  drift or collision, and derived-offset drift reject before an access plan is
  sealed.
  Compiler-derived source `Placed<P, T>` plans now retain exact synthesized-
  view, source-schema, and source-field symbols together with every field's
  stable member identity. Typed lookup uses the exact synthesized symbol, while
  validation independently reconstructs the schema/member binding, canonical
  layout slot, admitted access decision, and synthesized accessor. Numbered
  presentation renames remain nonsemantic; member, schema, access, or accessor
  substitution rejects fail closed.
  Compiler-derived non-atomic placed accessors now retain the exact generated
  machine and callable-state symbols for every admitted `read`, `take`, and
  `write` operation. Independent validation reconstructs the operation set,
  attachment, machine identity, and unique callable state, while statement-call
  authorization joins directly through the retained state symbol rather than
  presentation names. Operation or target substitution rejects fail closed;
  Atomic access remains on its distinct typed carrier.
  Compiler-derived placed field plans now retain the exact generated accessor
  data symbol independently from its diagnostic name. Build-time binding and
  validation replay that symbol against the exact synthesized field type, and
  Omega layout recognizes opaque accessor carriers by symbol rather than
  presentation spelling. Accessor-data substitution rejects fail closed;
  Atomic specialized carriers remain fenced behind their distinct typed
  operation law.
  Compiler-derived placed field plans also retain the exact synthesized
  accessor type reference independently from diagnostic spelling. Shell-aware
  typed lookup rejoins through that handle, while validation replays it against
  the exact synthesized view field and retained accessor-data symbol.
  Presentation-name substitution cannot redirect lookup; accessor-type
  substitution rejects fail closed. Atomic specialized carriers retain their
  separate typed operation fence.
  Compiler-derived placed-view plans now also retain the exact build-time
  `Policy::plan` machine symbol beside the nominal policy symbol and normalized
  placement. Independent validation rejoins both identities to the current
  policy data/machine attachment before field access. Policy-data or plan-
  machine substitution rejects fail closed; this binds existing evaluated
  evidence without re-evaluating policy code or establishing source-visible
  placement authority.
  Independent placed-view validation now also replays the complete accessible
  field inventory before accepting individual accessors: normalized non-
  inaccessible access rows, retained field plans, and synthesized record
  members must have identical cardinality, and the synthesized view must remain
  a record. Missing retained rows, extra synthesized members, or case-bearing
  drift reject fail closed before field lookup; no new placement or access
  authority is established. Post-typing nominal replay now lives in a focused
  305-line validation owner. Exact view/schema/policy-plan identities,
  accessible-field cardinality, stable member/layout/access correspondence,
  accessor type/data symbols, and unique operation targets retain fail-closed
  diagnostic order; the statement-use coordinator is now 378 lines with the
  exact 17-function inventory unchanged.
  Declared member-path resolution now lives in a focused 323-line owner. Local,
  parameter, and attached-data roots, nested record and sum-payload traversal,
  missing-field diagnosis, receiver-type recovery, and exact data-definition
  lookup retain their original resolution and diagnostic order; the place
  coordinator is now 374 lines with the exact 21-function inventory unchanged.
  Post-typing placed-view plan replay and installation now live in a focused
  208-line build-time owner. Policy data/machine, schema/view data, complete
  admitted field inventory, stable member identity, accessor type/data identity,
  and unique operation machine/state targets rejoin in the same order before
  the sealed typed plan is installed; the 107-function package inventory and
  public behavior remain unchanged. Probe/exact record synthesis now also lives
  in a focused private owner, including accessor naming, template cloning/
  retirement, operation selection, record construction, and exact type-
  reference rewriting. The cohesive discovery/two-pass orchestration root is
  now 298 lines.
  Exact reification of validated layout reports into source `Plan` values now
  lives in a focused 126-line private owner. Stable member lookup,
  `At`/`IntegerAt`/`Bits` encoding, capacity padding, dynamic-size
  representation, evaluation order, diagnostics, and public APIs remain
  unchanged. Evaluated `AccessPlan` and `PlacementPlan` normalization now lives
  in a focused 426-line private owner, including exact field-decision parsing,
  scalar transfer-width derivation, atomic operation permissions, exposure,
  boundary reach, and final sealed placement construction. The 107-function
  package inventory, public APIs, and diagnostic order remain unchanged; the
  remaining 88-line root is the natural policy-evaluation orchestration
  boundary.
  Plan-laid value layouts now retain the exact synthesized data symbol and
  ordered runtime field-symbol inventory independently from their diagnostic
  name. Interpreter record views, recast/relevance validation, boundary ABI
  shaping, and native layout selection rejoin through that identity and replay
  the field inventory before applying geometry. Presentation-name drift cannot
  redirect a plan, while same-cardinality data-symbol substitution rejects fail
  closed; no typed-content or placement authority is minted.
  Plan-laid layouts now also retain the exact source schema and ordered schema-
  field identities plus the exact nominal policy and build-time `Policy::plan`
  machine that produced their geometry. A dedicated validation pass replays
  those producer bindings together with synthesized data/field custody before
  any layout consumer runs. Presentation-name drift remains nonsemantic;
  schema, policy, plan-machine, or synthesized-data substitution rejects fail
  closed without re-evaluating policy code or minting materialization authority.
  Plan-laid validation now also replays the exact position-by-position
  correspondence between retained source-schema fields and synthesized runtime
  fields, including stable member identity (or positional identity where
  unnumbered) and normalized constrained type identity. Coordinated schema-
  symbol and field-inventory substitution rejects fail closed; cloned arena
  type-reference handles remain non-authoritative implementation coordinates.
  Plan-laid layout custody now also retains the exact validated target-neutral
  `LayoutPlanReport` beside its host-sized consumer projections. Independent
  validation reconstructs and compares size, alignment, field offsets, stored-
  integer geometry, repeated destination strides, and bit fragments before
  interpreter or backend consumption. Flattened-geometry drift rejects fail
  closed without re-evaluating policy code or treating compact identity as
  authority; stored-integer total-write capability remains a separate exact
  semantic type fact.
  Plan-laid validation now independently reconstructs that stored-integer
  total-write capability from the exact current schema field type/range and
  retained width/interpretation. Capability invention or removal rejects fail
  closed before layout consumers; the Boolean remains semantic type evidence
  separate from geometry and grants no new placement or mutation authority.
  Plan-laid validation now independently reconstructs the retained target-
  neutral report identity from the exact current typed schema, derives its
  `offsets` convenience projection from complete retained entries, and requires
  the current field inventory to account for every entry. Schema-fingerprint,
  derived-offset, or unmatched-entry drift rejects before host geometry or
  layout consumers; compact identities remain non-authoritative and no policy
  code is re-evaluated.
  This plan-laid semantic-custody strand is now at its implementation-only
  boundary: every retained producer, schema/data field identity, target-neutral
  report identity and complete entry inventory, host geometry projection, and
  stored-integer type capability is independently rejoined before consumption.
  Further coordinated provenance needs a settled sealed build-time evaluation
  receipt or policy re-evaluation; compact fingerprints and duplicate mutable
  reports cannot serve as authority. Sum placement remains blocked on tagged-
  case vocabulary, while placed lifecycle work remains a separate L6b strand.
  Terminal placed-access propagation currently stops on its first authority
  producer: the source core has no live `Placed<P,T>` establishment/retirement
  operation, checked Psi has no occurrence/resident/loan fact, and installation
  has no qualified placed-root binding. The Rust access-plan foundation's
  manually supplied occurrence IDs cannot become artifact authority. Land one
  settled source or installed-root establishment carrier before adding Terminal
  access events; never derive occurrence authority from plan identity, accessor
  identity, parameter ordinal, names, or offsets.
- Keep alias-exclusion admission separate from access rights; `&mut` does not
  claim exclusivity against a device. Sealed primitive events now specialize
  linearly into Stable read/take/write/swap, External read/take/write, or one
  exact Atomic operation and ordering while preserving the original authority
  on pre-event rejection. Carry the settled address-free placed-occurrence,
  resident-claim, loan, mapping/revision, exact footprint, and boundary-reach
  identities through Terminal Psi, installation, the interpreter, and both
  native backends without replaying source layout. Emit claim-local
  introduction, forwarding, transformation, exit, and loan rows.
  Every Stable primitive/compound, External primitive, and Atomic
  specialization independently replays the exact admitted effective-supply
  row—field key/name, width, authority-relative offset/address, and
  alignment—and returns the unchanged sealed request on drift. Sealed primitive
  requests also retain their validated field descriptor; every Stable,
  External, and Atomic specialization independently replays copied logical
  extent, concrete footprint, observation, and operation/borrow authorization
  before lowering, with unchanged authority-bearing custody returned on drift.
  Primitive specialization retains the complete sealed placement witness and
  independently replays its plan, profile receipt, admission, boundary reach,
  exact resource row/descriptor, source-loan polarity, resident claim, and
  placed occurrence; drift returns the unchanged authority-bearing request.
  Primitive requests also retain the original sealed field authorization and
  independently replay descriptor, current/source borrow polarities, and
  operation before specialization; coordinated privilege rewrites reject while
  returning the unchanged authority-bearing request.
  Stable read/write primitive specialization now exposes a borrowed outward-
  lowering preflight that independently replays the exact retained placement,
  admitted profile/resource row, descriptor geometry, resident-content
  custody, borrow polarity, authorization, and operation specialization.
  Rejection consumes nothing, so copied-evidence drift can be corrected and
  the same sealed request retried; no memory event or target lowering is
  established. Stable bounded compound-mutation specialization exposes the
  same borrowed preflight over its exact placement/profile/resource,
  descriptor and footprint, resident custody, exclusive current/source loans,
  authorization, and `CompoundMutation` identity. Rejection likewise consumes
  nothing and establishes no read-patch-write event or target lowering.
  External Read/Take/Write specialization now replays exact placement,
  profile/resources, descriptor/footprint, authorization, admitted External-
  or-conservative-Stable supply, and retained operation before outward
  lowering. Rejection performs no storage observation and consumes no custody,
  so repair and retry use the same sealed request; no external transfer or
  target lowering is established.
  Atomic primitive specialization now exposes borrowed outward-lowering replay
  of the exact placement/profile/resource authority, descriptor and footprint,
  resident custody, admitted Atomic supply, operation family, ordering law,
  and retained specialization. Rejection performs no atomic attempt and
  consumes no custody, so corrected retry uses the same sealed request; no
  target lowering or synthesized retry loop is established.
  Placement admission now retains the complete admitted resource profile
  through borrowed, owned, and borrowed-resident access; primitive
  specialization independently replays the exact profile/loan/plan join and
  rejects profile-root or compatibility drift while returning the unchanged
  request. Placed field projection now independently replays the retained
  placement plan, admitted profile/receipt, exact resource compatibility,
  admission, base, and source-loan polarity before field lookup or address
  derivation; rejection borrows and therefore preserves the complete placed
  authority for repair and retry. Placed field authorization independently
  replays the retained placement plan, admitted profile/resources, exact field
  descriptor and supply row, admission/reach, loan/resident identities, and
  derived primitive address before issuing an authorized access; rejection
  only borrows the projection, preserving its complete authority for repair
  and retry.
  Stable content adoption independently replays that retained profile against
  the exact owned extent and placement before establishing resident custody;
  rejection returns both the unchanged owned admission and provider content
  for corrected retry. Borrowed placed-view establishment now independently
  replays the retained placement, admitted profile/receipt, exact loan, and
  resource compatibility before creating a `PlacedView`; rejection returns the
  complete loan-bearing admission for corrected retry or withdrawal. Owned
  resident-view establishment now independently replays retained owned
  placement/profile/resource authority before activating a requested
  occurrence; rejection returns the exact dormant resident and occurrence for
  corrected retry, without claiming global occurrence freshness. Resident-
  preserving retirement now independently replays the active carrier's
  retained owned placement/profile/resource authority before returning dormant
  custody; rejection returns the exact active occurrence, resident claim,
  receipts, and Extent authority for corrected retry. Shared and exclusive
  borrowed-resident view establishment likewise replays the lender's retained
  owned placement/profile/resource authority before creating a whole-range
  loan; rejection consumes nothing, leaving the exact dormant resident
  authority available for repair and retry. Borrowed-resident retirement now
  independently replays the retained admitted profile/receipt, exact
  whole-range loan, placement plan, and resource compatibility before ending
  the placed occurrence; rejection returns the complete active borrowed
  carrier, preserving its loan, occurrence, resident claim, and provider
  receipts for corrected retry without reminting lender custody. Stable
  resident custody now retains the complete non-Clone provider existing-content
  grant rather than reducing it to copied receipt identities. Owned view,
  resident-preserving retirement, and shared/exclusive borrowed-resident
  establishment independently replay that grant's exact interpretation,
  origin, lineage, geometry, address space, provenance, era, resident claim,
  and provider receipts against the retained placement; drift returns the
  complete dormant or active carrier for corrected retry without reconstructing
  custody. Shared and exclusive borrowed-resident carriers now retain a
  lifetime-bound reference to that exact grant rather than copying claim and
  receipt identities. Borrowed retirement replays its interpretation, origin,
  lineage, geometry, address space, provenance, and era against the whole-range
  loan and placement before release; rejection returns the complete carrier for
  retry and neither clones nor remints lender custody. Placed projection, field
  authorization, and Stable/External/Atomic primitive specialization now also
  replay any retained resident grant against the exact owned extent or borrowed
  whole-range loan. Coordinated copied claim/occurrence rewrites cannot
  substitute unrelated custody; rejection borrows the carrier or returns the
  unchanged sealed request for corrected retry.
  The access-plan foundation's 5,318-line unit corpus now lives in a private
  test child rather than sharing its production root. Its four Stable,
  Stable-compound, External, and Atomic primitive specialization contracts and
  independent replay validators now live in a focused 578-line child, leaving
  a 3,992-line coordinator. Placement-resource compatibility and effective-
  supply derivation now live in a focused 226-line child behind the unchanged
  crate-root API. It preserves exact Stable/External/Atomic supply selection,
  conservative Stable substitution, full-region reach, transfer alignment, and
  base-congruence validation. The complete normalized access-plan validation
  judgment now lives in a focused 430-line child: retained-layout/cardinality
  replay, policy/transfer widths, exact whole/fragmented geometry, External and
  destructive whole-container rules, and Atomic overlap exclusion preserve
  their ordering. Provider resource-profile normalization now lives in a
  focused 159-line child, retaining canonical region sort/merge, bounds and
  overlap rejection, External/Atomic transfer-rule normalization, and exact
  empty/duplicate/invalid capability diagnostic order. Versioned normalized
  identities for access plans, placement plans, and resource profiles now live
  in a focused 198-line private owner; exact prefixes, tags, byte order, reach,
  transfer rows, and reserved-zero remapping cross only three typed-ID sibling
  contracts, and identity remains evidence rather than authority. Provider
  resource-profile grant/admission custody now lives in a focused 264-line
  owner behind unchanged re-exports: the non-Clone grant, retry-complete
  rejection, admitted profile, reach restriction, and exact range/address-
  space/provenance/era/origin/lineage/rights replay remain sealed. Borrowed and
  owned placement admission plus borrowed placed-view establishment now live in
  a focused 119-line owner, preserving exact profile restriction, resource
  compatibility, runtime base-congruence discharge, and retry-complete loan/
  extent/admission rejection order. Stable owned resident-content adoption and
  retained authority replay now live in a focused 157-line custody owner. It
  rebinds the non-Clone provider grant to exact interpretation, origin,
  lineage, geometry, address space, provenance, era, admitted resources, and
  Stable-only observation before each consuming lifecycle step, preserving both
  inputs on rejection. Access authorization and alias-exclusion judgments now
  live in a focused 112-line owner, retaining exact descriptor permissions,
  current/source borrow polarity, Stable compound exclusivity, Atomic family/
  order legality, and whole-transfer footprint conflict classification.
  Ordinary borrowed placed-view retirement, authority replay, and shared/
  exclusive projection now live in a focused 108-line lifecycle owner,
  preserving exact loan polarity, profile/resource replay, correspondence
  retirement composition, retry-complete recovery, and private carrier fields.
  The complete owned placement lifecycle now lives in a focused 250-line owner:
  permission-only extent withdrawal/rejection, dormant resident activation,
  active Stable projection, resident-preserving retirement, and retry-carrier
  recovery remain exact inherent transitions while carrier declarations remain
  root-private. Sealed primitive-request inspection and replay now live in a
  focused 243-line owner while the carrier and replay fields remain root-
  private. Effective-supply, descriptor/footprint, retained placement/
  correspondence/resident, and authorization replay preserve their validation
  and diagnostic order before specialization. The four-form private placement-
  authority witness and its independent resource, correspondence, resident-
  content, loan, admission, claim, and occurrence replay now live in a focused
  177-line owner without a public re-export. Pure placed-field projection and
  its exact authority/resource/correspondence/resident replay plus named-
  operation authorization now live in a cohesive 349-line owner; public
  projection remains re-exported while field-access/request construction stays
  sealed. That owner now also retains the sealed `PlacedFieldAccess` carrier
  and its sole transition into `PrimitiveAccessRequest`, completing projection
  through authorization to request custody behind unchanged re-exports. The
  1,289-line root has reached its natural boundary of normalized public
  vocabulary/value APIs, root-owned carrier declarations, and top-level
  placement validation orchestration. All 84
  unit tests, the production inventory, diagnostics,
  custody, retry behavior, and the public surface remain unchanged.
- Implement `Extent::Resident<P, T>` as the owned exact-range dormant-content
  qualification, including invariant type indices, mutual exclusion with
  `Vacant`, split/merge rejection, borrow versus owned-view continuity,
  resident-preserving retirement, partial-view retirement fences, and explicit
  migration through `Vacant`. Carry non-runtime custody in the resident claim.
  The concrete foundation now seals a provider-issued nonzero
  `ResidentClaimId` into distinct dormant owned Stable and Atomic-only content
  carriers. Each explicit owned view consumes its carrier into a fresh nonzero
  `PlacedOccurrenceId`; field/access/lowering requests retain both identities,
  and resident-preserving retirement returns the same claim and receipts for a
  later fresh view. Adoption, view, projection, specialization, and retirement
  independently replay the exact observation route, placement/profile/resource
  authority, and complete provider content grant; rejection returns every
  non-Clone input or carrier unchanged. Ordinary borrowed views retain neither
  identity. Stable borrowed resident views retain the lender's exact claim and
  provider receipts, one fresh placed occurrence, and a whole-range shared or
  exclusive `ExtentLoan`; ending the view releases only that loan and remints
  nothing. Provider-backed Atomic resident content now supports the same
  shared and exclusive whole-range borrowed views while retaining the lender's
  exact full placement/profile/resource/admission and provider-content
  authority, unchanged claim and receipts, caller occurrence, and loan
  polarity. Exclusivity adds no Atomic permission; retirement replays the
  retained authority transactionally, returns the complete active carrier on
  drift, and releases only the loan. Source-visible domain establishment,
  `Vacant` transitions, partial moves, Terminal propagation, and installation
  remain.
- Complete the atomic 2x2 compare-exchange family: existing observing decisive
  and single-attempt forms require copyable residents; new non-observing
  decisive and single-attempt forms return the proposal on failure and may
  transfer affine or linear
  custody using one copyable comparison key and exact selected encoding law.
  Add the four settled flat core carriers together:
  `AtomicCompareExchangeOutcome<T>`,
  `AtomicCompareExchangeOnceOutcome<T>`,
  `AtomicTryExchangeOutcome<T>`, and
  `AtomicTryExchangeOnceOutcome<T>`. Their canonical cases are failure-first:
  `Mismatched` is tag zero, `Exchanged` is tag one, and `Uncommitted` is tag two
  only on the single-attempt carriers. Observing failure carries `observed: T`;
  non-observing failure carries `proposed: T`; non-observing success always
  carries `displaced: T`. A selected encoding law cannot erase Type-side
  custody, so ordinary multiplicity alone decides whether the displaced value
  may be discarded. `Key` remains a copyable comparison input and the selected
  law remains checked call evidence; neither parameterizes the runtime outcome.
  The atomic access-policy vocabulary now retains that 2x2 permission family as
  four distinct authored and admitted rows: observing decisive
  `compare_exchange`, observing single-attempt `compare_exchange_once`, non-
  observing decisive `try_exchange`, and non-observing single-attempt
  `try_exchange_once`. Build-time evaluation, permission containment,
  resource/access identity, and checked placed-field plans preserve those rows
  without cross-axis substitution. Checked placed fields with either observing
  permission now also retain a non-authorizing resident/result contract: exact
  field symbol, resident type, normalized unrestricted multiplicity, transfer
  width, decisive/single-attempt axes, and their distinct closed result shapes
  are independently replayed. Affine or linear observing residents reject;
  `try_exchange*`-only fields remain rowless and gain neither observing nor
  selected-encoding authority. A separate provider-backed Atomic-only owned
  lifecycle now retains one exact runtime resident claim, provider receipts,
  admission, and placed occurrence through Atomic projection and primitive
  specialization, then returns the unchanged claim on resident-preserving
  retirement. A non-authorizing checked/runtime carrier now independently
  replays the complete checked placed view, field, unrestricted resident/result
  contract, and the provider-backed request authority, then joins only the
  matching observing decisive or single-attempt operation. It compares the
  full retained placement structure rather than treating its compact plan ID
  as authority, requires the exact key, width, claim, occurrence, operation,
  and closed result shape, and returns the unchanged non-Clone request on every
  rejection. It performs no atomic attempt. The shared compiler ordering
  carrier now preserves observing decisive and single-attempt operations as
  distinct variants with their exact success/failure orderings, and checked
  placed-view validation authorizes only the matching permission row. The
  checked interpreter and legacy native state-graph boundary scan the complete
  checked expression arena and refuse single-attempt lowering: neither has the
  runtime result operation or a target operation identity. Only the existing
  observing-decisive source call is currently derivable. All four flat generic
  core outcome declarations are now live together. Their compiler-owned closed
  shape identities retain the decisive/single-attempt and
  observing/non-observing axes, canonical failure-first case order, and exact
  payload disposition; source canaries independently replay the public generic
  parameter and case/payload schemas. `Key` and the selected encoding law do
  not enter any runtime outcome identity. Operation desugaring and checked
  result custody remain prerequisites for `compare_exchange_once`; reusing the
  decisive prior-value carrier would erase `Uncommitted`. Retain the exact
  carrier through checked source, Terminal Psi, interpretation, lowering,
  package review, and replay before admitting that operation. Migrate the
  decisive scalar-return call with a targeted diagnostic rather than a scalar
  overload. The try operations remain fenced until their custody and
  encoding-law rows exist. Source calls for the other families,
  runtime-result custody, an atomic attempt or retry, the non-
  observing comparison key/selected-encoding law, Terminal rows, provider
  selection or installation, backend target operation identity, and executable
  lowering remain open.
- Close generic `ResidentContentTransfer<P, T>` applications at final
  composition from concrete and symbolic artifact demand, verify one selected
  provider covers the reconstructed application set, and bind exact issuance
  occurrences at installation. Do not create a slot per monomorph. A first
  non-authorizing foundation now retains exact package-qualified indexed
  schemas, concrete and artifact-qualified symbolic application demands, exact
  substitutions, and generic or exact-family coverage tied to one selected
  provider plan. It canonically deduplicates the reconstructed concrete set,
  rejects unresolved or unused substitutions, schema/arity/plan drift, and
  incomplete exact coverage, and commits the selected closure, plan,
  applications, and coverage to one content-derived identity without minting
  per-application slots. Native realization now retains that exact nonzero
  selected-closure identity beside the source-free provider-plan projection,
  and component-candidate replay requires both to match independently. Thus a
  coverage or resolved-reach change cannot hide behind unchanged selected plan
  rows.
  Verifier-derived concrete/symbolic demand and coverage rows, final-
  composition wiring, and installation-bound exact issuance occurrences remain
  engineering rungs.
- Schema/device correspondence now has a distinct provider-issued,
  provenance-bearing authority carrier separate from storage compatibility.
  It binds one exact validated placement and resource-profile grant to a
  provider identity and stable device instance; optional runtime revision
  evidence retains its observation, predicate, observed value, and the same
  provider/device/grant identities. Admission independently replays every
  binding, and rejection returns the complete non-Clone grant or revision
  evidence for corrected retry. This establishes no storage compatibility,
  content validity, device observation, placed access, or publication.
  Admitted schema/device correspondence can now bind transactionally to one
  exact borrowed placement admission while remaining separate from storage
  compatibility. The join independently replays revision/provider/device/
  profile evidence and the admission's exact loan, placement, admitted profile,
  and resource compatibility. Rejection returns both complete non-Clone inputs
  for corrected retry; withdrawal returns the original loan and
  correspondence. No placed view, content qualification, field access, or
  device operation is established.
  Schema/device provider grants and admitted correspondence now retain the
  complete validated placement plan—layout, access policy, and boundary
  reach—rather than treating compact `PlacementPlanId` as authority. Admission
  and every later placement/view/access/retirement replay compare exact
  structure; same-ID/different-geometry or policy drift rejects transactionally
  and returns the complete non-Clone inputs for repair. Compact identities
  remain reporting/cache keys only.
  Corresponded borrowed placement admission now establishes a sealed placed-
  view carrier only after independently replaying both its physical
  correspondence and exact loan/profile/plan/resource admission. Rejection
  returns the complete bound carrier for corrected retry or withdrawal. The
  established carrier retains correspondence beside the placed view but
  deliberately exposes no projection or inner-view escape until primitive
  requests can carry and replay that evidence; no content, field access, device
  operation, or target lowering is established. Corresponded borrowed views
  now project through a distinct lifetime-bound placement-authority variant
  that retains the exact admitted schema/device correspondence through field
  projection, authorization, primitive requests, and External specialization
  preflight. Each boundary independently replays provider/device/revision
  evidence against the retained view placement/profile; ordinary views remain
  correspondence-free. Drift rejects without consuming the view/request, and
  no Terminal, device operation, or target-lowering authority is established.
  Stable primitive/compound, External primitive, and Atomic primitive outward
  specialization carriers now expose the exact lifetime-bound admitted schema/
  device correspondence retained by their sealed primitive request when one
  exists. Their borrowed preflights independently replay that correspondence
  with the complete placement authority; drift rejects without consuming the
  specialization, repair/retry preserves the same provenance, ordinary
  storage remains correspondence-free, and no device operation or target
  lowering is established. Corresponded borrowed placed views now retire
  transactionally: retirement independently replays the correspondence-to-
  view plan/profile identity and the exact loan/plan/admitted-profile/resource
  join, returns the original loan and non-Clone correspondence as distinct
  authorities on success, and returns the complete view on drift for corrected
  retry. Coordinated copied-receipt drift and correspondence drift fail closed;
  retirement establishes no content or device operation. Ordinary borrowed
  placed views now retire through a checked loan-release transition that
  independently replays the exact loan, placement, admitted profile/receipt,
  and resource compatibility. Drift returns the complete view for corrected
  retry; success returns the original loan with its origin, lineage, geometry,
  and polarity unchanged and establishes no content, vacancy, or destruction.
  Corresponded retirement reuses the same placement replay before returning
  its separate non-Clone correspondence. Every Stable primitive/compound,
  External primitive, and Atomic primitive outward specialization now exposes
  a shared borrow of its exact sealed primitive request. Consumers can inspect
  the complete lifetime-bound placement, authorization, resident, and optional
  schema/device provenance without copied-identity reconstruction or mutation;
  rejection preserves that same carrier, repaired replay reproduces it exactly,
  and no transfer, device operation, or target lowering is established.
  Provider/device-bound External lowering now crosses a distinct
  correspondence-required preflight after generic External specialization. The
  sealed carrier retains the exact lifetime-bound non-Clone schema/device
  correspondence and independently replays the complete placed request,
  supply, operation, and correspondence identity; correspondence-free or
  substituted authority rejects without observation and returns the exact
  External specialization for repair or alternate use. No provider operation
  is selected, no transfer occurs, and no target lowering is established.
  Provider/device-bound Atomic lowering now crosses the same distinct
  correspondence-required boundary after generic Atomic specialization. Its
  sealed carrier retains the exact lifetime-bound non-Clone correspondence and
  independently replays the complete placed request, admitted Atomic supply,
  operation/ordering law, and correspondence identity; correspondence-free or
  substituted authority rejects without an atomic attempt and returns the
  exact Atomic specialization for repair or alternate use. No provider
  operation is selected and no target lowering is established.
  Provider/device-bound Stable primitive lowering now crosses a distinct
  correspondence-required preflight after generic Stable read/write
  specialization. Its sealed carrier retains the exact lifetime-bound non-
  Clone correspondence and independently replays the complete placed request,
  admitted Stable supply, operation, and correspondence identity;
  correspondence-free or substituted authority rejects without a memory event
  and returns the exact Stable specialization for repair or alternate use.
  Ordinary Stable storage remains correspondence-optional; no provider
  operation is selected and no target lowering is established. Provider-bound
  bounded Stable compound lowering now applies the same boundary after generic
  `CompoundMutation` specialization, replaying exclusive placed custody,
  admitted Stable supply, bounded read-patch-write identity, and exact
  correspondence. Rejection performs no read or write and returns the exact
  compound specialization; ordinary Stable compound access remains
  correspondence-optional.

#### L6c — symbolic materialization

- Carry symbolic sources, placement constraints, immutable post-handoff bytes,
  exact footprint, and invocation plan through final artifacts. Connect placed
  fragments to source-level provider invocation after establishment; provider
  preparation generates no host code. Validate exact bytes and placement;
  fingerprints remain report/cache identity, never authority. Numbered
  symbolic fields now rejoin fragmented layout rows by stable member identity
  rather than presentation spelling: renames preserve generated-writer
  identity, while identity drift or collision rejects before resolver
  invocation. Symbolic materialization now preflights every retained write's
  static bit geometry and byte range before invoking any provider/compiler
  resolver; an invalid later field produces no resolver observation or partial
  action plan. Post-handoff execution resolves each exact relocation target
  once, immediately validates that value against every retained same-target
  write, and does not observe unrelated targets after rejection. Fully resolved
  materialization then independently replays every write's geometry and
  stored-integer fit before staging any byte; tampered or out-of-range values
  reject without truncation or destination mutation. Static writer validation
  and reusable-fragment lowering consume the same known-value validator, so
  invalid pre-resolved fit evidence rejects before any dynamic resolver
  observation or destination mutation. Reusable post-handoff invocation
  evidence is sealed behind validated lowering; installation, external-root,
  and instruction-selection consumers may inspect but cannot reconstruct or
  weaken the exact fragment, placement, source-slot, or fit evidence. Sealed
  invocation evidence now also supports independent borrowed structural replay
  of its context ABI, placement alignment, exact fragment geometry, canonical
  source-slot order, target uniqueness, stored-fit linkage, and recomputed
  fingerprint before source values are accepted; rejection leaves the
  invocation unchanged for corrected retry. Instruction-selection binding now
  independently replays the sealed invocation, target architecture, exact
  re-encoded bytes, state footprint, normalized fragment identity, and emitted
  fingerprint; every rejection returns the unchanged lowered writer evidence
  for corrected retry without regeneration. External-root prepared-writer
  execution independently replays the retained invocation structure,
  writer-derived invocation, opaque context binding, and exact installed-code,
  artifact, and architecture identities before destination mutation; rejection
  returns both the exact prepared invocation and destination for corrected
  retry. External-root writer binding independently replays retained lowered
  invocation structure, canonical bytes, footprint, and emitted identity;
  every bind rejection returns the exact lowered fragment and non-clonable
  provider preparation for corrected retry. Bound external-root writer
  execution independently replays the retained lowered fragment and exact
  provider preparation/context relationship before destination consumption;
  rejection returns the complete bound carrier and exact destination for
  corrected retry. Writer
  derivation, lowering, validation, and execution uniformly require at least
  one retained fragment; an empty provider program cannot claim materialization.
  Validation also binds every supplied source word to any exact pre-resolved
  value sealed for that slot, so numeric substitution under unchanged evidence
  rejects before resolver observation or destination mutation. Post-handoff
  writer execution now stages the complete resolved fragment program and
  commits its writer range once. Any late application rejection leaves the
  provider's exact destination bytes unchanged for recovery/retry, while
  successful bytes remain unpublished until the existing consumer-specific
  validation/publication transition. Successful post-handoff destination
  writing now retains the exact non-clonable resolved context, including sealed
  invocation, placement, source-slot values, and fingerprint, rather than
  reducing it to copied report identities. Failure returns both the context and
  prepared destination intact for corrected retry. The external-root consumer
  replays that context against the exact installed realization and destination
  preparation before writing and again before exposing the still-unpublished
  written carrier; this establishes neither consumer semantics nor publication
  authority. Successful bound external-root writer execution now returns a
  sealed non-clonable carrier retaining the exact AOT-lowered fragment beside
  the installation-owned written destination and resolved context. The outward
  consumer independently replays canonical lowered bytes, footprint, emitted
  identity, invocation, target architecture, and the exact installed
  realization; rejection only borrows the carrier, preserving every input for
  corrected retry. The destination remains unpublished and this transition
  establishes neither consumer semantics nor publication authority. Successful
  external-root writer execution now also retains the exact admitted provider
  execution, target architecture, source invocation, writer plan, and
  installation-owned written destination/context. Its outward consumer
  independently re-lowers and replays the writer against those retained
  provider and installation facts, while the compiler's written bound carrier
  retains that complete provider evidence beside the exact AOT-lowered
  fragment. Validation rejection only borrows the carriers, preserving complete
  retry ownership; no consumer semantics or publication authority is
  established.
  Written-but-still-unpublished external-root writer destinations now expose a
  checked recovery transition. The external-root and compiler-bound consumers
  independently replay the exact invocation, lowered fragment, provider
  execution, installed realization, mapping, and destination preparation
  before returning the sealed prepared/bound invocation with its exact
  destination for retry. Rejection returns the complete non-clonable written
  carrier unchanged; success preserves the current unpublished bytes and
  establishes neither consumer semantics nor publication authority. Compact
  fingerprints remain replayed identity only and create no authority.
  External-root post-handoff writer preparation now binds the admitted entry to
  one exact canonical provider-resolved source slot. A copied pre-resolved
  numeric entry cannot substitute for sealed provider resolution. The selected
  entry identity and source-slot correspondence remain attached through
  prepared, written, and recovered non-clonable carriers, and each consumer
  independently replays them; drift rejects while preserving the complete
  carrier for corrected retry. This establishes no provider-operation
  authority, consumer semantics, publication, or native execution, and compact
  fingerprints remain identity rather than authority.
  External-root symbolic writer preparation now retains the exact validated
  requirement-bearing root evidence beside its admitted provider-execution
  evidence throughout preparation, writing, and recovery. Each consumer
  replays their full structural equality before accepting the terminal summary,
  selected entry, and canonical provider-resolved source slot; a separately
  valid root with substituted requirement identity rejects while preserving
  the complete carrier for corrected retry. Compact normalized identities
  remain consistency/report keys rather than authority, and this establishes
  no provider-operation authority, consumer semantics, publication, or native
  execution.
  Compiler-side external-root writer binding now consumes and retains the exact
  selected source `ServiceSchema` beside the lowered fragment and non-clonable
  provider preparation. Binding and every later bound/written/recovery consumer
  replay the selected provider-plan identity, unique exact requirement row,
  boundary arity, complete parameter-identity row cardinality, calling-plan
  identity, and admitted entry claims against the retained requirement-bearing
  root evidence. Rejection returns the selected schema, lowered writer, and
  prepared invocation intact for corrected retry. The schema and compact
  fingerprints remain identity/shape evidence rather than provider-operation
  authority; no device operation, consumer semantics, publication, or native
  execution is established. Selected source-schema correspondence is now also
  preflighted before provider preparation resolves any symbolic source or
  populates the opaque writer context. The compiler borrows the exact admitted
  provider/root evidence and replays the same provider-plan, requirement,
  boundary, calling-plan, and entry claims; drift rejects with every input
  unchanged and no resolver observation. AOT binding and later consumers retain
  their independent replay.
  Selected external-root source schema and provider-populated writer context now
  cross preparation as one sealed non-clonable carrier. Preparation consumes
  the exact selected plan only after preflighting provider identity,
  requirement, boundary shape/calling identity, and entry claims; every
  rejection returns that selected plan unchanged before resolver observation.
  AOT binding accepts only the sealed preparation, and binding rejection returns
  the exact lowered writer plus complete preparation for corrected retry,
  preventing same-plan schema substitution after context population. Successful
  binding transfers the original schema/context pair through bound, written,
  and recovery custody. This grants no provider-operation authority and
  establishes no device event, publication, or native execution.
  Selected external-root writer preparation now consumes the exact AOT-lowered
  fragment and preflights its canonical structure, target architecture, and
  invocation against the retained provider writer plan before the installed
  resolver may observe any symbolic source. The non-clonable preparation seals
  selected source schema, lowered fragment, and provider-populated context
  together; binding accepts only that carrier and independently replays all
  three. Early schema/lowering drift and later destination-preparation rejection
  return the exact selected schema and lowered fragment for corrected retry.
  This establishes no provider-operation authority, consumer semantics,
  publication, or native execution.
  External-root writer preparation now consumes and seals the exact activated,
  pinned, writable, unpublished destination before the installed resolver
  observes symbolic sources. The destination's non-clonable mapping,
  preparation receipt, placement, and mutable byte view remain joined to the
  selected schema, exact AOT lowering, and provider-populated context through
  binding and execution. Preparation rejection returns the selected schema,
  lowering, and destination intact; execution rejection and written recovery
  return the complete bound carrier with that same destination, preventing
  same-geometry destination substitution after resolution. This establishes no
  provider-operation authority, consumer semantics, publication, or native
  execution.
  Prepared post-handoff destinations now expose a borrowed exact replay of
  their activated mapping, provider receipt, required write rights, pinning,
  unpublished state, placement, and byte-view geometry. External-root writer
  preparation performs that replay before the installed resolver observes
  symbolic sources; drift returns the selected schema, lowering, and complete
  non-clonable destination unchanged. Corruption rejects without modifying
  destination bytes, and repaired evidence supports retry through the same
  carrier. This grants no provider-operation, write, publication, consumer-
  semantic, or native-execution authority.
  Prepared post-handoff destinations now cross symbolic-source resolution
  through sealed non-clonable validated custody rather than reverting to a raw
  mapping after borrowed preflight. Consuming validation replays the exact
  activated mapping, provider receipt, write rights, pinning, unpublished state,
  placement, and byte geometry; rejection returns the complete raw destination
  before resolver or write observation. Compiler preparation, external-root
  execution, write failures, and validated recovery retain that validated
  carrier end to end. This establishes no provider-operation authority,
  consumer semantics, publication, device event, or native execution.
  Instruction-selection's standalone post-handoff entry-writer binder no
  longer accepts bare destination length and placement while resolving symbolic
  entry values. It consumes and retains the exact validated non-clonable
  prepared destination beside the lowered fragment and opaque resolved context;
  every lowering, architecture, resolution, or context rejection returns both
  lowered evidence and destination custody unchanged. This closes the parallel
  preflight bypass without granting provider-operation authority, exposing
  resolved words, publishing bytes, or claiming device/native execution.
  External-root writer preparation now independently replays the complete
  admitted provider execution before symbolic-source resolution: exact
  validated root structure, retained validated boundary carrier, execution-to-
  root binding, exit assurance, and recomputed normalized execution identity.
  `ValidatedExternalRoot` preserves `ValidatedBoundaryEntryPlan` rather than
  downgrading it to raw plan data, while its existing raw-plan accessor remains
  available. Execution-fingerprint drift rejects before resolver observation;
  repaired evidence supports retry unchanged. This establishes no provider-
  operation authority, consumer semantics, publication, or native execution.
  External-root writer preparation now retains the exact borrowed installed-
  code realization beside the selected schema, AOT lowering, activated
  unpublished destination, and provider-populated context before symbolic-
  source resolution. Binding independently replays the context against that
  exact installation and destination; execution, outward validation, and
  written recovery reuse the same retained installation rather than accepting
  a substitutable resolver parameter. A colliding installed artifact rejects
  during preparation while returning the selected schema, lowering, and
  destination intact for corrected retry. This establishes no provider-
  operation authority, consumer semantics, publication, or native execution.
  Compiler-bound written external-root destinations now require a consuming
  outward-consumer replay against the exact retained installed realization
  before bytes or decomposed written state are observable. An equal-looking
  substitute installation rejects before observation and returns the complete
  non-clonable carrier for corrected retry; only the validated still-
  unpublished carrier exposes bytes, parts, and recovery. This establishes no
  provider-operation authority, consumer semantics, publication, device event,
  or native execution.
  The external-root written destination now requires its own consuming outward
  replay before bytes or decomposed installation-written state are observable.
  Its sealed non-clonable validated carrier retains exact provider/root,
  invocation, installed realization, mapping, context, and destination
  evidence; rejection returns the complete raw carrier unchanged. Compiler-
  bound validation retains this lower validated carrier instead of downgrading
  it after replay, so observation and recovery remain gated through both
  custody layers. Bytes remain unpublished and this establishes no provider-
  operation authority, consumer semantics, device event, or native execution.
  Installation-owned written post-handoff destinations now require a consuming
  exact replay before their resolved context, bytes, prepared recovery state,
  or raw mapping parts are observable. The sealed non-clonable validated carrier
  retains the exact installed realization, activated mapping, provider receipt,
  placement, byte geometry, and a hash-free exact copy of the complete
  successful destination image. Every consuming replay compares the current
  complete byte view with that producer output before observation, including
  bytes outside the writer fragment footprint; mutation or retained-image drift
  returns the complete raw carrier for repaired retry. External-root and
  compiler-bound validated custody retain this lower validated carrier through
  observation, decomposition, and recovery, rather than downgrading evidence
  between layers. Bytes remain unpublished, the retained image establishes no
  consumer-specific semantic value, and this adds no provider-operation,
  publication, device-event, or native-execution authority.
  Written external-root destinations now retain the installation layer's sealed
  non-clonable validated written custody instead of downgrading it after
  successful replay. The outer consuming validation independently replays that
  retained installed-artifact and destination evidence before exposing bytes,
  while provider/root drift returns the complete outer carrier unchanged for
  corrected retry. Installed-artifact identity drift and compiler-level exact-
  realization substitution remain distinct checks. Bytes remain unpublished,
  and this establishes no provider-operation authority, consumer semantics,
  device event, or native execution.
  The external-root foundation's 2,299-line unit corpus now lives in a private
  test child rather than sharing its 4,186-line production coordinator; all 28
  unit tests and the public external-root surface remain unchanged.
  Installed terminal-entry stack-demand composition now lives in a focused
  714-line child, owning local stack evidence, nesting relations, cycle/reentry
  rejection, worst-case simultaneous-use peaks, and exact fingerprints. The
  production coordinator is 3,498 lines; its public root re-exports, exact
  107-function inventory, validation/error order, and behavior are unchanged.
  Installed terminal entry/segment fuel binding and exact acyclic fixed-fuel
  provider-graph composition now live in a focused 555-line `fixed_fuel`
  child. The 2,965-line coordinator retains external-root admission, execution,
  interrupt lifecycle, and ledger orchestration; public re-exports,
  diagnostics, behavior, and the 107-function inventory remain unchanged.
  Opaque provider-exit assurance, admitted provider-execution identity, and
  exact post-handoff writer preparation/writing/consumer-validation/retry
  custody now live in a cohesive 1,077-line `provider_execution` child. The
  1,906-line coordinator retains root validation, admission publication,
  interrupt lifecycle, and ledger orchestration; public re-exports,
  diagnostics, behavior, and the 107-function inventory remain unchanged.
  External-root candidate schema, canonical validation, and exact normalized
  root fingerprinting now live in a focused 425-line `root_validation` child.
  The remaining 1,496-line coordinator is the cohesive admission-publication,
  interrupt-lifecycle, and installed-root-ledger owner; public re-exports,
  diagnostics/order, behavior, and the 107-function inventory remain
  unchanged. This is the natural modularization boundary for that crate.
  Struct-literal construction bounds now live in a focused 324-line private
  owner. Literal, declared-range, local-initializer, sequence-length, and
  capacity facts; saturating interval arithmetic; exact symbolic equality; and
  fail-closed tri-state proof replay retain diagnostic order and no-implicit-
  check behavior. Struct-literal field obligations now live in a focused 410-
  line private owner. Exact common/payload field-type lookup, shape/class/
  domain-weakening checks, scalar/range narrowing, and recursive fixed-array
  length/element validation retain diagnostic order and the crate-root API. The
  construction-proof owner also owns default-domain valuation, exact membership
  replay, and fail-closed diagnostics, leaving a one-way coordinator dependency.
  The natural root is now 364 lines while diagnostic order, crate APIs, and the
  exact 25-function inventory remain unchanged.

Acceptance: UART/MMIO, shared-page IPC, and ordinary RAM use one extent/layout
foundation with different profiles. Misalignment, insufficient rights,
unplanned offsets, narrow External writes, destructive reads through a
repeatable accessor, overlapping transfer footprints, forged profile evidence,
and unsupported atomic operations reject before code generation.

### P3 — Terminal Psi, proof-carrying artifacts, and fuel

Owners:

- `wiki/architecture/pipeline/terminal_psi.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/architecture/bootstrap_lattice/proof_kernel.md`

Remaining:

- **COMPILER-DRIVER-CLEANUP — restore `compiler.rs` as a thin pipeline
  coordinator.** The large alternate driver is gone. `compiler.rs` now declares
  only `Compiler`, delegates one typed request to `driver.rs`, and owns no
  language or target data. The hidden test product adapter and stale
  `write_output` compatibility Boolean are now retired. Component-progress
  admission now remains wholly with its provider-planning owner: the native
  product stop passes the complete manifest to that owner and cannot inspect
  pending rows itself. Report-facing production-subject projection now also
  consumes one complete `CheckedCompilation` under `pipeline/reporting`; the
  driver receives only the resulting report-owned subject and no longer
  reconstructs package/build/target custody itself. Release rollback now also
  closes through one owner-built `OptimizationRollbackSettlement`: its
  effective selection and optional report receipt cannot drift across native
  realization and final report assembly, and the driver no longer reconstructs
  the empty-request identity case. No other product-stop domain policy is
  currently identified. Requested-product stopping and final report assembly
  remain coordinator work. The callback-placement rejection remains an
  intentional fail-closed Terminal handoff fence until canonical callback-use
  custody lands as a PSIIR vertical slice.

  The first request-normalization rung is live. One typed `CompileRequest`
  now owns the production compile options, requested product, artifact policy,
  admission profile, and optional reconciled package graph. The canonical
  production `compile` operation now accepts only that request. Integration
  fixtures construct the same typed request behind local test helpers; the
  options-based library compatibility seam is removed. The
  real `omega` and `omega-run` command-line callers now construct that request
  directly, including their observation policy. The obsolete
  `compile_with_artifact_policy` permutation is removed; its differential and
  report canaries construct the typed request too. Package-aware integration
  now does the same, and the callsite-free package and executable-policy
  permutations are removed. The former entry override and worker ceiling are
  gone, including the hidden `CompileHarnessRequest`; integration tests now
  construct the ordinary request and explicitly publish retained native
  products when they need a file. `RequestedCompileProduct`
  now makes `Check`, terminal artifact, and retained native artifact explicit.
  `TerminalArtifact`
  now runs the Psi-owned checked frontend and exact canonical producer, returns
  the complete report-owned artifact, and never enters StateGraph, native
  emission, output, or installation; unsupported Terminal vocabulary rejects
  instead of selecting another backend. `NativeArtifact` now enters the
  same Psi-owned frontend and canonical producer, then crosses one source-free
  native realization operation shared with component staging. Its retained
  report payload owns the canonical Terminal artifact, exact target and
  provider projections, object/relocation/text evidence, and replayed final
  image while owning no output path, publication, installation, or runtime
  authority; the report records `wrote_output == false`. Unsupported Terminal
  vocabulary and pending component progress reject rather than selecting or
  silently retaining another backend. Component-progress rejection and exact
  source root resolution now live with the manifest owner; the driver invokes
  its exact ordered rejection and architecture tests prevent compiler product
  or reporting code from inspecting pending rows directly. Selected-provider
  external-binding projection and its source-boundary plan replay live under
  `provider_plans`. Legacy wrapper insertion and output publication are no
  longer compiler stages. The typed-to-checked phase result retains the Accepted-only
  pre-lowering generic-template classification and fingerprint rows in exact
  typed order; trust reporting consumes that carrier, so the driver no longer
  captures typed facts before lowering as an out-of-band courier. The obsolete
  boundary/backend report couriers and alternate output path are deleted;
  capability and wire validation remain semantic checks when auxiliary output
  is suppressed. Selected external-
  binding projection now also settles transactionally onto the checked phase
  result from its retained typed, selected-plan, and evaluated calling-plan
  evidence. Complete rows publish in original selected-plan order only after
  full success; equal/empty settlement preserves sidecar identity, rejection
  preserves its prior identity and contents, and backend planning consumes only
  that retained sidecar instead of a driver-couriered pre-lowering vector. The
  selected `ProgramEntry` now travels through one build-owned typed settlement
  carrier as well: exact target-slot resolution, source-signature validation,
  and optional physical/semantic/storage calling-plan settlement occur in their
  original diagnostic order, remain joined through component-progress and
  provider projection, and split only beside the backend/storage consumers.
  Target-scoped provider-default declarations
  cross source filtering and typed construction in one owner-controlled
  carrier; both frontend routes consume it for exact typed-machine rebinding
  instead of couriering a raw machine-name vector, while authored row order and
  `build override > target default > unique declaration default` precedence
  remain unchanged. Native publication is now one explicit product-owned
  operation after compilation: it validates the retained native
  artifact, stages and replays exact bytes, atomically exposes the file, and
  returns a publication receipt. It is not a compiler route or request product.
  `CompileOptions` now contains only source, build-directory, and target
  coordinates. Production callers select `RequestedCompileProduct` on the
  request; tests use an explicit local `Check` versus
  `NativeArtifactAndPublish` choice and publish only after retained-native
  success. The exact-native source index recognizes that explicit product
  choice rather than a Boolean seed, while `CompileReport::wrote_output`
  remains solely a post-publication custody observation. Architecture tests
  reject reintroducing output/product policy into options or request defaults.
  Contract-entailment stand-down capture now lives on the typed-to-checked
  `CheckedProgramSurface` beside accepted-template classification. It records
  the existing pristine-typed machine/contract/fact/reason ledger before the
  ownership-moving check and carries that phase-owned result into package
  review; the driver no longer captures or couriers a raw side vector.
  Target-dependent callback closure now also enters that transition through
  one explicit `TypedToCheckedSettlementInput`. The phase closes callback
  materializations while checked Psi is uniquely owned, validates the resulting
  placements, transactionally binds selected-provider receipt facts, and
  returns both sidecars on `CheckedProgramSurface`; the driver no longer uses
  `Arc::get_mut`, calls the provider-fact binder, or replaces the checked
  program after the transition. Preliminary package-selection validation uses
  a distinct checked observation and cannot construct an incomplete final
  surface. Selected execution now closes through one consuming
  `SelectedExecutionSettlementInput` as well. In the existing diagnostic order
  it constructs component progress from the exact retained entry root, settles
  operator and float dispatch, retains compiler-intrinsic review provenance,
  settles boundary-adapter dispatch, and elaborates task activations from the
  rewritten call tables. `SelectedExecutionSettlementSurface` owns the final
  program and every resulting sidecar; `checked_entry` only derives the exact
  progress root and consumes the complete surface. Remaining cleanup belongs
  to the broader semantic-owner and observation/report consolidation below,
  not post-check mutation or a mutable provenance courier. Checked
  observations now enter one typed `CheckedObservationInput` under
  `pipeline/reporting`. Trust-obligation reconstruction, admission settlement,
  derived trust-report construction, and independent report consistency
  validation run unconditionally before the reporter's sole auxiliary-policy
  branch. `Full` then preserves the existing trust-report and ordered checked-
  snapshot writes and appends `00_timings.html`; `OutputOnly` constructs no
  report writer and adds no reporting filesystem effect. Ordered accumulated
  `CompileTimings` travel on `CheckedCompilation` for that observation, while
  custom semantic equality deliberately excludes their nondeterministic
  measurements. The driver no longer owns trust-report algorithms, writers,
  snapshot arguments, timing output, or observation-policy branching, and the
  checked snapshot writer is no longer re-exported from the pipeline root.
  Production-compilation subject projection lives beside those reporting
  observations and consumes the complete checked result in its original
  diagnostic order. It alone joins package identity, build-machine identity,
  evaluation usage, observation custody, target profile, and native target into
  the `omega-compilation-report` artifact; architecture tests prevent the
  driver from recovering those fields directly. Request-owned product
  admission is now explicit as well. Consuming
  `CompileRequest::validate_for_execution` returns a private
  `ValidatedCompileRequest` before the driver may acquire source. The request
  owner enforces the existing cross-field rule that a nonempty optimization
  rollback can accompany only `NativeArtifact`, preserving the exact
  diagnostic and the pre-source/no-output rejection. The driver no longer
  inspects rollback contents or formats request-policy diagnostics. The
  rollback owner now returns one complete settlement whose effective selection
  alone enters native realization and whose optional receipt alone enters the
  final report; empty rollback preserves the exact build selection without
  minting a receipt. Architecture tests prevent the driver from rebuilding
  that fallback. It only coordinates an admitted request through the shared
  frontend and selected product stop.

  Restore the driver contract:

  1. Replace the public `compile_with_*` Cartesian product with one
     `CompileRequest` and one production `compile(request)` entry. Model the
     requested compiler product explicitly (`Check`, terminal artifact, or
     retained native artifact). Installation/publication consumes a completed
     product afterward; it is not a fourth compiler mode. Package inputs,
     deployment/admission policy, observation/report policy, and resource
     budget are typed request fields rather than distinct compile modes.
  2. Keep test-only controls out of the production API. Entry overrides,
     bounded inner-worker counts, and the hidden harness request are deleted.
     Differential-oracle controls and fixture artifact suppression remain local
     test concerns, never exported compiler entry points.
  3. Unify `compiler.rs` and `checked_entry.rs` around one Psi-owned frontend
     operation. Source acquisition, build-time evaluation, target filtering,
     resolution, typing, checking, provider-selection inputs, adapter closure,
     and task facts execute once. Ordinary compilation, checking,
     interpretation, differential tests, and terminal production consume the
     same closed result rather than maintaining parallel copies of that logic.
  4. Make each phase result complete for the next phase. Remove the driver's
     `Arc::get_mut` post-check rewrites and the pattern of capturing typed facts
     "before ownership moves" so that a later stage can recover them. Required
     identities and evidence travel in the owning phase artifact or in one
     explicitly typed settlement input; the driver does not act as an
     out-of-band fact courier.
  5. Move external-binding projection, source-boundary entry-plan selection,
     provider/calling-plan settlement, component-progress construction,
     program-storage bridging, executable-TCB authorization, and publication
     custody to their actual semantic owners. Their unit tests move with those
     owners. `compiler.rs` contains no local domain algorithm or policy fixture.
  6. Make snapshots, diagrams, timing, trust/wire/boundary reports, and other
     auxiliary artifacts observations of retained phase results through one
     reporter interface. Reporting policy may suppress observations; it must
     not create a distinct semantic compilation path or interleave bespoke
     control flow throughout the driver.
  7. Isolate the large-stack host-thread workaround and worker-pool lifetime as
     execution infrastructure around the pipeline. They must not require every
     request combination to grow another public wrapper.
  8. Preserve the completed Psi/Omega cutover rather than adding an abstraction
     layer around retired backends. The final coordinator orders the single
     Psi-to-terminal-to-Omega route fenced by the architecture tests.

  Acceptance: one production compile entry and one request type cover every
  current caller; one Psi frontend execution supplies check, interpreter,
  differential, terminal, and native consumers; checked results are not
  mutated by the driver; reporting does not branch semantic execution; policy
  helpers and their tests reside with their owners; unused compile wrappers are
  gone; and `compiler.rs` contains only request normalization, ordered stage
  calls, requested-product stopping, and final report assembly. Preserve all
  existing diagnostics and canary behavior while shrinking the non-test driver
  to a reviewable coordinator rather than distributing the monolith under new
  filenames.

- **PSIIR.** Extend terminal Psi only as complete vertical slices: canonical
  encoding, independent obligation reconstruction and verification,
  interpretation, fixed fuel, Omega lowering, native evidence, artifact/image
  custody, and installation must move together **through the one mandatory
  production seam**. A slice
  extends the representation behind that seam; it must not add or retain a
  parallel source-to-native route. The detailed accepted
  vocabulary and current fences live in
  [`terminal_psi.md`](wiki/architecture/pipeline/terminal_psi.md); do not
  duplicate its operation-by-operation ledger here.

  Producer and verifier proof reconstruction independently own one mutable,
  invocation-local affine/cast index. Both cache exact definition frontiers,
  target-filtered words, cast spines, and literal landings; the producer also
  memoizes completed affine subproofs before independent kernel replay. No
  shared proof authority, global cache, concurrency, or viewer suppression
  hides the search. The
  producer-side index closes the previously omitted half of this performance
  slice: the exhaustive mixed-nominal source regression now completes instead
  of exceeding a 120-second lowering cutoff, while the canonical mixed-shift
  artifact retains its 317 obligations, proof codec round trip, independent
  verification, and tamper rejection. Keep performance claims scoped to the
  measured producer or verifier phase; the earlier 8.68-second verifier result
  was not an end-to-end producer baseline.

  The accepted baseline covers bounded scalar/direct and structural/content
  calls, guarded crash continuations, structural results, fixed-array custody,
  exact affine cleanup and partial transfer, bounded acyclic control, selected
  provider catalog/dispatch, and verified Boolean/integer shared cleanup
  convergence. Integer leaves retain the documented policy arithmetic, casts,
  shifts, division/remainder, exact-operation evidence, bounded nesting, and
  independent exact leaves in distinct proof-free subtrees across interpreter
  and every native target. A finite same-carrier exact-add chain may have a
  landed literal sibling at each link, while a finite exact-subtract chain may
  continue only through its left operand and must have a landed literal right
  operand at each link. A finite same-carrier chain may mix exact addition and
  subtraction when both kinds occur, every link continues through its left
  operand, and every right operand is a landed same-carrier literal. The
  verifier combines additions and mathematical negations of subtrahends in a
  checked sign/magnitude offset and derives every carrier-tight prefix bound
  from the direct root. A finite same-carrier chain may also mix exact divide
  and remainder while continuing only through its left operand, with a landed
  nonzero unsigned divisor or signed divisor other than `0` and `-1` at every
  link. A finite same-value-carrier exact-right-shift chain may likewise
  continue only through its left operand, with an independently landed fixed
  native integer count satisfying `0 <= count < value width` at every link.
  A finite same-value-carrier exact-left-shift chain may likewise continue only
  through its left operand, with independently landed fixed native integer
  counts satisfying `0 <= count < value width`; count carriers may differ.
  The verifier accumulates those counts with checked arithmetic and derives the
  carrier-tight bound on the direct root for every prefix, including the
  zero-only root when the cumulative count reaches the value width.
  A finite same-carrier exact-multiply chain may continue only through its left
  operand, with an explicitly landed same-carrier nonnegative literal factor at
  every link. All seven forms require a direct machine-parameter root. The
  verifier walks ordered definitions for addition, subtraction, their mixed
  chain, multiplication, and left shift and reconstructs every retained
  operation's safety obligation independently;
  multiplication accumulates nonnegative factors with checked arithmetic and
  derives carrier-tight root bounds, while divide/remainder and right-shift
  links need no producer-definition authority because each safe landed divisor
  or count reconstructs independently. One direct
  fixed-integer parameter may also pass through a finite chain of valid
  widenings and then exactly narrow back to its original carrier; Terminal
  retains every operation and independently derives the narrowing obligation
  from the ordered, uniquely defined widening chain. Separately, one exact
  fixed-native cast may consume a finite nonempty left-associated same-carrier
  exact-add/subtract literal-offset chain rooted at a direct machine parameter.
  The verifier retains every prefix proof, accumulates the checked offset, and
  independently derives target-range-minus-offset bounds intersected with the
  source carrier, including signed and cross-sign conversions. One
  validator-legal partial fixed-native cast may also consume a finite nonempty
  left-associated same-source-carrier exact-multiply chain rooted at a direct
  machine parameter, with independently landed nonnegative literal factors.
  Every multiply prefix keeps its own evidence; the cast uses the checked
  cumulative product to reconstruct the inverse target interval and intersect
  it with the source carrier. Product zero makes only the cast obligation true,
  product one uses the ordinary target/source intersection, and larger products
  divide the signed or unsigned target bounds without erasing earlier proofs. A
  validator-legal partial fixed-native cast may likewise consume a finite
  nonempty left-associated same-source-carrier exact-left-shift chain rooted at
  a direct machine parameter, with independently landed legal fixed-native
  counts whose carriers may differ. Every shift prefix keeps its own evidence;
  the cast uses the checked cumulative count to shift the target interval right
  and intersect it with the source carrier. Count zero uses the ordinary
  target/source intersection, a sub-source-width count uses signed or unsigned
  inverse target bounds, and a source-width-or-larger count makes only the cast
  true because any successfully produced exact source result is zero.
  A finite nonempty same-source-carrier exact-right-shift chain may feed the
  same partial cast under the same direct-root and heterogeneous landed-count
  fences. The cast independently reconstructs the arithmetic/zero-fill shift
  preimage of the target interval; at or above source width, unsigned roots
  yield zero while signed roots yield `-1` or `0` and therefore require a
  nonnegative root only when the target is unsigned.
  A finite nonempty same-source-carrier exact-divide/remainder chain may now
  feed the same partial cast when verifier-owned toward-zero division and
  dividend-sign remainder interval-hull replay maps the full source carrier
  wholly inside the target. Every arithmetic prefix and the cast retain
  independent evidence; guard-sensitive nonconvex preimages remain fenced.
  Conversely, one
  validator-legal partial fixed-native cast of a direct parameter may root a
  finite nonempty left-associated same-target-carrier exact-add/subtract chain
  with independently landed literal right siblings. The cast keeps its own
  direct representability evidence; every arithmetic prefix keeps distinct
  evidence for the target interval shifted by its checked cumulative offset and
  intersected with the source carrier. Cancellation cannot erase an earlier
  prefix obligation. The same direct partial-cast root may instead feed a
  finite nonempty left-associated same-target-carrier exact-multiply chain with
  independently landed nonnegative literal factors. Every multiply prefix
  keeps distinct evidence for the target interval divided by its checked
  cumulative product and intersected with the source carrier; zero and one
  produce a true current-prefix obligation without erasing earlier proofs. The
  direct partial-cast root may also feed a finite nonempty left-associated
  same-value-carrier exact-left-shift chain with independently landed in-range
  fixed-native counts whose carriers may differ. Every prefix keeps distinct
  evidence for the target interval shifted right by its checked cumulative
  count and intersected with the source carrier; a cumulative count at least
  the target width admits only the zero root.
  One direct fixed-native parameter may now also root a finite left-associated
  same-carrier affine chain that contains both an exact add/subtract offset and
  an exact multiply. Every right sibling is an independently landed
  same-carrier literal, multiply factors are nonnegative, and every ordered
  prefix retains independent evidence. The verifier replays each prefix as
  `A * parameter + B` with checked nonnegative `A` and checked signed `B`, then
  derives the carrier preimage; constant prefixes are true or false from `B`
  alone. A later zero factor or offset cancellation cannot erase an earlier
  proof. The same unified mixed affine chain may now feed one validator-legal
  partial fixed-native exact cast. The cast independently reconstructs the
  target interval through `(A, B)` and intersects it with the source carrier;
  `A == 0` decides only the cast from target representability of `B`, while all
  earlier arithmetic-prefix evidence remains mandatory.
  The converse unified family is now retained as well: one validator-legal
  partial fixed-native exact cast of a direct parameter may root a finite
  nonempty same-target-carrier affine chain containing both offset and multiply
  operations. The verifier independently reconstructs every prefix through
  checked `(A, B)` composition and the target/source interval intersection;
  `A == 0` decides only the current prefix from target representability of `B`,
  while cast and earlier arithmetic evidence remain mandatory.
  The direct partial-cast root may now also feed a finite nonempty
  left-associated same-value-carrier exact-right-shift chain with independently
  landed heterogeneous legal counts. The cast proof remains independent, and
  every shift prefix is reconstructed from only its own `0 <= count < width`
  fact without cumulative count, value-definition, or evidence import.
  The same direct partial-cast root may now feed a finite nonempty
  left-associated same-target-carrier exact-divide/remainder chain. Every
  prefix retains independent evidence derived only from its own landed safe
  divisor; cast evidence, prior operation proofs, value definitions, and
  quotient/remainder algebra supply no authority.
  The direct-root and post-cast exact-divide/remainder chain families now share
  one runtime-divisor widening when at least one right sibling is a direct
  same-carrier machine parameter. Every runtime divisor retains an independent
  positive or at-most-`-2` proposition. The joint signed `-1` exception remains
  restricted to the first direct-root operation when its dividend bound is
  independently available; computed and post-cast dividends import no prior
  proof authority. Literal-only chains remain on their existing paths.
  One direct machine-parameter root may now feed any finite left-associated
  same-carrier chain containing both exact-left and exact-right shifts, with
  independently landed heterogeneous legal counts. Every left prefix maps its
  carrier-tight safe interval backward through all prior canonical mixed-shift
  definitions, intersecting the carrier after each inverse left or right step;
  every right proof remains its own legal-count proposition. No prior shift
  proof supplies authority, so later right shifts cannot erase unsafe prefixes.
  The same finite mixed exact-shift chain may now feed one validator-legal
  partial fixed-native exact cast. The cast starts from the target/source
  carrier intersection and independently maps that interval backward through
  every ordered mixed-shift definition; mathematical emptiness reconstructs
  falsehood, while checked interval-arithmetic failure admits nothing. Every
  shift prefix and the cast retain separate mandatory evidence.
  Conversely, one validator-legal direct partial fixed-native cast may root a
  finite nonempty same-target-carrier chain containing both exact-left and
  exact-right shifts. Each left prefix independently replays the ordered
  canonical post-cast definitions back to the cast, intersects its surviving
  target interval with the source carrier, and reconstructs source-root bounds;
  the cast and every shift retain separate evidence. Mathematical emptiness is
  falsehood, while checked transfer failure admits no family.
  A unified finite exact-arithmetic prefix may now feed a finite exact-shift
  suffix on the same fixed-native carrier when the arithmetic prefix is a
  left-associated add/subtract/nonnegative-multiply literal chain and the
  suffix contains at least one exact-left shift. Every left prefix maps its
  safe interval backward through prior shifts and the checked affine form
  `A * root + B`; every arithmetic operation, count, and left prefix retains
  independent evidence. `A == 0` decides only the current left obligation,
  mathematical emptiness is falsehood, and checked replay failure admits no
  family.
  Conversely, a finite exact-shift prefix may now feed a finite exact-
  arithmetic suffix on the same fixed-native carrier. Each arithmetic prefix
  maps the carrier backward through checked `A * shifted_root + B`, then
  replays the complete ordered shift prefix to the direct root. Every count,
  left overflow, and arithmetic obligation remains independently mandatory;
  `A == 0` decides only the current proposition after full shape validation,
  mathematical emptiness is falsehood, and checked replay failure admits no
  family.
  A unified finite affine/cast/affine sandwich may now cross one validator-legal
  partial fixed-native exact cast. Both sides are nonempty left-associated
  add/subtract/nonnegative-multiply literal chains. The cast independently maps
  its target/source interval through the checked source form; every target
  prefix maps the target carrier through its own checked form, intersects with
  the source carrier, then maps through the complete source form. Every source
  arithmetic, cast, and target arithmetic obligation remains independently
  mandatory. A zero coefficient on either side decides only the current
  proposition after full ordered shape validation; mathematical emptiness is
  falsehood and checked replay failure admits no family.
  A unified finite exact-shift/cast/exact-shift sandwich may likewise cross one
  validator-legal partial fixed-native cast, with nonempty left-associated
  shift chains on both sides and independently landed heterogeneous legal
  counts. Every source shift, cast, and target shift keeps separate mandatory
  evidence. Each target-left prefix replays its target definitions to the cast,
  intersects the surviving interval with the source carrier, then replays the
  complete source chain to the direct parameter. Mathematical emptiness is
  falsehood; checked transfer failure admits no family.
  The two heterogeneous affine/shift cast sandwiches are retained as one
  consolidated family. A nonempty source affine chain may cross one partial
  fixed-native exact cast into a nonempty target shift chain, or a nonempty
  source shift chain may cross the cast into a nonempty target affine chain.
  Each side uses its established landed-literal/count rules and ordered
  canonical replay. Every source operation, the cast, and every target
  operation keeps separate mandatory evidence; zero coefficients decide only
  the current obligation after full shape validation. Mathematical emptiness
  is falsehood, while checked composition or interval-transfer failure admits
  no family. Empty-sided shapes remain on their narrower existing paths.
  A consolidated divide/remainder cross-cast family now covers all four
  compositions between one nonempty landed-literal exact-divide/remainder
  chain and one nonempty affine or shift chain. When divide/remainder precedes
  the cast, the existing carrier-total quotient/remainder hull must fit the
  target carrier; each target prefix is true when that full hull lies inside
  its reconstructed safe interval, false when disjoint, and otherwise remains
  unadmitted rather than inventing a guard-sensitive nonconvex preimage. In the
  converse direction, the source affine or shift chain and cast reconstruct by
  their existing rules while every target divisor proof stays independent.
  Every source operation, cast, and target operation retains separate evidence.
  Landed affine-sibling custody is now canonical: `IntegerAffineWitness`
  carries one position-aligned optional earlier semantic-equality index per
  definition, proof-bundle v19 encodes it, and independent producer/verifier
  selection accepts only one exact same-carrier Value-to-signed-literal landing
  before the affine definition. Kernel replay records landing then definition
  and rejects missing, late, redirected, ambiguous, mistyped, or unused
  custody. A source-to-Terminal/verifier/codec mutation regression is green.
  That prerequisite now supports one bounded affine-to-partial-cast exact
  divide/remainder composition. Production follows the unique nonempty cast
  spine to its non-cast source, remaps the canonical endpoint into that carrier,
  and invokes the finite affine selector only on axioms before the first cast;
  the proof is exactly `IntegerCastBound(IntegerAffineBound(...))`.
  Reconstruction independently repeats source, endpoint, prefix-boundary,
  affine, and cast checks. Real divide and remainder source pass codec,
  mutation rejection, verification, and interpretation. Direct/literal/fixed-
  alias precedence is unchanged. The bounded dual now also admits one directly
  cited same-carrier source bound, a unique nonempty partial-cast spine, and one
  strictly later finite affine word for exact divide/remainder. Production
  constructs `IntegerAffineBound(IntegerCastBound(Assumption))`; reconstruction
  independently repeats direct custody, cast/remap, strict post-cast boundary,
  and affine checks. Real divide/remainder source passes v19 codec, mutation,
  verification, interpretation, and existing host-native exact-divide gates.
  One exact forward affine/cast/affine sibling now also composes a directly
  cited root bound through one finite pre-cast affine word, the unique nonempty
  cast spine, and one strictly post-cast affine word, producing
  `IntegerAffineBound(IntegerCastBound(IntegerAffineBound(Assumption)))`.
  Reconstruction independently replays both witnesses, strict boundaries,
  endpoint conversion, and cast custody; real divide/remainder source and
  host-native gates are green. No inverse/alias search, schema, trust-status,
  or fixed-frontier change is made. Shift/cast, joins, correlated, and broader
  affine/cast shapes remain trusted, with fully-derived false unchanged.
  The next landed-literal affine/cast/affine sibling stops at source
  integration: contract equality fails checked transitive-plan construction,
  while branch-local equality is not materialized as a canonical pre-site
  Value-to-literal row before proof selection. Its producer/verifier prototype
  was fully removed. Shift conversion separately lacks a ruled overflow/
  preimage proposition and versioned bound proof; correlated conversion still
  lacks dedicated proposition authority.
  The corresponding direct same-carrier family now retains all four nonempty
  divide/remainder-to-affine/shift compositions without a cast. A leading
  divide/remainder chain supplies its complete verifier-owned carrier hull to
  each following affine or left-shift safe interval: containment is true,
  disjointness is false, and partial overlap remains unadmitted. In the
  converse direction, affine or shift proofs use their established direct-root
  reconstruction while each following divide/remainder proof depends only on
  its own landed safe divisor. Every operation retains independent evidence.
  A finite nonempty exact-divide/remainder chain may also cross one
  validator-legal partial fixed-native exact cast into another finite nonempty
  exact-divide/remainder chain. The cast replays the complete carrier-total
  source hull and is admitted only when that hull wholly fits the target
  carrier; it does not manufacture a partial-overlap or falsehood case. Every
  source divisor proposition, the cast, and every target divisor proposition
  keeps separate mandatory evidence, and each target operation depends only on
  its own independently landed safe divisor.
  The three homogeneous exact-multiply placements—direct, feeding one partial
  exact cast, and rooted at one direct partial exact cast—also admit finite
  signed-carrier chains containing at least one negative independently landed
  factor. A checked sign/magnitude cumulative product reverses negative
  preimages, handles the signed minimum without host negation, and keeps zero
  local to only the current proposition. Every multiply prefix and cast
  remains independently mandatory; mathematical emptiness is falsehood while
  checked product or interval failure admits no family.
  A separate signed-affine family now covers three placements: a direct
  signed-carrier chain, the same chain feeding one partial fixed-native exact
  cast, and one direct partial cast feeding the chain. Each chain is finite,
  nonempty, left-associated, rooted at one direct machine parameter, contains
  at least one add/subtract offset and at least one negative multiply, and has
  an independently landed same-carrier literal on every right edge. The
  verifier replays every shrinking prefix as checked sign/magnitude
  `A * root + B`; a negative `A` reverses the interval preimage, `MIN` never
  uses host negation, and `A == 0` decides only the current obligation. Every
  arithmetic prefix and cast retains independent evidence. Mathematical empty
  preimages are falsehood, while checked coefficient, offset, division, or
  interval failure admits no family. Homogeneous signed products,
  nonnegative-affine chains, two-sided sandwiches, and conversion-chain forms
  remain on their existing or fenced paths.
  A consolidated two-sided signed-affine sandwich now crosses exactly one
  validator-legal partial cast between signed fixed-native carriers. The
  source and target are finite nonempty left-associated add/subtract/multiply
  chains with independently landed same-carrier signed literals. Either the
  source itself contains an offset and a negative multiply, permitting any
  target affine prefix, or the source remains on the established nonnegative
  affine algebra and the current target prefix contains both an offset and a
  negative multiply. The verifier replays checked sign/magnitude `(As, Bs)`
  and `(At, Bt)`, reverses either negative preimage, intersects the exact cast
  carriers, and reconstructs only the current target obligation. Zero on
  either side remains local after full shape validation; every source prefix,
  cast, and target prefix keeps separate evidence. Mathematical emptiness is
  falsehood, while checked coefficient, offset, division, or interval failure
  admits no family. The all-nonnegative sandwich, one-sided signed-affine and
  homogeneous signed-product paths, thin product/offset permutations, and
  conversion-spine forms retain their existing priority or fence.
  A finite chain of at least two validator-legal partial fixed-native exact
  casts may now start at one direct integer machine parameter. For each cast
  prefix, the verifier follows only ordered shrinking cast definitions and
  intersects the root carrier with every carrier reached so far. The resulting
  canonical root bounds prove only that cast; every earlier cast retains its
  own mandatory evidence. A mathematical empty intersection is falsehood,
  while malformed definitions or carrier reconstruction failure admit no
  family.
  The same finite cast core may now follow one nonempty already-admitted
  computed prefix: landed-literal affine arithmetic (including the homogeneous
  signed-product path), an exact-shift chain, or a carrier-total exact-
  divide/remainder chain. For every cast prefix, the verifier intersects every
  carrier reached so far and reuses only that computed family's verifier-owned
  inverse algebra to reconstruct the direct root. Every computed-prefix and
  cast obligation remains independently evidenced. Empty affine/product/shift
  preimages are falsehood, checked replay failure admits no family, and the
  divide/remainder hull remains admissible only by complete containment.
  Conversely, a finite chain of at least two partial exact casts may feed one
  nonempty already-admitted target-carrier affine, homogeneous signed-product,
  exact-shift, or landed-safe-literal divide/remainder suffix. The verifier
  first validates and intersects the complete ordered cast chain without
  importing any cast evidence, then reuses only the selected post-cast
  family's existing inverse algebra for the current suffix prefix. Every cast
  and suffix operation remains independently evidenced; mathematical empty
  preimages are falsehood and checked replay failure admits no family.
  The two directions compose into one unified nonempty computed-prefix,
  at-least-two-partial-cast, nonempty computed-suffix sandwich across the same
  affine, homogeneous signed-product, exact-shift, and carrier-total landed-
  divisor families. Each source prefix, each shrinking cast prefix, and each
  target prefix is reconstructed independently from ordered canonical
  definitions. The verifier intersects every cast carrier, applies only the
  selected target inverse and source inverse/hull algebra, and never imports
  another operation's evidence. Zero coefficients remain local to the current
  target obligation; a mathematical empty interval is falsehood, while
  malformed shape or checked transfer failure admits no family.
  A separate wider-arithmetic composition now permits one nonempty admitted
  computed prefix to pass through a finite nonempty chain of strict valid
  fixed-native integer widenings and feed one nonempty admitted computed
  suffix. Both sides independently select affine, homogeneous signed-product,
  exact-shift, or landed-safe-literal divide/remainder algebra. Every widening
  definition is retained and validated in order; each target interval pulls
  back through numeric-identity widening by intersecting the source carrier,
  then reuses only the selected source inverse or carrier-total hull. Every
  exact operation retains independent evidence. Mathematical emptiness is
  falsehood, divide/remainder partial overlap and checked replay failure admit
  no family, and zero coefficients remain local to the current obligation.
  A heterogeneous conversion-spine sandwich now composes the same nonempty
  computed prefixes and suffixes across a finite contiguous word containing at
  least one strict valid fixed-native integer widening and at least one
  validator-legal partial fixed-native exact cast. Every adjacent carrier and
  shrinking definition is validated in order. Each cast prefix independently
  intersects all preceding conversion carriers and replays only the selected
  source inverse or complete-hull algebra; each target prefix walks the entire
  conversion word before the same source replay. Widenings remain retained
  numeric-identity operations without invented evidence, while every source
  operation, partial cast, and target operation keeps separate evidence. Pure
  widening, pure cast, one-edge, direct, and narrower sandwich shapes retain
  their existing dispatch priority. Mathematical emptiness is falsehood,
  source divide/remainder casts require complete hull containment without a
  partial or falsehood admission, and checked replay failure admits no family.
  A same-root affine fork/join now admits one exact add or subtract whose two
  operands are disjoint, nonempty, independently admitted landed-literal
  affine branches over the same fixed-native carrier and the exact same direct
  machine parameter. The verifier replays each branch separately as checked
  sign/magnitude `Al * root + Bl` and `Ar * root + Br`, then reconstructs the
  join from `(Al + Ar, Bl + Br)` or `(Al - Ar, Bl - Br)`. A zero combined
  coefficient decides only the join after both complete ordered branch walks;
  every operation in both branches and the join retains separate evidence.
  Mathematical empty preimages are falsehood, while checked coefficient,
  offset, division, or definition-walk failure admits no family. The branch
  definition walks must be disjoint and source ordered apart from their common
  root. Distinct-root joins, one empty side, outer operations other than add or
  subtract, conversions, runtime siblings, locals, members, calls, effects,
  and stale or redirected definitions remain fenced.
  A distinct-root signature-bounded affine fork/join separately admits the
  same outer exact add or subtract when the two disjoint, nonempty, ordered
  landed-literal affine branches end at different direct machine parameters
  of one fixed-native carrier. For each root, the verifier selects only the
  tightest landed unary lower and upper bounds appended by its signature,
  intersects them with the carrier, and maps the resulting interval forward
  through that branch's checked signed affine form. The join range is the
  Minkowski sum or difference of the two independent branch ranges. Complete
  containment in the join carrier yields the canonical conjunction of the
  selected root bounds; a wholly disjoint range yields falsehood; partial
  overlap admits no family. Relational cross-root premises, absent or
  one-sided unary bounds, shared roots, overlapping or reordered branch walks,
  computed roots, carrier drift, conversions, and unchecked arithmetic remain
  fenced. Every operation in both branches and the join retains independent
  evidence.
  A same-root signature-bounded signed affine quadratic product join now
  admits one outer exact multiply whose two disjoint, nonempty, ordered
  landed-literal affine branches end at the same direct signed fixed-native
  parameter with nonzero coefficients. The verifier selects the tightest
  landed unary lower and upper signature bounds, composes the correlated
  integer quadratic, and evaluates its exact discrete range at both endpoints
  and the in-range floor/ceiling lattice points adjacent to the rational
  vertex. Complete carrier containment yields the canonical two-bound
  conjunction; a wholly disjoint range yields falsehood; partial overlap or
  checked coefficient, vertex, or evaluation failure admits no family. Every
  branch operation and the outer multiply retains separate evidence. Constant
  collapse, distinct roots, relational premises, one-sided bounds, unsigned
  carriers, malformed walks, computed roots, conversions, and stale evidence
  remain fenced.
  A same-root signature-bounded signed affine divide/remainder safety join now
  admits one outer exact divide or remainder whose two disjoint, nonempty,
  ordered landed-literal affine branches end at the same direct signed
  fixed-native parameter with nonzero coefficients. The verifier selects the
  tightest landed unary lower and upper signature bounds and solves the exact
  integer-lattice equations for divisor zero and divisor `-1`. A `-1` root is
  forbidden only when the correlated dividend evaluates to the carrier
  minimum at that exact root. No forbidden root emits the canonical two-bound
  conjunction; forbidden roots covering the whole integer interval emit
  falsehood; a partially unsafe interval or checked equation/evaluation
  failure admits no family. Every branch operation and the outer divide or
  remainder retains separate evidence. Distinct roots, constant collapse,
  relational premises, one-sided bounds, unsigned carriers, malformed walks,
  computed roots, conversions, and stale evidence remain fenced.
  A distinct-root signature-bounded signed affine product join now admits one
  outer exact multiply whose two disjoint, nonempty, ordered landed-literal
  affine branches end at different direct signed fixed-native parameters. The
  verifier requires and selects the tightest landed unary lower and upper
  signature bounds for both roots, maps each interval forward through its
  checked signed affine form, and takes the exact hull of all four rectangle
  corner products. Complete carrier containment yields the canonical
  four-bound conjunction; a wholly disjoint hull yields falsehood; partial
  overlap or checked corner overflow admits no family. Every branch operation
  and the outer multiply retains separate evidence. Same-root quadratic
  correlation, relational premises, one-sided bounds, unsigned carriers,
  overlapping walks, computed roots, conversions, and stale definitions remain
  fenced.

  Next engineering frontiers are other proof-bearing results feeding another
  proof-bearing operation, wider multivariate and other computed-sibling joins,
  other computed exact-cast and wider exact-arithmetic
  premises, member/comparison mixtures, calls and effects, wider partial-value
  cleanup, nested ownership, returned transfer, loops, suspension, scoped
  ordering, and ranked tail recursion. Dynamic/nested indexing, wider
  projections and signatures, content-bearing splits, and unsupported contracts
  remain fail-closed until independently verifier-owned.

  Retire checked/source-tree consumers with each slice. Nothing below terminal
  Psi may depend on typed/source trees, `ExpressionHandle`, source rendering, or
  an Omega-to-Psi bridge. Partition replay binds the exact operation and
  verifier-selected callee guarantee; fingerprints are identity, never
  authority.
- **CRASH-CONTRACT.** Extend guarded implication beyond the accepted acyclic
  scalar slice. Direct and staged calls retain invocation-specific substitutions
  and verifier-reconstructed continuations. Canonical Boolean and fixed-integer
  member paths rebase across whole-root, fixed-index, and all-field-projected
  structural calls. The proposition carrier covers Boolean composition,
  relevant-record equality, fixed-width bitwise terms, policy-distinct integer
  arithmetic, evidence-bounded division/remainder, and exact or wrapping shifts;
  codecs, verification, fuel, and interpretation reject missing or redirected
  premises. Genuinely empty-record equality normalizes to the existing Boolean
  constant carrier through calls and terminal verification; all-erased records
  remain distinct and fenced. The distinct `addr` carrier is also explicitly
  excluded from both direct structural-member predicates and whole-record leaf
  expansion, and Terminal lowering rejects the retained source contract rather
  than encoding address equality as fixed-integer evidence. Built-in IEEE
  `f32`/`f64` equality now retains one atomic, format-annotated proposition per
  relevant structural leaf, including whole-record expansion and projected-call
  rebasing. Direct structural-field `!=` uses the same atomic carrier with an
  explicit comparison kind. The verifier resolves both exact paths and formats
  independently; the carrier preserves IEEE NaN and signed-zero behavior rather
  than laundering either operator through mathematical `Equal`. Whole-record
  float `!=` canonically negates the already-sorted equality conjunction as
  `P -> Falsehood`; projected calls rebase every leaf below that implication.
  Aggregate equality now also retains byte-sequence fields as one content atom
  over two nonempty canonical structural paths. The checked and Terminal
  carriers distinguish borrowed views from bounded owned storage (including
  the exact owned capacity) without admitting native pointer/descriptor layout
  into semantic identity; equality itself is live length plus the exact live
  byte prefix, never pointer, capacity, or unused-byte equality. Both roots are
  independently resolved and rebased through structural calls. Borrowed
  `&[u8] in Domain` and bounded `[u8; N] in Domain` fields participate in
  synthesized `Equatable` record equality, while literals and direct text `!=`
  remain outside this slice. Payload-less sums now retain a closed structural
  sum shape with exact case identities. Intrinsic `==` lowers to the flat
  canonical conjunction of both membership implications for every declared
  case; `!=` is that equality proposition implying falsehood. The verifier
  independently resolves both structural subjects and every case identity.
  Payload-bearing pure sums now retain exact case-payload field identities and
  direct relevant Boolean, fixed-integer, IEEE, and byte-sequence leaf types.
  Their intrinsic `==` is the canonical disjunction of per-case conjunctions:
  matching membership for both roots plus that case's exact payload-leaf
  equalities. `!=` is that complete equality proposition implying falsehood.
  The same expansion is path-relative when an acyclic relevant record field
  reaches the sum: the checked and Terminal paths retain every enclosing field
  followed by the exact case and payload-field identities. Direct Unit-call
  rebasing through a sum-bearing projection remains gated with runtime sum
  projection and cleanup. An acyclic relevant record or pure-sum tree directly
  held by a case-payload field now expands its Boolean, fixed-integer, IEEE,
  and byte-sequence leaves transitively too. Checked and Terminal paths preserve
  every exact alternating `Case -> Field(payload)` and record-field segment
  through nested sums; whole-root Unit calls independently rebase both operands,
  and codec, verifier, fixed-fuel, and interpreter replay retain both `==` and
  `!=`. Unknown or redirected case/field identities reject independently.
  Direct whole-root mixed shapes now retain both their common fields and closed
  case roster. Equality canonically conjoins the common-field leaf equalities
  first with one source-ordered disjunction of per-case membership and payload
  conjunctions; inequality is that complete equality proposition implying
  falsehood. Exact common-field, case, and payload-field identities survive
  whole-root Unit-call rebasing, and semantic codec, verifier, fixed-fuel, and
  interpreter replay reject redirected or reordered structure. The first
  bounded nested rung now admits exactly one acyclic relevant field of a
  whole-root record whose type is that existing mixed shape. Every mixed
  common-field, case-membership, and payload-leaf path is prefixed by the exact
  enclosing field; `!=` retains the complete equality as its falsehood premise,
  and whole-root Unit-call rebasing preserves the same prefixes. Independent
  verification rejects an enclosing-field substitution, while codec,
  interpreter, and fixed-fuel reuse the existing proposition vocabulary.
  The next bounded rung admits exactly two enclosing acyclic relevant record
  fields before that same sole mixed occurrence. Every mixed common-field,
  case-membership, and payload-leaf path retains both field identities in
  order; `==`, `!=`, whole-root Unit-call rebasing, codecs, verification,
  fixed fuel, and interpretation replay the exact `Field -> Field -> Mixed`
  chain. Independent mutation of either enclosing field rejects. The following
  bounded rung admits exactly three enclosing acyclic relevant record fields
  before the same sole mixed occurrence. Every common-field, case-membership,
  and payload-leaf path retains all three field identities in order through
  `==`, `!=`, whole-root Unit-call rebasing, codec, verification, fixed fuel,
  and interpretation; independent mutation of any prefix rejects. A fourth
  bounded rung now admits exactly four enclosing acyclic relevant record fields
  before that sole mixed occurrence. The same complete path, call-rebase,
  codec, verifier, fuel, interpreter, and per-prefix mutation replay covers all
  four ordered field identities. A fifth bounded rung now admits exactly five
  enclosing acyclic relevant record fields before that sole mixed occurrence,
  with the same replay covering all five ordered field identities. Six or more
  enclosing fields, case-payload or mixed-under-mixed placement, two mixed
  sibling fields, direct projected mixed comparisons, recursive cycles,
  address and erased payload equality, and runtime sum layout remain fenced.
  Semantic codec format 33 / vocabulary
  35, proof-bundle v19, and installation-record v40 retain the structural
  shapes, case-payload paths, and proposition. Continue with those fenced
  wider nested/projected mixed, recursive, and erased aggregate cases. Concrete
  machine/state contracts plus domain/data predicates and trait requirement signatures,
  machine-parameter requirements, and root/domain operator contracts now reject
  direct binary and named-float `Trapping` arithmetic plus direct Trapping
  conversions. Comparisons, bitwise inspection, float classification,
  Wrapping/Saturating operations, and non-reserved custom float calls remain
  total; proof expressions do not create crash sites. Wrapping/Saturating
  division and remainder now form in concrete and direct abstract Prop only
  when an independently accepted prior fact proves the divisor interval
  nonzero; carrier-overflow policy does not define division by zero, and the
  fact containing a partial term cannot justify that term's own formation.
  Exact and Saturating shifts in direct abstract Prop now retain the same
  independently-prior `[0, operand_width)` count obligation already enforced
  for concrete machine/state contracts; `Saturating` defines value overflow,
  not an invalid count, while `Wrapping` continues to define every count by
  modulo reduction. A count bound inside the proposition containing the shift
  cannot authorize that shift's formation.
  Exact division and remainder in concrete machine/state and direct abstract
  Prop now retain the catalog's complete primitive-definedness judgment: an
  independently accepted prior fact must exclude a zero divisor and, for
  signed carriers, the `MIN / -1` primitive pair (including remainder's shared
  hardware-definedness edge). A guard in the proposition containing the
  operation supplies no authority for that operation's own formation.
  Explicit fixed-integer/address proof embeddings are production-capable:
  checked `embed(u64)` yields proof `Int` with carrier-range facts. The
  inductive climbing-sum and multiplication-distributivity canaries express
  unbounded theorem arithmetic entirely through `embed`, while the climbing
  false twin still rejects on its intended transition arm. Raw Exact `u64`
  contract arithmetic remains rejected without independent bounds.
  Explicit same-carrier policy-erasure `as` coercions now retain the
  ordinary Exact representability obligation in concrete machine/state Prop
  and across the direct abstract signature form; only independently accepted
  prior `requires` facts discharge it, so the proposition containing an
  operation cannot justify that operation's own formation. Add the per-
  primitive Exact, Wrapping, Saturating, and Trapping denotation bridges;
  compiler-derived Trapping guards remain executable crash-site facts rather
  than predicate effects. Imported crash capsules remain blocked on artifact
  identity and certificate binding.
  Direct-site guard coverage and propagated call-route coverage now reject for
  every published crash interface, including an explicitly authored ceiling on
  a private checked machine. Private machines that omit the interface still
  retain ordinary body inference. Raw guard-entailment tests inspect the
  pre-admission facts through a test-only lowering path, so non-entailment
  coverage cannot weaken production admission.
- **PROOF-CERTIFICATION-BRIDGE.** Emit kernel-checkable certificates from source
  automation. One recursive certificate owns one SCC, cites its ranking and
  well-foundedness evidence once, and proves every internal edge decreases;
  ordinary calls remain contract applications. Normalization cites exact
  conformance/law evidence and preserves transitive trust. Thread recursive
  components and cited laws through the accepted trust record and synopsis.

  Acceptance: perturbing any recursive edge decrease, component
  well-foundedness reference, normalized-law identity, or cited premise
  rejects or changes the recorded trust closure; measured mutual proof
  recursion checks while an unmeasured cycle rejects; an admitted law makes
  every dependent normalization admission-dependent.

  The existing exact unsigned ranked-countdown slice now has a narrower
  verified reporting path. Only its opaque native-ranked verifier authority can
  render deterministic component, rank, bound, guard, and successor-custody
  rows alongside the accepted subtract proof and current trust graph. The row
  names the closed verifier-reconstructed countdown rule and does not claim a
  general recursive-component certificate. Codec round-trip, structural
  mutation, and missing-proof canaries reject before reporting. General proof
  SCC production, well-foundedness propositions, and normalization-law trust
  remain open.
- **SUBJECT-QUALIFIED-ARTIFACT-PROOFS.** Make the settled semantic-subject graph
  enforceable in canonical ledgers, certificates, artifact seals, deployment
  records, replay, and human-facing reports. The verifier reconstructs one
  formal-target-to-canonical-source operational-refinement root and its exact
  observation profile; producers may supply neither. Supporting rows name an
  exact intended mathematical model or global theory and reach the root only
  through explicit checked bridge rows.

  Give every subject and bridge a versioned identity. Retain exact theory/model
  satisfaction, domain- and operation-indexed `embed`/`as` representation
  relations, and the rank-to-operational-edge join used by `terminates by`.
  Exact arithmetic owes no-overflow and rejects without it; Wrapping,
  Saturating, and Trapping use distinct commuting/outcome relations. Add
  positive and negative canaries for swapped models, semantics versions,
  target capsules, bridge directions, arithmetic domains, observation profiles,
  and admissions.

  Begin with exact observation-profile equality as a sound conservative replay
  gate. Define the normative cross-profile relation through checked canonical
  forgetting projections, permitting incomparable profiles and rejecting an
  empty or producer-weakened profile. Keep formal-target-to-silicon evidence as
  a deployment-scoped admission rather than contaminating the reusable artifact
  seal. Render every verdict with source/artifact subjects, semantics versions,
  profile, and disclosed artifact/deployment admissions; never emit an
  unqualified `verified` label.

  The first bounded D39 `TerminalTraceV1` rung is live for internal-only
  modules. Terminal owns a typed version-1 schema, exact semantic-value
  comparison marker, mandatory root input/result schemas, and crash-site rows.
  The standalone canonical profile codec begins with D39's domain separator,
  retains the schema version, exact `TerminalPsiIdentity`, explicit root/crash
  tags and counts, and explicit zero ordinary-event and terminal-external
  counts. Unknown versions or vocabulary, empty roots, unknown tags, malformed
  type/comparison rows, zero module commitments, noncanonical qualification or
  `(machine, block, edge)` crash order, duplicate crash coordinates, padding,
  and nonzero later-row counts reject. The public module-bound acceptance path
  first canonical-validates the Terminal module and computes its identity,
  then compares decoded bytes exactly with verifier-derived root and complete
  crash rows; stale or substituted modules and missing, extra, reordered, or
  duplicate rows reject. The verifier accepts no producer fingerprint or row
  list. Its exhaustive operation classification rejects `BoundaryCall` and
  `PortWrite` rather than silently omitting their required version-1 events.
  This adds no Terminal module field, operation, format, vocabulary, runtime
  outcome, or fuel charge.

  Continue D39 with canonical ordinary-event and terminal-external rows; exact
  runtime semantic value comparison; maximal finite and infinite trace
  refinement; and checked forgetting projections before any cross-profile
  replay. Every new operation classification and any producer-supplied
  weakening must remain fail closed. Carry an explicit checked
  `TerminatesExternally(effect_identity)`-class completion fact and terminal
  invocation form from boundary declaration through checked trees, Terminal
  codec/verifier/interpreter, provider selection, and target emission. Delete
  the interpreter spelling match and backend-only invention as authority once
  that join is live. Keep fixed fuel, compiler diagnostics/artifacts, and
  deployment admissions in their D39-defined consumer/product layers.
- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace the current trusted Rust fusion of
  artifact traversal and algebraic reduction with the settled two-part closure.
  A total low-rung generator consumes canonical terminal-Psi bytes, validates
  the exact structure, directly denotes each primitive operation, and emits one
  ordered canonical ledger of goals plus local premise introductions. Clever
  interval, affine, shift, quadratic, and divide/remainder analysis becomes an
  untrusted certificate producer that proves the unchanged canonical goal.

  First expose the migration state honestly: add exact trust-graph nodes for the
  Rust decoder/verifier, each sufficient-form reduction family, the ledger
  framework, each unproved leaf denotation schema, and each unproved call-
  composition row. Every dependency must resolve
  through an acyclic graph to a registered root with kind, semantic subject,
  digest/version, owner, scope, rationale, and accepting policy; unknown leaves
  reject. Do not encode an uncertified reducer as an admitted program premise.

  Current migration inventory: the canonical proof synopsis now publishes one
  validated source-bound trust graph for the exact Rust decoder, proof kernel,
  verifier, eight sufficient-form reduction families, the current unproved
  ledger framework, 36 closed leaf-schema rows, and four separate call-
  composition rows covering all 40 `OperationKind` variants. Node
  digests bind the exact deciding Rust/specification bytes and explicit
  versions; the graph identity also binds every canonical dependency edge.
  Unknown, cyclic, unreachable, duplicate, malformed-root, and noncanonical
  graphs reject, and the current artifact closure reports `fully-derived false`.
  The first production Rust ledger slice now has one closed
  `psi-terminal-semantics` table covering all 40 operation kinds and preserving
  the 36 leaf / 4 call-composition custody split. Twenty goal-free scalar leaves
  carry explicit result, operand, denotation, goal, fact, crash, fuel, and
  frontier axes and reconstruct their local equations through one generic
  interpreter. Exact lookup rejects missing or duplicate rows. The terminal
  verifier and codec trust graph consume that shared inventory instead of
  maintaining independent operation matches. Structural/effect rows and
  call/control composition remain separate and are not promoted into the
  goal-free scalar table. The second production Rust ledger slice
  now owns a separate exact-unique four-row structural/effect table: byte-
  sequence literal establishment, Boolean field reads, port writes, and trivial
  affine-local establishment keep result, custody, action, external-effect,
  fuel, and place-frontier axes explicit. One generic interpreter emits distinct
  fact, effect, or frontier observations; the verifier consumes its Boolean
  equation instead of reconstructing that row independently. The trust graph
  consumes the same table as 32 scalar-denotation plus four structural/effect
  nodes while preserving the 36 leaf / four call-composition operation-custody
  split.
  The modular verifier source split is also fully rebound into trust identities:
  evidence provenance, integer foundations, proof-bundle custody,
  reconstruction, and substitution bytes can no longer change outside the
  registered verifier/ledger dependency digests.
  The third production Rust ledger slice now owns an exact-unique four-row
  call-composition table. Scalar, structural Unit, structural-scalar, and
  boundary calls
  retain independent target, result, argument, requirement, transfer, outcome,
  crash-route, evidence-lifetime, fuel, and frontier policies. The verifier's
  contract composition moved out of general operation reconstruction into one
  focused table-selected module; existing module validation still proves the
  concrete signature, movement, coverage, substitution, outcome, crash, and
  evidence invariants before composition. Call policy and implementation bytes
  are both bound into the same four call trust nodes.
  The fourth production Rust ledger slice now owns the twelve proof-bearing
  scalar leaves in a separate exact-unique table. Exact cast, left/right shift,
  exact add/subtract/multiply, and exact/wrapping/saturating divide/remainder
  retain declared result and operand shape, direct denotation, one of six
  canonical goal shapes, normal-successor result equation, crash policy, fuel,
  and frontier policy as independent axes. One generic interpreter emits a
  typed canonical-goal carrier and the post-discharge result equation; malformed
  type/row custody rejects before reduction. Artifact reconstruction consumes
  that observation instead of rebuilding twelve result equations. The current
  sufficient-form algorithms remain trusted migration dependencies and select a
  reduced proposition through one isolated dispatcher; neither the table nor
  the dispatcher falsely claims a kernel derivation of the canonical goal. The
  trust graph binds the table to exactly those twelve denotation nodes and the
  dispatcher to every affected reducer.
  Eleven of those Terminal rows now rejoin the settled shared integer-policy
  catalog by exact primitive/domain identity: exact add/subtract/multiply,
  exact divide/remainder, exact left/right shift, and wrapping/saturating
  divide/remainder. Row
  validation derives their existing canonical goal shapes from the catalog's
  formation conditions, so representability, divisor, and shift-count policy
  are no longer a disconnected Terminal authority. Exact cast remains
  explicitly unbound; no policy is inferred past the settled vocabulary.
  The independent Terminal verifier's structural crash-policy validation now
  also obtains exact/wrapping/saturating divide/remainder and exact left/right-shift
  formation conditions directly from the shared catalog before applying its
  own retained-fact safety checks. Exact division still requires both nonzero
  and representability custody, policy division requires nonzero custody, left
  shift requires count and representability custody, and right shift requires
  count custody. Remainder reuses the divisor rules without changing its
  existing toward-zero or signed `MIN / -1`-pair behavior.
  Checked-to-Terminal integer operation emission now allocates formation
  obligations from the shared catalog's nonempty formation-condition rows for
  exact add/subtract/multiply/divide/remainder, exact left/right shift, and
  wrapping/saturating divide/remainder. Goal-free catalog rows allocate none.
  One operation-local allocator replaces repeated policy-specific
  identity arithmetic; obligation identity and Terminal operation shapes are
  unchanged.
  The dedicated concrete/abstract Exact division definedness checker now also
  obtains its nonzero-divisor and signed-result-representability requirements
  from the shared catalog before applying its existing interval/fact analysis.
  Exact remainder now selects the parallel catalog primitive before applying
  the same two-condition hardware-definedness analysis.
  Diagnostic ordering, accepted fact frontier, and rejection behavior are
  unchanged.
  The next bounded proof-calculus parity slice exposes canonical disjunction
  introduction in the production certificate kernel. One
  `DisjunctionIntroduction` node owns exactly one independently checked child
  and one selected canonical arm index; a non-disjunction conclusion, absent or
  out-of-range arm, mismatched child conclusion, stale proof vocabulary, or
  excess proof depth rejects. Canonical proof-bundle v13 assigns rule tag 9.
  The registered proof-calculus root now binds the exact proposition,
  proof-rule, primitive/evidence, and proof-codec definitions, while the Rust
  kernel remains an explicit trusted implementation. Independent Beta/Gamma
  `inl`/`inr` gates agree. This adds certificate capability only: all eight
  sufficient reducers and all unproved semantic rows retain `TrustedJudgment`,
  and terminal codec v18 / installation record v24 remain unchanged.
  The next bounded certificate capability is also complete.
  `NonzeroDivisor` now has an exact fail-closed kernel proposition projection:
  unsigned fixed integers use `1 <= d`, signed fixed integers of at least two
  bits use the ordered disjunction `(d <= -1) OR (1 <= d)`, and signed one-bit
  integers use `d <= -1`; address and mismatched carriers reject. The other
  canonical goal shapes were initially unprojected.
  `ExactShiftCount` now also has an exact fail-closed projection of the settled
  `[0, width)` law. Fixed value/count carriers and exact count identity are
  mandatory; direct literals normalize to `Truth` or `Falsehood`, while
  symbolic counts retain only non-carrier-implied lower and upper bounds in
  canonical order. Carrier-total counts project to `Truth`. Exact right shift
  is the bounded production pilot: complete prior bounds select the unchanged
  canonical goal and the existing untrusted recursive producer emits a kernel-
  checked citation/conjunction proof. This pilot initially retained the trusted
  sufficient-reduction fallback for missing or redirected evidence. The later
  exact-cast/shift/arithmetic closure made `kernel_proposition` total, removed
  production sufficient-reducer authority, and routes exact left-shift result
  representability through producer-serialized, kernel-checked
  `IntegerAffineBound` certificates. Focused integration coverage now locks the
  canonical reconstructed question and accepted-rule report and rejects
  mutated, missing, or stale exact-left-shift evidence. General low-rung ledger
  reconstruction and global composition remain open.
  `IntegerLessOrEqualTransitivity` checks two recursively derived `<=` premises
  with an identical middle and exact outer endpoints, allowing existing
  `d <= -2` evidence plus the closed `-2 <= -1` relation to establish the
  negative disjunct. Proof-bundle v14 assigns rule tag 10; the registered
  calculus is v11 and the Rust kernel v3. This initially landed as capability
  only; the bounded wrapping-divide pilot below is its first production
  consumer.
  The next bounded certificate capability adds
  `IntegerLessOrEqualSubstitution`. Two independently checked children prove
  one integer `<=` relation and one equality; endpoint 0 or 1 selects the left
  or right relation endpoint to replace, the other endpoint must remain exact,
  and either equality orientation is accepted. A non-order relation,
  non-equality evidence, unknown endpoint, changed untouched endpoint, or
  mismatched replacement rejects. Proof-bundle v15 assigns rule tag 11; the
  registered calculus is v12 and the Rust kernel v4. This initially landed as
  capability only; the bounded wrapping-divide pilot below is its first
  production consumer.
  The complete `WrappingIntegerDivide` semantic row now reconstructs the
  canonical `NonzeroDivisor` goal and uses an untrusted, kernel-checked
  certificate producer over only machine requirements and pre-site semantic
  axioms. The producer deterministically prefers the signed negative arm and
  supports exact citation, integer-order transitivity, and literal equality
  substitution. Missing projection or evidence rejects with no operation-result
  self-justification and no fallback to the legacy reducer. The complete
  `WrappingIntegerRemainder` row now uses the same canonical proposition and
  untrusted, kernel-checked prior-fact certificate producer. Reconstruction
  selects the goal solely from the exact operation tag, fails closed without a
  valid certificate, and cannot cite the operation's own result equation. Both
  wrapping divide/remainder rows are now canonical. The complete
  `SaturatingIntegerDivide` row now also uses canonical `NonzeroDivisor`
  reconstruction and the same untrusted, kernel-checked prior-fact certificate
  producer. Signed `MIN / -1` remains total through the saturating denotation,
  so nonzero is the complete precondition; reconstruction is exact-tag selected
  and fails closed without a valid certificate. The complete
  `SaturatingIntegerRemainder` row now uses exact-tag canonical
  `NonzeroDivisor` reconstruction and the same untrusted, kernel-checked
  prior-fact producer. Signed `MIN % -1` remains total with result zero. All
  four wrapping/saturating divide/remainder rows are now canonical. One
  complete family shared by exact divide and exact remainder now also bypasses
  trusted sufficient reduction: when the pre-site semantic ledger lands an
  unsigned nonzero literal divisor, or a signed literal divisor other than
  zero and `-1`, reconstruction selects canonical `ExactDivisionDefined` and
  the existing untrusted recursive producer proves its order arm solely from
  that landing equality and a closed integer relation. The complete signed
  `-1` exceptional family is canonical too when the dividend is independently
  landed as any literal above the carrier minimum; the producer recursively
  composes the exact third disjunct, or the two-conjunct `i1` goal, from both
  landing equalities and closed order. The next complete existing-fact family
  accepts the same landed `-1` divisor when the exact canonical
  `MIN + 1 <= dividend` proposition (`0 <= dividend` for `i1`) is independently
  retained in the machine requirements or pre-site semantic ledger. The
  producer cites that proposition directly as the second recursive premise;
  it does not import a reduced obligation or infer a wider interval. Missing,
  stale, or weaker bounds reject. The next complete one-hop family accepts a
  retained literal lower bound `K <= dividend` when closed same-carrier order
  independently proves `MIN + 1 <= K`. The producer composes that primitive
  relation with the exact prior citation through integer-order transitivity;
  reversed, mistyped, weaker, or wrong-dividend facts reject. Missing or zero
  divisor evidence, or a `-1` divisor without either a nonminimum dividend
  landing or the exact retained bound, rejects these paths. The next complete
  direct safe-divisor family now selects the canonical goal from an exact prior
  `1 <= divisor` proposition for unsigned or signed fixed carriers, or
  `divisor <= -2` for signed widths of at least two. Unsigned certificates cite
  the goal directly; signed
  certificates cite the selected first or second disjunct and wrap it with
  disjunction introduction. The complete signed-width-at-least-two joint family
  now selects the third canonical arm when both `divisor <= -1` and
  `MIN + 1 <= dividend` are independently available through the supported exact
  citation or checked transitivity paths. The producer proves each conjunct,
  constructs their conjunction, and introduces only that ordered disjunct;
  either missing premise or wrong operand identity rejects. The mixed member of
  that family also accepts a retained `divisor <= -1` bound with an independently
  landed nonminimum dividend literal. The producer derives the dividend floor
  only by closed integer order plus substitution of that exact landing equality;
  a minimum or wrong-identity landing rejects. The same complete substitution
  family now accepts exact literal equalities retained as machine requirements,
  not only pre-site semantic landings. The selector checks every same-carrier
  equality for the exact operand, and the producer cites it as an `Assumption`;
  zero-only, minimum-dividend, mistyped, or redirected premises reject. The next
  complete endpoint-transport family pairs an exact retained bound on `K` with
  an independently retained equality connecting `K` to the canonical divisor
  or dividend endpoint in either orientation. The producer cites both children
  under `IntegerLessOrEqualSubstitution`, replacing only that endpoint. Dividend
  transport remains inside the signed joint arm and therefore also requires its
  independent `divisor <= -1` premise. A missing companion bound, unrelated
  equality, weak bound, or changed untouched endpoint rejects. The complete
  signed `i1` transport family may independently transport both conjuncts:
  `Kd <= -1` through `Kd == divisor`, and `0 <= Kn` through `Kn == dividend`.
  Both substitutions and both canonical conjuncts remain mandatory; missing or
  crossed equalities reject. The next complete nested family transports a
  one-hop stronger bound: closed same-carrier order first derives the canonical
  bound on `K` (for example `2 <= K` or signed `K <= -3`), then endpoint
  substitution carries it to the divisor. The producer nests one checked
  transitivity node beneath substitution; weak bounds, missing equalities, or
  wrong endpoints reject. The next complete nested family replaces that
  transitivity node's closed side with a second exact citation: unsigned
  `1 <= M` and `M <= K`, or signed `K <= M` and `M <= -2`, followed by
  `K == divisor`. The producer nests the two-citation transitivity proof beneath
  endpoint substitution in deterministic ledger order. A missing or
  disconnected middle relation, weak signed ceiling, redirected equality, or
  wrong endpoint rejects. The signed joint arm now admits the corresponding
  complete dividend sibling: an exact `divisor <= -1`, plus
  `MIN + 1 <= M`, `M <= K`, and `K == dividend`. The producer constructs the
  ordered conjunction and nests the two dividend-floor citations beneath
  endpoint substitution; a missing or disconnected middle fact cannot prove
  the arm. The complete nested signed-`i1` family transports both mandatory
  conjuncts from two exact citations each: `Kd <= Md`, `Md <= -1`,
  `Kd == divisor`, and `0 <= Mn`, `Mn <= Kn`, `Kn == dividend`. The producer
  emits the ordered conjunction of two transitivity-under-substitution proofs;
  either missing middle relation rejects the whole goal. The signed
  width-at-least-two joint arm is also complete when both conjuncts use direct
  two-citation chains: `divisor <= K`, `K <= -1`, and
  `MIN + 1 <= M`, `M <= dividend`. The producer introduces only arm 2 and
  constructs its ordered conjunction from the two transitivity proofs; a
  missing or disconnected citation rejects the entire arm. A signed `i1`
  divisor fact alone remains
  insufficient because its canonical conjunction also requires the dividend
  premise. The complete retained-bound `i1` family now selects that conjunction
  when both exact prior propositions `divisor <= -1` and `0 <= dividend` are
  independently present; the untrusted producer cites both and composes them
  through conjunction introduction. A missing premise or wrong operand identity
  rejects. One-hop stronger retained safe-divisor facts are now complete too:
  `K <= divisor` is accepted when closed same-carrier order proves `1 <= K`,
  and signed `divisor <= K` is accepted when it proves `K <= -2`. The verifier
  selects only the exact operand identity and the untrusted producer composes
  the canonical arm by integer-order transitivity before disjunction
  introduction. Missing, reversed, weakened, mistyped, or wrong-divisor facts
  reject. The next complete transitive family replaces the closed side of that
  step with a second exact prior citation: `1 <= K` plus `K <= divisor`, or
  signed `divisor <= K` plus `K <= -2`. Reconstruction requires the exact shared
  middle term and operand identity; the producer cites both facts in deterministic
  ledger order under one checked transitivity node. Missing, disconnected,
  reversed, or redirected pairs reject. An exact retained canonical goal is now
  cited directly, and an exact retained canonical arm is introduced at its
  ordered disjunct index. Reconstruction uses the same recursive
  `LessOrEqual`/conjunction/disjunction shape as the producer instead of separate
  safe-divisor and exceptional branches; redirected goals, reordered joint
  conjunctions, or wrong operand identities reject. The operation result is not
  available as proof authority. The current
  proof rules and proof-bundle v18 codec carry the certificates without a
  vocabulary change. All remaining
  exact divide/remainder families stay on trusted sufficient reduction, so
  neither complete row changes trust status. Their exact-defined
  prerequisite is nevertheless canonical and exact: unsigned requires
  `1 <= d`; signed widths at least two require the disjunction of `d <= -2`,
  `1 <= d`, and `(d <= -1) AND (MIN + 1 <= n)`; `i1` requires
  `(d <= -1) AND (0 <= n)`.
  Address or type mismatch rejects. Existing kernel rules suffice, but the
  producer does not yet materialize canonical certificates for the accepted
  affine and correlated families. Keeping the exact rows trusted is therefore
  an implementation gap, not a language-design blocker. The producer now has
  a kernel-checked recursive compositor for exact prior citations, atomic
  integer-order proofs, conjunctions, and arbitrary ordered disjunctions,
  covering the common certificate spine for the signed three-arm and `i1`
  exact goals. A producer-visible proof-kernel checker now binds signed fixed
  same-carrier affine normalization to a nonempty, strictly ordered set of
  prior semantic-axiom equalities. It independently replays exact
  add/subtract/multiply-by-literal definitions and recomputes checked
  `A * root + B`, rejecting stale, reordered, malformed, ambiguous,
  cross-carrier, non-value-root, target-drifted, or overflowing witnesses. This
  is a common prerequisite for direct definition chains and both affine
  branches used by same-root/correlated analysis, not an order proof or a
  serialized proof rule. A companion producer-visible checker now maps one
  independently established canonical root `<=` proposition through that
  checked affine form. It preserves order for positive coefficients, reverses
  it for negative coefficients, deterministically maps zero coefficients to
  the constant offset, and rejects wrong shapes, checked-arithmetic overflow,
  or an out-of-carrier endpoint. Proof rule `IntegerAffineBound` now performs
  the intentional integration. One recursively checked root-bound child and
  one `IntegerAffineWitness` bind the exact root, target, and strictly ordered
  semantic-axiom definition indices; the kernel rechecks normalization and the
  mapped conclusion, and records every selected definition in accepted premise
  closure. Non-order or wrong-root children, stale/reordered/malformed words,
  target/carrier drift, arithmetic overflow, or a mismatched mapped bound
  reject. Proof-bundle v19 retains tag 12; the registered calculus is v16 and
  the Rust kernel v8, with the affine and cast checkers included in both
  trust-graph source sets. The first bounded producer family now uses the rule
  for one to seven prior signed fixed affine definitions whose exact retained
  root bound maps directly to a canonical safe-divisor arm. Reconstruction and
  production enumerate shortest words first and advance only prefixes accepted by the
  affine witness checker; within each depth, semantic-axiom indices stay
  strictly ordered. The kernel independently checks continuity, algebra, the
  mapped conclusion, and accepted-premise custody. Missing root custody,
  incomplete, reversed, redirected, or stale words, wrong targets, and
  noncanonical mapped arms reject.
  The seven-definition successor changes only that fixed producer-enumeration
  ceiling. Proof vocabulary, codec, calculus, kernel identities, and logical
  and fixed-fuel accounting remain unchanged: the seven source arithmetic
  operations retain their ordinary charges, while certificate replay adds no
  executable units. Root custody may now also use one exact prior landed
  literal or value-alias transport. A typed `root == literal`
  citation substitutes the root into either endpoint of one closed reflexive
  relation; a value alias instead combines one directly cited integer bound at
  the alias endpoint with its independently cited equality. One exact
  two-citation order chain may instead reconstruct the root bound through one
  shared SSA middle under a checked transitivity child. Direct roots remain
  preferred, then landed literals, alias transport, and transitivity; equality
  facts stay in ledger order, while bound and second-leg indexes use their exact
  value endpoint. A missing bound, equality, or order leg, unsafe or mistyped
  literal, identity, non-value, disconnected, redirected, cross-carrier, or
  same-citation join rejects. Three-or-more-alias or three-or-more-leg root
  reconstruction, words of eight or
  more definitions, joins, cast/shift compositions, and correlated results
  remain on trusted reduction; neither complete exact row changes trust and
  `fully-derived false` remains. An exact mapped affine bound may also close to
  the canonical arm through one typed closed-literal order bridge on the
  unchanged target endpoint. A stronger lower bound places the primitive
  bridge before `IntegerAffineBound`; a stronger upper bound places it after.
  Candidate mapping supplies no authority: the kernel rechecks the exact
  affine conclusion and the enclosing transitivity certificate. A nonclosed,
  mistyped, redirected, or weaker bridge rejects, and no variable-endpoint or
  cited-fact search is added. Affine completion now lives in dedicated,
  side-local `affine_custody` modules. Production and reconstruction
  independently own the fixed seven-definition witness frontier, exact mapped
  bound, and optional closed relaxation; no authority is shared. Fixed affine-
  witness candidate enumeration now lives in independent side-local
  `affine_custody/frontier` modules. Production and reconstruction each
  enumerate shortest definition words first, preserve source-ordered semantic-
  axiom indices, advance only prefixes independently accepted by the affine
  witness checker, and stop at the explicit seven-definition ceiling. Candidate
  pruning grants no proof authority: mapped-bound construction, optional closed
  relaxation, and final proof or retained-bound checking remain in each side's
  affine-custody parent. Witness order, rejection behavior, proof shapes, and
  the finite frontier are unchanged. Affine frontier prefix replay now lives in
  paired, side-local `affine_custody/frontier/prefix` modules. Producer and
  reconstruction independently validate each indexed equality row, retain
  left-before-right Value-target precedence, and ask the proof kernel to replay
  the exact accumulated definition word before that prefix advances. Fixed-
  depth frontier expansion, proof shape, rejection behavior, and the
  seven-definition boundary remain unchanged.
  Ordered affine-witness candidates now live in paired, side-local
  `affine_custody/candidates` modules. Producer and reconstruction independently
  require an exact `LessOrEqual` goal, enumerate left-before-right Value targets
  and the existing definition-word frontier, and construct the same
  `IntegerAffineWitness`; root-evidence custody and completion remain in their
  prior side-local authorities. Exact fixed-target completion now lives in
  independent `affine_custody/candidates/fixed` children: each parent builds
  the bounded word catalog once and preserves target-first then word-order
  precedence. Literal alignment, witness shape, callbacks, rejection, and the
  fixed frontier are unchanged.
  Optional affine endpoint relaxation now
  lives in independent side-local `affine_custody/relaxation` modules.
  Production alone maps the checked affine root bound, constructs
  `IntegerAffineBound`, and places one closed primitive bridge on the exact
  unchanged endpoint before final certificate checking; reconstruction
  independently recomputes and kernel-checks the mapped conversion before
  replaying the same closed relation. Direct affine conversion remains
  preferred, while witness selection and final acceptance remain in each
  affine-custody parent. Endpoint orientation, proof shape, nonclosed,
  mistyped, redirected, or weaker rejection, and the single-bridge frontier
  are unchanged. Affine
  evidence selection now lives in dedicated, side-local `affine_selection`
  modules. Production and reconstruction independently preserve the exact
  preference order across direct, literal-landed, fixed one-/two-alias, and
  exactly-two-leg transitive custody before invoking affine completion; no
  generic path search or additional evidence shape is introduced. Cast-
  adjacent selection now uses matching small producer/verifier dispatch
  facades over independent side-local direct, sandwich, and endpoint modules.
  Direct-root evidence remains before the fixed affine/cast/affine family;
  citation order, strict cast boundaries, endpoint conversion, proof shapes,
  rejection, and fixed frontiers remain unchanged, with no authority shared
  across the trust boundary. Direct cast-to-affine candidate enumeration now
  lives in paired, side-local `affine_selection/cast/direct/candidates`
  modules, preserving semantic cast-root order, unique source-spine recovery,
  last-cast identity, and requirement/root-bound order. Resolved completion
  remains in independent side-local `affine_selection/cast/direct/completion`
  modules. Direct-before-sandwich order, assumption identity, endpoint remapping, exact cast and post-
  cast affine proof bytes, strict last-cast rejection, and the fixed frontier
  remain unchanged. Affine/cast/affine candidate enumeration now lives in
  paired, side-local `affine_selection/cast/sandwich/candidates` modules,
  preserving semantic cast-root order, exact source-spine recovery, strict
  first/last-cast identity, requirement order, and left-before-right root-
  endpoint order. Resolved proof completion remains in independent side-local
  `affine_selection/cast/sandwich/completion` modules; mapped-prefix,
  exact-cast, affine-suffix proof shape, strict boundaries, rejection, and the
  fixed frontier remain unchanged. Boundary-aware affine custody now likewise uses
  independent producer/verifier `boundary` modules for strict post-boundary
  completion and `mapped` modules for exact pre-boundary mapping, while the
  parent retains ordinary root completion. Citation order, strict inequalities,
  proof shapes, rejection, and the fixed seven-definition frontier are
  unchanged. Pre-boundary affine-mapping completion now lives in paired, side-
  local `affine_custody/mapped/completion` modules. Producer and reconstruction
  independently enforce strict definition- and literal-axiom boundaries,
  validate every enumerated witness, and construct or replay its exact mapped
  bound; only production materializes and kernel-checks the proof. The `mapped`
  parents retain requested-target and definition-word order, so proof bytes,
  candidate rejection, and the fixed seven-definition frontier remain
  unchanged. Post-boundary affine-custody completion now lives in paired, side-
  local `affine_custody/boundary/completion` modules. Producer and reconstruction
  independently enforce strict definition- and literal-axiom boundaries before
  delegating every eligible witness to ordinary affine custody; only production
  materializes and kernel-checks the proof. The `boundary` parents retain goal-
  target and definition-word order, so proof bytes, candidate rejection, and
  the fixed seven-definition frontier remain unchanged. Direct
  affine-root custody now lives in independent side-local
  `affine_selection/direct` modules. Production alone retains the exact root-
  bound citation and tries its left then right value endpoints before
  constructing affine completion; reconstruction independently scans
  requirements then semantic axioms, tries the same endpoint order, and
  rechecks the retained root bound through affine custody. Direct evidence
  remains preferred before landed literals, one-alias transport, direct and
  alias-substituted two-leg transitivity, and two-alias transport. Citation and
  endpoint order, proof shapes, missing, redirected, or mistyped rejection, and
  every finite evidence frontier are unchanged.
  Source-ordered direct retained affine-bound candidates now live in paired,
  side-local `affine_selection/direct/candidates` modules. Producer and
  reconstruction independently enumerate requirements before semantic axioms,
  exact `LessOrEqual` rows, and left-before-right Value endpoints; only the
  producer retains citation custody. The direct custody completion, proof
  shape, rejection behavior, and fixed search frontier remain unchanged.
  Fixed affine root-alias
  completion now lives in independent side-local `affine_selection/alias`
  modules. Production alone adapts the existing origin-indexed one- and two-
  alias substitution proofs into affine completion; reconstruction
  independently adapts its reconstructed root bounds and rechecks affine
  custody. The common handoff after the distinct bounded one-/two-alias
  selectors now lives in paired, side-local
  `affine_selection/alias/completion` modules; only production carries proof
  nodes into its own affine custody. Direct, landed-literal, one-alias, direct-transitive, alias-
  transitive, then two-alias precedence is unchanged. Equality/citation order
  and distinctness, nested substitution shapes, missing, reused, cyclic, or
  mistyped rejection, and the explicit one-/two-alias frontier remain
  unchanged; no hop parameter or graph search is introduced. Exact two-
  citation affine-chain custody now lives in independent side-local
  `affine_selection/transitive/chains` modules. Production preserves citation
  identities while reconstruction independently retains propositions; each
  enumerates left facts in ledger order, indexes right legs by the exact shared
  value endpoint, and rejects reuse of the same fact before completion. Direct
  transitive affine custody and its fixed one-equality substitution remain
  separate consumers with unchanged precedence, endpoint orientation, proof
  shapes, and rejection behavior. The catalog exposes exactly two legs—no
  depth parameter, recursion, or generalized path search.
  Ordered transitive affine
  right-leg indexes now live in paired, side-local
  `affine_selection/transitive/chains/right_index` modules. Producer and
  reconstruction independently index exact `LessOrEqual` rows by Value left
  endpoint in requirements-before-semantic-axioms order; only the producer
  retains citation custody. Outer-chain traversal, Value-middle eligibility,
  same-row rejection, proof shape, and the fixed two-citation frontier remain
  unchanged.
  Ordered transitive affine left-leg discovery now lives in paired, side-local
  `affine_selection/transitive/chains/left_legs` modules. Producer and
  reconstruction independently traverse requirements before semantic axioms,
  retain exact `LessOrEqual` rows with a Value middle endpoint, and preserve
  producer-only citation custody. `TwoCitationChains` now owns only source-
  ordered joining to the right-leg index and same-row rejection; proof shape
  and the fixed two-citation frontier remain unchanged.
  One-equality
  transitive affine-root custody now lives in independent side-local
  `affine_selection/transitive/alias` modules. Production alone retains the
  equality citation and ordered two-leg citation identities, constructs one
  transitivity child and one endpoint substitution, then invokes affine
  completion; reconstruction independently rechecks the same distinct value
  alias, exact two-leg chain, substituted root bound, and affine custody.
  Direct transitive affine custody remains in each parent. Equality and chain
  order, endpoint precedence, proof shapes, missing, reused, redirected, or
  mistyped rejection, and the fixed two-citation/one-alias frontier are
  unchanged.
  Prior-evidence primitives now live in dedicated, side-local
  `integer_evidence` modules. Production alone owns citation indices and proof
  nodes; reconstruction independently resolves retained integer literals and
  replays closed order. Selectors depend on these leaf helpers without sharing
  authority, changing precedence, or expanding the search frontier. Canonical
  integer coordination now lives in dedicated, side-local `integer_selection`
  modules. Production independently builds the recursive
  Truth/conjunction/disjunction/order proof shape before the public entry
  applies the kernel check; reconstruction independently replays canonical
  proposition shape and fixed bound dispatch. Each preserves its prior
  precedence and finite evidence frontier. Primitive integer-order selection
  now lives in independent side-local `integer_selection/order` modules:
  production alone builds exact-citation, closed-strengthening, and exact
  two-citation transitivity proofs, while reconstruction independently checks
  its retained literal, closed-strengthening, and exact two-fact forms. Fixed
  endpoint substitution likewise lives in independent side-local
  `integer_selection/substitution` modules; each side owns its existing one-
  and two-equality completion without sharing authority. Fixed one-equality
  endpoint substitution now lives in independent side-local
  `integer_selection/substitution/one` modules. Production alone owns citation
  indices and constructs the outer proof; reconstruction independently
  enumerates equalities and rechecks the inner relation. One-before-two
  precedence, orientation, source order, endpoint identity, proof shapes,
  rejection, and fixed frontiers are unchanged. One-substitution
  inner-relation custody now lives in independent side-local
  `integer_selection/substitution/relation` modules. Production alone preserves
  exact or closed-strengthened prior relation, exact two-fact transitivity,
  affine custody, then eligible pure closed relation precedence;
  reconstruction independently rechecks its retained-fact, two-fact, and
  affine forms. Equality orientation, citation identity, endpoint selection,
  and the outer `IntegerLessOrEqualSubstitution` proof remain in each parent.
  The fixed two-equality affine sibling, proof shapes, rejection behavior, and
  finite search frontier are unchanged. Fixed two-equality endpoint
  substitution now lives in independent side-local
  `integer_selection/substitution/two` modules. Production alone retains the
  outer and inner equality citations, proves the final-alias affine relation,
  and nests inner then outer `IntegerLessOrEqualSubstitution` nodes on the
  unchanged endpoint; reconstruction independently rechecks the same three
  distinct same-carrier values and final affine relation. One-equality relation
  custody remains preferred in each parent. Equality order, citation
  identities, endpoint orientation, proof shape, missing, reused, redirected,
  mistyped, or cyclic rejection, and the exact two-equality frontier are
  unchanged; a third alias remains outside. Those established
  `integer_selection/substitution/two` APIs now act as facades over independent
  side-local `two/selection` owners. Outer equality, orientation, distinct
  inner equality, final-alias affine-relation order, exact fact non-reuse,
  endpoint identity, inner-then-outer substitution proof bytes, rejection,
  one-before-two precedence, and the exact two-equality frontier remain
  unchanged. Exact two-fact integer-order
  custody now lives in independent side-local
  `integer_selection/order/transitive` modules. Production alone retains the
  ordered left/right citation identities and constructs one
  `IntegerLessOrEqualTransitivity` proof; reconstruction independently rechecks
  the exact goal-left endpoint, shared middle value, and goal-right endpoint.
  Direct retained relations, closed strengthening, and landed-literal checks
  remain in each order parent. Citation order, proof shape, disconnected or
  missing-leg rejection, and the exact two-fact frontier are unchanged; no
  third leg or generalized path search is introduced. One-bridge closed
  integer-order custody likewise now lives in independent side-local
  `integer_selection/order/closed` modules. Production preserves retained
  citation order and constructs either retained-bound-then-closed-tail or
  closed-head-then-retained-bound transitivity; reconstruction independently
  checks the same endpoint and closed bridge. Exact retained relations remain
  preferred, while landed-literal and exact two-fact custody remain separate.
  Citation order, endpoint orientation, proof shape, nonclosed, mistyped, or
  weaker rejection, and the single-bridge frontier are unchanged. Canonical
  compound-proposition custody now lives in independent side-local
  `integer_selection/logical` modules. Production alone constructs conjunction
  children in source order and selects the first provable disjunction arm;
  reconstruction independently requires every member of a nonempty
  conjunction and accepts the first retained disjunct. Exact retained
  proposition precedence, recursive child dispatch, Truth and atomic-bound
  ownership, arm indices, proof shapes, incomplete or reordered rejection, and
  the finite evidence frontier are unchanged. Exact whole-proposition custody
  now lives in independent side-local `integer_selection/exact` modules.
  Production alone resolves the first exact assumption or semantic axiom in
  ledger order and constructs its origin-indexed citation proof;
  reconstruction independently checks the same requirements-before-semantic-
  axioms retained order. Exact custody remains preferred before Truth, atomic-
  bound dispatch, and compound recursion. Citation origin/index, proposition
  identity, precedence, proof shape, redirected or reordered rejection, and
  the finite evidence frontier are unchanged. Atomic integer-bound custody now
  lives in independent side-local `integer_selection/bound` modules.
  Production alone preserves exact/closed order, exact two-citation
  transitivity, fixed substitution, cast, then affine proof precedence;
  reconstruction independently preserves closed/direct-literal/two-fact,
  substitution, cast, then affine retained-evidence precedence. Exact whole-
  proposition citation remains preferred in each parent, while Truth and
  recursive compound coordination remain separate. Citation identities,
  endpoint orientation, proof shapes, rejection behavior, and every finite
  evidence frontier are unchanged. Canonical proposition-kind dispatch now
  lives in independent side-local `integer_selection/dispatch` modules. After
  exact whole-proposition custody, production alone routes Truth, atomic bounds,
  conjunctions, and ordered disjunctions to their existing proof owners;
  reconstruction independently routes atomic and compound retained-evidence
  checks. Recursive children return through each side's entry facade,
  preserving exact-first selection at every depth. Variant order, arm and
  conjunct order, proof shapes, unsupported-shape rejection, and every finite
  evidence frontier are unchanged. Recursive proposition
  coordination, precedence, ledger citation order, equality orientation,
  endpoint selection, proof shapes, rejection behavior, and the finite search
  frontier are unchanged. Certificate-entry custody now lives
  in dedicated, side-local `certificate_entry` modules. Production exposes a
  selected proof only after the kernel accepts its exact context, goal,
  assumptions, and semantic axioms; reconstruction independently projects the
  canonical scalar goal before retained selection. Invalid projection or
  failed checking yields no authority, and neither side imports the other's
  decision. The producer's 30 certificate regressions and reconstruction's 25
  independent selection regressions now live in side-local `tests` modules.
  Production facades are 35 and 608 lines respectively, while every test name
  and assertion is retained; no proof logic, authority, precedence, or search
  frontier moved between sides. Reconstruction control-flow evidence
  propagation now lives in a side-local `path_facts` module. It alone decodes
  retained condition predicates, binds successor parameters, emits edge
  equalities before rewritten facts, and deduplicates propagated facts. The
  reconstruction parent still owns traversal, merge intersection, and
  certificate selection; this extraction grants no proof authority and changes
  no fact order. Per-operation obligation reconstruction now lives in a
  side-local `operation_facts` module. It preserves the exact goal-free,
  proof-bearing, structural-effect, then call dispatch order; only the
  proof-bearing branch may choose canonical certificate custody or trusted
  sufficient reduction before recording the pre-result axiom snapshot. CFG
  traversal and return intersection remain in the parent, and an unclaimed
  validated operation still fails closed. Exact divide/remainder now compute their
  existing literal-aware trusted reduction before probing retained canonical
  certificates, and an exact `Truth` reduction keeps precedence; nontrivial
  obligations still select canonical certificate custody, while wrapping and
  saturating rows remain unchanged. Terminator custody now lives in a
  side-local `terminator_facts` module. It owns the exact
  Jump/Conditional/return/crash dispatch, successor fact propagation,
  scalar-result equality, nominal-cleanup obligations, structural-return facts,
  and the rule that Crash contributes no normal exit. CFG scheduling and final
  all-return intersection are separately owned below; cleanup order, axiom
  snapshots, and noncanonical cleanup status are unchanged. Immutable machine
  reconstruction context now lives in a side-local `machine_context` module.
  It alone derives the existing path-fact enablement predicate, exact
  value-type proposition context, machine-parameter custody set, and
  block/machine identity indexes. Traversal consumes that read-only context;
  operation and terminator modules retain their independent decision authority,
  and no dispatch, fact, proof, or search order changes. Deterministic machine
  fact flow now lives in a side-local `machine_flow` module. It owns the
  existing sorted-ready topological schedule, per-block all-incoming fact
  intersection, and final all-return fact intersection. The parent retains
  operation-before-terminator traversal; no successor, fact, exit, proof, or
  search order changes. Direct cast-root custody now lives in independent side-
  local `cast_selection/direct` modules. Production alone retains the exact
  root-bound citation and tries its left then right value endpoints before
  checked cast completion; reconstruction independently scans requirements
  then semantic axioms, tries the same endpoint order, and rechecks cast
  custody. Direct retained-bound candidate enumeration now lives in paired,
  side-local `cast_selection/direct/candidates` modules. Producer and
  reconstruction independently preserve assumptions-before-semantic-axioms
  traversal and exact `LessOrEqual` filtering before their existing completion;
  only production carries citation proofs. Non-order goals still reject in each parent, and direct evidence
  remains preferred before landed-literal and fixed alias transport. Citation
  and endpoint order, cast proof shape, missing, redirected, or mistyped
  rejection, and every fixed cast evidence frontier are unchanged. Direct
  affine-to-cast endpoint enumeration now lives in paired, side-local
  `cast_selection/affine/candidates` modules, preserving right-before-left
  endpoint order, value eligibility, and exact target orientation. Resolved
  completion remains in independent side-local
  `cast_selection/affine/completion` modules; unique-source recovery, literal remapping,
  prefix-bounded affine custody, cast proof bytes, rejection order, direct-cast
  precedence, and the fixed frontier are unchanged. Direct
  landed-literal candidate enumeration now lives in paired, side-local
  `cast_selection/literal/candidates` modules. Producer and reconstruction
  independently preserve assumptions-before-semantic-axioms traversal,
  equality orientation, and exact typed value/literal filtering; only
  production carries citation proofs. Completion remains in independent side-
  local `cast_selection/literal/completion` modules: production alone remaps the source endpoint,
  constructs the closed relation and one substitution proof, then completes
  the cast, while reconstruction independently replays the same endpoint,
  closed-order, root-bound, and cast checks. Direct-root, landed-literal, then
  alias precedence, citation order, endpoint orientation, proof shape, unsafe,
  redirected, or mistyped rejection, and the fixed evidence frontier are
  unchanged. Exact integer-cast definition-spine selection now lives in
  independent side-local `cast_custody/chain` modules. Production and
  reconstruction each walk backward from the selected target through exactly
  one retained `IntegerExactCast` definition per value, reject ambiguous or
  reused definitions and failure to reach the exact root, then require the
  recovered semantic-axiom word to be source ordered. Production's cast-
  custody parent still owns target precedence, witness/proof construction, and
  full certificate checking; reconstruction independently owns witness and
  bound-conversion checking. Cast legality, continuity, carrier validation,
  target order, proof shape, rejection behavior, and the finite unique-spine
  frontier are unchanged; no alternate-path, permutation, or generic graph
  search is introduced. Known-root definition recovery and unique non-cast
  source discovery are now separated further into independent side-local
  `cast_custody/chain/definitions` and `cast_custody/chain/source` modules.
  Backward ledger order, ambiguity/reuse/cycle rejection, first-cast identity,
  proof shape, and the finite single-spine frontier remain unchanged. Exact
  integer-cast certificate completion now lives in
  independent side-local `cast_custody/completion` modules. Each consumes only
  its own deterministic exact-cast spine selection. Production alone preserves
  target-endpoint order, constructs the `IntegerCastChainWitness` and
  `IntegerCastBound` proof, and accepts it only after full kernel certificate
  checking; reconstruction independently checks its witness and mapped bound
  conversion. The cast-custody facades retain the existing entry points and
  literal remapping. Root-bound custody, witness indices, target order, proof
  shape, rejection behavior, and the finite unique-spine frontier are
  unchanged; no alternate-path, permutation, or generic graph search is
  introduced. Per-target completion now lives in paired, side-local
  `cast_custody/completion/target` modules. Producer and reconstruction
  independently recover and validate the deterministic exact-cast word for
  every ordered value endpoint and construct or replay its bound conversion;
  only production materializes and kernel-checks `IntegerCastBound`. The
  completion parents retain left-before-right endpoint order and value
  eligibility, so proof bytes, per-target rejection, and the finite unique-
  spine frontier remain unchanged. Exact integer-literal carrier remapping now lives in independent
  side-local `cast_custody/literal` modules. Production and reconstruction each
  resolve the retained literal's exact source carrier and value, apply exact
  integer-cast semantics, and rebuild the target-carrier literal before their
  own direct, alias-landed, or stronger-bound cast completion proceeds. The
  cast-custody facade keeps the same private entry point, while chain selection
  and certificate completion remain separate. Candidate order, endpoint
  orientation, citation and proof shapes, failed or out-of-range conversion
  rejection, and every fixed cast-evidence frontier are unchanged; no generic
  literal or path search is introduced. Cast-
  specific alias transport now lives in independent
  side-local `alias_transport/cast` modules. Production alone constructs the
  closed-strengthening and alias-landed-literal substitution proofs before cast
  completion; reconstruction independently enumerates and rechecks the same
  retained facts. Generic one-/two-alias transport remains in each parent, and
  citation order, endpoint precedence, proof shapes, rejection behavior, and
  the finite search frontier are unchanged. Fixed-depth alias transport now
  lives in independent side-local `alias_transport/one` and
  `alias_transport/two` modules. Production alone retains origin-indexed
  equality and bound citations and constructs respectively one substitution or
  the exact inner-then-outer two-substitution proof; reconstruction
  independently scans and indexes retained facts and rebuilds the same root
  bounds without importing producer authority. The facade retains separate
  named one-/two-alias entry points and exposes no depth parameter, recursion,
  or graph search. Requirements-before-semantic-axioms order, equality
  orientation/distinctness, endpoint-index order, proof shapes, missing,
  reused, cyclic, or mistyped rejection, and both finite frontiers are
  unchanged. Fixed transitive affine evidence
  now lives in independent side-local `affine_selection/transitive` modules.
  Production alone constructs the exact two-citation transitivity proof and
  its optional single equality substitution before affine completion;
  reconstruction independently indexes and rechecks those same retained
  facts. Direct, literal-landed, one-alias, transitive, alias-transitive, then
  two-alias precedence, citation order, rejection behavior, proof shapes, and
  the finite search frontier are unchanged. Affine landed-literal custody now
  lives in independent side-local `affine_selection/literal` modules.
  Production alone constructs the closed reflexive order and one or two exact
  substitutions for direct and fixed one-intermediate-alias literal roots;
  reconstruction independently enumerates and rechecks the same retained
  equalities and typed literals. Direct-bound, literal, one-alias, transitive,
  alias-transitive, then two-alias precedence, citation and endpoint order,
  rejection behavior, proof shapes, and the finite search frontier are
  unchanged. Direct landed-literal affine-root custody now lives in independent
  side-local `affine_selection/literal/direct` modules. Production alone
  preserves exact equality citation origin and orientation, constructs the
  closed reflexive relation plus one endpoint substitution, and completes the
  affine proof; reconstruction independently scans requirements then semantic
  axioms and rechecks the same typed literal, root-bound orientations, and
  affine custody. Direct literal landing remains preferred before the fixed
  one-intermediate-alias sibling. Citation/order orientation, proof shape,
  unsafe, missing, redirected, or mistyped rejection, and both finite literal
  frontiers are unchanged.
  Source-ordered direct landed-literal affine candidates now live in paired,
  side-local `affine_selection/literal/direct/candidates` modules. Producer and
  reconstruction independently enumerate requirements before semantic axioms,
  both equality orientations, and the exact Value/integer carrier eligibility;
  only the producer materializes the retained equality citation. Completion,
  affine custody, proof shape, rejection, and the fixed search frontier remain
  unchanged.
  Direct landed-literal affine completion now lives in
  independent side-local `affine_selection/literal/direct/completion` modules.
  Each parent retains its own requirements-before-semantic-axioms equality
  discovery, citation and orientation order, value-root custody, and typed-
  literal filtering; production alone constructs the closed reflexive
  relation, one endpoint substitution, and affine proof, while reconstruction
  independently rechecks the same two root-bound orientations through affine
  custody. Direct literal landing remains preferred before the fixed one-
  intermediate-alias sibling. Proof shape, endpoint order, unsafe, missing,
  redirected, or mistyped rejection, and both finite literal frontiers are
  unchanged; no recursive alias or generic evidence search is introduced.
  Fixed one-intermediate-alias affine literal custody now lives in
  independent side-local `affine_selection/literal/alias` modules. Production
  alone retains the distinct root-alias and alias-literal citation identities
  and constructs the closed reflexive relation followed by two exact
  substitutions; reconstruction independently rechecks the same typed
  equalities and both root-bound orientations. Direct landed-literal custody
  remains preferred in each parent. Citation order, endpoint orientation,
  nested proof shape, reused, redirected, or mistyped rejection, and the
  single-intermediate-alias frontier are unchanged; no recursive alias search
  is introduced.
  Source-ordered one-alias landed-literal affine candidate catalogs now live in
  paired, side-local `affine_selection/literal/alias/candidates` modules.
  Producer and reconstruction independently index only integer-literal
  equality landings by their exact alias while retaining assumptions-before-
  semantic-axioms outer traversal, equality orientation, inner citation order,
  distinct same-carrier Value checks, and same-row rejection. Completion and
  affine custody remain unchanged; this removes repeated full inner-ledger
  scans without changing proof shape, rejection, or the fixed definition
  frontier.
  Source-ordered one-alias literal landing indexes now live in paired, side-
  local `affine_selection/literal/alias/candidates/landing_index` modules.
  Producer and reconstruction independently index requirements before semantic
  axioms, both equality orientations, exact Value aliases, and integer-literal
  landings; only the producer retains citation custody. Outer root/alias
  traversal, same-row rejection, carrier checks, proof shape, and completion
  precedence remain unchanged.
  One-intermediate-alias affine literal completion now lives in
  independent side-local `affine_selection/literal/alias/completion` modules.
  Each parent retains its own outer-then-inner equality discovery, distinct
  citation/value custody, and typed literal filtering; production alone
  constructs the closed reflexive relation, inner alias substitution, outer
  root substitution, and affine proof, while reconstruction independently
  rechecks the same two root-bound orientations through affine custody. Direct
  landed-literal custody remains preferred. Equality/citation order and
  distinctness, endpoint orientation, nested proof shape, missing, reused,
  redirected, or mistyped rejection, and the fixed one-intermediate-alias
  frontier are unchanged; no recursive alias search is introduced.
  Source-ordered one-alias transitive affine candidate traversal now lives in
  paired, side-local `affine_selection/transitive/alias/candidates` modules.
  Producer and reconstruction independently enumerate assumptions before
  semantic axioms, both equality orientations, and the existing ordered exact
  two-citation chains; the producer alone materializes citation proofs, while
  reconstruction retains proposition references only. Completion, affine
  custody, proof shape, rejection, and the fixed non-recursive frontier remain
  unchanged.
  One-alias transitive affine candidate selection now uses paired side-local
  stateless functions rather than one-shot candidate structs. Producer and
  reconstruction each build their own ordered two-citation index once per
  invocation, then independently scan assumptions or requirements before
  semantic axioms and left-before-right equality orientation; only production
  retains citation proofs. Equality distinctness, exact shared-middle chain
  order, citation/proof shape, alias completion, rejection behavior, and the
  fixed one-alias/two-citation frontier remain unchanged.
  Two-citation affine chain catalogs now retain only their reusable side-local
  right-leg indexes and source slices; their non-indexed left-leg scans are
  paired stateless functions rather than one-shot slice-holder structs.
  Production independently preserves citation-bearing assumptions-before-
  semantic-axioms traversal, while reconstruction independently preserves
  requirements-before-semantic-axioms traversal. Exact shared-middle lookup,
  same-fact rejection, chain reuse across alias candidates, completion order,
  proof shapes, and the fixed two-citation frontier remain unchanged.
  Source-ordered oriented equality catalogs now live at the paired side-local
  affine-selection boundary and are reused by direct/one-alias landed-literal
  selection and one-alias transitive selection. Production independently
  retains citation custody while reconstruction independently retains
  propositions; both preserve assumptions or requirements before semantic
  axioms and left-before-right orientation. Literal landing indexes, equality
  distinctness, direct-before-alias precedence, two-citation chain order, proof
  shapes, rejection behavior, and every fixed affine frontier remain unchanged.
  Exact affine root/alias eligibility now lives at paired side-local affine-
  selection boundaries and is reused by direct/one-alias landed-literal
  selection and one-alias transitive selection. Producer and reconstruction
  independently require distinct exact `Value` roots/aliases; literal
  selection retains its additional exact carrier and landed-integer checks.
  Equality/citation order, same-fact rejection, direct-before-alias precedence,
  proof shapes, completion rejection, and every fixed affine frontier remain
  unchanged.
  Ordered affine `Value`-endpoint eligibility now lives in paired side-local
  affine-selection authorities and is reused by direct retained-bound
  candidates and direct two-citation completion. Producer and reconstruction
  independently retain left-before-right endpoint order and skip non-`Value`
  endpoints before their distinct custody/proposition handoffs. Citation
  order, root-bound construction, proof cloning and shapes, completion
  precedence, rejection behavior, and the fixed affine frontier remain
  unchanged.
  Exact affine `Value`-term eligibility now lives in paired side-local affine-
  selection authorities and is reused by ordered root endpoints, distinct
  root/alias checks, direct literal binding, literal landing indexes, and two-
  citation left/right-leg admission. Production and reconstruction retain
  independent citation-bearing versus proposition-only indexes and scans.
  Requirements/assumptions-before-semantic-axioms order, left-before-right
  orientation, literal/type checks, same-fact rejection, proof shapes,
  completion precedence, rejection behavior, and all fixed affine frontiers
  remain unchanged.
  Landed-integer type recognition and distinct retained-fact identity now live
  in paired side-local affine-selection eligibility authorities. Literal
  landing indexes reuse the exact integer-literal classifier, while one-alias
  literal joins and two-citation chains reuse the exact nonidentity predicate;
  producer retains citation proof custody and reconstruction independently
  retains propositions. Ledger/orientation order, carrier checks, same-fact
  rejection, proof shapes, completion precedence, and all fixed affine
  frontiers remain unchanged.
  Retained affine `LessOrEqual` enumeration now lives in paired side-local
  ordered-bound catalogs. Producer selection still derives citation custody
  from assumptions before semantic axioms, while reconstruction independently
  enumerates retained propositions in the same order; direct endpoint
  candidates and bounded two-citation left/right indexes reuse those
  authorities without changing value eligibility, direct-before-transitive
  precedence, proof shape, same-fact rejection, or any fixed affine frontier.
  Fixed two-citation affine-chain authorities now retain the already-validated
  outer endpoints beside each ordered left/right fact. Certificate production
  converts the exact two citations into proof nodes inside its own chain
  authority, while reconstruction independently exposes the corresponding
  retained endpoints; direct-transitive and one-alias-transitive completions no
  longer rematch propositions downstream. Citation/source order, same-fact
  rejection, direct-before-alias precedence, proof shapes, and the fixed two-
  citation frontier remain unchanged.
  Exact typed value-to-integer-literal bindings now live in paired side-local
  affine equality authorities. Direct landed-literal selection and one-alias
  landing indexes reuse the same source- and orientation-ordered eligible
  stream; certificate production independently retains citation custody while
  reconstruction retains propositions, and the alias join continues to reject
  reuse of one equality as both legs. Direct-before-alias precedence,
  root/literal carrier equality, proof shapes, rejection behavior, and the
  fixed one-intermediate-alias frontier remain unchanged.
  Exact value-to-integer-literal carrier recognition is now private to the
  paired affine equality authorities that own those ordered binding catalogs.
  Generic affine eligibility no longer exposes literal-specific helpers;
  producer and reconstruction still classify bindings independently, and
  source/orientation order, direct-before-alias precedence, same-fact
  rejection, proof shapes, and fixed frontiers remain unchanged.
  Distinct value-to-value alias orientations now live in paired side-local
  affine equality catalogs shared by literal landing and transitive
  substitution. Producer selection independently retains equality citation
  custody while reconstruction independently retains propositions; literal
  aliases still require the exact same carrier, transitive aliases still must
  match one reconstructed endpoint, and source/orientation order, same-fact
  rejection, proof shapes, precedence, and fixed frontiers remain unchanged.
  Left-before-right `Value` endpoint enumeration now belongs to the paired
  side-local affine bound authorities and is reused by direct retained-bound
  selection and fixed two-citation completion. Producer and reconstruction
  still enumerate independently; source/citation order, endpoint precedence,
  root custody, proof shapes, rejection behavior, and every fixed affine
  frontier remain unchanged.
  One-alias affine-literal landing indexes now own the exact indexed inner-row
  join. Both sides independently reject reuse of the outer equality as the
  landing row; production alone converts the selected inner citation into its
  proof before completion, while reconstruction retains the matching
  proposition. Outer equality/source order, root same-carrier validation,
  nested outer-then-inner proof shape, direct-before-alias precedence,
  rejection behavior, and the fixed one-intermediate-alias frontier remain
  unchanged.
  Those landing indexes now own the complete indexed join: exact root/alias
  carrier agreement, outer-versus-inner row nonidentity, and the selected
  literal. Production independently converts both selected equality citations
  into the existing outer-then-inner proof pair inside its join authority,
  while reconstruction retains the matching propositions. Outer alias source/
  orientation order, completion precedence, nested substitution shape,
  rejection behavior, and the fixed one-intermediate-alias frontier remain
  unchanged.
  Affine `Value` classification and retained-row identity now have separate
  paired side-local authorities. Bound catalogs own `Value` admission for
  ordered root endpoints and fixed two-citation legs, while fact-identity
  modules independently reject row reuse in two-citation chains and one-alias
  literal joins. Producer retains citation/proof custody and reconstruction
  retains propositions; traversal order, proof shapes, rejection behavior,
  precedence, and every fixed frontier remain unchanged.
  Affine bound authorities now expose exact source-ordered left-`Value` and
  right-`Value` row streams for the fixed two-citation chain. Right-leg indexes
  and left-leg scans consume those side-local streams without revalidating
  endpoints; production retains citation-bearing assumptions-before-axioms
  enumeration and reconstruction independently retains proposition-only
  requirements-before-axioms enumeration. Shared-middle order, row
  nonidentity, proof shapes, completion precedence, rejection behavior, and
  the fixed two-leg frontier remain unchanged.
  Direct affine retained-bound selection now owns its exact evidence handoff.
  Production converts only the selected origin-indexed citation into a proof
  node before completion, while reconstruction independently passes the
  retained proposition to its custody replay. Assumptions/requirements-before-
  axioms enumeration, left-before-right `Value` endpoints, root custody, proof
  shape, rejection, direct precedence, and the fixed definition frontier
  remain unchanged.
  Direct affine-selection parents now own the final side-local custody handoff
  after their selectors produce completion-ready evidence. Production passes
  its independently constructed cited proof directly to affine custody, while
  reconstruction passes its independently retained proposition; the former
  pass-through completion modules are removed. Source/citation order, left-
  before-right endpoints, proof shape, direct precedence, rejection behavior,
  and the fixed definition frontier remain unchanged.
  Producer-side one-alias transitive affine candidate traversal now short-
  circuits directly through its source-ordered `Value`-alias and fixed two-
  citation catalogs. Equality citation custody and the ordered left/right proof
  pair remain inside the selected callback, while reconstruction independently
  retains its proposition-only short-circuit traversal. Equality/chain order,
  proof shape, alias completion, rejection, precedence, and the fixed one-
  alias/two-leg frontier remain unchanged.
  Producer-side direct affine candidates and fixed-chain left legs now short-
  circuit directly through their exact source-ordered bound streams. Direct
  selection preserves bound-before-left/right endpoint order and selected
  citation proof construction; chain selection preserves right-`Value` leg
  order and its indexed join custody. Reconstruction retains its independent
  proposition-only short-circuit traversals, with proof shapes, rejection,
  precedence, and fixed frontiers unchanged.
  Fixed affine indexed joins now short-circuit directly through their retained
  ordered slices. Two-citation chain authorities independently reject reuse of
  the left row before completing against each indexed right leg, while the
  producer alone materializes the accepted citation proof pair; the producer
  literal-landing index likewise rejects outer/inner row reuse before
  materializing its existing proof pair. Shared-middle and alias lookup order,
  carrier checks, proof shapes, completion precedence, rejection behavior, and
  the fixed two-leg/one-alias frontiers remain unchanged.
  Ordered affine goal-target eligibility now lives in paired, side-local
  `affine_custody/candidates/targets` modules. Producer and reconstruction
  independently require a `LessOrEqual` goal, retain left-before-right endpoint
  order, and admit only exact `Value` targets. Candidate parents retain source-
  ordered definition-word enumeration and independent proof completion, so
  witness order, proof shape, rejection behavior, and the fixed seven-definition
  frontier remain unchanged.
  Affine witness candidate authorities now independently build one invocation-
  local fixed definition-word catalog after confirming an eligible goal target,
  then reuse that exact source-ordered catalog across left-before-right `Value`
  targets. Producer and reconstruction retain separate catalogs and completion
  logic; invalid goals still reject before frontier replay, while witness order,
  kernel checking, proof shapes, rejection behavior, and the seven-definition
  frontier remain unchanged.
  One-layer affine frontier expansion now lives in paired, side-local
  `affine_custody/frontier/layer` modules. Producer and reconstruction
  independently retain each prefix word, next admissible source index, and
  current exact `Value` endpoint; each layer advances through its own ordered
  definition index and invokes its own kernel-checked prefix replay. Frontier
  parents retain the exact seven-layer limit and accumulated word order, so
  candidate order, witness/proof shape, rejection behavior, and the fixed
  frontier remain unchanged.
  Affine-definition input projection now lives in paired, side-local
  `affine_custody/definition_index/candidates/inputs` modules. Producer and
  reconstruction independently preserve exact add/multiply left-before-right
  input order, subtract-left-only projection, and unsupported-operation
  rejection; parent catalogs retain equality orientation, source order,
  `Value` eligibility, and index insertion. Proof replay, witness shape,
  rejection behavior, and the fixed seven-definition frontier remain unchanged.
  Affine-definition equality orientation now lives in paired, side-local
  `affine_custody/definition_index/candidates/orientations` modules. Producer
  and reconstruction independently require an equality with an exact `Value`
  target and preserve left-target before right-target expression order; source-
  row traversal, affine input projection, input `Value` eligibility, and index
  insertion remain in their existing owners. A mirrored accepted regression
  now pins the reversed equality orientation. Witness order, proof shape,
  rejection behavior, and the fixed seven-definition frontier remain unchanged.
  Affine-definition input owners now complete operand eligibility locally.
  Producer and reconstruction independently project the supported exact add,
  multiply, and subtract inputs and admit only `Value` operands before
  returning their ordered streams; parent catalogs now solely retain semantic-
  row traversal, oriented expression selection, and index recording. Input
  order, witness/proof shape, rejection behavior, and the fixed seven-definition
  frontier remain unchanged.
  Ordered affine-prefix target projection now lives in paired, side-local
  `affine_custody/frontier/prefix/targets` modules. Producer and reconstruction
  independently require the indexed definition to remain an equality and
  enumerate only its `Value` endpoints left before right; prefix parents retain
  independent witness construction and proof-kernel replay. Definition-word
  order, proof shape, rejection behavior, and the fixed seven-layer frontier
  remain unchanged.
  Unique earlier affine-sibling literal-landing discovery now lives in paired,
  side-local `affine_custody/frontier/prefix/literals/landing` modules. Producer
  and reconstruction independently validate and scan only the semantic-axiom
  prefix before the current definition, preserve source-row order and both
  equality orientations, and require exactly one same-carrier `Value`-to-
  signed-literal match. Their `literals` parents retain definition-word replay,
  arithmetic-step orientation, sibling position, and target completion.
  Witness bytes, missing/late/redirected/ambiguous rejection, and the fixed
  seven-definition frontier remain unchanged.
  Landed affine-sibling definition-step decoding now lives in paired, side-local
  `affine_custody/frontier/prefix/literals/step` modules. Producer and
  reconstruction independently require an exact same-carrier `Value` target,
  accept only exact integer add/subtract/multiply, preserve left-operand
  precedence, and permit the right operand only for commutative add/multiply.
  Their `literals` parents retain definition-word traversal, unique landing
  alignment, equality orientation, and final-target completion. Witness bytes,
  arithmetic orientation, rejection, and the fixed seven-definition frontier
  remain unchanged.
  Source-ordered `Value`-keyed affine candidate storage now lives in paired,
  side-local `affine_selection/value_index` modules and is reused by literal-
  landing and two-citation right-leg catalogs. Producer and reconstruction
  independently retain their citation-bearing versus proposition-only
  payloads; the storage owner preserves per-`Value` insertion order and empty-
  miss behavior, while catalog owners retain carrier checks, row identity,
  proof construction, and completion. Source order, proof shapes, rejection
  behavior, precedence, and both fixed frontiers remain unchanged.
  Ordered affine-definition index recording now lives in paired, side-local
  `affine_custody/definition_index/recording` modules. Producer and
  reconstruction independently consume their syntactic candidate streams,
  preserve source-row order, and adjacent-deduplicate repeated inputs from the
  same row before constructing their immutable `Value`-to-definition maps.
  Query behavior, prefix replay, witness/proof shape, rejection, and the fixed
  seven-definition frontier remain unchanged.
  Affine-definition recording owners now retain the complete invocation-local
  index carrier, ordered candidate insertion, adjacent-row deduplication, and
  empty-miss query behavior. Producer and reconstruction expose the unchanged
  side-local `DefinitionIndex` path through narrow re-exports, while syntactic
  discovery and prefix replay remain independently implemented. Source order,
  witness/proof shape, rejection behavior, and the fixed seven-definition
  frontier remain unchanged.
  Exact affine evidence precedence now lives in paired, side-local
  `affine_selection/dispatch` modules. Producer and reconstruction
  independently retain direct bound, landed literal, one-alias, direct two-
  citation, one-alias two-citation, then two-alias order; entry modules remain
  responsible for constructing their invocation-local definition indexes.
  Evidence custody, proof shapes, rejection behavior, and every fixed alias,
  citation, and definition frontier remain unchanged.
  Start-bounded affine-definition queries now belong to the paired, side-local
  index owners. Producer and reconstruction independently select the exact
  source-ordered suffix with `partition_point`, while frontier layers consume
  that iterator without reaching into raw candidate slices. Prefix replay,
  witness/proof shape, rejection behavior, and the fixed seven-definition
  frontier remain unchanged.
  Affine frontier cursor custody now lives in paired, side-local
  `affine_custody/frontier/layer/entry` modules. Producer and reconstruction
  independently retain each prefix word, next admissible source index, and
  current exact `Value`; cursor fields remain private to the owning layer and
  only root construction is exposed to the frontier parent. Expansion order,
  prefix replay, witness/proof shape, rejection, and the fixed seven-layer
  frontier remain unchanged.
  Affine frontier cursor owners now complete their custody boundary: fields are
  fully private, and producer/reconstruction cursors independently enumerate
  exact start-bounded definition extensions, clone and append each source index,
  and construct accepted successor cursors. Layer parents retain kernel prefix
  replay and accepted-word accumulation. Source order, witness/proof shape,
  rejection behavior, and the fixed seven-layer frontier remain unchanged.
  Affine selection dispatch now expresses its complete fixed precedence as one
  lazy side-local short-circuit chain. Producer and reconstruction independently
  retain direct bound, landed literal, one-alias, direct two-citation, one-alias
  two-citation, then two-alias order without an imperative first-branch special
  case. Evidence custody, proof shapes, rejection behavior, and every fixed
  frontier remain unchanged.
  Fixed affine frontier parents now terminate immediately when an expansion
  layer yields no successor cursors. Producer and reconstruction independently
  preserve every accumulated word and the exact seven-layer ceiling while
  avoiding redundant empty-layer allocation on rejected or shorter chains.
  Source order, prefix replay, witness/proof shape, and rejection behavior
  remain unchanged.
  Fixed affine frontier ceilings no longer materialize unusable successor
  cursors after the fourth accepted definition layer. Producer and
  reconstruction independently preserve the same source-ordered prefix replay
  and accumulated definition words, while final-layer acceptance moves each
  word directly into the catalog instead of cloning it into a dead cursor.
  Proof shapes, rejection behavior, and the exact seven-definition frontier
  remain unchanged; the measured 5.45s versus 5.44s mixed-shift hotspot shows
  this is allocation cleanup, not a material end-to-end speedup.
  One-equality transitive affine completion now lives in independent side-local
  `affine_selection/transitive/alias/completion` modules. Each parent retains
  its own ledger-ordered equality discovery, distinct root/alias custody, and
  exact two-citation chain enumeration; production alone constructs the
  transitivity child, one endpoint substitution, and affine proof, while
  reconstruction independently maps the same root-bound endpoint and rechecks
  affine custody. Direct transitive affine custody remains preferred. Equality
  and chain order, endpoint orientation, citation identity, nested proof shape,
  missing, reused, redirected, or mistyped rejection, and the fixed two-
  citation/one-alias frontier are unchanged; no generalized path or alias
  search is introduced. Direct two-citation affine completion now lives in
  independent side-local `affine_selection/transitive/completion` modules.
  Each parent retains its own exact ordered `TwoCitationChains` enumeration and
  citation custody; production alone constructs the
  `IntegerLessOrEqualTransitivity` child, tries the left then right value root,
  and completes the affine proof, while reconstruction independently rebuilds
  the same retained root bound, endpoint order, and affine custody. The fixed
  one-equality alias sibling and all outer precedence remain unchanged.
  Citation identity/order, shared-middle continuity, proof shape, missing,
  reused, disconnected, or mistyped rejection, and the exact two-leg frontier
  are unchanged; no longer path, permutation, or generic graph search is
  introduced. A
  single exact prior value equality may also transport a completed affine bound
  from its checked target alias to the canonical goal endpoint. The producer
  replaces that one endpoint, constructs the bounded affine relation directly,
  and wraps it in `IntegerLessOrEqualSubstitution`; reconstruction repeats the
  same exact identity selection. A missing, redirected, crossed, or mistyped
  target equality rejects. The affine relation builder cannot recurse into
  another target alias, so this adds one wrapper only and no alias-chain search.
  One fixed sibling may instead carry a completed affine bound across exactly
  two distinct same-carrier target equalities. It nests two
  `IntegerLessOrEqualSubstitution` nodes outside `IntegerAffineBound`; missing,
  reused, redirected, cyclic, or mistyped equalities reject. The constructor
  builds the affine relation directly at the final alias and never recurses
  through the general order prover, so a third target alias remains outside the
  family.
  One bounded mixed root-custody sibling may instead compose exactly two prior
  order citations at an alias endpoint, transport that completed bound through
  exactly one retained value equality to the affine root, and then apply
  `IntegerAffineBound`. Its proof nests `IntegerLessOrEqualTransitivity` beneath
  `IntegerLessOrEqualSubstitution`; missing or disconnected order legs and
  absent or redirected equalities reject. The constructor calls the affine
  builder directly, so it cannot add another equality or order leg and does not
  introduce recursive path search. Three-or-more-alias and three-or-more-leg
  custody remain outside the producer. One fixed two-alias sibling may instead
  transport one directly cited bound to the affine root through exactly two
  distinct retained value equalities. Its proof nests two
  `IntegerLessOrEqualSubstitution` nodes beneath `IntegerAffineBound`; the root,
  middle alias, and bound alias must be distinct same-carrier values. A missing,
  reused, redirected, crossed, cyclic, or mistyped equality rejects. The
  constructor has no recursive alias walk, and a third alias remains outside
  the producer. One literal-ending sibling may land the affine root through
  exactly one intermediate value alias and one exact same-carrier literal
  equality. It proves a closed reflexive integer order, substitutes the alias,
  substitutes the root, and only then applies `IntegerAffineBound`. Missing,
  redirected, reused, or mistyped equalities reject, and a second value alias
  is not followed. This is another fixed two-substitution path, not a recursive
  alias search. A second non-serialized common checker now
  normalizes the contiguous pure
  fixed-integer cast spine used by the accepted one-cast and multi-cast
  sandwiches. It binds strictly ordered canonical semantic equalities to exact
  root/target SSA values, validates every adjacent partial 8/16/32/64
  `IntegerExactCast`, retains all selected indices and carriers, and computes
  their exact surviving root-range intersection. Identity, widening-shaped,
  address, non-native, reversed, stale, reordered, discontinuous, cyclic, and
  target-drifted words reject; narrowing and cross-sign edges claim only their
  representable intersection, never total or lossy conversion. The checker
  accepts no proof authority, does not establish machine-parameter custody or
  surrounding prefix/suffix algebra, and leaves heterogeneous widening/cast
  words separate. `IntegerCastBound` is the versioned integration for that
  core. One recursively checked root-bound child and one nonempty contiguous
  word of partial fixed-native exact-cast definitions map the same mathematical
  literal endpoint into the final carrier. The kernel rechecks the complete
  cast witness and conversion and records every selected definition in accepted
  premise closure. A non-order or wrong-root child, empty, stale, reordered,
  discontinuous, total/widening-shaped, or cyclic cast definitions,
  target/orientation drift, or a changed endpoint reject. Proof-bundle v18
  retains tag 13; the producer and reconstruction independently follow the
  unique exact-cast SSA definition spine backward from the goal, reject
  ambiguous target definitions, and require its source-ordered ledger word.
  They perform no recursive path or permutation search. Cast-chain custody now
  lives in dedicated, side-local `cast_custody` modules. Production and
  reconstruction independently own unique-spine selection, exact
  witness/kernel replay, and final `IntegerCastBound` completion; the broader
  evidence selectors retain their existing order and proof shapes. Cast
  evidence selection now lives in dedicated, side-local `cast_selection`
  modules. Production and reconstruction independently preserve direct-bound,
  landed-literal, fixed one-alias, closed-strengthening,
  alias-landed-literal, then fixed two-alias precedence; source-carrier literal
  remapping remains with cast custody. No proof shape or search frontier
  changes. Direct landed-literal cast custody now lives in independent
  side-local `cast_selection/literal` modules. Production alone constructs the
  closed source-carrier relation and exact equality substitution before
  `IntegerCastBound`; reconstruction independently remaps the target endpoint
  and rechecks the same typed literal landing. Existing direct-bound,
  direct-literal, one-alias, stronger-alias, alias-literal, then two-alias
  precedence, citation orientation, endpoint order, rejection behavior, and
  the finite frontier are unchanged. This completes contiguous cast-chain
  custody for exact divide/remainder goals. Fixed cast alias-family dispatch
  now lives in independent side-local `cast_selection/alias` modules.
  Production alone constructs one-alias, closed-strengthened alias,
  alias-landed-literal, then two-alias proofs before cast completion;
  reconstruction independently enumerates and rechecks those fixed families.
  Closed-strengthened and alias-landed-literal transport are further separated
  into paired `alias_transport/cast/stronger` and `cast/literal` modules. Each
  producer constructs only its exact closed bridge/substitution proof, while
  reconstruction independently enumerates and rechecks the same typed facts;
  the cast-alias parent is now a small facade over those authorities. Stronger
  cast-alias completion now lives in independent side-local
  `alias_transport/cast/stronger/completion` modules. Each parent retains its
  own ledger-ordered exact equality and bound candidate discovery; production
  alone remaps the typed source endpoint, constructs the one closed
  transitivity bridge and root substitution, then completes the cast proof,
  while reconstruction independently replays the same carrier, endpoint,
  bridge, root-bound, and cast checks. Equality/bound citation order,
  orientation precedence, proof shapes, redirected, mistyped, or nonclosed
  rejection, and the single-alias/single-bridge frontier are unchanged.
  Closed-strengthened cast-alias transport now separates ledger-ordered fact
  discovery from completion in independent side-local
  `alias_transport/cast/stronger/candidates` modules. Equality-first,
  orientation-second, bound-third order, exact citation identity,
  carrier/endpoint eligibility, closed bridge and substitution proof bytes,
  rejection, family precedence, and the single-alias/single-bridge frontier
  are unchanged.
  Stronger alias-bound endpoint eligibility now lives in paired, side-local
  `alias_transport/cast/stronger/candidates/bound` modules. Producer and
  reconstruction independently require the selected alias at the left endpoint
  before the right fallback, decode the opposite endpoint as a fixed-integer
  literal, and require its carrier to match the root. Candidate parents retain
  equality-first, orientation-second, and bound-third citation order.
  Completion inputs, proof bytes, rejection, cast-family precedence, and the
  fixed single-alias/single-bridge frontier remain unchanged.
  Closed stronger alias-bound transport for exact casts now lives in
  independent side-local
  `alias_transport/cast/stronger/completion/bound` modules. Each completion
  parent retains exact goal/target projection, literal carrier remapping, and
  cast-custody completion, while its outer parent retains ledger-ordered
  equality and bound discovery. Production alone constructs the closed bridge,
  one `IntegerLessOrEqualTransitivity` child, and one endpoint substitution;
  reconstruction independently checks the same closed relation and rebuilds
  the same root-bound proposition. Citation order, endpoint orientation, nested
  proof shape, weaker, nonclosed, redirected, or mistyped rejection, and the
  fixed one-alias/one-bridge frontier are unchanged; no recursive strengthening
  or generic search is introduced.
  Direct landed-literal root-bound construction for exact casts now lives in
  independent side-local `cast_selection/literal/completion/bound` modules.
  Each completion parent retains exact goal/target precedence, literal carrier
  remapping, and cast-custody completion, while its outer parent retains
  requirements-before-semantic-axioms equality discovery. Production alone
  constructs the closed relation and one endpoint substitution;
  reconstruction independently checks the same closed relation and rebuilds
  the resulting root-bound proposition. Citation and endpoint order, proof
  shape, unsafe/missing/redirected/mistyped rejection, and the fixed direct-
  literal frontier are unchanged; no recursive alias or generic search is
  introduced.
  Direct retained root-bound completion for exact casts now lives in
  independent side-local `cast_selection/direct/completion` modules. Each
  parent retains requirements-before-semantic-axioms retained-order discovery
  and exact citation/proposition custody; production completion preserves
  left-then-right value-root order and applies its own citation proof before
  cast custody, while reconstruction independently preserves the same endpoint
  order and rechecks the retained proposition through cast custody. Direct
  evidence remains first in cast selection. Citation order, endpoint order,
  proof shape, non-order/non-value/missing/redirected rejection, and the fixed
  direct-root frontier are unchanged; no generic evidence search is introduced.
  Fixed two-alias exact-cast completion now lives in independent side-local
  `cast_selection/alias/two` modules. Each child adapts only its side's existing
  exact two-alias transport to cast custody: production retains origin-indexed
  equality/bound citations and the nested two-substitution proof, while
  reconstruction independently replays retained facts and the resulting root
  bound. The alias-family parents preserve direct one-alias, stronger-bound,
  landed-literal, then two-alias precedence. Citation identity/order, equality
  orientation/distinctness, proof shape, missing/reused/cyclic/redirected/
  mistyped rejection, and the exact two-alias frontier are unchanged; no third
  alias, depth parameter, recursion, or graph search is introduced.
  Fixed one-alias exact-cast completion now lives in independent side-local
  `cast_selection/alias/one` modules. Each child adapts only its side's existing
  exact one-alias transport to cast custody: production retains origin-indexed
  equality and bound citations plus the single endpoint-substitution proof,
  while reconstruction independently replays retained facts and the resulting
  root bound. The alias-family parent preserves one-alias, stronger-bound,
  landed-literal, then two-alias precedence. Citation order, equality
  orientation, endpoint order, proof shape, missing/redirected/mistyped
  rejection, and the exact one-alias frontier are unchanged; no depth
  parameter, recursion, or graph search is introduced.
  Fixed two-alias transport now places ledger/index enumeration in independent
  side-local `alias_transport/two/candidates` modules behind unchanged callback
  APIs. Outer equality, orientation, inner equality, indexed-bound order, exact
  fact non-reuse, citation identity, nested substitution proof bytes, callback/
  rejection order, and the fixed two-alias frontier are unchanged. Fixed two-
  alias bound completion now lives in independent side-local
  `alias_transport/two/completion` modules shared only through each side's own
  private facade. The producer parent retains origin-indexed outer/inner
  equality citations, distinct same-carrier value custody, cycle/reuse
  rejection, and endpoint-indexed relation discovery, then its completion alone
  nests inner followed by outer `IntegerLessOrEqualSubstitution` nodes.
  Reconstruction independently retains equality order/distinctness and bound
  indexing, then substitutes the same exact endpoint before invoking its
  consumer. Cast and affine consumers, citation and endpoint order, nested proof
  shape, rejection behavior, and the exact two-alias frontier are unchanged;
  no third alias, recursion, or graph search is introduced.
  Fixed one-alias order-transport candidate enumeration now lives in paired,
  side-local `alias_transport/one/candidates` modules. Producer and
  reconstruction independently retain assumptions-before-semantic-axioms
  traversal, equality orientation, and indexed relation order before endpoint-
  substitution completion; only production materializes citation proofs. The
  existing `alias_transport/one` facades, proof bytes, rejection order, and the
  exact one-alias frontier remain unchanged.
  One-equality endpoint-substitution completion now lives in paired, side-local
  `integer_selection/substitution/one/completion` modules. Producer and
  reconstruction independently choose the matching goal endpoint, rebuild the
  replacement relation through their bounded relation authority, and construct
  or replay the outer substitution; only production carries proof nodes. The
  `substitution/one` parents retain assumptions-before-semantic-axioms citation
  order and equality orientation, so inner-relation precedence, substitution
  proof bytes, rejection, and the fixed one-equality frontier remain unchanged.
  Two-equality endpoint-substitution completion now lives in paired, side-local
  `integer_selection/substitution/two/selection/completion` modules. Producer
  and reconstruction independently rebuild the final-alias affine relation and
  construct or replay the inner-then-outer endpoint substitutions; only
  production carries proof nodes. The `two/selection` parents retain outer
  equality, orientation, inner equality, alias eligibility, and exact fact non-
  reuse, so affine precedence, proof bytes, rejection order, and the fixed two-
  equality frontier remain unchanged.
  Fixed two-equality endpoint-alias eligibility now lives in paired, side-local
  `integer_selection/substitution/two/selection/aliases` modules. Producer and
  reconstruction independently resolve the exact goal endpoint, require
  distinct same-carrier `Value` roots, middle aliases, and target aliases, and
  accept either inner equality orientation. Their `selection` parents retain
  assumptions-before-semantic-axioms fact enumeration, outer orientation order,
  exact fact non-reuse, and completion. Affine precedence, proof bytes,
  rejection order, and the fixed two-equality frontier remain unchanged.
  Fixed one-alias bound completion now lives in independent side-local
  `alias_transport/one/completion` modules. The producer parent retains origin-
  indexed equality and bound citations, same-carrier distinctness, equality
  orientation, and endpoint-indexed relation discovery, then its completion
  alone constructs the single `IntegerLessOrEqualSubstitution` node.
  Reconstruction independently retains equality and bound order and substitutes
  the same exact endpoint before invoking its consumer. Cast and affine
  consumers, citation and endpoint order, proof shape, missing/redirected/
  mistyped rejection, and the exact one-alias frontier are unchanged; no depth
  parameter, recursion, or graph search is introduced.
  Fixed-alias endpoint-bound indexing now lives in independent side-local
  `alias_transport/index/bounds` modules. Production preserves citation origins
  while scanning assumptions then semantic axioms; reconstruction independently
  scans requirements then semantic axioms. Each inserts a value's left endpoint
  before its distinct right endpoint, suppresses the duplicate reflexive right
  entry, preserves per-endpoint ledger order, and uses deterministic `BTreeMap`
  lookup. Value-identity custody and endpoint substitution remain separate in
  each index facade. One-/two-alias candidate order, citation identity, proof
  shapes, rejection behavior, and finite frontiers are unchanged; the index
  grants no proof authority or graph search.
  Direct retained bounds and direct landed literals remain earlier in each
  parent. Citation orientation, endpoint order, proof shapes, rejection
  behavior, and the finite two-alias frontier are unchanged. Alias-landed-
  literal cast transport now separates ledger-ordered fact discovery from
  completion in independent side-local
  `alias_transport/cast/literal/candidates` modules. Root equality,
  orientation, distinct landing equality order, exact fact non-reuse, citation
  identity, carrier eligibility, nested substitution proof bytes, target-
  endpoint order, rejection, family precedence, and the single-alias/single-
  landing frontier are unchanged. Its completion lives in independent side-local
  `alias_transport/cast/literal/completion` modules. Each parent retains its
  own ledger-ordered discovery of distinct root-alias and alias-literal
  equalities; production alone remaps the typed source endpoint, constructs the
  closed relation and nested alias-then-root substitutions, then completes the
  cast proof, while reconstruction independently replays the same carrier,
  endpoint, closed-order, root-bound, and cast checks. Equality citation order
  and distinctness, endpoint precedence, proof shapes, redirected, mistyped,
  or unsafe rejection, and the fixed two-equality frontier are unchanged.
  Landed-literal alias root-bound transport for exact casts now lives in
  independent side-local `alias_transport/cast/literal/completion/bound`
  modules. Each completion parent retains exact target precedence, literal
  carrier remapping, and cast-custody completion, while its outer parent retains
  ledger-ordered root and literal equality discovery. Production alone
  constructs the closed relation, inner literal-to-alias endpoint substitution,
  and outer alias-to-root substitution; reconstruction independently checks the
  same closed relation and rebuilds the resulting root-bound proposition.
  Citation order and distinctness, endpoint orientation, nested proof shape,
  unsafe, missing, reused, redirected, or mistyped rejection, and the fixed
  two-equality frontier are unchanged; no recursive alias or generic search is
  introduced.
  These slices do
  not promote either whole row: affine/cast,
  shift/cast, joins, and correlated results remain trusted-reducer work, and
  `fully-derived false` is unchanged. The root-bound child may now also come
  from exactly one retained same-carrier `root == literal` fact when that
  literal equals or strengthens the canonical bound endpoint. The producer
  remaps the endpoint into the source carrier, checks the closed bridge to the
  landed literal, substitutes the root endpoint once, then applies the cast
  rule; reconstruction independently selects the same exact equality and
  rechecks the bridge. Direct bounds remain preferred. Missing, redirected,
  mistyped, or weaker facts reject. One exact same-carrier `root == alias`
  citation may instead transport one directly cited canonical bound at that
  alias. Its fixed proof nests one `IntegerLessOrEqualSubstitution` under
  `IntegerCastBound`; reconstruction repeats the same exact equality/bound
  selection. Missing, redirected, cross-carrier, or weaker bounds reject.
  Production now routes this one-alias order transport for both cast and affine
  completion through one indexed constructor; reconstruction independently
  mirrors that constructor, so the family is no longer re-enumerated per
  completion rule. One
  closed source-carrier endpoint bridge may also strengthen the cited alias
  bound. Its fixed proof nests `IntegerLessOrEqualTransitivity` under the one
  substitution; exact alias bounds remain preferred. Production and
  reconstruction recheck the same bound, bridge, and equality. They do not
  search alternate bounds or aliases, and a weaker bridge rejects. One fixed
  sibling may instead land that alias through exactly one same-carrier
  `alias == literal` citation. It proves the closed canonical bridge,
  substitutes the alias, substitutes the root, then applies
  `IntegerCastBound`; production and reconstruction select the same two exact
  equalities. Missing, reused,
  redirected, mistyped, or weaker literals reject. One fixed two-alias sibling
  may instead transport one directly cited canonical bound through exactly two
  distinct same-carrier value equalities. It nests two
  `IntegerLessOrEqualSubstitution` nodes under `IntegerCastBound`; production
  and reconstruction independently enumerate that exact three-citation shape
  through their own local indexed constructor shared by cast and affine
  completion. Endpoint-indexed alias-bound custody now lives in independent
  side-local `alias_transport/index` modules. Production preserves ordered
  citation origin, proposition identity, and endpoint orientation for every
  retained bound; reconstruction independently catalogs only the retained
  proposition and endpoint it must recheck. The fixed one- and two-alias
  constructors consume those separate indexes without sharing authority.
  Ledger order, same-carrier identity checks, endpoint substitution,
  citation/proof shapes, rejection behavior, and the finite two-alias frontier
  are unchanged; no hop-count parameter or recursive search is introduced.
  Those fixed one-/two-alias constructors now live in dedicated,
  side-local `alias_transport` modules rather than the broader certificate and
  reconstruction engines. The cast-specific closed strengthening and
  alias-landed-literal shapes live beside them while retaining their distinct
  transitivity and substitution proofs. They prefer every one-alias family and
  perform no recursive or parameterized alias walk. Missing,
  reused, redirected, crossed, cyclic, mistyped, or weaker facts reject. A
  third alias, literal landing through two aliases, affine/cast, shift/cast,
  joins, and correlated results remain outside this sibling; neither complete
  exact row changes trust and `fully-derived false` remains.
  A third
  non-serialized common checker now normalizes the
  complete exact-shift core shared by direct, cast-adjacent, affine-adjacent,
  and divide/remainder-adjacent families. It binds a nonempty, strictly ordered
  word of canonical exact-left/right semantic equalities from one fixed-native
  SSA root to its target. Closed counts require no cited fact; every nonclosed
  count must be landed by an exact earlier canonical equality. Heterogeneous
  fixed-native count carriers are retained, and every mathematical count must
  be nonnegative and less than the value width. The checked form preserves the
  exact direction/count/index word rather than an unsound cumulative summary
  for mixed shifts. The legacy trusted exact-left mixed-chain reducer now
  retains its exact latest-definition and prior nonclosed-count landing
  coordinates, orders them root-to-target, invokes this independent checker,
  and computes its unchanged interval preimage only from checked direction,
  count, carrier, and root custody. Its prior duplicate direction/count
  interpretation is gone. Unsupported carriers, nonexact operations, unlanded,
  late, reversed, mistyped, negative, or out-of-range counts, stale or reordered
  definitions, discontinuity, cycles, and target drift reject. This checker
  consumption promotes no trust: the reducer remains trusted, accepts no proof
  authority, establishes no new root custody, and proves neither left-shift
  overflow safety nor a surrounding interval/preimage claim. Other shift
  families and certificate routing remain unchanged.
  A fourth checker now binds the complete correlated
  forbidden-root family shared by exact divide and remainder. It independently
  replays both nonempty landed-literal affine branches, requires disjoint
  source-ordered definitions ending at the same direct signed fixed-native
  signature parameter with nonzero coefficients, and binds exact prior landing
  facts for nonclosed siblings. It reselects the tightest strict unary signature
  bounds after the definition boundary, requires their exact axiom identities,
  and solves the divisor's zero and `-1` lattice roots. The latter is forbidden
  only when the dividend evaluates to the carrier minimum at that same root.
  No forbidden root yields the canonical ordered two-bound conjunction; roots
  covering the complete interval yield falsehood; partial safety rejects.
  Stale definition, literal, or bound identity; correlation/order/type/root
  drift; constant collapse; one-sided bounds; and checked arithmetic failure
  reject. The untrusted exact divide/remainder producer now deterministically
  retains the exact definition/literal and tight requirement coordinates,
  constructs this complete witness, and serializes proof tag 14. Admission
  reconstructs the semantic-plus-requirement ledger, requires the exact
  semantic boundary and machine-parameter root, invokes the checker, requires
  the empty forbidden-root set, and converts the checked operands to the
  unchanged canonical `ExactDivisionDefined` proposition. Every used
  definition/literal axiom and both requirement bounds enter the accepted
  premise closure. The former trusted correlated reducer and duplicate
  lattice-root/value computation are gone. Proof-bundle format 21 and canonical
  proof-calculus trust root 21 retain the conversion; terminal semantic format
  39, vocabulary 42, proof-system marker 1, semantic-operation schemas, and
  installation encoding are unchanged. Other partial exact rows remain outside
  this slice, so the accepted trust closure remains `fully-derived false`.
  The historical bounded Gamma feasibility spike established that exact
  canonical-byte decoding and ordered semantic-ledger reconstruction fit the
  low rung without making the Rust verifier authoritative. Its final measured
  checkpoint assembled to 4,982 typed Gamma lines / 198,971 bytes / 423
  functions with maximum source nesting 25. Closed row tables eliminated
  per-operation builder branches; Gamma's monomorphic decoder-result types
  caused most remaining repetition. The format-bound implementation was retired
  after its format-18/vocabulary-20 decoder fell behind the live artifact;
  commit `a5cfd83cc` and its follow-ups retain the executable provenance. Its
  reusable structure now lives in production's exact-unique 40-row inventory:
  32 scalar denotations plus four structural/effect rows form 36 leaf rows, and
  four call-composition rows remain a separate algebra with mutation coverage.
  Reusable low-rung byte, scalar/type/value, UTF-8, and structural-leaf grammar
  fragments remain gated without claiming a fixed terminal header or complete
  live decoder. The full assurance-owned low generator, row proofs, and
  composition bridges remain required; the retired spike marks no trust-graph
  dependency derived. The first Rust producer-modularity checkpoint is also
  complete. Structural Unit
  planning no longer owns the shared Boolean/integer convergence classifier
  body: that sufficient-form family and its forty focused tests live in
  dedicated `shared_convergence` modules. The six exact binary families and
  exact-cast family select their existing ordered recognizers through seven
  declarative registries and one generic dispatch path rather than repeated
  `or_else` permutations. The production parent shrank from 10,926 to 5,626
  lines; shared convergence is now a 493-line orchestration module plus four
  responsibility modules for cast chains, affine forms, products/divisors, and
  shifts/cross-family composition, none larger than 1,317 lines. The test
  parent shrank from 10,785 to 2,915 lines, and its forty classifier cases are
  separated into chain, affine-join, and nominal-cleanup modules, none larger
  than 3,411 lines. The remaining structural parent is now a 250-line
  orchestrator over return analysis, control/boundary construction, cleanup,
  call closure, and type/shape custody modules, none larger than 1,356 lines;
  its fifty-three tests are separated into cleanup and call-closure modules
  behind a 57-line test root. The downstream checked-to-terminal producer no
  longer embeds its 3,891-line shared runtime-parameter classifier in the
  23,735-line crate root. That classifier now has a 706-line orchestration and
  registry module over Boolean, conversion, affine, product/divisor, and
  shift/cross-family responsibilities, none larger than 1,083 lines. Six exact
  binary cohorts plus exact cast consume named ordered registries through two
  generic dispatchers rather than maintaining another set of repeated
  `or_else` permutations. Structural scalar-return custody is now separate as
  a 553-line lowering/orchestration module over a 258-line expression-shape
  responsibility and a 1,431-line nominal-cleanup specialization behind one
  parent-facing entry point. The distinct structural Unit control path is a
  601-line module. Structural Unit cleanup is a 733-line nominal
  lowering/orchestration module over separate 828-line ordered-nominal and
  352-line partial-affine responsibilities. Attached Unit closure assembly is
  an 826-line orchestrator over 132-line provider discovery, 203-line exact
  call closure, 487-line type/domain/service catalog, and 168-line parameter
  transfer responsibilities. Result-bearing boundary custody and general
  structural-result transfer are separate 393-line and 314-line modules over a
  shared 246-line structural-type retention responsibility. Scalar-graph
  terminal-module assembly is now a separate 1,061-line responsibility behind
  one parent-facing builder. Content conservation, identity reshuffling, and
  partition composition now form one 788-line lowering module with only three
  public APIs and two explicit internal contracts. The root-level regression
  corpus is now a separately compiled 334-line fixture/orchestration parent over
  isolated 767-line Unit-cleanup, 179-line scalar-graph, 506-line content-ledger,
  957-line structural-control, 457-line attached-Unit, and 852-line
  structural-return families instead of a second responsibility embedded in
  production. The 9,597-line `nominal_affine_source` regression file is now a
  31-line root over five focused responsibilities with all 33 tests retained;
  its remaining 6,238-line integer-comparison case is one atomic cross-layer
  mutation matrix, not a completed decomposition target. Proposition
  vocabulary, evidence-term identity, contract lanes,
  proof-output invocations, and producer provenance now form one 906-line evidence
  module behind a single lower-and-install API. Scalar and structural crash
  routes, checked crash-site/frontier custody, argument-root substitution, and
  canonical proposition construction now form one 1,727-line module with
  eleven explicit internal contracts.
  Terminal operation emission and proof finalization now form one 597-line
  module with five explicit entry points. Short-circuit Boolean decisions and
  terminal control emission now form one 734-line module, while replaceable
  debug-map presentation is a separate 188-line module. Scalar-graph
  preparation, validation, partial evaluation, and lowering now form one
  1,297-line module with fourteen explicit internal contracts. Reachable
  scalar-call discovery and multi-machine assembly now form one 158-line
  module with two explicit entry points. The crate root is now 1,017 lines.
  The verifier's former 9,239-line sufficient-form reconstruction test parent
  is now a 15-line root over fifteen cast, conversion, add/subtract,
  multiply/affine, join, shift, and divide-policy responsibilities. All 76
  cases remain, and no family module exceeds 1,248 lines.
  Terminal proof replay now has the same production boundary. Its former
  2,233-line root is a 256-line public verification orchestrator over a
  44-line canonical proof-bundle model, 1,036-line executable-site/path-fact
  reconstruction, exact evidence-producer provenance (139), integer arithmetic
  foundations (337), and proposition/value/place substitution (494). The
  existing public proof and substitution surfaces remain explicitly
  re-exported, while sufficient-form selection retains its specialized owners
  instead of being merged into one generic permutation dispatcher.
  Exact-shift reduction now follows that same boundary. Its former 2,376-line
  production file is a 237-line precedence/orchestration parent over a
  944-line direct-chain/foundation responsibility and a 1,254-line
  cross-family cast/arithmetic/divide composition responsibility. The existing
  public reducer surface and precedence are unchanged, and the integer-shift
  trust node binds all three exact source files.
  Exact conversion reduction now has the same split. Its former 2,219-line
  file is a 243-line cast-precedence/direct-fallback parent over a 977-line
  conversion-chain and interval-foundation responsibility and a 1,063-line
  divide/product/affine/offset composition responsibility. Existing reducer
  contracts and ordering remain unchanged, and the integer-conversion trust
  node binds every implementation source.
  The checked-lowering regression file that had accumulated ranking,
  operational-contract, write-frame, crash-route, and data-fact verification
  is now a 23-line root over eight exact test families. All 67 tests and the
  shared exact-symbol helper remain, and no family exceeds 3,614 lines.
  Checked-tree visualization has also separated view production from its
  regression corpus: the former 11,465-line file is now a 5,092-line
  production module with a 609-line shared fixture parent over eleven exact
  behavior, content, qualification, carry, and machine-contract test families.
  All 188 embedded tests and 215 test/helper functions remain, and no family
  module exceeds 1,043 lines.
  The checked interpreter now follows the same responsibility boundary. Its
  former 9,938-line evaluator is a 1,205-line state/model parent over separate
  execution, statement/call, wire-codec, host-dispatch, filesystem, console,
  expression/value-call, name/place, cast/recast, record-view, type-metadata,
  scalar-operation, and typed-program-lookup modules. The complete function
  and declaration inventories remain; cross-responsibility collaboration is
  narrowly exposed through `pub(super)`, local helpers remain private, and no
  child responsibility exceeds 1,408 lines. This is a semantics-preserving
  split; exact host-service grant custody remains a separate authority task
  rather than being hidden inside the refactor.
  Target-neutral call validation has begun the same responsibility split. Its
  former 9,103-line `calls.rs` parent is now 1,065 lines. Its 137-line
  `calls/inline_assembly.rs` child owns shared-catalog lookup, source operand
  constraints, and value-producing intrinsic destination checks; the existing
  crate destination query and two parent-private validation seams are
  unchanged. A 112-line `calls/generic_bounds.rs` child owns bodyless-boundary
  executable admission and positional machine type-parameter bound checking.
  A separate 53-line `calls/result_use.rs` leaf owns strict non-Unit result
  consumption and the proof-citation exemption. Both expose one parent-private
  validation entry point and preserve existing diagnostics. Runtime
  recursive-call position checking lives in a 222-line `calls/recursion.rs`
  child; its
  943-line `recursion/proof_machines.rs` child owns proof-machine
  structural/cited decrease validation, substitution matching, guard
  provenance, and sub-state descent closure. The proof validator is its only
  crate-visible export and reuses one parent-private self-call identity check.
  Value-position per-call bound validation and exact diagnostics form a
  748-line `calls/expression_scanning.rs` child. Its 838-line
  `expression_scanning/traversal.rs` child owns source-ordered recursive
  statement/expression scanning, malformed-name checks, and nested-indexed-read
  fences, delegating through one parent-private call-validation seam. A
  separate 222-line `expression_scanning/target_resolution.rs` child owns
  declared-receiver type discovery, lowering-aligned target-channel replay,
  and the fail-closed unresolved-call decision. Existing crate queries are
  unchanged; only type-shell normalization and unresolved-call reporting are
  shared privately back to per-call validation. A separate 273-line
  `expression_scanning/result_realization.rs` child owns the fail-closed
  runtime-result fences for LET-local receivers, nested unmaterialized machine
  calls, and void callees in value position. It exposes the same two
  crate-visible diagnostics plus one parent-private void-callee check; target
  selection, argument validation, diagnostic text, and source order are
  unchanged.
  Complete-or-opaque caller write-frame inference, alias-origin propagation,
  and transition-cycle frame equations now form a 2,921-line
  `calls/write_frames.rs` child. Its 459-line `write_frames/demand.rs` child
  owns the public resolver facade plus expression/statement demand collection
  and conservative fallback; a separate 123-line
  `write_frames/boundary_calls.rs` child owns boundary-trait signature
  resolution and receiver/exclusive-argument write frames. A focused 214-line
  `write_frames/isolation.rs` child owns caller-isolated local/aggregate
  classification, exact struct-literal field/type lookup, and bounded direct-call
  initializer-shape admission through six parent-private predicates; it has no
  callback into frame inference. A separate 52-line
  `write_frames/isolated_initializers.rs` leaf owns complete caller-isolated
  initializer admission, including the symbol-table and isolated-write fences;
  recursive frame collection remains in the parent behind one callback. A
  separate 99-line `write_frames/transparent_effects.rs` leaf owns recursive
  syntactic effect classification, compiler-owned slice-view transparency, and
  place-root symbol recovery through three parent-private queries, likewise
  without resolving or summarizing a call frame. A 72-line
  `write_frames/place_paths.rs` leaf owns exact-versus-collection-coarse frame
  path provenance, root/suffix composition, and typed-expression path recovery;
  collection coarsening remains absorbing and the leaf has no call-resolution
  dependency. An 87-line `write_frames/state_paths.rs` leaf owns state-relative
  visibility, positional parameter-root normalization, exact symbol forwarding,
  and duplicate-free visible-path collection; it has no call- or
  frame-resolution callback. A 50-line `write_frames/type_capabilities.rs` leaf
  owns constrained-reference recognition and the type/parameter classification
  for carrying caller-visible writes, with no expression traversal or
  resolution callback. A 243-line `write_frames/local_aliases.rs` leaf owns
  canonical local-alias path rebasing, direct-place resolution through already
  established stable origins, syntactic mutable-reborrow detection for stable
  parameter/local bindings, and read-only reference-shaped replacement
  classification; it neither recursively infers origins, mutates bindings, nor
  resolves frames. A separate 59-line `write_frames/alias_bindings.rs` leaf
  owns exact stable-local rebinding admission and slot mutation through one
  immutable origin-inference callback; recursive origin analysis remains in
  the parent. A 114-line `write_frames/parameter_aliases.rs` leaf owns the narrow
  parameter-relative origin carrier, exact symbol/name alias lookup, and
  syntax-only transparent mutable-reborrow detection; recursive origin and call
  analysis remain in the parent. A 125-line
  `write_frames/transition_topology.rs` leaf owns named-edge target resolution
  and acyclicity checking within one machine plus exact write-capable namespace
  preservation for cycle-closing edges, without constructing or solving frame
  equations. A 91-line `write_frames/transition_equations.rs` leaf owns
  the private equation/edge carriers, exact named-edge capture, and read-only
  equation-graph reachability; construction, permutation validation, and
  fixed-point solving remain in the parent. A 61-line
  `write_frames/assignment_targets.rs` leaf owns
  declared target-type lookup and structural/effectful assignment-place shape
  classification; it depends only on typed-place and syntactic-effect queries,
  not alias mutation or frame resolution. An 80-line
  `write_frames/call_targets.rs` leaf owns free-machine entry selection, exact
  state-symbol lookup, and the fail-closed concrete discarded-result shape
  query; the established crate/calls visibility surface is re-exported
  unchanged, and the leaf performs no write-frame inference. An
  81-line `write_frames/path_instantiation.rs` leaf owns receiver/parameter/local
  substitution for relative write paths and preserves exact versus
  collection-coarse origins; its only callback is the existing parent-private
  actual-argument origin query. The parent
  preserves the existing public and crate-private query surface;
  receiver-member-chain and resolved-state lookup remain the only top-level
  sibling seams. The frame engine privately reuses two demand collection
  helpers, while the boundary child exposes only its two existing queries and
  one engine-internal parts helper; every other decrease, citation,
  provenance, expression-walk, frame-equation, and diagnostic helper remains
  private to its owner. Validation order, the 141-function inventory, and
  public API are unchanged.
  Privileged inline-assembly effect discharge now lives in a focused 449-line
  owner. Hosted/freestanding authority gating, exact catalog/service mapping,
  direct and transitive declaration checks, cycle-safe call-path recovery, and
  its complete normalized call-path renderer and symbol/state labeling stack
  remain one settled judgment. Exact transitive service diagnostics remain byte-
  for-byte ordered; the natural effects root is now a 135-line behavior/service-
  ceiling facade with the exact 23-function inventory unchanged.
  Pure-result discard validation now lives in a focused 181-line owner. Proof-
  context and citation exemptions, checked-machine and boundary-signature
  resolution, recursive service/operational purity, mutable-output detection,
  and the existing warning remain unchanged; it composes through that same
  natural effects facade without widening the 23-function inventory.
  Profiling the differential corpus also ruled out a wholesale Arena-to-
  `PagedArena` migration as a concurrency fix: `PagedArena` provides stable
  paged storage, not concurrent mutation, and the existing sound parallel
  pattern remains worker-local `Arena`s followed by deterministic ordered
  merge. Checked lowering now builds one call-frame/incoming-guard index for
  all range, contract, crash, and multiplicity consumers, reducing the
  helper-rich Mandelbrot canary's checked phase from about 15.6s to 9.5s.
  Backend state-value planning now builds the separate exact value-call
  dependency closure required before pruning. Runtime-flow and required-call
  states seed canonical `(machine symbol, state symbol)` identities; the index
  transitively visits local initializers, transition guards and values,
  terminal expressions, and nested call receivers and arguments through the
  simplifier's shared symbol-based resolver. Known-state resolution ambiguity
  conservatively retains the full program. Collection omits only states outside
  that closure: `StateValueUse.required` remains an independent emission fact,
  and the `runtime_nested_named_conversion_alias_exit` regression still exits
  70 with its off-flow nested value-machine expansion retained.
  On the exact warmed Mandelbrot stress canary, state-value planning fell from
  the documented 27.9s to 2.062ms while state storage took 1.360ms and the
  complete backend plan took 30.451ms. A full artifact-producing compile took
  7.470s, of which checked lowering was 7.207s; the exact one-canary
  interpreter/native differential run took 21.29s end to end, down from the
  documented 69.4s profile, with identical output and exit status. The
  dependency slice preserves worker-local `Arena`s and deterministic ordered
  merge; neither `PagedArena` nor cross-worker mutation is involved. The old
  hotspot remains useful history: a 10-second sampled profile landed in the
  `simplify_call_expression` / `helper_state_model` recursion; its two hottest
  leaf stacks are source-provenance `Arc` clone and drop (2,639 and 2,629
  samples), reflecting repeated reconstruction of expression trees and their
  identifiers. Prefer memoized normalized helper models or an indexed
  expression recipe over changing the backing arena. A linear structural
  helper-model cache was prototyped and rejected: on the exact warmed
  `text/runtime_mandelbrot_render_exit` differential canary, disabling it took
  60.21s wall/385.84s aggregate CPU while enabling it took 75.44s wall/76.77s
  CPU. It removed duplicated parallel work but moved the critical path onto one
  worker doing linear cache scans, increasing latency by 25%. No cache code was
  retained; the next attempt needs an indexed canonical key/recipe.
  Two later sampled Stage-05 fixes keep that rule. Default-domain analysis now
  builds one invocation-local call-frame resolver instead of reconstructing it
  for every fixpoint state visit, reducing the warmed checked phase from 6.754s
  to 5.338–5.386s. Named result-overload resolution now builds one source-
  ordered machine-family index keyed by exact normalized path and parameters,
  with direct entry-symbol lookup; operator and trait semantics are unchanged
  and the index is not retained. On the same canary that reduced Stage 05 again
  to 4.743–4.817s and full compile wall time to 5.51–5.64s.
  Checked-fact loop-invariant analysis now also reuses that pass's existing
  immutable call-frame resolver instead of rebuilding it below indexed-access
  checking. On a fresh exact profile this removed the former 1,056-sample
  construction stack and reduced warmed Mandelbrot Stage 05 from 4.810s to
  3.421s, with byte-identical output and native/interpreter exit 70.
  A subsequent eager complete-state write-summary prototype was rejected rather
  than retained: against a fresh 3.505s warmed baseline it moved 170 samples
  into resolver construction while targeting a 130-sample recursive summary
  stack, and regressed Stage 05 to 3.606–3.648s. Output remained byte-identical,
  but the remaining cyclic summaries need a genuinely lazy memo or SCC/fixpoint
  design; no cache code remains.
  A fresh output-only profile of
  `expressions/runtime_numeric_cast_exit` confirms that its 4.4s warm compile
  is likewise semantic work rather than viewer generation: Stage 05 consumed
  3.700s, split across typed validation (1.328s), checked-fact construction
  (1.201s), and checked-fact replay (1.031s), while normalization,
  specialization, overload resolution, and terminal cleanup totaled about
  140ms. The exact canary still exits 70. This profile gives no support to a
  shared mutable `PagedArena`; the next useful optimization boundary is an
  indexed or worker-local/deterministically merged implementation inside those
  three measured semantic phases. A subsequent exact native stack sample of
  `runtime_float_operations_exit` placed all 375 validation samples in default-
  domain analysis, including 150 in repeated state/call-frame summary
  recursion. The immutable call-frame resolver now uses a genuinely lazy memo:
  it caches only a statement call's complete-or-opaque normalized frame under
  its exact owning-machine and program-node identity, behind a concurrency-safe
  lock. Default-domain samples fell to 152 and the warmed exact canary moved
  from 4.75s to 4.08–4.12s while retaining exit 70. No eager whole-program
  summary or shared mutable arena was introduced.
  A follow-up lazy exact-state cache was profiled and rejected rather than
  retained. A fresh three-second sample placed 219 stacks in one demanded
  `inferred_state_write_frame`, including 100 below recursive state
  summarization, but the exact warmed float canary moved from 4.13s to 4.21s:
  the cost was one expensive state per resolver, not repeated identical state
  queries. No state-cache code remains. The next optimization must reduce or
  incrementally solve that demanded recursive summary itself. Recursive
  statement-call inference now shares one invocation-local completed-state
  summary set instead of discarding it at every resolved call boundary. The
  same exact sample reduced the demanded state-frame stack from 219 to 180 and
  recursive summarization from 100 to 80; warmed canary runs moved from 4.13s
  to 3.93–3.98s with unchanged exit 70. The memo remains local to one demand,
  carries exact state identities and complete relative paths, and introduces no
  eager whole-program solve or shared mutable state.
  Full phase artifacts for the two broad float canaries likewise put 92–93% of
  measured time in TypedTrees-to-CheckedTrees (3.23–3.38s); every backend stage
  was at most 22ms. Proof-plan assignment collection had rebuilt an immutable
  `CallFrameResolver` per assignment, with 87/97 samples in that branch landing
  in top-level-symbol construction. It now constructs one resolver per proof-
  plan invocation and reuses its existing cache, reducing full-artifact wall
  time by roughly 190–220ms without changing fail-closed frame semantics. The
  next measured duplicate is the same resolver rebuild under
  `assignment_guard_is_stable`; do not redirect this work toward backend or
  arena concurrency.
  Value-fact construction now owns one lazy exact-program
  `AssignmentRangeContext`, reusing that resolver only within its immutable
  invocation while the public range query remains a one-shot wrapper. This
  reduced the two float canaries' checked phases by 56–97ms (3.248s to 3.151s
  and 3.172s to 3.116s); unchanged allocation counts confirm the win is avoided
  symbol/arena rescanning rather than allocation suppression.
  Index-compatibility construction now resolves one source-ordered call catalog
  per state and reuses it for outer calls and nested value-context lookup,
  preserving first-match and unresolved-call fallback semantics. Sampling had
  placed 76/110 index-compatibility samples in repeated call-site lookup; the
  two broad float canaries' checked-phase means fell about 1.6% and 1.35%, with
  essentially flat allocation volume.
  Mutation-fact construction now shares completed acyclic state-write summaries
  across one source-ordered machine batch while leaving opaque and cyclic
  fallbacks uncached. Allocation-enabled checked phases fell 2.3–3.1%, removing
  roughly 22,000 allocations and 0.9 MiB per broad float canary; uninstrumented
  wall time remained noisy, so this is an allocation/instrumented-phase result,
  not a claimed wall-time win.
  Exact call-site resolution now indexes the already-known statement before
  replaying that statement's recursive source/ordinal call order, instead of
  traversing every state statement even though ordinals reset per statement.
  The two allocation-enabled float checked phases fell from 3.029s/3.026s to
  2.708s/2.700s with exactly unchanged allocation counts and bytes; out-of-
  range and unresolved lookups remain fail-closed.
  State-symbol lookup in write-frame equations now validates the retained
  handle, selects its exact owning machine from symbol parentage, and preserves
  source order only within that machine instead of scanning every machine's
  states. Sampling had placed 762 stack rows in the former traversal; the two
  allocation-enabled float checked phases fell another 10.0%/9.9% to 2.436s/
  2.432s with allocation counts and bytes exactly unchanged. Stale, non-state,
  and mismatched symbols remain fail-closed.
  Machine, owned-data, and state typed-handle lookup now validates exact
  retained parent/name ownership directly instead of rescanning hierarchy/name
  tables; attached-data fields retain the broader machine-child path because
  their handles belong to the data definition. No-allocation checked phases
  fell from 2.449s/2.481s to 1.899s/1.906s, while allocation-enabled phases fell
  17.1%/19.9% with counts and bytes exactly unchanged. Stale, redirected, or
  mismatched handles remain fail-closed.
  Complete acyclic state-write summaries now persist across every query in one
  immutable `CallFrameResolver` invocation rather than only one mutation-
  machine batch. The cache retains exact state identities and source-ordered
  relative paths; opaque results and permuted-cycle fallbacks remain one-shot,
  and poisoned cache access recomputes locally. A fresh sample reduced
  aggregate `summarize_state_written_paths` samples from 353 to 179.
  Allocation-enabled checked phases fell from 2.020s/1.949s to 1.910s/1.884s,
  removing exactly 54,008 allocation calls and 1.91 MiB in each broad float
  canary; warm no-allocation phases fell from 1.899s/1.906s to 1.848s/1.846s.
  Raw full-run wall remained noisy, so the claim is limited to phase timing and
  allocation evidence.
  Checked semantic-call state lookup now uses a valid state symbol's retained
  parent as an exact owning-machine fast path, while retaining the prior
  source-ordered global scan when table metadata is unresolved or disagrees
  with storage. Invalid handles remain rejected and the final exact state-
  handle match is unchanged. Allocation-enabled broad-float checked phases
  fell from 1.947s/1.920s to 1.772s/1.663s with allocation counts and bytes
  exactly unchanged, so this is avoided machine/state traversal rather than
  allocation suppression. A structural-resolution cache and in-place vector-
  reuse prototype were rejected and fully removed because they increased
  allocations or produced no measurable phase benefit.
  Structural contract application unfolding now retains one zero-allocation
  last-machine hint per judge. A hit revalidates the complete unattached-
  machine/name predicate, while a miss still scans from the beginning; source-
  order selection, clone behavior, unfolding depth, proof acceptance, and
  rejection remain unchanged. In uncontaminated three-run comparisons, the
  broad float canaries' allocation-enabled Checked-phase medians fell from
  1.732s/1.711s to 1.679s/1.672s, with allocation counts and bytes exactly
  unchanged. Lower-sample effects-owner and machine-parameter parent-lookup
  prototypes were removed after failing to establish a benefit.
  Backend state-value helper expansion now spends at most ten model builds per
  top-level expression instead of 20,000. Exhaustion is the existing semantic
  no-fold path: the unsimplified call remains for downstream lowering, while
  the pinned nested guarded-helper fold establishes the compatibility floor.
  State-value/backend package tests, four helper-dependent reverse canaries,
  exact total-order interpreter/native parity, the GUI compile-only path, and
  the complete pass umbrella remain green. The exact pass umbrella fell from
  125.09s to 51.84s; auxiliary viewers were already disabled and were not the
  source of the exponential work.
  The pass-canary scheduler now defaults to twelve independent outer compiles
  with one backend worker each. On the current 215-compile umbrella this
  reduced exact test time from 51.60s at outer eight to 46.15s at outer twelve;
  the explicit overrides remain available for host-specific profiling.
  The native/interpreter differential scheduler likewise now defaults to
  twelve independent outer compiles with one backend worker. On a warm,
  representative 64-canary slice, four jobs repeated at 10.45--10.77s while
  twelve completed in 5.62s with all 64 results identical; `DIFF_JOBS` remains
  the host-specific override.
  Re-profiling the broad float Checked phase after bounded helper expansion put
  both representative fixtures near a 1.60s median. A sampled source-ordered
  operator-signature key catalog was rejected and fully removed: compare/cast
  moved only -0.75% while float operations were flat at +0.10%. The remaining
  validation profile is diffuse, so no speculative cache was retained.
  Default-domain validation now delegates conservative symbolic values,
  literal/sequence measures, valuation folding, canonical symbolic equality,
  and recursive call detection to a focused 281-line child while state walking,
  invariant-window lifecycle, diagnostics, and crash evidence remain separate.
  Its 2,016-line parent retains the same API, 45-function inventory, accepted
  judgments, and diagnostic order.
  Standing reader-hypothesis interval derivation now lives in a focused 279-
  line `where_fact_intervals` child, preserving the crate-visible query,
  recursive depth cap, declared-range/product guards, and fail-closed behavior
  while leaving write analysis and diagnostic order in a 1,739-line parent.
  Callback-free cross-state flow primitives now live in an 83-line `state_flow`
  child, owning exact transition-edge reconstruction and transported literal-
  valuation must-meet while the 1,669-line parent retains fixpoint scheduling,
  statement walking, and diagnostic order; the 41-function inventory remains
  unchanged.
  Read-only place/schema queries now live in a 174-line `place_queries` child,
  owning exact place rendering, attached/declared data resolution, self-root
  classification, and standing-fact field participation without flow or
  diagnostic callbacks. The 1,509-line parent retains the same API, behavior,
  diagnostic order, and 41-function inventory.
  Structural call-target and establishment-summary queries now live in a 68-
  line `call_summaries` child, preserving exact state-to-machine identity
  resolution and recursive expression traversal while leaving flow mutation,
  fixpoint scheduling, and diagnostics in a 1,452-line parent; the 41-function
  inventory remains unchanged.
  The measured nominal affine integer-comparison reconstruction hotspot now
  uses independent producer- and verifier-local affine-definition indexes.
  Each immutable invocation maps an exact current Value term to source-ordered
  semantic equality rows that can extend the fixed add/subtract/multiply
  definition frontier; candidate prefixes and completed proofs are still
  independently replayed by the proof kernel, so seven-definition depth,
  citation precedence, proof shapes, rejection, and the producer/verifier trust
  boundary are unchanged. The exact mixed nominal regression fell from
  approximately 306s to 27.50s test-body time (29.29s wall; 476,823,552-byte
  maximum resident set), while the exact mixed-shift regression fell from
  92.78s to 6.08s test-body time (6.76s wall; 421,036,032-byte maximum resident
  set). No persistent cache or generalized search was introduced. Affine-
  definition candidate indexing now lives in paired, side-local
  `affine_custody/definition_index` modules. Producer and reconstruction
  independently own their immutable source-order Value-to-definition indexes,
  while frontier modules only enumerate the existing seven-definition words and
  replay each candidate through the proof kernel. This responsibility split
  changes no citation order, proof shape, rejection, or search frontier. The
  complete checked-to-Terminal package suite consequently fell from 401.56s to
  35.68s wall while all tests remained enabled and green.
  Syntactic affine-definition discovery now lives in paired, side-local
  `affine_custody/definition_index/candidates` modules. Producer and
  reconstruction independently retain semantic-row order, both equality
  orientations, exact Value-target eligibility, and add/multiply left-before-
  right versus subtract-left input projection; the invocation-local
  `DefinitionIndex` remains responsible for ordered per-input insertion and
  adjacent-row deduplication. Proof shape, rejection behavior, and the fixed
  frontier are unchanged.
  Exact affine relaxation mapping now lives in paired, side-local
  `affine_custody/relaxation/mapping` modules. Producer and reconstruction
  independently derive the mapped literal endpoint, carrier, sign reversal,
  and overflow-checked coefficient/offset image. The producer parent retains
  affine-proof construction and the closed transitivity bridge;
  reconstruction independently rechecks kernel conversion and closed-order
  relaxation. Candidate order, proof shape, rejection, and the fixed affine
  frontier are unchanged.
  Exact affine-root endpoint custody now lives in paired, side-local
  `affine_custody/relaxation/mapping/endpoint` modules. Producer and
  reconstruction independently require a retained `LessOrEqual` row and
  preserve left-root-before-right-root selection while returning the same bound
  endpoint and lower-versus-upper orientation. Signed carrier validation,
  checked affine mapping, sign-directed target orientation, proof shape,
  rejection, and the fixed frontier remain unchanged.
  Checked affine scalar mapping now lives in paired, side-local
  `affine_custody/relaxation/mapping/value` modules. Producer and reconstruction
  independently require an exact signed-integer endpoint of the affine carrier,
  apply checked coefficient multiplication and offset addition, and reject
  overflow or an unrepresentable mapped scalar. Endpoint custody, sign-directed
  target orientation, proof shape, rejection behavior, and the fixed frontier
  remain unchanged.
  Sign-directed mapped affine-bound orientation now lives in paired, side-local
  `affine_custody/relaxation/mapping/orientation` modules. Producer and
  reconstruction independently reverse lower-versus-upper direction only for a
  negative coefficient and preserve the exact target-versus-mapped endpoint
  placement. Endpoint custody, checked scalar mapping, proof shape, rejection
  behavior, and the fixed frontier remain unchanged.
  Direct retained affine-bound custody handoff now lives in paired, side-local
  `affine_selection/direct/completion` modules. The producer independently
  converts the selected origin-indexed citation into its exact proof before
  affine custody, while reconstruction independently passes the retained
  proposition into its own custody replay. Parent selectors retain assumptions-
  before-semantic-axioms traversal and left-before-right value endpoints, so
  citation order, proof shape, rejection, and the fixed definition frontier are
  unchanged.
  Closed affine-relaxation completion now lives in paired, side-local
  `affine_custody/relaxation/completion` modules. After their independent mapped-
  endpoint derivations, the producer child constructs the exact closed-order
  bridge and `IntegerLessOrEqualTransitivity` proof, while reconstruction
  independently checks the same endpoint alignment and closed relation.
  Mapping, kernel affine conversion, citation order, proof shape, rejection,
  and the fixed affine frontier are unchanged.
  Closed affine-relaxation bridge selection now lives in paired, side-local
  `affine_custody/relaxation/completion/bridge` modules. Producer and
  reconstruction independently require mapped and goal `LessOrEqual` rows,
  preserve right-endpoint alignment before the left-endpoint fallback, and
  select the exact closed bridge endpoints; only the producer records whether
  that bridge precedes or follows the affine proof. Closed-fact construction,
  transitivity shape, rejection behavior, and the fixed frontier remain
  unchanged.
  Per-witness affine custody completion now lives in paired, side-local
  `affine_custody/completion` modules. The producer independently constructs and
  kernel-checks the direct `IntegerAffineBound` proof before its existing
  relaxed fallback; reconstruction independently normalizes the same enumerated
  witness and checks direct conversion before its own relaxation replay. Parent
  custody retains exact goal-endpoint and source-ordered definition-word
  enumeration, so precedence, proof shape, rejection, and the seven-definition
  frontier are unchanged.
  Direct affine-witness completion now lives in paired, side-local
  `affine_custody/completion/direct` modules. Reconstruction independently
  replays exact affine-bound conversion after witness validation, while the
  producer independently constructs the `IntegerAffineBound` proof node and
  validates the complete certificate. Direct-before-relaxation precedence,
  fallback witness replay, proof shape, rejection behavior, and the fixed
  frontier remain unchanged.
  Relaxed affine-witness completion now lives in paired, side-local
  `affine_custody/completion/relaxed` modules. Reconstruction independently
  replays the mapped-bound relaxation, while the producer independently
  constructs the relaxed proof and validates the complete certificate before
  release. Direct-before-relaxed precedence, fallback witness validation, proof
  shape, rejection behavior, and the fixed frontier remain unchanged.
  The two ordered landed-literal alias root bounds now live in paired, side-
  local `affine_selection/literal/alias/completion/bound` modules. The producer
  independently constructs the exact closed reflexive relation and nested
  inner-alias then outer-root substitutions for endpoint 1 before endpoint 0;
  reconstruction independently rebuilds the same two root-bound propositions.
  Completion parents retain affine custody, so equality order, proof shape,
  rejection, and the fixed definition frontier are unchanged.
  Direct two-citation affine root-bound construction now lives in paired, side-
  local `affine_selection/transitive/completion/bound` modules. The producer
  independently constructs the exact `IntegerLessOrEqualTransitivity` node from
  the ordered citations, while reconstruction independently rebuilds the same
  `left <= right` proposition. Completion parents retain left-then-right value-
  root traversal and affine custody, so citation order, proof shape, rejection,
  and the fixed definition frontier are unchanged.
  Alias-substituted transitive affine root-bound construction now lives in
  paired, side-local `affine_selection/transitive/alias/completion/bound`
  modules. The producer independently constructs the ordered two-citation
  transitivity proof and substitutes endpoint 0 or 1 from the exact alias
  equality; reconstruction independently selects and rebuilds the same
  resulting root-bound proposition. Completion parents retain affine custody,
  so citation/equality order, proof shape, rejection, and the fixed definition
  frontier are unchanged.
  The two ordered direct landed-literal affine root bounds now live in paired,
  side-local `affine_selection/literal/direct/completion/bound` modules. The
  producer independently constructs the closed reflexive relation and endpoint
  substitution for endpoint 1 before endpoint 0; reconstruction independently
  rebuilds the same two root-bound propositions. Completion parents retain
  affine custody, so equality order, proof shape, rejection, and the fixed
  definition frontier are unchanged.
  Corpus-level bounded parallelism is viable at the harness boundary: the
  differential runner now defaults to four independent jobs with one native
  backend worker each, retains deterministic corpus-order reporting, and
  exposes `DIFF_JOBS`, `DIFF_LIMIT`, and exact `DIFF_CANARY` controls. On this
  14-core host the first eight canaries fell from 8.30s to 4.00s; a 32-canary
  concurrency probe passed completely, while eight outer jobs improved only
  101s to 97s over four and therefore is not the default. The native leg now
  selects output-only artifact emission because it consumes only the certified
  executable, not pipeline viewers or diagnostic reports; the same cached
  eight-canary probe fell again from 4.00s to 2.86s with all pairs matching.
  Semantic validation, trust policy, and final-footprint certification remain
  enabled. The disposable `omega-run` probe now follows that same policy while
  preserving full reports under explicit `--keep`. On the exact warmed
  Mandelbrot compile, suppressing reports that were immediately deleted reduced
  median wall/CPU from 4.36s/4.43s to 4.05s/4.12s and retired instructions from
  73.4B to 67.6B; a small unary-entry probe fell from 0.05s/654M instructions to
  0.02s/292M. `--keep` still produced the complete 35-file, 1.8 MiB inspection
  directory, and native/interpreter results remained identical. The native leg
  uses each original source's authored target-owned `ProgramEntry` when present
  and the bounded legacy `Main::main` seam only for the remaining unrooted
  corpus; the former generated target wrapper discarded value-returning entry
  codes and produced false mismatches.
  The compiler pass/fail corpus umbrellas now use the same deterministic outer
  scheduler for checked-only, cross-target, rooted-target, Windows-host, and
  active backend members instead of leaving the large backend registries
  serial. The first measured version used two inner workers, with
  `OMEGA_CANARY_JOBS` and `OMEGA_CANARY_INNER_WORKERS` retained as explicit
  profiling controls. On an eight-program heavyweight probe, four outer jobs
  and two inner workers reduced wall time from the one-outer/one-inner 65.42s
  to 31.12s. The dominating float total-order canary fell from 58.61s with one
  inner worker to 29.63s with two; four and fourteen inner workers provided no
  material wall improvement (29.11s and 29.28s) while increasing aggregate CPU
  from 78.29s to 112.16s and 226.53s. This measured ceiling is why the harness
  does not inherit unrestricted host parallelism inside every outer job. After
  repairing the corpus drift this broader gate exposed, the complete active
  pass umbrella finishes in 234.92s and the complete active fail umbrella in
  21.05s on the same host; both collect the whole registry rather than stopping
  at the first failure. Dedicated native-canary helpers initially used the same
  two-worker ceiling instead of multiplying Rust test-thread concurrency by a
  host-wide compiler pool. The exact float total-order test fell from 128.30s
  wall/998.98s aggregate CPU with fourteen inner workers to 91.83s/256.84s with
  two. A later fixed 112-compile mixed-corpus profile established the stronger
  harness boundary now in production: eight independent outer jobs with one
  inner worker completed in 13.00s, versus 21.26s for four outer jobs with two
  inner workers; twelve outer jobs were no faster and consumed more memory.
  Corpus compiles therefore default to outer eight / inner one, retain both
  environment overrides, and leave production compiler defaults unchanged.
  The compiler canary integration suite is no longer a 48,301-line permutation
  file. Its shared compile helpers, exact corpus registries, and umbrella
  orchestration now form a 3,277-line root over twenty-one responsibility
  modules for target artifacts, reports, content, ranges, arithmetic, providers,
  calls, ABI, proofs, layouts, and runtime families. All 1,241 tests and 1,272
  functions remain; the sole cross-family float differential helper is imported
  explicitly, and no family module exceeds 3,795 lines.
  Artifact presentation has begun the same responsibility split. The
  2,963-line `omega-artifacts` root retains artifact carriers and general
  writing orchestration. A focused 296-line `wire_report.rs` child owns only
  the stable wire-protocol text projection and its field/case/verdict
  formatters, while a separate 197-line `timing_report.rs` child owns phase
  timing/allocation aggregation, table layout, and numeric presentation. The
  public `ArtifactWriter` methods, exact outputs, and 79-function inventory are
  unchanged.
  Atomic artifact-directory installation now lives in a focused 123-line
  `artifact_writer` child, owning temporary-file replacement, byte/text/HTML
  writes, stale-file removal, and executable-container encoding. Report
  rendering remains in the 2,855-line parent and focused renderer children;
  public methods, exact outputs, and the 79-function inventory are unchanged.
  Human-readable target, contract, unchecked-policy, and capability-blast-
  radius presentation now lives in a focused 112-line `boundary_report` child.
  The 2,751-line parent retains artifact carriers and general report
  orchestration; public methods, exact output, and the 79-function inventory
  are unchanged.
  Source-load totals/file tables and syntax-tree identity/file presentation,
  including their source/AST row formatting, now live in a focused 157-line
  `frontend_reports` child. The 2,477-line parent retains artifact carriers and
  later-stage orchestration; public methods, exact HTML output, and the 79-
  function inventory are unchanged.
  Emission-plan text, native image installation/reporting, stale-output cleanup,
  direct-executable finalization, permission installation, and finalization
  presentation now live in a focused 202-line `native_output_reports` child
  behind unchanged crate-root re-exports. The 2,416-line parent retains report
  carriers and non-native orchestration; public APIs, exact output, and the 79-
  function inventory are unchanged.
  Exact Build-selected source/backend audit-surface construction now lives in a
  focused 51-line `backend_surface` child, including machine containment and
  explicit entry-point selection. The 2,371-line parent retains report carriers
  and presentation; the crate-root API, selected-entry behavior, and the 79-
  function inventory are unchanged.
  Canonical boundary-call and value-placement JSON projection now lives in a
  focused 278-line `calling_plan_json` child, owning shapes, registers, stack/
  indirect locations, call control, machine regime, stack domain, and
  preemption vocabulary. External-root reports reuse three narrow projection
  helpers, while the 2,099-line parent retains carriers and orchestration; the
  public `value_placement_json` API, exact bytes, and 79-function inventory are
  unchanged.
  Canonical provider/runtime-owned external-root ledger projection now lives in
  a focused 387-line `external_root_report` child, including stack/fuel summary
  evidence, machine-state ceilings, component pins, and normalized identity
  formatting. It reuses the calling-plan vocabulary without numeric entry
  addresses; the 1,722-line parent retains carriers and general orchestration,
  while public APIs, exact JSON, and the 79-function inventory are unchanged.
  Chapter-10 trust commitments, generic accepted instances, provider
  requirements, and qualification rows now render from a focused 251-line
  `trust_report` child. The 1,476-line parent retains artifact carriers, shared
  HTML presentation, and general orchestration; public APIs, exact Markdown,
  and the 79-function inventory are unchanged.
  The 11 artifact construction/projection regressions now live in a dedicated
  778-line `tests` child, leaving a 698-line production root over carrier
  definitions, shared HTML infrastructure, and module wiring. Test coverage,
  public APIs, exact artifacts, and the 79-function inventory are unchanged.
  Development and test profiles now both omit full DWARF by default, with an
  explicit `CARGO_PROFILE_{DEV,TEST}_DEBUG=2` escape hatch for debugger
  sessions. On the same macOS host, rebuilding the development CLI after the
  profile change reduced the executable from 140,687,560 to 118,904,896 bytes;
  the semantic canaries remained 0.01-second work once compiled. This reduces
  codegen/link and artifact-I/O pressure without weakening compiler diagnostics
  or semantic validation.
  A later apparent Rust frontend regression was build-cache accumulation, not
  compiler execution: `target/debug/deps` had grown to 1,359,819 entries and a
  rustc sample spent its startup in `SearchPath::new`/`readdir` before parsing
  the crate. `cargo clean` removed 1,483,970 derived files (195.1 GiB). The same
  proof-codec target then fell from 58.8s incremental to 2.86s cold and 0.68s
  after touching `psi-core`. Treat a uniform per-crate pre-parse delay as Cargo
  cache hygiene first; it is not evidence for Arena concurrency, test sharding,
  linker changes, or disabled semantic gates.
  The real-source terminal-Psi differential suite now applies the same boundary:
  its former 10,520-line file is an 852-line artifact/native execution harness
  over ten contract, call/control, exact-arithmetic, scalar-operation, and
  crash/admission families. All 115 tests and 137 functions remain, and no
  family exceeds 2,030 lines.
  native machine emission has undergone the same split: its
  12,922-line crate root is now an 891-line production orchestrator with the
  complete 58-case, 5,028-line regression corpus compiled separately.
  Unit-body and calling-policy emission, per-target parameter homes, aggregate
  argument staging/copying, and Unit stack/fuel/effect evidence form a separate
  1,301-line responsibility. Scalar-return and Boolean-control cleanup,
  nominal-cleanup admission, exact residual partitioning, and cleanup
  stack/fuel/call evidence form a separate 1,120-line responsibility. Scalar
  control/expression emission now has a 31-line orchestration/re-export root
  over distinct 1,861-line x86-64 encoding, 1,775-line AArch64 encoding, and
  1,067-line shared conditional-shape/stack-evidence responsibilities. All
  eighty-five implementation functions retain their exact architecture or
  shared owner, and the parent-facing surface remains explicit rather than
  becoming one permutation dispatcher.
  Terminal-module validation has begun the same split: its parent shrank from
  7,498 to 282 lines, with structural/service foundation (956 lines),
  structural/boundary operation custody (822), public error vocabulary (803),
  structural ownership/frontier cleanup (750), per-machine
  registration/orchestration (716), scalar crash/frontier and Boolean-predicate
  custody (674),
  content-conservation validation/replay (465) with equality/separate algebra,
  exact projection selection, entry/current structural-place reconstruction,
  identity replay, and qualification normalization in a focused 545-line child,
  operation operand/type custody
  (522), partial/nominal affine cleanup custody (473), evidence/proposition
  custody (410), control-flow/dominance validation (301), proposition-root
  projection (146), contract proposition scope (120), and call-graph acyclicity
  (68) in separate responsibilities. Public validation types remain
  re-exported from the crate boundary.
  Final-image validation has begun the same responsibility split: its parent is
  down from 22,945 to 712 lines. Its regression corpus is a 25-line root over
  separate 701-line final-validation, 1,197-line place-replay, and 1,037-line
  guard/assembly families instead of a second responsibility embedded in
  production. Imported-call replay now has a 1,335-line parent, while table and
  vtable indirect calls form a separate 549-line responsibility. Runtime
  byte/line/text-boundary replay is a separate 504-line responsibility,
  and syscall replay plus exact relocation-target derivation is a separate
  507-line responsibility. The contract-entailment owner has begun a matching
  split: its 7,124-line arithmetic, inductive, citation, and structural-law
  parent delegates the 242-line exact quotient-congruence judgment to a focused
  child. That child alone recognizes quotient mint equality and requires the
  quotient's retained relation premise; it performs no ambient proof discovery
  and cannot fall through to generic arithmetic or structural tiers.
  Its structural-term algebra, explicit ring/semiring licensing, substitution,
  and structural judgment now form a separate 1,839-line responsibility; the
  remaining coordinator then delegates exact polynomial normalization,
  interval propagation, difference-bound closure, and arithmetic verdicts to
  a separate 984-line child. Inductive transition-arm recognition, path-fact
  preparation, strict-decrease discharge, and hypothesis instantiation form a
  separate 456-line child. Boundary-operator contract matching, proposition-law
  binder synthesis, carrier-slot substitution, and structural diagnostics form
  a separate 1,040-line conformance child. The remaining citation coordinator
  is 2,535 lines, with the existing parent-facing judgments, conformance checks,
  and proved-index-algebra surface unchanged.
  Compiler footprint derivation now has a 509-line composition/partition parent
  over a declarative four-family registry: 249-line control/entry, 621-line
  storage/place, 866-line outbound-call, and 512-line buffer/wire/text
  responsibilities. A
  separate instruction-selection boundary-footprint owner has begun the same
  split: its 2,255-line `entry.rs` parent delegates all eleven compact-binary
  append/read footprint derivations to a 433-line `entry/wire.rs` child, while
  a separate 373-line `entry/text.rs` child owns bounded-buffer, string-
  descriptor, and runtime-text assembly footprints. A focused 152-line
  `entry/runtime_values.rs` child owns atomic and conversion-write footprint
  derivation over the retained runtime-operand arena. A focused 222-line
  `entry/guards.rs` child owns static, runtime-text, place-shaped, and recursive
  runtime-value dispatch-guard footprints. A 199-line `entry/control.rs` child
  owns ordinary call/return mechanics and compiler-generated dispatch-scaffold
  footprints. A 139-line `entry/assembly.rs` child owns the x86 checked-
  assembly catalog footprint over retained selected instructions and runtime
  operands. A 158-line `entry/exit.rs` child owns the derived-exit carrier,
  normalized result placement, and direct/indirect result footprints. The
  277-line `entry/inbound.rs` sibling owns inbound-storage carriers, normalized
  parameter/result-pointer writes, descriptor scratch, and exact target
  clobber validation. A 264-line `entry/place_writes.rs` child owns immediate
  integer, address, runtime-binary, and bit-field write footprints. A 433-line
  `entry/place_copies.rs` sibling owns ordinary place-copy shape dispatch to
  exact target encoder clobber contracts. A 240-line `entry/runtime_io.rs`
  child owns byte-read, byte-write, and line-read host-adapter footprint
  derivation. A 75-line `entry/constant_results.rs` child owns per-target
  constant host-result materialization footprints. A 901-line
  `entry/direct_imports.rs` child owns all sixteen direct-import footprint
  classifications and their shared retained-plan evaluator. A 121-line
  `entry/indirect_calls.rs` child owns table/vtable call footprints without
  conflating them with direct import relocation. A 709-line
  `entry/syscalls.rs` child owns the complete simple, relocatable-argument,
  result, and timespec syscall footprint family plus its closed-shape test. The
  public re-export surface, validation order, and 135-function inventory are
  unchanged; the
  children depend only on retained instructions/operands, the validated
  boundary plan, place-shape classification where applicable, and architecture
  encoder clobber/state facts. A
  separate 1,547-line
  module owns assembly footprints, operand-loader semantics, exact instruction
  bytes, and retained relocation checks behind two parent entry points; and a
  1,598-line module owns exact compiler relocation sets, symbol custody, and
  unchanged instruction-bit validation. Compiler atomic-operation replay and
  recursive runtime-operand storage-site derivation now form a separate
  752-line responsibility. The closed place-copy shape vocabulary and exact
  classifier form a 1,218-line responsibility; indexed and pointee offset
  decomposition is a separate 946-line responsibility. Place-pair and
  place-copy shapes map to exact
  architecture-specific relocation sites in a separate 505-line module. The
  closed place-write shape vocabulary and its exact classifier family form a
  separate 304-line responsibility. Retained place-write encoding plus exact
  register and relocation-site derivation form a separate 1,039-line
  responsibility. The closed compiler instruction-relocation recipe vocabulary
  and exact final-byte/site replay form a separate 1,539-line responsibility.
  Exhaustive expected-byte, class, position, and relocation-recipe
  reconstruction now has a 55-line specification-family dispatcher behind a
  single typed entry point; fixed mechanics, guards, return transport, and
  entry and dispatch transport form a separate 477-line family, while compiler atomics,
  place copies and writes, and storage results form a separate 1,083-line
  family. Imported calls, runtime I/O, indirect calls, and syscalls form a
  separate 858-line family. Bit fields, bounded buffers, wire encoding, and
  text materialization form a separate 1,480-line family. Binary arithmetic
  and scalar conversion writes form a separate 478-line family. The separate
  native-refinement lane now applies the same engineering boundary to x86-64
  byte encoding: the public root is down from 19,412 to 89 lines and
  re-exports 106-line function-frame, 591-line entry/result ABI,
  662-line privileged-effect, 578-line Linux-syscall, and 760-line atomic
  responsibilities with their focused byte/width tests. Compact Binary wire
  append/read, scalar, byte-slice, nested, repeated-field, predicate, and UTF-8
  encodings now form a separate 1,880-line responsibility. Stored/literal text
  append, materialization/comparison, Win64/Linux line reads, and bounded text
  carriers form a separate 1,580-line responsibility. Generic host dispatch,
  authored imports, normalized Win64/System V argument and result placement,
  direct/vtable/table calls, byte I/O, and exact relocation-site replay form a
  separate 4,399-line production responsibility; its 2,005-line ABI regression
  corpus is separately compiled. Runtime value comparison, operand replay,
  binary arithmetic, conversion, and text equality now form a 4,161-line
  scalar parent. Its recursive-operand register-write ceilings, stack/control-
  state traversal, and comparison/binary/conversion machine-state contracts
  live in a focused 187-line child; the exact 104-function inventory, public
  surface, bytes, widths, and failure behavior remain unchanged. Integer/bit-
  field/indexed place writes and copy-layout
  contracts form a separate 675-line responsibility. Their 652-line arithmetic
  and conversion regression corpus is separately compiled. Dispatch-loop,
  case-entry, state-write, case-leave, and static-guard encoding now form a
  separate 176-line responsibility. Shared register moves, loads/stores,
  displacement checks, copy-chunk iteration, and atomic byte helpers form one
  explicit 1,114-line crate-internal primitive layer. These
  are semantics-preserving responsibility splits, not trust promotions: the
  full low generator, row proofs, and composition bridges remain open, and no
  trust-graph node becomes derived from the spike. The corresponding AArch64
  cleanup has begun: its former 14,216-line runtime-storage parent is now an
  810-line address/load/store orchestration parent. Runtime operand replay,
  text equality, integer and floating arithmetic, arithmetic-domain policy,
  classification, and their exact width contracts form a separate 2,172-line
  runtime-value responsibility. Atomic load/store, read-modify-write, ordering,
  result-site, and width policy form a separate 697-line responsibility, and
  scalar conversion, placement, trap, saturation, and width policy form a
  separate 746-line responsibility. Direct place-pair, place-value,
  computed-value, register, machine-state, and exact failure-branch comparison
  contracts form a separate 356-line responsibility. Recursive-operand
  register/state contracts plus
  immediate integer, bit-field, direct binary, pointee binary, saturation, and
  trapping writes form a separate 1,048-line scalar-write responsibility.
  Direct, pointee, indexed, and double-indexed bounded-buffer writes plus
  literal and source appends form a separate 935-line responsibility. Direct,
  pointee, indexed, and double-indexed string-descriptor writes plus their
  register/state ceilings form a separate 392-line responsibility. Direct,
  pointee, frame-indexed, machine-indexed, and double-indexed place-address
  writes plus exact clobber/state ceilings form a separate 440-line
  responsibility. Descriptor, pointee, frame, and machine single- and
  double-indexed integer/binary writes form a separate 897-line responsibility.
  Direct, pointee, single-/double-indexed, cross-region, and indexed-pair copy
  encoders plus exact chunk and clobber contracts form a separate 2,597-line
  responsibility. The 3,361-line byte/width/policy regression corpus remains
  separately compiled, and the exact public, function, and test inventories
  remain preserved.

  Define a closed typed schema language with no opaque callbacks. One row per
  leaf operation owns well-formedness, direct mathematical denotation, canonical
  goals, post-discharge facts, crash behavior, and local fuel/frontier effects.
  Missing operation rows reject mechanically. A change to control, validity,
  effect, or frontier machinery is a visible ledger-algebra revision rather than
  an ordinary row addition. Schema and artifact identities pin the exact state
  model, mathematical definitions, operational clauses, and semantics version.

  Before committing the full low implementation, build a Gamma spike that
  canonical-decodes bytes and covers Exact and Wrapping arithmetic, signed
  divide/remainder with toward-zero behavior and `MIN / -1`, one conditional
  result equation, one branch-local premise, an asymmetric join that rejects,
  the positive all-predecessor merge dual, exact call-requirement enumeration
  and substitution, justification ranking, dominance, and invalidation. Measure
  Gamma/specification size, audit complexity, Beta-reference runtime and memory,
  ledger size, and prospective reconstruction-certificate size. Difficulty does
  not weaken the endpoint; an inability to express the total definition cleanly
  triggers a rung-design correction.

  The production ledger records premise origin, prerequisites, establishment
  point, value/place versions, validity scope, invalidating events, and an
  acyclic logical-justification rank. Rank prevents circular evidence but does
  not replace dominance and all-path availability. Ordinary merge evidence is
  acyclic and requires valid matching facts on every predecessor; cyclic control
  requires invariant establishment and preservation. Partial-operation result
  equations become available only on the proved normal successor. Calls check
  clause coverage separately from capture-free positional instantiation across
  arity, binder kinds/types, state versions, moves/reborrows, outcome guards,
  crash routes, and evidence lifetimes.

  Establish every deployed ledger by direct low-rung evaluation or a low-kernel-
  checked derivation of the same total definition. Rust agreement is a
  differential oracle whose disagreement rejects and whose agreement grants no
  authority. Convert reduction families incrementally: a converted family emits
  a certificate; an unconverted family remains an exact versioned trusted-
  judgment dependency.

  Prove separate composition bridges. Safety/partial correctness combines
  exhaustive derivation, sound schema rows, valid premises, and checked goals.
  Progress/total correctness combines well-founded measures, per-edge descent,
  complete SCC/call closure, and explicit environmental progress premises. Fuel
  is sponsor scheduling and discharges neither. Row proofs are universally
  quantified low-rung metatheory, with derived status computed from an accepted
  proof and exact dependencies. Conservative semantic extensions need checked
  transport; relevant changes require reproof, while old artifacts retain their
  pinned semantics identity. Native ISA/hardware refinement remains a separate
  trust closure.

  Acceptance: byte mutation, omitted sites, extra premises, stale versions,
  one-arm-only join facts, post-write stale facts, circular justification,
  wrong call substitution, premature result equations, altered schema rows,
  unknown roots, and changed semantic dependencies all reject or change the
  recorded closure. A reducer cannot replace the canonical goal with its
  sufficient preimage. An artifact report lists every remaining trusted
  implementation/row and cannot appear fully derived until both applicable row
  proofs and the relevant global composition bridge are accepted. A Psi-hosted
  kernel port alone emits no ledger and supplies no reconstruction assurance.
- **IRFUEL.** Keep logical fuel at evaluator and analysis boundaries.
  Entry/segment certificates may cover loops, build-time evaluation, static
  work reports, and WCET inputs. Installed-code correspondence may bind such
  a theorem to exact bytes as non-authorizing PCC evidence. Native lowering
  inserts no charges, allowance context, exhaustion dispatcher, transfer or
  resume stub, and no sponsor route. Failure to derive a finite bound reports
  `Unknown` or `NoFiniteGuarantee`; it does not change runtime execution.
- **PROOF-RELEVANCE-MIGRATION.** Finish binding-level `[erased]`, checked
  noninterference, erased-stripped layout, and obligation preservation across
  the remaining consumers. Explicit relevance remains in semantic/proof
  identity while supported runtime carriers recursively omit erased storage,
  initialization, topology, bytes, tags, and ABI transfer; runtime use rejects
  and omitted evidence remains a required semantic term.

  Erased runtime-use noninterference now lives in a focused 463-line owner.
  Proof, runtime, and erased contexts, recursive expression traversal, struct-
  initializer completeness, runtime field/payload rejection, and proof-machine
  call fencing retain exact diagnostic order. Erased-shape admission now lives
  in a focused 246-line owner. Boundary, placed, and attached-machine fences,
  closed-record and case support, unresolved generic use rejection, and
  recursive erased-field discovery retain exact diagnostic order; the natural
  relevance root is now a 176-line statement-context facade with the exact 15-
  function inventory unchanged.

  Psi-owned pre-resolution evaluation now returns target-filterable syntax
  beside one opaque, non-cloneable pre-check continuation. That continuation
  privately retains the exact plan-laid rows, placed-view rows, and optional
  package-selection authority from the same pre-resolution run, then is
  consumed once against the matching typed tree. Both ordinary Omega frontend
  routes may interpose only target filtering, resolution, and typing; they no
  longer courier raw rows or select a second authority before pre-check.
  Const-length, const-domain, plan-laid, placed-view, and wire-plan sequencing
  remains Psi-owned and keeps its existing fail-fast order.

  Continue moving any remaining target-neutral generic/build-time probe
  sequencing out of `omega-compiler`; Psi owns those services and normalized
  plan carriers, while Omega owns target filtering and ABI/provider realization.
  This is engineering, not a language-design blocker. Unsupported computed,
  chained, dynamic-receiver, unresolved-generic, non-checked-supply, and
  unresolved-machine-parameter shapes keep failing closed. Carry the settled
  `Placed<P, T>` non-runtime-field input paths and per-outcome dispositions
  through checked and terminal representations. The first bounded checked
  input carrier is live for direct concrete state references, including entry
  and subordinate states: it retains exact machine/state/parameter position
  and identity, reference access,
  binding mode, synthesized view, policy, producing `Policy::plan` machine,
  schema, and the complete validated placement rather than its compact report
  fingerprint. Value-form inputs remain fenced, and no
  runtime storage, accessor, provider, or ABI authority is created. The same
  bounded row now crosses Terminal through hermetic machine/state/parameter,
  policy, producing-plan-machine, and schema identities, a canonical
  policy/schema-derived view identity, plus the plan's
  domain-separated canonical layout/access/reach commitment and compact report
  coordinate. Canonical codec order and independent representation validation
  reject missing machines, duplicates, reordering, noncanonical hermetic or
  derived identities, owned access, and zero commitments. Per-outcome
  dispositions remain open. Relevance does not invent a runtime carrier or
  public ABI for otherwise non-layoutable types.
- **EFFECTFUL-TYPED-COMPUTATION:** specify the value/computation judgments
  connecting effectful machines to the future typed proof calculus. Treat both
  migrations as staged semantic work, not prerequisites for extending the
  existing terminal vocabulary.

Acceptance: a canonical terminal artifact can be verified after source and
producer state are discarded; the verifier independently reconstructs every
obligation and rejects missing/extra/mismatched evidence; interpretation and
native execution consume that same verified artifact; proof replacement does
not change semantic identity. Crash sites are never represented as ordinary
terminal transitions or absent cleanup, and concrete safe invocations can
disprove all crash routes.

### P4 — Calling plans, final footprints, and callbacks

Owners:

- `wiki/design_briefs/calling_plans.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_23_inline_assembly.md`

#### ENT2c — normalized ABI lowering

- Finish foreign-storage custody and provider-view invalidation. Borrowed
  custody ends at return; durable retention consumes an owned claim and ends
  through a receipt. Successful bodyless terminal boundary calls now retain the
  exact verifier-derived completion-receipt set through canonical encoding,
  interpretation, native lowering, machine-code evidence, and installation;
  provider rejection records no receipt and leaves custody live.
  Final object construction and decoded installation replay now also require
  canonical receipt order and reject duplicate acknowledgment of one claim
  across foreign arguments; argument bounds remain independently fail-closed.
  Boundary completion now also retains the exact caller entry/content-claim
  source catalog through abstract planning, target assignment, machine
  settlement evidence, final images, and canonical installation records.
  Object and installation replay independently reject missing, extra,
  reordered, duplicated, or source-mismatched receipts after terminal source
  state is discarded.
  The checker accepts only one compatible consumed input
  for inferred post-return custody and rejects borrow-only sources. Ambiguous
  multiple-owned sources are accepted only when an exact authored equality
  relates one whole input entry projection directly to the whole current result
  projection in the same content algebra; partition/subplace equations and
  borrowed selections remain fail-closed. Primitive result-bearing calls now
  carry exact whole-root receipts from source checking through terminal Psi
  encoding, verification, and retry-safe interpretation. Omega retains the
  result in its abstract plan and rejects the old metadata-only settlement path
  rather than dropping it. An admitted x86-64 `u8` port-read provider now has
  an exact result-returning native realization whose arguments, receipts,
  instruction interval, and provider identity survive installation. Its exact
  checked terminal result value identity, unsigned `u8` scalar type, and native
  result placement now survive as one tuple through machine settlement and
  canonical installation records. Final replay rejects missing, mistyped,
  misplaced, metadata-only, or unsupported-target tuples. The tuple also
  retains its exact returning terminal edge through machine settlement and
  canonical installation format 29; object and installation replay bind that
  edge to the unique one-unit return-instruction fuel interval and reject edge
  drift independently from value, type, and placement. Other result shapes and
  targets remain fail-closed. AArch64 `u8` ABI result placement exists, but the
  sole result-bearing Terminal provider is the x86 `in`-port realization;
  AArch64 needs an admitted target operation/provider contract with exact
  hardware authority and state footprint before result/receipt/edge custody can
  land as one vertical. Explicit provider views now
  borrow one linear validity claim: consuming invalidation is accepted after
  the view's last use and rejected while the view remains live. Projected/
  content-bearing result calls remain fail-closed. Provider-view invalidation
  remains checked-only: native custody needs a Terminal-Psi validity-claim/
  invalidation identity carrier and the complete projected/content-bearing
  boundary-result vertical. Physical external-loan receipts additionally lack
  an authored correlation to terminal completion claims; an Omega-only bridge
  would invent custody semantics.
- **WRITE-ONLY-BORROW — finish the settled `&write T` access mode.** The first
  checked whole-value, fixed-byte-element, and nested plain-record-field rungs are
  live. The parser accepts
  `&write T`,
  `&'a write T`, `&write self`, and expression-form `&write place`; syntax,
  resolved, typed, checked, state, and control representations retain exact
  shared/mutable/write-only access through one closed `Borrow` node and access
  enums, with no compatibility dereference that can erase the mode. Display,
  snapshots, matching, normalized identity, loans, and call-access facts keep
  it distinct. Checked Omega bodies may explicitly attenuate an exclusive
  mutable whole place to `&write` for an unrestricted primitive scalar or fixed
  recursively literal-length array whose ultimate element is either an
  unrestricted primitive scalar or an eligible material nongeneric,
  invariant-free `[copy]` record or sum, or for a closed material nongeneric,
  invariant-free `[copy]` sum, replace the whole referent, and forward the loan
  only through an explicit `&write` argument.
  Such a checked fixed array additionally permits literal or dynamic element
  replacement after the ordinary range checker proves the index in bounds.
  Literal mutation and caller-visible write frames retain the exact
  `FixedIndex`; a dynamic index retains its runtime expression internally and
  conservatively invalidates the whole collection in the caller-visible frame.
  A fixed byte array also permits replacement of a
  statically normalized half-open range by a same-width array literal. The
  mutation and invalidation facts retain an exact `FixedRange`; half-open
  overlap preserves untouched siblings, while range loans and Terminal/native
  lowering remain gated. Replacement through a finite common-field path is now
  admitted for a non-generic checked record with no authored default domain,
  provided every intermediate receiver is likewise a non-generic checked
  record with no authored default domain, every selected field is relevant and
  unconstrained, and the displaced leaf is an unrestricted primitive or a whole
  supported literal-length fixed array. An eligible path may
  now also end in a whole nongeneric, invariant-free `[copy]` record leaf or a
  closed material `[copy]` sum leaf. Replacement treats either leaf as one
  freely discardable value and retains one final `Field` segment without
  decomposing or observing its members, case, or payload; the incoming sum
  supplies the complete tag and payload. Affine or linear, generic, qualified,
  invariant-bearing, quotient, erased, and other non-discardable leaves remain
  fenced. Sum case/payload projection and sum arrays remain fenced. The ordinary
  mutation summary retains the complete exact, ordered field-symbol path;
  nested array replacement introduces no element or range segment. Such an
  eligible record path may additionally end in one in-bounds literal or
  ordinarily proven-in-bounds dynamic element of that fixed-array leaf. Its
  checked mutation and caller-visible write frame retain the ordered field
  symbols followed by the exact `FixedIndex` for a literal or runtime `Index`
  for a dynamic expression. Literal siblings remain distinct; existing overlap
  and invalidation conservatively treat the runtime index as the whole array
  leaf without losing disjoint record siblings. The same
  eligible path may end in a statically normalized half-open byte range with a
  required known end and an exact-width array-literal replacement. Its checked
  mutation and caller-visible write frame retain the ordered field symbols and
  exact `FixedRange`; existing half-open overlap preserves adjacent windows
  and record siblings. The same statically normalized closed-range replacement
  now applies to every supported literal fixed array, directly or through an
  eligible record path. Primitive and eligible `[copy]` record or sum elements
  each remain one atomic array position: bounds normalize to one exact half-open
  element-ordinal `FixedRange`, and the replacement remains an array literal of
  exactly the same element count. Fixed/runtime element frames likewise retain
  only `FixedIndex`/`Index` and never decompose record members or sum tags and
  payloads. Recursively literal fixed arrays now participate in that same
  operation set, but each selected inner array remains one atomic outer element
  or range position; a second inner index is not an admitted place. Symbolic or
  open-ended ranges, slices, and nonliteral replacements remain fenced. Atomic,
  qualified, constrained, generic, erased, noncopy, and other non-discardable
  ultimate element forms still reject; matching and case/payload projection
  remain observations rather than array operations.
  A direct `&write [u8]` root may read its exact `.len` descriptor metadata,
  and a direct supported literal fixed-array root may read `.len` as static type
  metadata; neither inspects the referenced bytes. The same static `.len`
  metadata is now readable for any literal-length fixed array reached through
  a finite eligible common-field path of plain invariant-free records. Generic,
  qualified, constrained, sum/case, and record-held slice paths remain fenced.
  The slice may replace one
  byte through a runtime index whose ordinary range obligation is proved
  against that length. The checked mutation and caller-visible write frame
  retain a runtime `Index`, which existing overlap and invalidation
  conservatively treat as the whole slice. Other descriptor/member names,
  record fields named `len`, metadata reached through a record-held slice,
  elements reached through a record-held slice descriptor,
  whole-slice replacement, and slice ranges remain fenced.
  Whole aggregate replacement still requires an unrestricted/discardable root
  with an admitted closed shape.
  Referent observation, readable
  widening, implicit `&mut` attenuation, symbolic/open-ended ranges, sum
  projection, qualified fields, invariant-bearing records, and
  bodyless/provider declarations reject with directed diagnostics. Focused
  parser, semantic pass/fail, exact-place/frame, checked-loan, and
  checked-to-state-to-control remap tests pin the live slices.

  A bounded direct-call subloan rung now also admits
  `&write root.field...leaf` only at the immediate checked-call argument when
  every segment is an eligible common field and the leaf satisfies the existing
  non-observing replacement referee. One successor additionally permits that
  field path to end in exactly one in-bounds literal index of a nonempty literal
  fixed-array leaf with an unrestricted primitive element. Checked and Terminal custody retain the
  ordered `Field` identities followed by the exact `FixedIndex`; the independent
  verifier replays the field and array shapes, bounds, type, multiplicity, and
  write-only access. Reusable local aliases, direct-root indexing, dynamic or
  range projection, a second index, case/payload, qualified/generic/invariant/
  constrained paths, aggregate or recursively nested array elements,
  multi-parameter structural calls, and provider boundaries remain fenced.
  Exact unrestricted record-leaf and literal-index canaries cross
  checked Unit planning, Terminal codec replay, and verification. Neither form
  creates a Terminal write event.

  The first forwarding-only Terminal rung is also live. Checked and Terminal
  structural parameter and call-argument rows carry a closed
  owned/shared/mutable/write-only access value independently of structural type
  identity. Real source proves shared forwarding and explicit mutable-to-write
  attenuation across lowering, canonical format 27 encoding/decoding, and
  independent verification. The Terminal verifier rejects argument/target
  access disagreement, access not supplied by the source place, overlapping
  exclusive arguments, and structural Boolean observation through write-only
  access. That forwarding checkpoint did not itself admit a Terminal write
  event or native/provider realization.

  A bounded executable Terminal rung is now live for one direct whole-root
  unrestricted primitive integer replacement inside a checked in-module Unit
  callee. Checked planning retains the exact literal expression and
  write-only parameter coordinate. Terminal format 42/vocabulary 45 retains an
  honest `PrimitiveScalar` structural referent plus a Unit
  `WriteOnlyPrimitiveStore` naming the destination place and preceding SSA
  value. Codec replay and independent verification require exact scalar type,
  dominating value, parameter-root place, `WriteOnlyBorrow` access,
  unrestricted multiplicity, and empty qualifications/claims. The reference
  interpreter mutates stable target-neutral backing shared across call frames;
  fuel is charged before mutation, so exhaustion and resume neither partially
  commit nor replay the store. Omega target-neutral lowering now retains that
  exact store through abstract operations and optimization identity/validation:
  the complete structural-parameter row and typed preceding SSA use remain
  bound, scalar rewrites update only the value use, and dead-scalar removal
  cannot drop the structural-state event. Abstract-to-target lowering stops at
  a dedicated `UnsupportedWriteOnlyPrimitiveStore` fence before structural
  layout can manufacture address, width, or store authority. Opaque provider
  candidates and physical realization remain closed until non-observation and
  physical address/width/store custody are specified.

  Remaining work is the broader executable access discipline: add
  broader content-independent aggregate and symbolic range projection, finer
  symbolic dynamic-index footprints,
  reject take/swap/read-modify-write, content-driven projection,
  non-discardable displacement, and invariant restoration that depends on
  reading the referent, and retain exact per-outcome write footprints so
  untouched ranges and their facts survive. Carry the admitted operation set
  through provider selection, broader Terminal write operations, the remaining
  execution/native engines, and physical ABI lowering. Opaque providers still
  need a specified, implementation-pinned non-observation judgment; do not
  infer one from ABI shape. Migrate byte-output boundary surfaces only after
  that gate exists, and never reinterpret
  `&write` as vacant storage or typed construction.
- **BORROW-PROOF-CONVERGENCE — make ordinary borrow checking a proof-producing
  compatibility tactic.** Preserve existing `&T`, `&mut T`, and `&write T`
  source syntax. Keep loan existence, owner provenance, access polarity,
  temporal containment, and restoration in the Type/resource ledger. Permit
  `Prop` only to establish relationships over already-existing, versioned
  values, places, and authority occurrences; it must never create, amplify,
  transfer, extend, return, consume, or duplicate authority.

  Land the work in independent rungs. First normalize symbolic half-open place
  ranges so identical/forwarded bounds and structural adjacency prove ordinary
  disjointness without requiring literal endpoints. Then introduce one
  canonical captured-place compatibility judgment for spatial disjointness,
  spatial containment, and non-interference. Structural, literal, symbolic,
  domain, arithmetic, and explicit-theorem reasoning are tactics consuming the
  same path- and version-valid fact context, never a sequence of fallback
  obligation kinds.

  The first normalization rung is live for exact integer bounds, exact resolved
  bare-name bounds, and finite immutable local bare-name copy chains. Two
  half-open windows whose exclusive end and start normalize to the same value
  are ordinarily disjoint. Inclusive symbolic ends, mutable or computed local
  aliases, ambiguous local identities, cycles, and distinct unresolved bounds
  remain conservative.

  The transient checked structural judgment is now canonical for ordinary
  borrow conflicts. `CapturedPlace` identity is one exact root symbol plus its
  ordered field, case, fixed-index, fixed-range, or retained selector
  positions; compatibility reports spatial disjointness, directed containment,
  and access-aware non-interference independently. Read/read access remains
  non-interfering, while an exclusive access cannot use spatially disjoint
  sibling fields to evade a shared dependent-data fact. Existing literal and
  symbolic half-open tactics feed the same judgment, and unknown, mutable,
  computed, ambiguous, cyclic, and inclusive-symbolic selectors remain
  conservative. Access polarity stays in the resource ledger and is only a
  premise to this transient result. The first durable checked-only certificate
  rung now records every automatically admitted loan/loan non-interference
  result in a proof arena separate from the loan-resource rows. Each
  zero-premise `Structural` row retains the exact machine/state/statement
  formation coordinate, both exact state-owned loan handles, both frozen
  captured places, and the normalized disjointness/containment/non-interference
  conclusion. Every dynamic selector position consulted by that judgment now
  also has one ordered formation snapshot keyed by forming/active side, path
  segment, and scalar/start/exclusive-end coordinate. A row retains the exact
  normalized integer or immutable-symbol value, or closes the position as
  conservatively unknown. Checked replay independently normalizes the exact
  typed formation expression, requires equality with every frozen row, and
  then consumes the snapshot; runtime changes cannot retarget the retained
  immutable-symbol occurrence. A genuinely new pair still uses current
  formation normalization. Recording deterministically rebuilds on repeated
  checked-fact validation, and exact resource rejoin rejects changed handles,
  places, or formation coordinates without changing which programs admit.
  Checked-fact validation also independently recomputes the complete structural
  conclusion from the frozen places, selector snapshot, and resource ledger's
  exact access polarities. Missing, reordered, malformed, or conclusion-changing
  selector rows and duplicate or stale certificate roster entries reject
  alongside independent disjointness, containment, non-interference,
  derivation, and access drift; conclusion bits are no longer treated as
  resource facts. Proposition-consuming tactics, general captured value
  versions, premise tokens, dominance, richer proof derivations, and Terminal
  certificates remain open.

  The first checked Type/resource prerequisite is also live for direct-root
  loans. One deterministic non-authorizing row retains the exact state-owned
  loan, owner/path, captured place, access polarity, activation and weakening
  coordinates, parent state/root lifetime, and restoration obligation.
  Independent checked replay rejects missing, duplicate, state, place, access,
  formation, closure, reason, or restoration drift, and direct-root structural
  certificates must rejoin their two exact resource rows. This direct-root
  arena excludes reborrows and borrow-carrying transfers; these checked rows
  grant no Terminal authority.
  The first exact lineage rung is now live for uniquely resolved explicit
  reference-local reborrows. Each child retains its immediate prior same-state
  parent loan handle, and independent replay reconstructs source owner/path,
  formation order, and the parent's captured place plus the direct projection
  remainder. Invalid, self, later, sibling-state, unrelated, ambiguous, tag,
  source, or place substitution rejects. Aggregate/helper transfers remain
  explicitly unretained; reborrow lifetime/restoration resources and Terminal
  authority were still open at this rung.
  A separate checked reborrow-resource arena now closes the child side of that
  lifecycle. Each retained child links by typed handle to its exact direct-root
  or earlier reborrow resource, retains activation/weakening coordinates and
  reason, and carries a pending restoration obligation naming the immediate
  parent. Complete validation precedes an infallible topological rebuild that
  remaps both resource arenas' handles; missing, duplicate, reordered, parent,
  child, lifecycle, or restoration drift rejects transactionally. Certificates
  rejoin through the exact child resource. Reborrows whose parent lineage is
  unretained remain outside the arena, as do aggregate/helper transfers. The
  obligation does not prove parent activity, reactivation, temporal
  containment, completed restoration, or Terminal authority.
  The next checked-only join now retains the exact parent-suspension formation
  boundary for every retained child: one typed child activation and the one
  exact parent-loan constraint present immediately before it, both joined to
  the existing child/parent resource identities. Independent replay rejects
  missing, duplicate, reordered, substituted, or cross-state occurrences
  before either resource arena rebuilds. This boundary proves only that the
  parent occurrence was available when the explicit child reborrow formed.
  Parent activity after formation, suspension-interval containment,
  reactivation, completed restoration, and Terminal authority remain open;
  current lexical weakening rows cannot establish those claims.
  A checked-only weakening-order join now retains the exact parent and child
  weakening handles plus a closed lexical status at child end: parent retired
  before, retired at the same boundary, or remained live past the child.
  Ordering is derived from semantic statement phases (`LastUseExpired` before
  entry, `LocalReassigned` after the right-hand side, and `StateExit` last),
  never raw arena insertion order. Replay rejects handle, resource, status, or
  phase drift transactionally. This classification remains purely lexical; it
  does not prove suspension containment, authority return, reactivation,
  cascade through retired parents, completed restoration, or Terminal
  authority. A checked-only resource-lifecycle arena now independently merges
  those activation and weakening events in the same semantic phase order. Its
  access-exact state distinguishes an available carrier, a carrier suspended
  by one exact exclusive child, a mutable carrier frozen by an exact shared
  cohort, and either carrier retired while its descendants still own the
  pending route. The checked representation retains each child's exact parent
  access and classified effect. `Read -> Read` releases independently;
  `Mutable -> Read` joins one cohort and restores the parent's original access
  only after its final member ends; `Mutable -> Mutable`, `Mutable ->
  WriteOnly`, and `WriteOnly -> WriteOnly` retain the exclusive suspension
  path. The other four access cells reject with directed borrow diagnostics
  before lifecycle reconstruction. Bare authored `&` now survives parsing as
  an exact shared-borrow node, closing the source/certificate correspondence
  without treating an ordinary reference-name use as a reborrow. Nine-cell,
  concurrent-sibling, final-restoration, and transactional tamper canaries are
  live. Each row keeps exact child, parent, resource, activation, weakening,
  cohort/path, phase, and target identity; missing, reordered, substituted, or
  drifted rows reject before resource arenas rebuild. The former overloaded
  terminal disposition is now split exactly: a same-boundary lineage closure
  ends at the coincident retained carrier, while only a `StateExit` event whose
  final target is the exact direct-root lifetime records direct-root handoff.
  Swapping either label, phase, path, or target rejects under independent
  replay. These dispositions remain checked-only and grant no restored use,
  cleanup, root custody, or Terminal authority. A sibling checked-only
  suspension/freeze-containment arena is now live for every retained
  `Mutable -> Read` freeze and permitted exclusive suspension. Each row rejoins
  the exact child and typed parent resources, both access polarities and their
  classified effect, child activation and parent-entry formation identities,
  both weakening identities, and the exact parent/child captured places plus
  their ordered projection remainder. `Read -> Read` releases retain no such
  row. The complete semantic-phase lifecycle replay must succeed before these
  rows are derived; independent replay rejects missing, duplicate, reordered,
  access-amplified, retargeted, or otherwise drifted containment evidence before
  rebuilding either resource arena. The evidence remains non-authorizing and
  proves no completed restoration or Terminal custody.

  Complete the settled reborrow-restoration model:

  - publish restored use or root custody to Terminal only after independent
    replay. Root handoff must not authorize cleanup, transfer, or linear
    discharge.

  Acceptance: every access cell has a positive or directed negative canary;
  concurrent shared descendants do not produce internal drift; premature or
  duplicate cohort restoration rejects; access amplification, missing
  containment, changed path/order/target, and cleanup inferred from root
  handoff reject; and checked plus Terminal replay agree transactionally.

  Loan formation freezes exact owner/place occurrences and evaluated range
  values. Every premise must dominate the formation event and be valid at the
  captured versions; the conclusion is scoped to the resulting loan
  occurrences and does not expire merely because a selector expression later
  changes. Reject any derivation depending on the loan or effects it is being
  used to authorize. Empty footprints may discharge every relational conflict
  while retaining complete Type-side lifetime and return obligations.

  Retain a checked and Terminal compatibility certificate naming the exact
  formation event, captured loan/place identities, normalized conclusion,
  premise fact tokens, and derivation. Terminal independently reconstructs
  dominance/path availability, checks premise versions and validity, replays
  the proof, matches captured places to the resource rows, and separately
  replays owner lineage, polarity, temporal containment, and restoration.
  Diagnostics remain borrow-oriented unless source explicitly cites proof
  vocabulary.

  Do not add public footprint syntax in this task. Ordinary contracts expose
  value relationships such as index inequalities. Defer abstract footprint
  contracts until an opaque modular API requires them, and keep semantic
  `Content<A>`, logical place footprints, and physical effect footprints
  distinct except through explicit checked carrier/operation bridges.

#### ENT4 — registered callbacks

- **CALLBACK-PARAMETER-REQUIREMENT — implement the settled nominal binder.**
  Parse and resolve `where machine Selected satisfies Trait::requirement`,
  deriving the complete callable contract from one uniquely resolved
  requirement row. Centralize that exact resolver for domain `established by`
  clauses and every other signature-free requirement site. Reject overloaded paths,
  structural coincidence, and visible-unique selection. Retain a checked
  per-use row with call site, static-machine ordinal, selected
  machine/satisfaction row, exact requirement overload, separate published and
  actual envelopes plus their refinement proof, and the target entry recipe.
  Separately evaluate and validate the registrar's fixed callback-
  materialization row from exact binder-slot identity to one nominal native
  parameter or validated layout-field place. Its fingerprint must not include
  the selected callback machine. Emit the private
  relocation only from validated binding lowering. Registration is linear,
  explicitly unregisters, retains required code/component leases, and keeps
  selected identity in provenance without importing narrower facts unless an
  API contract forwards them. Add declaration-side compatibility reporting
  when a new overload makes signature-free references ambiguous, with pass/fail
  canaries covering both callback binders and domain establishment clauses.

  The declaration and admission slice is implemented. Syntax, resolved, and
  typed trees retain a discriminated structural-or-nominal contract; nominal
  binders normalize to one exact trait/requirement symbol pair through the
  shared signature-free resolver. Selection requires one explicit satisfaction
  row for that exact requirement, rejects structural coincidence and a row for
  another trait, and keeps nominal and structural specializations distinct in
  template identity. Checked-only filesystem pass/fail canaries now pin unique
  and overloaded signature-free paths for both nominal callback binders and
  authored domain establishment clauses. Declaration-side compatibility
  reporting is also implemented: after symbols are assigned and before authored paths are
  normalized, one diagnostic names each overloaded declaring-trait family and
  source-ordered diagnostics name every affected nominal binder or domain
  establishment clause. The checked identity spine is also implemented:
  every admitted nominal use retains its exact statement/expression site,
  static-machine ordinal, registration operation, selected machine and entry,
  unique satisfaction trait/requirement, and canonical requirement-overload
  identity. Validation captures that authority before specialization consumes
  the authored arguments and again after each fixed-point cloning round, while
  structural machine parameters publish no nominal row. Each row now also
  pins the normalized published-requirement contract identity separately from
  the selected machine's normalized declared contract identity and retains an
  explicit admission-refinement receipt binding those endpoints. Requirement
  capsules now also retain canonical published service reach and synchronous
  invocation rows plus suspension, blocking, termination, and crash ceilings.
  A separate exact-machine realized envelope aggregates effective checked
  reach/invocation, transitive suspension/blocking, checked termination/crash,
  mutation frames, and capability-flow evidence without relabeling any of it as
  public contract identity. Crash evidence is refreshed after path-conditioned
  checked validation, rather than snapshot before that pass. Resource ceilings
  remain independent until their checked representation exists. The
  checked row now also retains an optional nonzero evaluated boundary-calling
  plan fingerprint. Ordinary nominal binders retain no callback placement;
  boundary callback uses gain the exact join key that target lowering must use
  to recover its already-evaluated `BoundaryEntryPlan`, without exposing a
  runtime code address. Both check-only and native orchestration immediately
  consume that key, revalidate the retained target plan, and reject missing,
  duplicate, invalid, or fingerprint-drifted realizations before backend
  lowering. That join now materializes one target-owned callback-use/thunk row
  containing the exact nominal-use site and ordinal, selected machine/entry,
  satisfaction identity, fingerprint, and validated `BoundaryEntryPlan`.
  The normalized outbound `CallPlan` foundation now owns the separately
  settled fixed registrar binder-slot-to-native-place catalog. A focused
  348-line owner defines nonzero nominal binder, requirement, native-parameter,
  layout, and field-slot identities; direct-parameter and nested-field places;
  typed private demands; exact closure validation; and ABI fingerprinting.
  Bare plans cannot validate nonempty catalogs, while the context-bound path
  rejects missing, duplicate, unknown, overlapping, empty-path, and
  requirement-incompatible rows. Empty ordinary plans preserve their prior
  identities. The closed source calling vocabulary now publishes the bounded
  callback-materialization catalog and direct/nested `NativePlace` grammar;
  build-time decoding preserves nonzero binder, parameter, layout, and ordered
  field-slot identities, rejects invalid tags/counts/zero identities/empty
  paths, and never silently drops a nonempty row. The ordinary no-context
  validator continues to reject nonempty catalogs. The compiler now publishes
  the nominal binder half of that validation context: binder identities derive
  from the exact owner overload plus the compiler-static-machine ordinal and
  binder name, while requirement identities derive from the exact canonical
  target-requirement overload. It retains each opaque identity beside the
  exact parameter, trait, requirement, and ordinal symbols on the boundary
  realization, rejects invalid or duplicate retained rows, and publishes the
  bounded catalog to calling policy. The source prerequisite for the
  compiler-known slot trait is now explicit: trait declarations accept
  proof-static `machine`
  parameters as requirement-identity binders, conformance arguments retain a
  source-backed qualified path, and resolution accepts exactly one
  signature-free `Trait::requirement` declaration while rejecting ordinary
  types, concrete machines, unknown traits, and overloads. That distinct kind
  survives syntax snapshots, resolved and typed trees, and checked
  proof-contract traversal. Public package review v56
  now retains it as a closed payload-free parameter kind, distinct from both
  structural and nominal machine contracts. The core now publishes the inert
  `PrivateCallbackSlot<machine Requirement>` declaration, and package review
  retains an explicitly named public conformance with its exact toolchain-owned
  trait and requirement-identity argument. No ambient lookup is introduced.
  Native layout evaluation now publishes
  the target-neutral half of the other catalog: an executed sealed
  `Plan::place_private<Conformance>` call returns the ordinary `Plan` unchanged
  while recording the exact closed conformance, active layout subject,
  canonical signature-free callback requirement overload, and authored
  offset. Plan-laid layouts retain those rows beside semantic geometry;
  ordinary layout consumers reject rather than silently discard them, and
  native layout identity includes them. The remaining slice is to close each
  row with the selected target's function-pointer extent/alignment, prove fixed
  bounds and semantic/private and private/private non-overlap, publish the
  resulting demand catalog to `BoundarySignature`, derive the layout half of
  `CallbackMaterializationContext`, admit the outbound plan through that
  context, and retain the same context through later callback-placement
  revalidation before private relocation emission.
  The declaration shape is now settled. A target package declares one stable
  typed slot as an explicitly named
  `Layout satisfies PrivateCallbackSlot<Trait::requirement>` conformance, and
  its layout policy explicitly cites that exact evidence through the bounded
  `Plan::place_private` vocabulary. The conformance is inert until cited; no
  ambient lookup or owner-only exception exists. Its subject and static
  argument derive layout/requirement identity, while the evaluated plan derives
  the physical offset. The authoritative layout may author or compute that
  offset, but neither the slot identity nor the calling plan contains a
  repeated raw offset. The source ABI, non-callable requirement-identity binder,
  sealed evaluator receipt, exact wrong-layout/overload/duplicate checks, and
  target-neutral catalog are implemented. Target closure mints `LayoutPlanId`
  and `LayoutSlotId` and publishes the exact catalog to the existing closure
  validator in the next rung.
  Checked-only compilation exposes those rows and native compilation retains
  them on `BackendPlan`, so no later thunk pass may replace the recipe with a
  convention oracle or silently discard it. Native backend planning now also
  resolves each selected machine/entry pair to one exact `ControlFlow`
  `StateKey`, rejects a lost entry before instruction selection, and assigns a
  deterministic compiler-private thunk symbol joined by placement-row index.
  That identity includes every source/selected handle generation and duplicate
  private identities reject before instruction selection.
  The canonical checked-to-Terminal product route now carries this sidecar as
  an opaque by-value input/output beside the source-free artifact. Success and
  rejection return the exact ordered rows, and the compiler report retains the
  artifact and rows as one product with no consuming artifact-only escape.
  The driver supplies checked provenance and order; the report carrier only
  replays each row's structural validity and cannot reconstruct that
  provenance independently. Check-only requests remain valid and retain the
  rows. The canonical native-realization seam now also offers an opaque
  by-value adapter that returns the exact sidecar beside either its source-free
  native artifact or diagnostic rejection. It does not inspect, admit, lower,
  or fingerprint the rows, and the neutral `NativeArtifact` remains free of
  checked/source carriers. Native artifact requests remain fenced until the
  compiler report and publication route preserve the sidecar without an
  artifact-only escape; the deleted legacy backend route is not authority to
  discard it. These custody milestones grant no callback registration,
  invocation, address, lifetime, or publication authority.
  The symbol is planned object identity only and never an Omega value. A
  callback thunk now also has a distinct machine-function identity bound to
  its placement-row index and selected source entry, so an ordinary source
  function wearing the callback symbol cannot satisfy emission. That role
  survives the existing assigned-operation, machine-instruction, byte, and
  object carriers; object planning preserves its richer placement-derived
  symbol, and final emission requires the encoded identity to equal the exact
  planned callback role. Target-instruction lowering now validates the whole
  assigned function set before selecting any body, rejecting invalid roles or
  two functions that claim one source, wrapper, or callback identity instead
  of deferring ambiguity to object planning. Each internal direct-call target
  must resolve in that same exact assigned identity set, so role, callback
  placement, continuation-generation, or absence drift rejects before
  placeholder encoding. This pins the eventual thunk-to-selected-entry call
  edge but does not synthesize that thunk body. Missing, duplicate, redirected,
  role-drifted, or interval-drifted identities reject, so a plan row cannot be
  mistaken for emitted thunk evidence. Final compiler-function replay now also
  rejects invalid or duplicate identities and fingerprints each exact role,
  continuation handle/generation, segment, and callback placement alongside
  its byte/instruction partition. Role substitution therefore changes final
  derivation evidence even when all byte intervals remain unchanged. The same
  final replay independently resolves each encoded identity through one exact
  object function binding and requires its text-symbol interval to equal the
  encoded byte interval. Missing, duplicate, redirected, non-text/function, or
  interval-drifted bindings therefore reject uniformly for source, wrapper,
  and callback roles; object-local private spelling is not confused with an
  encoded source display name. Final replay independently rederives the target
  entry name and every non-entry source/wrapper private name; linkage renaming
  rejects, and callback identities cannot replace the process entry. The
  shared private-name primitive now binds role, machine/state arena indices
  and generations, and segment, so handle-generation drift changes linkage
  spelling instead of aliasing the earlier generation. The richer callback
  name remains bound by the placement-specific join. Final
  image construction now revalidates its copied function-symbol carrier before
  format emission: the entry handle and every identity-owned function name,
  text classification, interval, and kind must match exactly, while unowned or
  multiply owned function symbols reject. This does not expose an address or
  synthesize a body. After format placement, checked emission also rejoins each
  encoded identity/object symbol to exactly one compiler-function region with
  the same symbol, section offset, address, byte count, and final-byte
  fingerprint. Missing, duplicate, renamed, reclassified, or byte-drifted
  regions reject; import thunks remain a separate region namespace. Before
  consuming those rows, checked emission independently replays the complete
  placed inventory from final text: exact text identity, ordered region spans,
  derived addresses, per-span bytes, complementary gap partition, retained
  origin/footprint metadata, and aggregate inventory identity must agree.
  Stored-summary, overlap/order, origin, address, byte, or gap drift therefore
  rejects without claiming callback-body synthesis or registration-relocation
  placement. Compiler-function evidence now additionally retains the exact
  ordered identity-to-object-handle-to-final-region join, including the
  inventory identity and each region's index, symbol, address, interval, and
  byte fingerprint. Its binding fingerprint participates in the function
  evidence and final text derivation, preventing a validated function
  partition from being paired with another independently valid inventory.
  Boundary-footprint attachment now consumes an exact sealed entry projection
  of that join rather than searching by linkage spelling. The compiler-private
  identity, object symbol handle, region index and final span, inventory
  identity, and whole function-region binding identity participate in its own
  replayed fingerprint; any identity/handle/row/custody drift rejects before
  the inventory is mutated. That mutation now returns a checked custody receipt
  joining the sealed entry projection and whole function-region binding to the
  prior inventory, exact composed footprint, and resulting inventory.
  Boundary-bearing final footprint certificates require and fingerprint this
  receipt; missing, stale, redirected, or pre/post-inventory drift rejects. The
  complete certificate is now constructed and revalidated before executable or
  app-bundle installation, and auxiliary inventory serialization consumes that
  existing certificate rather than discovering a semantic failure only after
  executable bytes become visible. Publication now additionally seals that
  certificate to the exact emitted image evidence, final-text/inventory pair,
  output name and format, and complete container-byte identity. Flat executable
  and app-bundle installation consume only this validated view, so certificate,
  container, or output-identity drift rejects before either byte copy is
  published. Each executable copy now also replays the complete staged file
  byte-for-byte before its atomic rename and returns an exact installation
  receipt binding the publication identity, output path, byte count, and
  container identity. A redirected name or changed/partial staged file is
  removed and rejects before becoming visible. The compile report now retains
  that exact native-executable receipt through the orchestration return
  boundary, including certificate, inventory, publication, path, container,
  and installation identities. Check-only and object-container fallback paths
  retain no such receipt; it remains artifact custody, not runtime loading
  authority. Receipt minting now occurs only after the renamed destination is
  independently read and compared byte-for-byte with the sealed container.
  Missing or changed destination bytes are removed and reject before the
  orchestration return can expose a receipt or path. Mach-O GUI builds now
  retain the optional app-bundle executable receipt separately from the flat
  executable receipt; both bind the same publication/container identity but
  their exact destination paths cannot substitute for one another. Other
  targets retain no bundle receipt. Before returning the report, orchestration
  independently validates the pair: a bundle receipt requires one flat
  receipt, equal certificate/inventory/publication/container identities and
  output leaf, plus distinct paths and installation identities. Missing,
  substituted, or self-aliased pairs reject atomically. The compile report now
  also retains the exact output category. `NativeExecutable` requires the flat
  receipt, `ObjectContainer` requires both executable receipts absent, and
  `CheckOnly` requires no output or receipt. A dropped native receipt can no
  longer masquerade as a legitimate object-container fallback. Each receipt
  additionally retains its exact destination role. The native flat slot accepts
  only `FlatOutput`, the optional bundle slot accepts only `MacOsAppBundle`, and
  the role tag participates in installation identity; swapped otherwise-matching
  receipts reject. The bundle slot also rederives the canonical
  `<build>/<sanitized-project>.app/Contents/MacOS/<executable>` path from the
  report root and flat output, so a same-leaf receipt under another directory
  cannot substitute. The report root is now outwardly read-only because it
  participates in that derivation; a caller cannot redirect it after final
  validation and thereby change the canonical bundle identity. Immediately
  before either outward receipt is minted,
  installation replays the renamed destination bytes once more against the
  sealed container; interval drift removes the changed file and rejects instead
  of returning stale custody. The validated output flag, category, flat receipt,
  and optional bundle receipt are now outwardly read-only, so a report consumer
  cannot rearrange or drop one component after the compiler's final consistency
  check. Both early check-only and backend reports now use one checked
  constructor, which rejects an inconsistent output/category/receipt tuple
  before it can cross the orchestration return boundary. The constructor also
  rejoins the optional program-storage entry binding to its native bridge: both
  are absent together or the retained binding must equal the bridge's exact
  binding, while a dropped, unpaired, or redirected row rejects before return.
  Both retained fields are now outwardly read-only, so a consumer cannot mutate
  one side into a post-validation mismatch. Report construction also joins
  bridge phase to output category: check-only retains a pending bridge without
  final wrapper evidence, native executable output requires that evidence, and
  object-container fallback cannot carry a program-storage bridge. A native
  bridge's final wrapper evidence must also name the same executable-region
  inventory fingerprint as the flat publication receipt, preventing evidence
  from another valid final image from accompanying the published container.
  Receipts now additionally retain the sealed compiler-text derivation and
  compiler-function evidence fingerprints; flat/bundle copies and native
  wrapper evidence must rejoin the same pair rather than inventory alone. The
  receipt also retains the certificate's optional boundary-contract
  fingerprint; flat/bundle copies agree and native program-storage arrival
  evidence must name that same concrete contract. Report validation now also
  rejoins the retained selected entry binding's boundary-contract fingerprint
  directly to the native flat receipt. Check-only retains that binding without
  publication, while object-container output cannot carry it; matching arrival
  evidence therefore cannot conceal a redirected selected binding. Each
  receipt's installation seal is now independently recomputed from its exact
  destination role, publication identity, output path, and container byte
  identity. Flat-only and flat-plus-bundle reports reject a stale or
  substituted seal without relying on pairwise inequality alone. The private
  written-output handoff now also requires a native output path to equal its
  flat receipt's installed path before auxiliary reporting or report
  construction; object output carries no executable receipts, and check-only
  cannot masquerade as a written output. Before that handoff is consumed, its
  optional bundle receipt must also satisfy the same canonical root-derived
  path, role, shared publication/certificate/container identity, valid
  installation seal, and distinct-installation checks that final report
  construction independently replays. Native execution consumers no longer
  reconstruct an executable leaf from the build directory: the report returns
  its exact flat receipt path only after replaying complete publication and
  program-storage custody, and `omega-run` consumes only that checked path. The
  shared report-and-capability native runner now likewise consumes the checked
  report for all ten of its executions rather than reconstructing a name from
  the build directory; bundle-path tampering is pinned to expose no executable.
  The exact-native source index accepts that form only for an exact report-local
  binding plus literal exit status, adding seven unique rooted owners (795
  total); the twice-owned linear-transfer fixture remains fail-closed and
  unelided. The five authored-root value/type-check executions now also launch
  through their checked reports rather than reconstructing `out/<executable>`;
  their literal-status exact-owner identities remain unique, so the 795 pin is
  unchanged. Output-kind tampering is pinned to expose no native path.
  The first five authored-root value-call/dispatch executions likewise consume
  only the exact checked-report path; the 795 unique-owner pin remains stable,
  and compiler-function fingerprint drift between the flat and bundle copies
  exposes no executable path.
  The next five authored-root dispatch executions through the mixed return-type
  probe now use the same receipt-only launch boundary; the exact-owner pin stays
  795, and flat/bundle boundary-contract drift exposes no executable path.
  The following five value-call executions through the post-splice mutation
  probe also use the exact report receipt; the 795 pin remains unchanged, and
  flat/bundle executable-inventory drift exposes no executable path.
  Five further runtime executions—called-machine loop search, looping
  value/cast returns, the slice-length guard, and sleep—now use receipt-only
  launch. The 795 owner pin stays stable, and flat/bundle compiler-text
  validation drift exposes no executable path. Five additional authored-root
  native executions—write without newline, runtime exit code, borrow-carrying
  data-field access, and u8/i8 field arithmetic—now launch only from their
  exact checked-report receipts. The 795 exact-owner pin remains stable, and
  flat/bundle publication-evidence drift exposes no executable path. The first
  five authored-root range/storage executions likewise launch only from their
  exact checked-report receipts while retaining literal statuses 1, 1, 15, 7,
  and 9 and the guarded-binary cross-target check. The 795 exact-owner pin
  remains stable, and flat/bundle container-byte-count drift exposes no
  executable path. Five more authored-root range/arithmetic executions—guarded
  copy narrowing, ranged divide/modulo, ranged bitwise masking, and declared-
  range index read/write—now launch only from exact checked-report receipts
  while retaining literal statuses 7, 4, 3, 30, and 30. The 795 owner pin
  remains stable, and flat/bundle container-fingerprint drift exposes no
  executable path. Five additional authored-root range/indexed-structure
  executions—constant-expression range bounds, indexed struct-field read-
  modify-write and operand use, and machine-indexed scalar/struct-field
  arguments—now launch only from exact checked-report receipts while retaining
  literal statuses 40, 1, 1, 1, and 1. The 795 owner pin remains stable, and
  reused flat/bundle installation evidence exposes no executable path.
  Forty further authored-root indexed/slice executions now cross the same
  checked report boundary in eight exact cohorts. The first five cover by-value
  parameter/local indexed access; machine/frame read, write, RMW, dual-frame,
  operand, and argument use; nested and runtime-middle indexing; aggregate and
  cross-region indexed copies; and constant/computed index guards, all retaining
  literal status 1. Three later cohorts cover constructor/slice/member use
  (statuses 70, 1, 1, 1, 1), subslice/loop/post-clause delivery (3, 1, 1, 1,
  1), and slice-length/descriptor shrinking (5, 6, 3, 3, 3). The 795-owner pin
  remains stable. Certificate drift, flat/bundle substitution or omission,
  swapped destination roles, a dropped native-output flag, and related
  receipt-cardinality drift each expose no executable path.
  Five further authored-root subslice/carrier executions—runtime-end subslice
  element access, fixed-array length guarding, runtime-bounded subslice
  argument delivery, owned bounded-carrier concatenation, and borrowed bounded-
  carrier alias concatenation—now launch only from exact checked-report
  receipts while retaining literal statuses 20, 7, 3, 70, and 70. The 795
  exact-owner pin remains stable, and the existing receipt-drift matrix
  continues to expose no executable path.
  Five further authored-root carrier/control executions—frame-local bounded-
  carrier concatenation, slice-view carrier guarding, slice-view element
  argument delivery, linear-search early exit, and unary entry-result
  delivery—now launch only from exact checked-report receipts while retaining
  literal statuses 70, 70, 70, 70, and 1. The unary fixture retains its
  `linux_arm64` cross-target emission check, the 795 exact-owner pin remains
  stable, and the existing receipt-drift matrix continues to expose no
  executable path.
  Five further authored-root entry/control executions—computed entry result,
  widened cast result, nested-binary result, free-standing helper result, and
  iterative loop patterns—now launch only from exact checked-report receipts
  while retaining literal statuses 200, 70, 70, 7, and 70. The computed-result
  fixture retains Full emission for its boundary-footprint assertion, the cast
  fixture retains its `linux_arm64` output check, and the 795 exact-owner pin
  remains stable.
  Five further authored-root control/carrier executions—composite-initializer
  argument forwarding, captured-local preservation across source-field
  mutation, bounded-carrier pointee guarding, bounded-carrier slice-field
  writing, and Utf8 return-view equality—now launch only from exact checked-
  report receipts while retaining literal status 70 for every row. Stdout-
  bearing carrier probes remain unchanged, and the 795 exact-owner pin remains
  stable.
  Five further authored-root output/operator executions—bounded-carrier
  `write_line`, cross-state nested-carrier text building, shift operators,
  bitwise operators, and the popcount loop—now launch only from exact checked-
  report receipts while retaining literal statuses 70, 0, 70, 70, and 70. Both
  output probes retain their exact `Room A1` stdout assertions, and the 795
  exact-owner pin remains stable.
  Five further authored-root operator/value-call executions—xorshift PRNG
  composition, bitwise guard subjects, suffixed integer literals, value-
  position branching calls, and free-machine value calls—now launch only from
  exact checked-report receipts while retaining literal status 70 for every
  row. The 795 exact-owner pin remains stable, and the existing receipt-drift
  matrix continues to expose no executable path.
  Five further authored-root by-value machine executions—free-machine struct
  arguments, case-bearing parameter self-write, attached-machine struct
  arguments, record forwarding across a nested statement call, and free-
  machine struct returns—now launch only from exact checked-report receipts
  while retaining literal status 70 for every row. The 795 exact-owner pin
  remains stable.
  Five further authored-root machine/integer executions—free-machine mutable-
  argument value calls, looping free-machine value calls, widened integer
  comparisons, widened bitwise operations, and 16-bit cast roundtrips—now
  launch only from exact checked-report receipts while retaining literal status
  70 for every row. The 795 exact-owner pin remains stable.
  Five further authored-root versioning/equality executions—explicit version
  migration, two-era and three-era lineage matching, scalar `Equatable`
  equality/inequality guarding, and mixed-shape case membership—now launch
  only from exact checked-report receipts while retaining literal status 70
  for every row. The 795 exact-owner pin remains stable.
  Five further authored-root wire executions—max-one repeated-field roundtrip,
  honest Utf8 roundtrip, Utf8 edge-class validation, invalid-Utf8 refusal, and
  numbered schema-as-value use—now launch only from exact checked-report
  receipts while retaining literal status 70 for every row. Five more wire/
  comptime executions—decoded-field let comparison, repeated-then-string
  encoding, nested-plus-repeated roundtrip, transitive const-array length, and
  parenthesized bare-call-arm const-array length—use the same receipt-only
  launch boundary and statuses. The 795 exact-owner pin remains stable.
  The two-row scalar-operation entry-result probe now launches its builtin and
  comparison results only from exact checked-report receipts while retaining
  literal statuses 70 and 1. The 795 exact-owner pin remains stable. All
  four remaining authored-root numeric/float executions in this module—mixed
  numeric casts, float place comparison, float comparison guards, and float
  arithmetic—now also launch only from exact checked-report receipts while
  retaining literal status 70. The operator target-row regression is repaired,
  the 795 exact-owner pin remains stable, and this module no longer reconstructs
  any native executable from a build-directory/name convention.
  All five conventional native launches in the entry/ABI canary module—entry
  run-args, Utf16 literal delivery, case-array element writes, policy-authored
  wire plans, and nested policy-authored wire plans—now consume exact checked-
  report receipts while retaining literal statuses 5, 70, 36, 70, and 70. The
  run-args fixture retains Full footprint inspection, the nested policy fixture
  retains both cross-target checks, and the 795 exact-owner pin remains stable.
  All three conventional native launches in the artifact-footprint canary
  module—shared reference-parameter copy, pointee-pair copy, and record-view
  place addressing—now consume exact checked-report receipts while retaining
  literal statuses 42, 42, and 70. Artifact-producing cross-target tests remain
  unchanged, and the 795 exact-owner pin remains stable.
  The final conventional native launch in the reports/capabilities canary
  module—the linear obligation spanning a dispatched-call continuation—now
  consumes its exact checked-report receipt while retaining literal status 7,
  Full backend-report emission, and the complete permission-realization/event
  assertions. The 795 exact-owner pin remains stable.
  Five authored-root atomic executions—load/store ordering, fetch-add, fetch-
  sub, fetch-xor, and fetch-or—now launch only from exact checked-report
  receipts while retaining literal statuses 70, 70, 70, 70, and 75. Existing
  `linux_arm64` cross-target checks remain unchanged, and the 795 exact-owner
  pin remains stable.
  Five authored-root host/target executions—stdin command echo, qualified-case
  values, single-target internal filtering, target-machine gating, and ring-
  requirement conformance—now launch only from exact checked-report receipts
  while retaining literal statuses 0, 70, 70, 70, and 70. The stdin probe
  retains its exact `look\n` stdout assertion, and the 795 exact-owner pin
  remains stable. Five authored-root layout/generic executions—plan-laid value
  fields, erased plan-laid fields, distinct closed erased sums, mixed closed
  generic erasure, and exact generic call/return contexts—now launch only from
  exact checked-report receipts while retaining literal status 70. Existing
  semantic-layout and interpreter assertions remain unchanged; the 795 rooted
  and 3 legacy exact-owner pins remain stable. Five authored-root ABI/runtime-
  value executions—entry-field-write value calls, post-entry-state lets,
  runtime-local and constant self-array indexing, and a deep post-entry chain—
  now launch only from exact checked-report receipts while retaining literal
  statuses 70, 24, 99, 99, and 30. Existing interpreter assertions remain
  unchanged, and the 795 rooted/4 legacy exact-owner pins remain stable. Five
  further authored-root ABI/value-call executions—chained post-entry lets,
  cross-callee division, same-named cross-callee lets, nested value-call guards,
  and two-site struct results—now launch only from exact checked-report
  receipts while retaining literal statuses 2, 70, 70, 70, and 70. Five more—
  same-callee multi-site results, guarded and straight-line transition
  arguments, straight-line shared-slot results, and enum-self methods—retain
  literal statuses 70, 70, 12, 22, and 70 through the same receipt-only launch
  boundary. Five further ABI/dispatch executions—dispatch-bodied results,
  literal-length arm guards, value-call guard subjects, effectful guard/local
  and self-terminal delivery, and guarded effectful transition arguments—also
  launch only from exact receipts while retaining literal status 70. Existing
  interpreter and diagnostic-status assertions remain unchanged, and the 795
  rooted/4 legacy exact-owner pins remain stable. Four further authored-root
  ABI/value-call executions—nested-entry value calls, shared-name variant
  payload delivery, struct-payload cast fields, and branch-leaf multiple named
  conversions—now launch only from exact checked-report receipts while
  retaining literal status 70. Five further ABI/process-control executions—
  entry-host-state payload delivery, a contained health loop, sequential stdin
  buffering, Full artifact-backed text storage, and stderr writing—use the same
  receipt-only launch boundary while retaining literal statuses 70/75, 0, 0,
  0, and 70. All interpreter, stdin/stdout/stderr, and backend-report assertions
  remain unchanged, and the 795 rooted/4 legacy exact-owner pins remain stable.
  Fifteen further authored-root ABI text executions now use exact checked-
  report receipts: LF/CRLF line reads and indexed slice-string guards retain
  statuses 0, 0, 77, 70, and 72; string places across machine fields, local
  arrays, slices, and pointees retain 70, 70, 89, 70, and 70; mutable parameter,
  wrapped write-line, and struct-field concatenation retain 77, 77, 77, 77, and
  188. Five more string-assembly rows—stored suffix, lookup/large-frame/room
  lookup concatenation, and a call-argument slice alias—retain 193, 190, 192,
  200, and 77. Exact interpreter, stdout, and content-comparison assertions
  remain unchanged, and the 795 rooted/4 legacy exact-owner pins remain stable.
  Five further authored-root ABI/string-storage executions—mutable struct
  string-field copy/concat/write-line, machine-owned indexed integer writes,
  fixed- and runtime-indexed struct copies, and nested indexed exit writes—now
  launch only from exact checked-report receipts while retaining literal
  statuses 77, 79, 83, 85, and 89. Five further authored-root ABI/ordered-
  dispatch executions—direct, after-call, game-shape, and large-machine room
  dispatch plus guarded inline leaf-arm skipping—now use the same receipt-only
  boundary while retaining literal statuses 73, 83, 93, 103, and 70. Exact
  interpreter/stdout and existing diagnostic assertions remain unchanged, and
  the 795 rooted/4 legacy exact-owner pins remain stable.
  Four further ABI/dungeon executions—ordered-room dispatch and real-show-state
  stdin loops, the threaded mutable-argument interrupt soak, and nested value-
  call caller-local guarding—now launch only from exact checked-report receipts
  while retaining literal statuses 135, 145, 70, and 70. Five further authored-
  root domain/control executions—copy-then-read, full-width i64 operations,
  chained bounded-text append, descriptor append-in-place, and two-field
  bounded-text concatenation—use the same receipt-only boundary while retaining
  literal status 70. Existing stdin, interpreter, and diagnostic assertions
  remain unchanged, and the 795 rooted/4 legacy exact-owner pins remain stable.
  Five further authored-root domain/control executions—machine bounded-text
  append, local string-field copying through mutable parameters, bounded-
  carrier call returns, min-call result arithmetic, and direct Boolean
  conjunction dispatch—now launch only from exact checked-report receipts while
  retaining literal statuses 70, 70, 70, 70, and 21. Existing interpreter
  assertions remain unchanged, and the 795 rooted/4 legacy exact-owner pins
  remain stable.
  Five further authored-root executable-domain executions—local and imported
  membership expressions, imported membership guarding, and imported
  intersection/union guards—now launch only from exact checked-report receipts
  while retaining literal statuses 81, 91, 81, 219, and 217. Existing
  diagnostic assertions remain unchanged, and the 795 rooted/4 legacy exact-
  owner pins remain stable.
  Ten further authored-root executable-domain executions now use exact checked-
  report receipts: local intersection/union guards, local union/intersection
  values, and imported union values retain statuses 231, 241, 205, 233, and
  215; imported intersection values, local Boolean-or values, straight-line
  terminal local and field readback, and negated Boolean-place guards retain
  217, 251, 70, 70, and 73. Existing diagnostic assertions remain unchanged,
  and the 795 rooted/4 legacy exact-owner pins remain stable.
  Ten further authored-root control executions now use exact checked-report
  receipts. Local Boolean conjunction, scalar comparison, string comparison,
  Boolean-or guarding, and direct Boolean transition arguments retain statuses
  74, 76, 78, 71, and 211. Local Boolean transition arguments, Boolean
  transition arguments after string guards, machine-owned indexed nested-room
  copies, negated comparison guards, and case-member dispatch retain 201, 247,
  87, 75, and 70. Existing diagnostics remain unchanged, and the 795 rooted/3
  legacy exact-owner pins remain stable.
  Fifteen further authored-root case/data/control executions now use exact
  checked-report receipts. Case-payload construction, record field-value
  patterns, case-payload guard reads, case-membership values, and exhaustive-
  by-cases dispatch retain status 70 and every decoy-sensitive diagnostic.
  Exhaustive case-union domains, case-membership union guards, case
  reassignment, mixed-shape data, and array-literal String fields likewise
  retain status 70 and every wrong-arm/stale-data diagnostic. Struct-literal
  String fields, immutable parameter-domain forwarding, case-payload domain
  forwarding, tuple transitions, and room-use reentry retain statuses 70, 70,
  70, 22, and 41; both independent interpreter oracles remain unchanged. The
  795 rooted/4 legacy exact-owner pins remain stable.
  Five further authored-root dungeon/storage executions—enemy-clear reentry,
  clear/carve/render String fields, full-level wrapper String lookup, multi-room
  reentry, and mutable-slice element writes—now launch only from exact checked-
  report receipts while retaining literal statuses 51, 198, 202, 63, and 21.
  Both dungeon interpreter oracles and all diagnostics remain unchanged, and
  the 795 rooted/4 legacy exact-owner pins remain stable. Five further authored-
  root indexed-storage executions—straight-line and dispatched mutable-slice
  writes, runtime array indexed reads, indexed struct-field writes, and
  particle integration—now launch only from exact checked-report receipts
  while retaining literal statuses 70, 31, 70, 70, and 70. Existing alias,
  stale-fold, and self-check diagnostics remain unchanged, and the 795 rooted/3
  legacy exact-owner pins remain stable. Five further authored-root
  construction/call-identity executions—nested-struct construction, cross-
  machine substate-name resolution, value-call array-element writes, computed
  transition arguments, and by-value struct parameters—now launch only from
  exact checked-report receipts while retaining literal status 70 and every
  regression-specific diagnostic. The 795 rooted/4 legacy exact-owner pins
  remain stable.
  Five further authored-root value/result executions—value-call composition,
  struct-returning calls, Option-returning calls, Result matching, and entity-
  component state—now launch only from exact checked-report receipts while
  retaining literal status 70 and every pipeline, sum/error, and nested-field
  diagnostic. Five further structured-value executions—nested-struct state,
  array-element struct copies, deep nested value semantics, struct-array
  literals, and struct-valued enum payloads—use the same receipt-only boundary
  while retaining status 70 and every copy/layout diagnostic.
  Five further authored-root enum/nested/indexed-state executions—enum
  classification and dispatch, nested-field accumulation, indexed-write/
  constant-read, indexed temporary RMW, and indexed writes beside adjacent
  fields—now launch only from exact checked-report receipts while retaining
  literal status 70 and every dispatch, stale-constant, and out-of-bounds
  diagnostic. Five further bounds/index executions—join-meet bound propagation,
  dual indexed comparisons, array min/max reduction, indexed guard subjects,
  and nested payload range narrowing—use the same boundary while retaining
  status 70 and every bound/element-selection diagnostic; the paced host-timer
  legacy launch remains intentionally untouched.
  Five further authored-root arithmetic-policy executions—saturating wide
  boundaries, saturating parameter carry, saturating expression operands,
  wrapping guard operands, and signed MIN/-1 divide guards—now launch only from
  exact checked-report receipts while retaining literal status 70 and every
  policy-specific diagnostic. Across these cohorts the 795 rooted/4 legacy
  exact-owner pins remain stable.
  Five further authored-root operand-carrier executions—nested unsigned
  arithmetic, local indexed call operands, machine-indexed fused call
  arguments, saturating indexed guard operands, and nested float operands—now
  launch only from exact checked-report receipts while retaining literal status
  70 and every signedness, register-custody, and domain diagnostic. Five further
  shift-policy executions—shift-count domain resolution, guarded Exact shifts,
  at-width wrapping left and right shifts, and indexed shift targets—use the
  same receipt-only boundary while retaining status 70 and every policy
  assertion. Five further saturating-value executions—nested operands, unsigned
  one-direction clamps, the signed MIN idiom, saturating left shift, and 32-bit
  shift value overflow—likewise retain status 70 and every clamp/domain
  assertion. Across these cohorts the 795 rooted/4 legacy exact-owner pins
  remain stable.
  Five further authored-root conversion/float-policy executions—subword masked
  shifts, saturating float-to-int, unsigned/narrow saturating float-to-int,
  saturating float overflow, and direct trapping float overflow—now launch only
  from exact checked-report receipts while retaining every literal status,
  abnormal-exit check, and interpreter reason assertion. The two custom-ranking
  recursive-delivery executions and the u64-magnitude transition-delivery plus
  proven-range Exact shift-count executions likewise use exact receipts while
  retaining status 70 and all terminal-delivery/diagnostic assertions. Slow
  float-policy, helper-driven trapping, platform-gated, timer, and multi-fixture
  owners remain outside fast follow-up cohorts; the 795 rooted/4 legacy exact-
  owner pins remain stable. Profiling one float-policy owner attributes 3.263 of
  3.523 measured native-compile seconds (92.61%) to Stage 05 checked-tree
  construction, with samples concentrated in checked-fact and recursive call-
  frame write-demand summarization. The independent interpreter oracle repeats
  that frontend work, while backend emission is only 1.5 ms and `OutputOnly`
  already fences auxiliary reports.
  A broader test-topology audit built every `omega-compiler` test binary in
  4.61s wall, confirming that Rust test compilation is not the long pole. The
  canary umbrella already runs independent compiles with bounded outer
  parallelism (eight jobs by default), one inner backend worker, deterministic
  source-ordered result collection, and exact-native duplicate elision; ordinary
  helpers already disable auxiliary HTML/report emission through `OutputOnly`.
  Current measurements therefore do not justify an Arena-to-PagedArena rewrite
  or deleting report viewers for speed. Further work should target repeated
  Stage 05 semantic compilation/search and reuse checked-report receipts where
  one owner currently recompiles the same frontend.
  The 74-parameter
  `arbitrary_exact_mixed_shift_chains_retain_independent_prefix_proofs` owner
  is now a concrete Stage 05 performance target: during the ranked mutable-
  receiver checkpoint it remained in that single test for more than 100
  seconds before the bounded suite run was stopped. Profile its repeated
  checked-tree and exact-proof reconstruction, then reduce that work without
  weakening the independent-prefix assertions. This is an implementation task,
  not a language-design block, and the measurement does not authorize deleting
  the test, broadening Alpha tape, or introducing a speculative arena rewrite.
  Sample refresh no longer multiplies its machine-wide outer fan-out by a full
  backend worker pool per sample: each independent compile now owns one inner
  worker and uses `OutputOnly`, because the command consumes only the runnable
  program. On the measured 14-core host this removes a possible 196-worker
  oversubscription and avoids the unused report/HTML bundle while preserving
  parallel sample throughput. Focused external-root, terminal-image, and native
  fuel canaries remain fast, so this evidence still does not justify an arena
  rewrite or globally deleting diagnostic viewers.
  A follow-up harness audit measured the already-built exact canary at 0.02s,
  warm Cargo-filtered runs at 0.08–0.12s, and a schema-fanout `--no-run`
  rebuild/relink at 5.03s with 9.74s user CPU. Low-CPU multi-second outliers are
  shared-target/Cargo-lock waits; high-CPU ones are dependency rebuilds of the
  single 49,481-line, 2.08MB canary integration target. The 46GB shared debug
  cache is large but not in the previously pathological range. The smallest
  justified optimization is coordinated, batched focused gates after shared
  schemas stabilize; per-agent target directories, cache cleaning, test-target
  splitting, report-viewer deletion, and an Arena/PagedArena rewrite remain
  unsupported by the measurements.
  Five further authored-root lifetime/wire executions—method-view writes after
  last use, chained view-of-view writes, shrinking-slice recursion, primitive
  wire encoding, and wire era discrimination—now launch only from exact checked-
  report receipts while retaining literal status 70 and every alias, recursion,
  and byte-level diagnostic. The 795 rooted/4 legacy exact-owner pins remain
  stable.
  Five further authored-root wire-decoder executions—primitive roundtrip,
  ranged scalar and repeated fields, canonical Boolean enforcement, and
  canonical varint enforcement—now launch only from exact checked-report
  receipts while retaining literal status 70 and every hostile-input
  preservation/byte-canonicality diagnostic. The 795 rooted/4 legacy exact-
  owner pins remain stable.
  Five additional authored-root wire executions—scalar-width overflow
  rejection, nested-message roundtrip and malformed-length rejection, plus
  repeated-field roundtrip and overflow rejection—now launch exclusively
  through checked-report executable receipts while preserving literal status
  70 and all byte-shape diagnostics. Exact-owner inventory pins remain
  unchanged.
  Five further authored-root wire owners—wrong-era rejection, exact String and
  byte-slice encoding, zero-copy byte-slice decoding, and decoded-slice
  indexing—now execute solely from checked-report receipts while preserving
  status 70 and all byte-canonicality assertions. The adjacent auxiliary-report
  consumer remains on its report-bearing path, and exact-owner pins remain
  unchanged.
  Five further fast authored-root executions—decoded byte-slice length access,
  call-result binary composition, multi-arm value selection, unsigned value
  guards, and compile-time-sized array execution—now launch solely from checked-
  report receipts while preserving literal status 70 and their original
  failure diagnostics. Report-bearing and float/cast owners remain deliberately
  outside this cohort; exact-owner pins remain unchanged.
  Five further fast authored-root executions—fixed-vector roundtrip, eager
  combination of distinct value-call results, signed i64 arithmetic, high-bit
  bitwise operations, and unsigned high-value comparisons—now launch
  exclusively through checked-report receipts while preserving status 70 and
  all behavioral diagnostics. Report-bearing and known slow float/cast/policy
  owners remain deliberately excluded; exact-owner pins remain stable.
  Five further authored-root algorithm executions—Euclidean GCD, RPN stack
  evaluation, greedy activity selection, maze pathfinding, and graph BFS
  traversal—now launch exclusively through checked-report receipts while
  preserving status 70 and each result-specific diagnostic. Report-bearing and
  known slow float/cast/policy owners remain excluded; exact-owner pins remain
  stable.
  Five further authored-root collection executions—coin-change dynamic
  programming, open-addressed hashing, matrix multiplication, ring-buffer
  queuing, and bubble sorting—now launch solely through checked-report receipts
  while preserving status 70 and their exact result diagnostics. Report-
  bearing, known slow float/cast/policy, and exceptional historical-hang owners
  remain excluded; exact-owner pins remain stable.
  Five further authored-root indexed/container executions—2D transpose, guarded
  indexed access, binary search, two-pointer palindrome checking, and nested
  struct-array field access—now launch solely through checked-report receipts
  while preserving status 70 and exact result diagnostics. Exceptional
  historical-hang, report-bearing, float/cast, and policy owners remain
  excluded; exact-owner pins remain stable.
  Five further authored-root struct/index executions—enum-grid scanning, dual
  indexed reads, struct-field temporary arithmetic, runtime-indexed whole-
  struct writes, and indexed-read guard evaluation—now launch solely through
  checked-report receipts while preserving status 70 and exact regression
  diagnostics. Exceptional historical-hang, report-bearing, float/cast, and
  policy owners remain excluded; exact-owner pins remain stable.
  Five further authored-root aggregate executions—runtime-row/constant-column
  writes, nested-array constant indexing, whole-array and whole-struct value
  copies, and fixed-array field guards—now launch solely through checked-report
  receipts while preserving status 70 and exact data-flow diagnostics.
  Exceptional, report-bearing, float/cast, policy, and automaton owners remain
  excluded; exact-owner pins remain stable.
  The final three eligible fast owners in the wire/algorithm module—standard
  Optional matching, fixed-array field-value access, and fixed-array element
  guards—now launch solely through checked-report receipts while preserving
  status 70 and exact diagnostics. Its remaining conventional launches are
  deliberately retained exceptions: auxiliary-report consumers, known slow
  float/cast/policy cases, the historical-hang owner, and the automaton owner.
  Exact-owner pins remain stable.
  Three atomic authored-root executions—fetch-and, swap, and compare-exchange—
  plus Dutch-flag partitioning now launch natively solely through checked-
  report receipts while preserving their literal statuses, detailed
  diagnostics, and Linux ARM64 cross-target compilation assertions. The
  interactive two-mode console owner and all previously fenced exceptional,
  report, float/cast, and policy owners remain excluded; exact-owner pins
  remain stable.
  Five further authored-root UTF-8/content executions—parameter length-field
  access, regular-call literal length, literal and view content equality, and
  declared-domain field reads—now launch solely through checked-report
  receipts while preserving status 70 and exact content/domain diagnostics.
  Exceptional, interactive, report-bearing, float/cast, policy, and automaton
  owners remain excluded; exact-owner pins remain stable.
  Five further authored-root domain/carrier executions—domain field write/read,
  bounded-carrier content roundtrip, carrier length as both host argument and
  stored field, and carrier byte indexing—now launch solely through checked-
  report receipts while preserving literal statuses 73/10/70 and exact domain/
  content diagnostics. Exceptional, interactive, report-bearing, float/cast,
  policy, and automaton owners remain excluded; exact-owner pins remain stable.
  Five further authored-root byte-carrier executions—runtime-indexed reads and
  writes, indexed reads as value operands, the carrier cipher loop, and
  constant-byte writes at runtime indices—now launch solely through checked-
  report receipts while preserving status 70 and exact byte-level diagnostics.
  Numeric-conversion and all exceptional, interactive, report-bearing, float/
  cast, policy, and automaton owners remain excluded; exact-owner pins remain
  stable.
  Five further authored-root carrier algorithms—length guarding, FNV-1a
  hashing, CRC32, Base64 encoding, and run-length encoding—now launch solely
  through checked-report receipts while preserving status 70 and exact hash/
  encoding diagnostics. Numeric-conversion, rendering, exceptional,
  interactive, report-bearing, float/cast, policy, and automaton owners remain
  excluded; exact-owner pins remain stable.
  Five further authored-root text/byte executions—binary formatting, substring
  search, string palindrome checking, bounded-carrier byte writes, and slice-
  length field access—now launch solely through checked-report receipts while
  preserving literal statuses 70/5 and exact formatting/search/content
  diagnostics. Numeric-conversion, rendering, coercion, exceptional,
  interactive, report-bearing, float/cast, policy, and automaton owners remain
  excluded; exact-owner pins remain stable.
  The final four eligible fast owners in the content/carrier module—unary
  negation, UTF-8 literal length, user-domain literal grants, and bodyless-
  domain declaration spellings—now launch solely through checked-report
  receipts while preserving status 70 and exact arithmetic/domain diagnostics.
  Its remaining conventional launches are deliberately retained numeric-
  conversion, rendering, or coercion exceptions; exact-owner pins remain
  stable.
  Five further authored-root layout/value executions—plan-laid by-value
  parameters, fixed-array record and mutable views, nested fixed-array mutable
  views, and sequential value-call result slots—now launch solely through
  checked-report receipts while preserving status 70, interpreter parity, and
  Windows x64/Linux ARM64 cross-target assertions. The 2.88-second plain
  record-view owner joins the retained slow exceptions; all other fenced
  exception classes remain unchanged and exact-owner pins remain stable.
  Five further authored-root layout/value executions—nested-record, fixed-
  record-array, and ordinary mutable record views, sequential self-capture value
  calls, and nested local state arguments—now launch solely through checked-
  report receipts while preserving status 70, interpreter parity, and existing
  Windows x64/Linux ARM64 cross-target assertions. The 2.88-second plain record-
  view owner remains retained for a dedicated profiled migration; all other
  fenced exception classes remain unchanged and exact-owner pins remain stable.
  Four further cross-target plan-laid executions—compact-bit layout plus
  `IntegerAt` projection, total writes, and proved-fit writes—now launch
  natively solely through checked-report receipts while preserving statuses
  70/72, interpreter parity, and Windows x64/Linux ARM64 compilation assertions.
  The plain record-view owner remains retained: profiling attributes its 2.87-
  second body to four independent compilations (687ms checked, 727ms native,
  724ms Windows, 727ms Linux), while `CompileReport` currently retains no
  reusable `CheckedTrees` receipt; interpretation itself costs only 0.23ms.
  Two residual erased-wire owners now launch native executions solely through
  exact checked-report receipts while preserving in-memory semantic-schema/
  normalized-placement checks, interpreter parity, and status 70. The profiled
  plain record-view owner remains fenced because `CompileReport` retains no
  reusable `CheckedTrees` receipt; exceptional, interactive, report-bearing,
  slow float/cast/policy, numeric-conversion, rendering, coercion, and automaton
  owners remain unchanged, with exact-owner pins stable.
  The compiler now owns a private `NativeCompilationWithCheckedReceipt` seam:
  native realization borrows the exact targetful `CheckedCompilation` produced
  by that invocation, then the sealed non-clone carrier retains it beside the
  ordinary native report. Construction rejoins source count, target profile,
  native target, retained artifact, and any production manifest before custody
  escapes; the sole public `compile(request)` entry consumes the pair into its
  ordinary report. A crate-local four-target no-selection native regression
  rejoins source count and target identity, consumes the paired report, and
  independently replays the retained artifact without adding a test-only public
  compile wrapper. The profiled plain record-view migration remains fenced on
  an existing native-coverage gap: its three-state attached entry has byte
  stores, recast projection, conversion, locals, and conditional control,
  while the current exact checked structural-Unit carrier retains only
  signatures plus no-code return/jump/conditional terminators. Admitting that
  source through the existing carrier would erase work. Closing the gap requires
  operation custody through checked, Terminal, Omega, target, and native replay;
  it is engineering coverage rather than a language-design question.
  The two-entry residual scalar cohort now launches
  `guarded_transition_dispatch` and `record_array_field_access` solely through
  exact checked-report executable receipts while preserving literal status 0
  and diagnostics. Exact-owner pins remain stable; exceptional and deliberately
  fenced owners remain unchanged.
  The three recursive call-with-return executions—inline, direct value-call,
  and statement value-call walks—now launch solely through exact checked-report
  executable receipts while preserving literal status 70 and separator-count
  diagnostics. Exact-owner pins remain stable; all profiled, exceptional,
  interactive, report-bearing, slow float/cast/policy, numeric-conversion,
  rendering, coercion, and automaton owners remain fenced.
  Source-ordered affine literal root-alias discovery now lives in paired,
  side-local `affine_selection/literal/alias/candidates/root_aliases` modules.
  Producer and reconstruction independently traverse requirements before
  semantic axioms, preserve left-before-right equality orientation, and require
  distinct same-carrier Value endpoints; only the producer retains outer
  citation custody. Landing-index order, same-row rejection, literal carrier
  checks, proof shape, completion precedence, and the fixed one-intermediate-
  alias frontier remain unchanged.
  Three authored-root shared-reference executions—content-spilled member
  access, large-reference dereference, and large-reference direct assignment—
  now launch solely through exact checked-report executable receipts while
  preserving literal statuses 42, 42, and 70 and all address/content-custody
  diagnostics. Exact-owner pins remain stable; profiled record-view and
  exceptional, interactive, report-bearing, slow float/cast/policy, rendering,
  coercion, and automaton owners remain fenced.
  Exact affine literal alias-landing joins now live in paired, side-local
  `affine_selection/literal/alias/candidates/join` modules. Producer and
  reconstruction independently reject reuse of the outer equality as the
  literal landing and require the affine root carrier to match the indexed
  integer literal exactly. Root-alias order, landing-index order, producer-only
  citation custody, completion precedence, nested proof shape, and the fixed
  one-intermediate-alias frontier remain unchanged.
  Five authored-root aggregate/collection executions—independent same-type
  contained fields, sum-field payload storage, argmax indexing, stack bracket
  matching, and two-pointer palindrome detection—now launch solely through
  exact checked-report executable receipts while preserving literal status 70
  and all alias, payload, index, and mismatch diagnostics. Exact-owner pins
  remain stable; profiled record-view and exceptional, interactive, report-
  bearing, slow float/cast/policy, rendering, coercion, and automaton owners
  remain fenced.
  Source-ordered direct affine-literal equality discovery now lives in paired,
  side-local `affine_selection/literal/direct/candidates/equalities` modules.
  Producer and reconstruction independently traverse requirements before
  semantic axioms and preserve left-before-right equality orientation; only
  the producer retains citation custody. Exact Value/integer carrier
  eligibility, completion handoff, proof shape, rejection behavior, direct-
  before-one-alias precedence, and the fixed affine-literal frontier remain
  unchanged.
  Three authored-root indexed-guard executions—cross-array comparison, dual-
  index equality, and dual-index ordering—now launch solely through exact
  checked-report executable receipts while preserving literal status 70 and
  all base/index-confusion diagnostics. Exact-owner pins remain stable; the
  adjacent float section and all profiled, exceptional, interactive, report-
  bearing, slow, rendering, coercion, and automaton owners remain fenced.
  Exact direct affine-literal eligibility now lives in paired, side-local
  `affine_selection/literal/direct/candidates/eligibility` modules. Producer
  and reconstruction independently require a Value root, an exact integer
  literal, and identical integer carriers before completion. Source-ordered
  oriented equality discovery, producer-only citation custody, proof shape,
  rejection behavior, direct-before-one-alias precedence, and the fixed affine-
  literal frontier remain unchanged.
  Five authored-root scalar/indexed-storage executions—scoped constants,
  `u64::MAX`, guarded and direct computed indexing, and dual-indexed copying—
  now launch solely through exact checked-report executable receipts while
  preserving literal statuses 70, 70, 30, 1, and 50 and all width/index/copy
  diagnostics. Exact-owner pins remain stable; time-host and all other fenced
  owners remain unchanged.
  Fixed affine-literal root-bound orientation now lives in paired, side-local
  `affine_selection/literal/root_bounds` modules. Producer and reconstruction
  independently preserve `literal <= value` before `value <= literal`; the
  producer additionally binds substitution endpoint 1 then 0 for its existing
  direct and nested alias proof constructors. Direct and one-intermediate-alias
  completion now consume that common side-local order without sharing
  authority across the trust boundary. Direct-before-alias precedence, proof
  shapes, rejection behavior, and the fixed affine-literal frontier remain
  unchanged.
  Five authored-root indexed-container executions—double-indexed writes,
  generic setter and method-instance matrices, frame-resident double-indexed
  reads, and double-indexed read-modify-write—now launch solely through exact
  checked-report executable receipts while preserving literal status 1 and all
  placement, specialization, and stale-fold diagnostics. Exact-owner pins
  remain stable; all existing fenced owners remain unchanged.
  Exact affine root/integer-literal carrier eligibility now lives in paired,
  side-local `affine_selection/literal/eligibility` modules. Producer and
  reconstruction independently require an exact Value root whose integer
  carrier matches the landed literal; direct candidates and the fixed one-
  alias join consume that shared side-local judgment, while alias same-row
  rejection remains with the join. Direct-before-alias precedence, source and
  citation order, proof shapes, rejection behavior, and the fixed affine-
  literal frontier remain unchanged.
  Five authored-root indexed/reference executions—indexed transition
  arguments, shared-reference guards, distinct nested receivers, double-
  indexed member access, and double-indexed operands—now launch solely through
  exact checked-report executable receipts while preserving literal statuses
  1, 1, 9, 1, and 1 and all delivery, alias, receiver, and index diagnostics.
  Exact-owner pins and all existing ownership fences remain unchanged.
  Five authored-root indexed/local-storage executions—in-place reversal,
  transitive local copying, indexed frame-source writes, captured-local
  swapping, and looped dual-index copying—now launch solely through exact
  checked-report executable receipts while preserving literal status 70 and
  all stale-fold, capture, and copy-placement diagnostics. Exact-owner pins and
  all existing ownership fences remain unchanged.
  Source-ordered affine-literal equality traversal now lives in paired, side-
  local `affine_selection/literal/equalities` modules. Producer and
  reconstruction independently enumerate requirements or assumptions before
  semantic axioms and preserve left-before-right equality orientation for both
  direct literal discovery and outer root-alias discovery; only the producer
  retains citation custody. Direct carrier eligibility, root-alias distinct
  same-carrier eligibility, the indexed inner literal landings, direct-before-
  alias precedence, proof shapes, rejection behavior, and the fixed affine-
  literal frontier remain unchanged.
  The residual authored-root `i64::MIN` execution now launches solely through
  its exact checked-report executable receipt while preserving literal status
  70 and the signed-boundary comparison diagnostic. The time/indexed-storage
  module has no remaining ordinary fast filename-derived launches; time-host
  owners and all other established fences remain unchanged, with exact-owner
  pins stable.
  Four authored-root provider executions—adapter dispatch, checked boundary-
  operator dispatch, result-domain requirement-overload dispatch, and exact
  selected-provider dispatch—now launch solely through checked-report
  executable receipts while preserving literal status 70, interpreter parity,
  and all selection-identity assertions. Exact-owner pins and established
  exceptional/report/interactive/slow-owner fences remain unchanged.
  Three authored-root boundary-forwarding executions—adapter text forwarding,
  capability-state forwarding, and literal-byte output—now launch solely
  through exact checked-report executable receipts while preserving literal
  status 70, interpreter parity, selected-provider identity checks, and exact
  stdout. Exact-owner pins and established interactive/report/slow-owner fences
  remain unchanged.
  Five authored-root unsigned sign-class executions—landed folding, shift and
  divide/modulo argument delivery, and local/operand-position min/max—now
  launch solely through exact checked-report executable receipts while
  preserving literal statuses 70, 70, 70, 77, and 77, interpreter parity, and
  all signedness-regression diagnostics. Exact-owner pins and established
  exceptional/interactive/report/slow-policy fences remain unchanged.
  Three authored-root value-delivery executions—Boolean value-call return,
  struct-literal transition arguments, and runtime-indexed whole-element
  writes—now launch solely through exact checked-report executable receipts
  while preserving literal status 70, interpreter parity, and all delivery/
  materialization diagnostics. Exact-owner pins and established numeric/
  coercion/float/report/interactive fences remain unchanged.
  All affine-literal equality consumers now use paired, side-local ordered
  catalogs. Producer and reconstruction independently preserve requirements or
  assumptions before semantic axioms and left-before-right orientation for
  direct literal discovery, outer root-alias discovery, and the indexed inner
  alias/literal landings; only the producer catalog carries citation custody.
  Consumer-local carrier, distinctness, and same-row checks, per-alias landing
  order, direct-before-alias precedence, proof shapes, rejection behavior, and
  the fixed affine-literal frontier remain unchanged.
  Landed-literal affine-custody completion now lives in paired, side-local
  `affine_selection/literal/completion` modules. Reconstruction's identical
  direct and one-alias completion paths now share one independently checked
  root-bound replay, while production's distinct one- and two-substitution
  bound constructors feed exactly two ordered proofs into one producer-local
  affine-custody handoff. Producer and reconstruction remain independent
  across the trust boundary. Direct-before-alias precedence, equality/citation
  and endpoint order, proof shapes, rejection behavior, and the fixed affine-
  literal frontier remain unchanged.
  Five authored-root aggregate/ZII executions—aggregate transition arguments,
  deep nested writes, default composites, empty-carrier host output, and empty-
  carrier equality—now launch solely through exact checked-report executable
  receipts while preserving literal status 70, interpreter parity, exact
  stdout, and all placement/default-value diagnostics. Exact-owner pins and
  established fences remain unchanged.
  One-alias affine-literal candidate owners now consume their paired side-local
  ordered equality catalogs directly. Producer and reconstruction
  independently keep outer root/alias distinct-Value and same-carrier
  eligibility beside the indexed inner landing join, eliminating the
  redundant `candidates/root_aliases` wrappers; only the producer catalog
  carries citation custody. Requirements or assumptions before semantic
  axioms, left-before-right orientation, per-alias landing order, same-row
  rejection, direct-before-alias precedence, proof shapes, and the fixed
  affine-literal frontier remain unchanged.
  Five authored-root content/equality executions—owned-string byte views, tag-
  aware sum equality, text inequality, Boolean-position text equality, and
  terminal payload text equality—now launch solely through exact checked-report
  executable receipts while preserving literal status 70, interpreter parity,
  and all content, tag, and delivery diagnostics. Exact-owner pins and
  established fences remain unchanged.
  Exact affine-literal eligibility now includes the fixed one-alias join in
  paired, side-local `affine_selection/literal/eligibility` modules. Producer
  and reconstruction independently require distinct outer and inner equality
  rows plus an exact Value root whose integer carrier matches the landed
  literal, eliminating the redundant `alias/candidates/join` wrappers. Source
  and citation order, per-alias landing order, producer-only citation custody,
  direct-before-alias precedence, proof shapes, rejection behavior, and the
  fixed affine-literal frontier remain unchanged.
  Direct affine-root candidate selection now uses paired side-local stateless
  functions rather than one-shot candidate structs. Production independently
  walks cited assumptions before semantic axioms and retains citation custody;
  reconstruction independently walks requirements before semantic axioms.
  Both preserve exact LessOrEqual filtering, left-before-right Value endpoint
  order, direct custody completion, proof shape, rejection behavior, and the
  fixed affine evidence frontier.
  Five authored-root text/result executions—stored and value-position text
  equality, branching callee chains, bind-first recursive results, and
  recursive guard/transition-result roles—now launch solely through exact
  checked-report executable receipts while preserving literal status 70,
  interpreter parity, and all equality, call-result, and delivery-role
  diagnostics. Exact-owner pins and established fences remain unchanged.
  Producer-local affine-literal root-bound construction now lives in one
  `affine_selection/literal/root_bounds` authority. It retains separate fixed
  direct and one-alias entry points while sharing only the closed-order
  substitution constructor: direct emits one substitution, and one-alias emits
  the exact inner-then-outer pair. Reconstruction remains independently
  implemented. Root-bound orientation and endpoint order, equality citation
  order, proof shapes, rejection behavior, direct-before-alias precedence, and
  the fixed non-recursive affine-literal frontier remain unchanged.
  Five authored-root guarded/generic executions—guard-proven counters, guard-
  narrowed transition arguments, agreeing and monomorphic generic value calls,
  and nominal generic-bound static dispatch—now launch solely through exact
  checked-report executable receipts while preserving literal statuses 70,
  70, 70, 70, and 1, interpreter parity, and all range, materialization,
  specialization, and conformance diagnostics. Exact-owner pins and
  established trapping/GUI/platform/float/cast/coercion/report/interactive
  fences remain unchanged.
  Independent affine-literal reconstruction now keeps its fixed root-bound
  orientation directly in the common `affine_selection/literal/completion`
  authority. The one-use verifier `literal/root_bounds` wrapper is removed;
  completion still checks `literal <= root` before `root <= literal`,
  independently of producer proof construction. Direct-before-alias
  precedence, source/citation and endpoint order, proof shapes, rejection
  behavior, and the fixed affine-literal frontier remain unchanged.
  Affine-literal candidate selection now uses paired, side-local invocation
  functions rather than one-shot candidate structs. Producer and
  reconstruction independently build the same direct ordered catalog or the
  fixed one-alias outer catalog plus indexed literal landings, then apply their
  existing eligibility and completion callbacks; only the producer
  materializes citations. Construction and source order, direct-before-alias
  precedence, equality/citation and endpoint order, proof shapes, rejection
  behavior, and the fixed affine-literal frontier remain unchanged.
  Five authored-root generic specialization/layout executions—borrowed-place
  parameter inference, multiple specialization tuples, generic enum payloads,
  generic record instances, and literal const-data array extents—now launch
  solely through exact checked-report executable receipts while preserving
  literal statuses 70, 14, 70, 70, and 70, interpreter parity where present,
  the exact two-specialization count, and all materialization and layout
  diagnostics. The 795 rooted/4 legacy exact-owner pins and all established
  fences remain unchanged.
  Affine-literal equality catalogs now use paired, side-local stateless ordered
  iterators rather than one-shot wrapper structs. Producer and reconstruction
  independently enumerate assumptions or requirements before semantic axioms
  and left-before-right orientation for direct discovery, outer alias
  discovery, and landing-index construction; only the producer iterator
  attaches citation custody. Consumer eligibility, per-alias landing order,
  direct-before-alias precedence, proof shapes, rejection behavior, and the
  fixed affine-literal frontier remain unchanged.
  Five authored-root const-data specialization executions—forwarded array
  lengths, multiple layout instances, named values, closed arithmetic
  expressions, and symbolic expressions—now launch solely through exact
  checked-report executable receipts while preserving literal status 70 and
  all nested-extent, distinct-layout, named-value, and expression-
  specialization diagnostics. The 795 rooted/4 legacy exact-owner pins and all
  established fences remain unchanged.
  Five authored-root const-fact and dispatch executions—const-evaluated machine
  calls, const-only where-fact discharge, machine-backed const-domain facts,
  signed const-data specialization, and trait-default dispatch—now launch
  solely through exact checked-report executable receipts while preserving
  literal status 70 and all specialization, fact-discharge, and written-
  override diagnostics. The 795 rooted/4 legacy exact-owner pins and all
  established fences remain unchanged.
  Five authored-root generic/default executions—inherited and generic trait
  defaults, const-specialized container methods, coexisting concrete generic
  instances, and pure min/max guard-subject hoisting—now launch solely through
  exact checked-report executable receipts while preserving literal statuses
  70, 70, 70, 30, and 70 and all inheritance, specialization, layout, and
  guard-discrimination diagnostics. Existing `OutputOnly` policy, the 795
  rooted/4 legacy exact-owner pins, and all established fences remain
  unchanged.
  Five authored-root indexed/control executions—indexed true/false guard
  pairing, indexed-field local operands, indexed-local bitwise and comparison
  operands, and scalar min-guard true/false pairing—now launch solely through
  exact checked-report executable receipts while preserving literal status 70
  and all shared-subject, materialized-slot, bitwise, comparison, and guard-
  discrimination diagnostics. Existing `OutputOnly` policy, the 795 rooted/3
  legacy exact-owner pins, and all established fences remain unchanged.
  Five authored-root generic/reduction executions—nested generic instances,
  generic let-local instances, domain-carrying generic instances, one-pass
  array max/sum, and indexed reduction loops—now launch solely through exact
  checked-report executable receipts while preserving literal statuses 30,
  30, 42, 70, and 70 and all fixed-point monomorphization, domain-layout,
  indexed-read, and reduction diagnostics. Existing `OutputOnly` policy, the
  795 rooted/4 legacy exact-owner pins, and all established fences remain
  unchanged.
  Five authored-root indexed-storage/control executions—indexed read-modify-
  write loops, computed indexed writes, nested const-product indexing,
  hoisted-index writes, and mutable-local reassignment—now launch solely
  through exact checked-report executable receipts while preserving literal
  statuses 70, 70, 70, 7, and 2 and all index-width, neighboring-field,
  placement, stale-fold, and reassignment diagnostics. Existing `OutputOnly`
  policy, the 795 rooted/4 legacy exact-owner pins, and all established fences
  remain unchanged.
  Five authored-root tuple/dependent executions—Boolean tuple-matrix dispatch,
  finite sum-tuple matrix dispatch, tuple-case payload destructuring, dependent
  parameter ranges, and dependent product indexing—now launch solely through
  exact checked-report executable receipts while preserving literal status 70
  and all exhaustiveness, payload-binding, substituted-range, overflow, and
  indexed-element diagnostics. Existing `OutputOnly` policy, the 795 rooted/3
  legacy exact-owner pins, and all established fences remain unchanged.
  Five authored-root dependent-proof executions—dependent subtraction,
  ordering-chain indexing, requires-backed subtraction, guarded requires
  calls, and sibling-length indexing—now launch solely through exact checked-
  report executable receipts while preserving literal statuses 2, 7, 0, 6,
  and 7 and all established diagnostics. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green.
  Five authored-root alias/call-expansion executions—guarded-transition alias
  writes, loop-forwarded reference parameters, dispatched value calls through
  aliases, nested value calls in substates, and calls in inlined substates—now
  launch solely through exact checked-report executable receipts while
  preserving literal status 70 and all detailed diagnostics. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green.
  Five authored-root transition/result-flow executions—alias-indexed reads
  through transitions, dispatched binary call arguments, dispatched result-
  field binding, trailing-state mutable-parameter phases, and same-type second-
  receiver mutation—now launch solely through exact checked-report executable
  receipts while preserving literal status 70, interpreter parity, and all
  detailed diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Five authored-root dispatched-delivery executions—transition-argument
  results, effectful reentrant delivery, enum-case results, machine-array slice
  arguments, and field-read terminals—now launch solely through exact checked-
  report executable receipts while preserving literal status 70 and all
  detailed diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Five authored-root receiver-dispatch executions—nested same-type receivers,
  second-receiver dispatch, sibling dispatched value calls, repeated inline
  receiver calls, and non-entry second-receiver dispatch—now launch solely
  through exact checked-report executable receipts while preserving literal
  status 70 and all detailed diagnostics. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green; the
  adjacent timer owner stays explicitly fenced.
  Five authored-root nested/non-entry receiver-flow executions—self-call-chain
  second receivers, nested inline-chain results, non-entry inline second
  receivers, and nested local/field terminals through second instances—now
  launch solely through exact checked-report executable receipts while
  preserving literal status 70 and all detailed diagnostics. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green; adjacent float and timer owners stay explicitly fenced.
  Four authored-root multi-arm/text-scope executions—same-named arm locals,
  per-arm text-equality locals, pre-guard text-equality guard reads, and pre-
  guard argument forwarding—now launch solely through exact checked-report
  executable receipts while preserving literal status 70, interpreter parity,
  and all detailed diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Three authored-root parameter-receiver identity executions—second-instance
  binding, forwarded/reborrowed receiver chains, and the single-instance
  control—now launch solely through exact checked-report executable receipts
  while preserving literal status 70 and all detailed diagnostics. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green; intervening timer/control-flow owners remain untouched.
  Five authored-root dispatched-result delivery executions—alias-read
  terminals, slice-element terminals, binary terminals, multi-arm results, and
  guard-subject results—now launch solely through exact checked-report
  executable receipts while preserving literal status 70 and all detailed
  result-shape diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Four authored-root call-result-through-reference-field executions—scalar,
  string, paired-string, and offset-string delivery—now launch solely through
  exact checked-report executable receipts while preserving literal exits 183,
  186, 194, and 196 and all detailed pointer/descriptor diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; loop and reference-returned-slice owners stay separate.
  Five authored-root reference-returned/indexed-write executions—direct and
  parameter-forwarded slice-element references, nested guarded returned
  references, mutable local indexed parameters, and machine-owned indexed
  parameters—now launch solely through exact checked-report executable
  receipts while preserving literal exits 181, 70, 184, 171, and 173 and all
  detailed diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Five authored-root indexed-mutation executions—dynamic machine-owned indexed
  parameters, caller-local binary writes, helper-local alias addition, slice-
  alias field writes, and descriptor-indexed binary read-modify-write—now
  launch solely through exact checked-report executable receipts while
  preserving literal exits 175, 191, 181, 201, and 70, interpreter parity, and
  all detailed diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Two authored-root reference-forwarding executions—bare-name mutable-
  reference forwarding and frame-local slice descriptor forwarding—now launch
  solely through exact checked-report executable receipts while preserving
  literal status 70, interpreter parity, and all detailed diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; adjacent `f32` owners stay fenced.
  Five authored-root direct indexed-access executions—slice reads, indexed
  reads used as operands, direct and dispatched element copies, and frame-array
  slice-parameter aliases—now launch solely through exact checked-report
  executable receipts while preserving literal exits 41, 70, 51, 61, and 72
  and all detailed diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; loop/automaton and numeric-
  conversion owners stay fenced.
  Five authored-root subslice-boundary executions—length folding, bounded and
  end-only parameter ranges, local parameter subslices, and runtime-start
  ranges—now launch solely through exact checked-report executable receipts
  while preserving literal status 70 and all detailed descriptor diagnostics.
  Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift
  fences remain green; loop/automaton and numeric-conversion owners stay
  fenced.
  Five authored-root subslice-range executions—runtime-end ranges, nested
  parameter subslices, runtime-start-over-local ranges, inclusive-end parameter
  ranges, and range-length materialization—now launch solely through exact
  checked-report executable receipts while preserving literal exits 70 and 203
  and all detailed descriptor diagnostics. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green; loop/
  automaton and numeric-conversion owners stay fenced.
  Five authored-root subslice-index regressions—dynamic, bounded-dynamic, end-
  bounded dynamic, nested-dynamic, and nested-fixed indexing—now launch solely
  through exact checked-report executable receipts while preserving literal
  exits 207, 209, 211, 213, and 215 and detailed descriptor diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; loop/automaton, numeric-conversion, and transition/iteration
  owners stay fenced.
  Four authored-root slice-materialization regressions—bounded range length,
  range pointer bias, local aggregate elements carried into later lets, and
  field-array elements used as value operands—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving literal exits 215, 205, and 70 and detailed diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green.
  Three authored-root mutable-parameter regressions—machine-owned writes,
  local writes, and aliased read-modify-write—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving literal exits 141, 171, and 191 and detailed diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green.
  Two authored-root package/root-resolution regressions—build dependency alias
  mapping and core roster operation resolution—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving literal status 70 and diagnostics. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green; the
  adjacent product-index proof/loop owner remains untouched.
  Three authored-root fixed-integer arithmetic regressions—i16 signed
  arithmetic, u16 field arithmetic, and i64 signed arithmetic—now launch
  `OutputOnly` native execution solely through exact checked-report executable
  receipts while preserving literal status 70 and signed/unsigned width
  diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; address algebra and explicit conversion
  owners remain separate.
  Three authored-root address regressions—field round-trip, first-class
  parameter/return/local value flow, and legal address algebra—now launch
  `OutputOnly` native execution solely through exact checked-report executable
  receipts while preserving literal statuses 88, 70, and 70 and their address-
  specific diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; explicit conversion and
  dispatch owners remain separate.
  Two authored-root statically typed receiver-dispatch regressions—method
  dispatch through a mutable data-reference parameter and same-named methods
  on two concrete receiver types—now launch `OutputOnly` native execution
  solely through exact checked-report executable receipts while preserving
  literal status 70 and their detailed receiver-resolution diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; dynamic coercion and single-/multi-implementation dynamic
  dispatch owners remain separate.
  Two authored-root devirtualized dynamic-receiver regressions—the closed
  single-implementation trait case and a local named dynamic coercion through
  its exact selected row—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  status 70 and their detailed unresolved-call/exact-row diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; the two-implementation runtime-dispatch pair remains separate.
  The paired authored-root two-implementation dynamic-dispatch regressions—
  Circle then Square and the swapped Square then Circle order—now launch
  `OutputOnly` native execution solely through exact checked-report executable
  receipts while preserving literal status 70 and the complementary 94/49
  diagnostics that reject lexically fixed implementation selection. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green.
  Two authored-root runtime-boundary regressions—a `build`-named machine
  authored in main source remaining an ordinary runtime machine, and natural
  termination returning the oracle's zero status—now launch `OutputOnly`
  native execution solely through exact checked-report executable receipts
  while preserving literal statuses 70 and 0 and their diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; deep-state collision and u64 guard owners remain separate.
  Two authored-root state/guard regressions—deep-arm delivery past a live same-
  named entry local and exact `u64::MAX` round-trip through a let initializer
  plus equality guard—now launch `OutputOnly` native execution solely through
  exact checked-report executable receipts while preserving literal status 70
  and their detailed diagnostics. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green; saturating-time,
  float, and loop owners remain separate.
  Three authored-root unsigned-arithmetic regressions—high-bit min/max, modulo
  passed inline as a call argument, and modulo whose operand signedness is fixed
  by an explicit cast target—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  statuses 88, 70, and 70 and their detailed signedness diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; the nested named-conversion alias remains on its explicit
  legacy compile boundary.
  Three authored-root integer arithmetic-policy regressions—wrapping addition,
  saturating addition, and saturating signed divide/modulo including the
  `MIN / -1` corner—now launch `OutputOnly` native execution solely through
  exact checked-report executable receipts while preserving literal status 70
  and their exact wrap/clamp diagnostics. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green; trapping,
  float, and legacy-conversion owners remain separate.
  Three authored-root integer guard-arithmetic regressions—divide/modulo guard
  subjects, negative-i32 computed guard values, and mixed signed/unsigned
  divide-modulo signedness—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  status 70 and the detailed 71–74 wrong-arm diagnostics. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green; loop, float, trapping, and legacy owners remain separate.
  Three authored-root payload-layout regressions—multi-field case arithmetic,
  same-named fields across case payloads, and sum tag/payload field-storage
  round-trip—now launch `OutputOnly` native execution solely through exact
  checked-report executable receipts while preserving literal status 70,
  interpreter parity where present, and detailed wrong-field/tag/payload
  diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; nested-loop and mixed-width payload owners
  remain separate.
  The authored-root mixed-width sum-payload layout regression now launches
  `OutputOnly` native execution solely through its exact checked-report
  executable receipt while preserving interpreter parity, literal status 70,
  and the distinct wrong-variant versus wrong-offset/width diagnostics for
  `(i16, i16, i64)` payload reads. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green.
  Two authored-root saturating-multiply regressions—unsigned overflow clamping
  to 255 and signed overflow clamping to +127/-128—now launch `OutputOnly`
  native execution solely through exact checked-report executable receipts
  while preserving literal status 70 and exact clamp diagnostics. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green; trapping owners remain separate.
  Two authored-root in-range trapping-policy regressions—division `140 / 2`
  and multiplication `10 × 10`—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  status 70 and exact diagnostics; crash-process semantics are unchanged.
  Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift
  fences remain green.
  Four authored-root exact-narrowing regressions—guarded transition-argument
  decrement, one-sided `requires` range intersection, guarded transition-value
  decrement, and negated false-arm increment—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving literal statuses 70, 42, 42, and 70 and their Exact-proof
  diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; saturating-transition, cast-accumulator,
  crash, and legacy owners remain separate.
  Three authored-root arithmetic-boundary regressions—a saturating transition-
  argument accumulator with no Exact obligation, a slice-element domain-cast
  accumulator, and signed/unsigned Saturating/Wrapping boundary behavior—now
  launch `OutputOnly` native execution solely through exact checked-report
  executable receipts while preserving literal status 70 and their exact
  policy/source diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Three authored-root integer signedness regressions—cross-width signed/
  unsigned comparisons, arithmetic versus logical right shifts, and signed,
  unsigned, and left shifts evaluated directly in guard subjects—now launch
  `OutputOnly` native execution solely through exact checked-report executable
  receipts while preserving literal status 70 and detailed wrong-branch/`sar`-
  versus-`shr` diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Five authored-root guard-expression regressions—numeric casts, parenthesized
  subjects, And-of-Or DNF lowering, De Morgan negation, and the combined
  feature-composition case—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  status 70 and exact cast-width, parser, DNF, and negation diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green.
  Three authored-root narrow-integer regressions—saturating i8/u8/i16 add/
  subtract clamps, high-bit unsigned u32 divide/modulo/shift/compare, and signed
  i8/i16 two's-complement wrapping boundaries—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving literal status 70 and detailed width/policy diagnostics. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green.
  Three authored-root narrow signed guard/division regressions—negative i8
  compare/subtract/multiply guard subjects, i8/i16 signed divide/modulo guard
  subjects with sign extension, and saturating i8/i16 division including
  `TYPE_MIN / -1 -> TYPE_MAX`—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  status 70 and boundary diagnostics. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green.
  Three authored-root integer conversion/width regressions—mixed-width mixed-
  sign promotion, integer sign/zero extension plus truncation/reinterpretation
  threaded through transition parameters, and immediate i64 divide/modulo
  retaining 64-bit width—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  status 70 and detailed diagnostics. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green.
  Two authored-root float-breadth regressions—negative comparisons plus
  integer/float and f32/f64 casts with nested-field arithmetic, and broad f64/
  f32 arithmetic/cast/local-field coverage—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving literal status 70 and detailed diagnostics. Exact-owner ambiguity,
  the 795 rooted/4 legacy inventory, and receipt-drift fences remain green;
  trapping/crash semantics are unchanged. Both owners retain a measured 4.0–
  4.2s warm compiler-body cost for later phase-level profiling.
  Four authored-root range-inference regressions—multipath return-union
  inference, an inferred callee return bound, construction of a range-refined
  field from a provable non-literal value, and plain struct-field fact
  narrowing—now launch `OutputOnly` native execution solely through exact
  checked-report executable receipts while preserving literal status 70 and
  Exact-proof diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; payload-range owners remain
  separate.
  Four authored-root payload/range regressions—constrained case-payload
  arithmetic, guarded direct sum-payload pass-through, arithmetic over a
  guarded bounded payload, and exclusive/inclusive range-constraint syntax—now
  launch `OutputOnly` native execution solely through exact checked-report
  executable receipts while preserving literal statuses 70, 20, 70, and 70
  and their Exact-proof diagnostics. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green; crash semantics and
  legacy owners are unchanged.
  Three authored-root arithmetic/range regressions—FNV-1a wrapping arithmetic,
  min/max clamp narrowing, and modulo/division interval narrowing—now launch
  `OutputOnly` native execution solely through exact checked-report executable
  receipts while preserving literal exit 70 and the existing Exact-bound
  diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; crash semantics and legacy owners are
  unchanged.
  Four authored-root arithmetic-domain regressions—trapping multiply overflow,
  signed saturation, `requires`-proven Exact addition, and range-proven Exact
  addition—now launch `OutputOnly` native execution solely through exact
  checked-report executable receipts while preserving exit 70 for successful
  owners and the unconditional-trap diagnostic plus abnormal-exit-before-
  transition semantics for overflow. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green; legacy owners are
  unchanged.
  Four authored-root arithmetic-domain cast/trapping regressions—cross-domain
  saturating cast, in-range trapping arithmetic, field-path trapping overflow,
  and frame-slot `let` trapping overflow—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving exit 70 for successful owners and both exact unconditional-trap
  diagnostics plus abnormal-exit-before-transition semantics for overflow.
  Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift
  fences remain green; legacy owners are unchanged.
  Four authored-root arithmetic-boundary regressions—return-range-proven Exact
  propagation, trapping constant-fold overflow, constant trapping shift
  overflow, and dead trapping-`let` overflow—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts, including
  their exact nested `out/` publications. Exit 70, all exact unconditional-trap
  diagnostics, and abnormal-exit semantics remain unchanged; exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green.
  Four authored-root data/control-flow regressions—bare no-payload case-tag
  dispatch, transition arguments sourced from embedded calls, embedded value-
  call result-slot identity, and sequential self-field read/modify/write—now
  launch `OutputOnly` native execution solely through exact checked-report
  executable receipts while preserving literal exit 70 and each distinct
  failure-status diagnostic. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; crash semantics and legacy
  owners are unchanged.
  Three authored-root expression-selection regressions—value-position match,
  flat Boolean logic, and runtime-indexed enum matching with payload extraction—
  now launch `OutputOnly` native execution solely through exact checked-report
  executable receipts, including the exact nested `out/` publication. Literal
  exit 70 and existing mismatch diagnostics remain unchanged; exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green.
  Four authored-root trait/structural-equality regressions—written conformance
  validation and synthesized equality for record, payload-sum, and mixed
  shapes—now launch `OutputOnly` native execution solely through exact checked-
  report executable receipts while preserving literal exit 70 and each
  structural-omission diagnostic. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and receipt-drift fences remain green; crash semantics and
  legacy owners are unchanged.
  Three authored-root String-bearing `Equatable` regressions—structural
  equality in value position, structural inequality after De Morgan
  simplification, and structural equality directly in guard position—now
  launch `OutputOnly` native execution solely through exact checked-report
  executable receipts while preserving literal exit 70 and text-content-plus-
  scalar diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory,
  and receipt-drift fences remain green; crash semantics and legacy owners are
  unchanged.
  Four authored-root data-layout/copy regressions—deep nested-field access,
  struct value-copy semantics, whole-struct mutation copy with interpreter
  parity, and data-property declarations—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts, including
  exact nested `out/` publications. Literal exit 70 and all copy/layout
  diagnostics remain unchanged; exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Four authored-root operator regressions—compound assignment chaining,
  chained field mutation, guard comparison signedness, and value-position
  comparison signedness—now launch `OutputOnly` native execution solely through
  exact checked-report executable receipts while preserving literal exit 70
  and every mutation/signedness diagnostic. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green; interpreter/
  crash semantics and legacy owners are unchanged.
  Four authored-root signedness regressions—min/max, unsigned division/
  remainder/logical shift, signed division/remainder, and runtime right-shift
  with interpreter parity—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving literal
  exit 70 and every signedness diagnostic. The explicit named-conversion legacy
  owner remains untouched; exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green.
  Three authored-root signed overflow/division regressions—sign-correct
  saturating multiply, saturating `INT_MIN / -1` divide/modulo, and wrapping
  `INT_MIN / -1` divide/modulo—now launch `OutputOnly` native execution solely
  through exact checked-report executable receipts while preserving interpreter
  parity, literal exit 70, 72/73 diagnostics, and the no-#DE crash guard. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences
  remain green; explicit legacy owners are unchanged.
  Two authored-root narrow const-fold regressions—saturating clamps at i8/u8
  widths and wrapping-to-width folds at i8/u16—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving interpreter parity, literal exit 70, and width-regression exit-71
  diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; explicit legacy owners are unchanged.
  The authored-root nested-loop grid regression now launches `OutputOnly`
  native execution solely through its exact checked-report executable receipt
  while preserving literal exit 70 and the nested counter/reset diagnostic.
  Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift
  fences remain green; slow-float and explicit legacy owners remain untouched.
  Two authored-root proof/arithmetic regressions—`u64` termination measures and
  nested wrapping operand truncation—now launch `OutputOnly` native execution
  solely through exact checked-report executable receipts while preserving
  literal exit 70 and their measure/width diagnostics. Exact-owner ambiguity,
  the 795 rooted/4 legacy inventory, and receipt-drift fences remain green;
  slow-float, crash-specific, and explicit legacy owners remain untouched.
  Two authored-root dependent-data regressions—sum-payload construction with an
  integer cast operand and bounded-product dependent indexing with interpreter
  parity—now launch `OutputOnly` native execution solely through exact checked-
  report executable receipts while preserving literal exits 70 and 7 plus
  their construction/index diagnostics. Exact-owner ambiguity, the 795 rooted/
  3 legacy inventory, and receipt-drift fences remain green; slow-float, crash-
  specific, and explicit legacy owners remain untouched.
  Two authored-root value-flow regressions—trailing bare-local returns and
  same-type receiver-field post-entry routing with interpreter parity—now
  launch `OutputOnly` native execution solely through exact checked-report
  executable receipts while preserving literal exit 70, the trailing-local
  71/72/73 diagnostics, and exact receiver result flow. Exact-owner ambiguity,
  the 795 rooted/4 legacy inventory, and receipt-drift fences remain green;
  slow-float, crash-specific, timer/loop, and explicit legacy owners remain
  untouched.
  Three authored-root integer policy/width regressions—saturating bounds, cast
  sign/zero extension, and signed modulo plus arithmetic/logical/runtime
  shifts—now launch `OutputOnly` native execution solely through exact checked-
  report executable receipts while preserving literal exit 70 and every clamp/
  extension/shift diagnostic. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; slow-float, loop-heavy,
  crash-specific, timer, and explicit legacy owners remain untouched.
  Three authored-root declaration/resolution regressions—bundled core Rat use,
  free-floating constant substitution, and result-domain machine overload
  selection—now launch `OutputOnly` native execution solely through exact
  checked-report executable receipts while preserving literal exit 70 and
  every resolution diagnostic. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; cyclic/loop-heavy, slow-
  float, crash-specific, timer, and explicit legacy owners remain untouched.
  Three authored-root proof/index regressions—computed-index enum match
  subjects, guarded `u64` cap-store discharge, and declared-but-unconsumed
  proof-only data—now launch `OutputOnly` native execution solely through exact
  checked-report executable receipts while preserving literal exit 70 and
  every match/range/declaration diagnostic. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and receipt-drift fences remain green; cyclic/
  loop-heavy, report-bearing, slow-float, crash-specific, timer, and explicit
  legacy owners remain untouched.
  The authored-root integer-only narrow/widen conversion regression now
  launches `OutputOnly` native execution solely through its exact checked-
  report executable receipt while preserving literal exit 70 and the named-
  conversion/policy-qualified `u8` zero-extension plus `i8` sign-extension
  diagnostic. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; cyclic/loop-heavy, report-bearing, slow-
  float, crash-specific, timer, and explicit legacy owners remain untouched.
  The authored-root `u8 in Saturating` constant-fold regression now launches
  `OutputOnly` native execution solely through its exact checked-report
  executable receipt while preserving literal exit 70 and the exit-71 domain-
  drop diagnostic. Exact-owner ambiguity, the 795 rooted/4 legacy inventory,
  and receipt-drift fences remain green; profiled multi-compile, cyclic/loop-
  heavy, report-bearing, slow-float, crash-specific, timer, and explicit legacy
  owners remain untouched.
  The authored-root guarded dynamic `i64 -> u64` Exact-conversion regression now
  launches `OutputOnly` native execution solely through its exact checked-
  report executable receipt while preserving literal exit 70 and the value-
  preservation diagnostic. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and receipt-drift fences remain green; raw legacy conversion
  surfaces, trapping conversions, cyclic/loop-heavy, report-bearing, slow-
  float, crash-specific, and timer owners remain untouched.
  Two authored-root finite String regressions—concat membership and nested
  string-field concat—now launch `OutputOnly` native execution solely through
  exact checked-report executable receipts while preserving literal exits 71
  and 73 plus their concat-result and nested-write diagnostics. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and receipt-drift fences remain
  green; indexed-carrier, cyclic/loop-heavy, report-bearing, slow-float, crash-
  specific, timer, and explicit legacy owners remain untouched.
  Four authored-root integer coercion regressions—struct-literal field width,
  array-element width plus Saturating domain, transition-argument width
  wrapping, and const-fold cast signedness—now launch `OutputOnly` native
  execution solely through exact checked-report executable receipts while
  preserving interpreter parity, literal exit 70, and the existing 71/72/73
  diagnostics. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  receipt-drift fences remain green; cyclic/loop-heavy, report-bearing, slow-
  float, crash-specific, timer, and explicit legacy owners remain untouched.
  The authored-root integer suffix boundary-magnitude and suffix-landed
  operand-position regressions now launch solely through their exact checked-
  report executable receipts while preserving interpreter parity and literal
  exits 70/77. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  cross-copy receipt-drift fences remain green; rebuild/lock wall-time spikes
  remain distinct from their 0.04–0.05s compiler/interpreter bodies.
  The finite Darwin authored-import argument regression now launches solely
  through the exact macOS ARM64 executable retained by its checked compilation
  report while preserving the selected free-DllImport provider-plan identity,
  literal exit 70, and its documented no-interpreter-custom-capability
  boundary. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and
  cross-copy receipt-drift fences remain green.
  The bundled proof-only core-Nat declaration regression now launches solely
  through the exact executable retained by its checked compilation report while
  preserving literal exit 70 and the proof/runtime boundary. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and cross-copy receipt-drift
  fences remain green; structural-recursion/cyclic and accepted-axiom trust-
  report owners remain untouched.
  The finite computed-array-fill-via-field-temp regression now launches solely
  through the exact executable retained by its checked compilation report while
  preserving its five-element indexed-copy self-check and literal exit 70.
  Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and cross-copy
  receipt-drift fences remain green; structural/nested/recursion-heavy owners
  remain untouched.
  The finite init-hoisted-counter and write-first back-edge loop-invariant
  regressions now launch solely through the exact executables retained by their
  checked compilation reports while preserving bounded indexed-fill self-
  checks and literal exit 70. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and cross-copy receipt-drift fences remain green; their warm
  compiler/interpreter bodies remain 0.02s.
  The machine-owned single- and double-runtime-indexed bounded-carrier literal
  regressions now launch solely through the exact executables retained by their
  checked compilation reports while preserving inline-byte assignment/append
  and literal exits 85/87. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and cross-copy receipt-drift fences remain green. The remaining
  filename-derived launches in this canary module are deliberately fenced
  recursive/cyclic, structural, report-bearing, slow-float, nested-loop,
  crash-specific, or explicit legacy owners; no ordinary finite owner remains
  in the module.
  The finite decimal-text-to-integer parser regression now launches solely
  through the exact executable retained by its checked compilation report while
  preserving the `"12345"` to 12345 self-check and literal exit 70. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and cross-copy receipt-drift
  fences remain green; its warm compiler/interpreter body is 0.28s.
  The computed carrier-byte width-coercion regression now launches solely
  through the exact executable retained by its checked compilation report while
  preserving interpreter/native parity for computed 300 to `u8` low byte 44
  and literal exit 70. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and cross-copy receipt-drift fences remain green; its warm body is
  0.04s.
  The dedicated f32/f64 total-order satisfier owner now launches its host
  executable solely from the exact checked-report publication receipt while
  retaining checked-interpreter parity, raw NaN and signed-zero exit 70, and
  both Linux x64 and Linux ARM64 native compilations. The pass umbrella elides
  its duplicate host compile; the exact-owner inventory is 795 rooted and 4
  explicit legacy owners.
  The finite forward-array and decreasing-index loop regressions now launch
  solely through the exact executables retained by their checked compilation
  reports while preserving sum-to-100/backward-sum-to-10 self-checks and
  literal exit 70. Exact-owner ambiguity, the 795 rooted/4 legacy inventory,
  and cross-copy receipt-drift fences remain green; warm compiler/interpreter
  bodies remain 0.02s.
  The bounded runtime-slice and derived-adjacent-array indexed-read regressions
  now launch solely through the exact executables retained by their checked
  compilation reports while preserving the 20/40 content and `j + 1` sorted-
  adjacency self-checks plus literal exit 70. Exact-owner ambiguity, the 795
  rooted/4 legacy inventory, and cross-copy receipt-drift fences remain green.
  The guarded signed-index and relational two-pointer-sum regressions now
  launch solely through the exact executables retained by their checked
  compilation reports while preserving bounded sums 10/210 and literal exit
  70. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and cross-copy
  receipt-drift fences remain green; rebuild/lock waits remain distinct from
  their 0.02–0.03s compiler/interpreter bodies.
  The bounded two-pointer reverse and transitive branched-index-bound
  regressions now launch solely through the exact executables retained by their
  checked compilation reports while preserving in-place `[1..5]` to `[5..1]`
  mutation, the branch-bound re-read of 99, and literal exit 70. Exact-owner
  ambiguity, the 795 rooted/4 legacy inventory, and cross-copy receipt-drift
  fences remain green; their compiler/interpreter bodies remain 0.03–0.04s
  while observed 1.4–3s walls were relink or rebuild work.
  The bounded runtime-indexed array-write regression now launches solely
  through the exact executable retained by its checked compilation report while
  preserving `nums[i] = i + 100`, the read-back of 103 at index 3, and literal
  exit 70. Exact-owner ambiguity, the 795 rooted/4 legacy inventory, and cross-
  copy receipt-drift fences remain green; its warm compiler/interpreter body is
  0.03s.
  The finite runtime-subslice parameter and bare machine-field subslice-
  argument regressions now launch solely through the exact executables retained
  by their checked compilation reports while preserving descriptor/length
  self-checks and literal exit 70. Exact-owner ambiguity, the 795 rooted/3
  legacy inventory, and cross-copy receipt-drift fences remain green; their
  warm compiler/interpreter bodies remain 0.02–0.03s.
  The dispatch-path slice-index read and cross-transition slice-length
  regressions now launch solely through the exact executables retained by their
  checked compilation reports while preserving descriptor/read self-checks and
  literal exits 43/101. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and cross-copy receipt-drift fences remain green; Cargo-lock and
  concurrent-rebuild waits remain distinct from their 0.02–0.04s bodies.
  The transitioned fixed-index slice guard and local slice-length comparison
  regressions now launch solely through the exact executables retained by their
  checked compilation reports while preserving literal exits 121/191. Exact-
  owner ambiguity, the 795 rooted/4 legacy inventory, and cross-copy receipt-
  drift fences remain green; their warm compiler/interpreter bodies remain
  0.02s.
  The cross-transition slice-index and bounded slice-iteration regressions now
  launch solely through the exact executables retained by their checked
  compilation reports while preserving whole-element transition-copy exit 111
  and transitioned indexed-read iteration exit 91. Exact-owner ambiguity, the
  795 rooted/4 legacy inventory, and cross-copy receipt-drift fences remain
  green; both warm compiler/interpreter bodies are 0.02s.
  The machine-owned single- and double-runtime-indexed String-field concat
  regressions now launch solely through the exact executables retained by their
  checked compilation reports while preserving direct/double indexed writes
  and literal exits 81/83. Exact-owner ambiguity, the 795 rooted/4 legacy
  inventory, and cross-copy receipt-drift fences remain green; their warm
  compiler/interpreter bodies remain 0.02s.
  Final
  replay now also retains an exact
  selected-instruction-to-function-symbol owner map. Duplicate selected
  instruction identities, redirected instruction relocation origins, and
  instruction-origin rows without a retained owner reject, while semantic and
  materialization origins remain in their separate namespaces. Final
  emission also rejoins every validated placement row to exactly one private
  thunk plan. Missing,
  duplicate, or out-of-range placement indices, selected-entry drift, and
  repeated private thunk identities reject before encoded-function/object
  evidence is accepted. This does not
  materialize the registration relocation. Private-symbol derivation is now
  one shared backend-plan primitive, and final emission recomputes it from the
  exact site kind/index/generation, static ordinal, selected machine/entry
  handles, and evaluated calling-plan fingerprint. Symbol drift rejects even
  when forged encoded-function and object rows agree with each other. The
  callback-thunk planner now independently reconstructs the retained callback
  signature, revalidates and canonicalizes its complete target
  `BoundaryEntryPlan`, and requires the nonzero fingerprint to match before
  minting the private thunk identity. Final emission repeats that replay before
  accepting encoded-function/object evidence, so plan, canonical-form, or
  fingerprint drift rejects even when copied thunk and object rows agree. This
  originally retained plan custody only. Instruction selection now consumes
  that schedule for the first complete body slice: a zero-parameter,
  resultless, ordinary call-return callback whose canonical entry is a
  bodyless terminal leaf emits its exact private callback function with real
  enter/leave operations. Each callback retains an ordered, identity-bound
  boundary-footprint row separate from the process-entry contract through
  machine emission. Parameter, result, operation, transition, hidden-semantic,
  order, identity, and schedule drift reject. General multi-entry/re-entrant
  body lowering and the private registration relocation remain separate
  frontiers. Callback thunk
  planning now also requires the selected control-flow entry to be the
  canonical segment-zero `StateKey`. Final emission independently reconstructs
  that whole selected entry and rejects segment drift even when the callback
  role, encoded function, and object binding agree with the forged segment.
  This strengthens selected-entry custody only; thunk-body lowering and the
  private registration relocation remain separate. Callback thunk planning
  now also seals the complete checked placement identity—site, registration
  operation, ordinal, selected machine/entry, exact satisfaction row,
  canonical overload, and calling-plan fingerprint—beside the placement index.
  Final emission independently rederives that receipt and rejects placement-row
  drift before accepting otherwise-consistent role, encoded-function, or
  object-symbol evidence. The structurally replayed callback-placement receipts
  now also produce one ordered, domain-separated identity fingerprint covering
  the complete checked row and placement index. Native payload handoff retains
  that evidence, and final footprint certification folds it into the placement
  binding and certificate identity, so a valid image/certificate cannot
  substitute a different callback registration or satisfaction realization.
  The fingerprint is summary evidence only; exact structural receipt comparison
  remains authoritative before final emission. Checked executable-image
  evidence now retains that exact callback-placement identity summary, and
  publication requires it to equal the final certificate summary. A separately
  valid certificate recomputed for a substituted registration/satisfaction
  realization therefore cannot pair with unchanged image evidence. This seals
  structural callback identity through image-to-certificate publication
  without selecting the blocked private registration relocation. Callback-
  placement identity now also remains explicit through publication and
  installation: publication evidence, installed-publication evidence, and the
  retained compile-report receipt carry the exact summary; it participates in
  both fingerprints, and flat/app-bundle receipt replay requires equality.
  Callback identity drift therefore invalidates destination custody even when
  certificate and container fields are otherwise copied consistently, without
  choosing private registration placement. This structural-to-installation
  chain is complete at the summary boundary: further independent replay would
  require widening the report to retain the whole final certificate, or adding
  a redundant hash minted from the same copied values. Remaining callback work
  stays on the separately listed resource, body, lease, cross-target, and
  private-relocation frontiers. The checked resource-representation
  prerequisite and its first callback-placement receipt are now live. Every
  concrete machine retains one declaration-ordered row per exact entry. Each
  row independently binds the machine, entry, and realized contract identity
  to three distinct downstream derivation obligations: Terminal-plus-target
  stack closure, Terminal control under an explicit fuel schedule, and
  selected-instruction machine-state footprint. A boundary callback use now
  retains the exact selected row's machine, entry, actual-contract identity,
  three axis identities, and envelope identity beside its calling-plan join
  key. Target planning independently rejoins that receipt to the current
  checked roster before carrying it in the bound placement and complete
  placement identity; thunk/root/manifest replay and the existing callback
  identity summary therefore reject a substituted checked resource anchor.
  Structural replay rejects a missing, duplicate, reordered, cross-entry,
  cross-machine, cross-contract, fused-axis, or fingerprint-drifted row. These
  are compilation-local derivation anchors and carriage receipts, not numeric
  ceilings, realized demands, backend footprints, provider receipts, or
  installation authority. Resource-ceiling aggregation still stops after this
  identity carriage: next join each axis to its independently derived Terminal,
  target, and backend evidence. The
  existing three-column external-root rows remain installation-owned and cannot
  be promoted backward into callback admission. This does not relax the private-
  placement decision or infer resources from `BoundaryEntryPlan`. Callback
  thunk body lowering separately stops on multi-
  root activation planning: instruction selection currently emits one process-
  entry Source function over one root runtime-flow/dispatch/storage activation,
  while internal-call operations carry no ABI bridge. Add per-entry root
  schedules with activation-local frame/storage identity and a validated
  boundary-to-internal argument/result recipe before emitting callback
  functions; placeholder enter/call/return bodies are unsound. The first
  address-free prerequisite now exists: each private thunk retains one
  independently replayed canonical entry, activation-local
  runtime-flow/dispatch/storage/frame identity set, validated boundary plan,
  and exact internal argument/result bridge. It owns no bytes, native address,
  relocation, resources, or registration authority. The first body rung now
  consumes that schedule for exact payloadless, resultless terminal leaves and
  emits only their validated enter/leave mechanics. The remaining body rung
  must extend selection to parameter/result bridges, body operations, and
  multi-entry/re-entrant target instructions; placeholder calls or invented
  activation state remain unsound. This
  prerequisite is independent of private registration placement and checked
  resource ceilings. Registration lease/unregister machinery is already complete below
  source binding: an exact provider registration receipt owns an installed-root
  code borrow, and release requires exact provider unregistration plus
  independent root unreachability/quiescence. The remaining durable
  `Registration` slice needs an emitted thunk and settled private registration
  binding that can mint that receipt, plus a checked linear source carrier;
  creating a detached lease earlier would invent authority. The
  remaining slices are
  resource-ceiling aggregation, general callback body and multi-entry/re-entrant
  target instruction lowering, and the
  private registration relocation from the now-settled binder-slot/native-place
  row,
  registration leases/unregister,
  and cross-target registered-callback canaries.
- **CALLBACK-PRIVATE-MATERIALIZATION — close the outbound registrar plan.**
  Extend normalized `CallPlan` with callback-materialization rows and
  `NativePlace` with exact native-parameter and validated layout-field paths.
  Add the compiler-known `PrivateCallbackSlot<machine Requirement>`
  relationship and bounded `Plan::place_private<Conformance>` source
  vocabulary. The named conformance must be selected explicitly, its subject
  must equal the active layout-plan producer, and its signature-free
  requirement path must resolve uniquely. Normalize that declaration plus its
  target-closed plan placement into typed private-materialization demands that
  are absent from the source schema and cannot be read, written, serialized, or
  addressed by source. Permit physical offsets only inside the authoritative
  layout policy; no materialization row, `Binding`, or source field may repeat
  one or use it as identity. Require one compatible nonoverlapping supply per
  binder and demand; reject missing, duplicate, wrong-layout,
  wrong-requirement, inferred-order, hidden-argument, raw-offset, ambient-
  conformance, and unresolved forms. Retain the fixed registrar fingerprint
  separately from per-use selected-machine/thunk identity and join them only
  during private relocation emission.

  The first vertical rung is complete. Traits can bind one non-callable exact
  machine declaration identity; conformance applications retain and validate
  the exact free-machine or signature-free trait-requirement symbol. Core owns
  `PrivateCallbackSlot<machine Requirement>` and the sealed
  `Plan::place_private<Conformance>` operation. Evaluator receipts are emitted
  only for executed calls, normalized without ambient lookup, retained on
  `PlanLaidLayout`, and included in native-layout fingerprints. Authored
  lookalikes, wrong layout subjects, duplicate placement, wrong static
  categories, and ambiguous requirement overloads reject.

  The target-closure rung is also complete. `omega-layout` independently
  rejoins every retained private callback demand to the canonical layout
  subject and complete native-layout identity, supplies the selected target's
  function-pointer extent/alignment, and rejects size/alignment drift,
  out-of-bounds placement, semantic/private or private/private overlap,
  subject substitution, duplicate slots, and nominal identity collisions.
  `At`, `IntegerAt`, `Bits`, and repeated-element physical extents retain their
  exact occupancy; validated stride padding remains padding. Domain-separated
  nonzero layout, slot, and callback-requirement identities publish in the
  exact `LayoutPlan` demand catalog, with the requirement constructor shared
  by boundary binders. The outbound publication/context-join rung is complete.
  `BoundarySignature` publishes exact compiler-issued binder and target-closed
  private native-place demand catalogs. Selected-target checked/native paths
  re-evaluate the source policy against that catalog and retain the exact
  `CallbackMaterializationContext`; the later nominal callback consumer replays
  the same context and rejects missing, duplicate, wrong-requirement,
  wrong-destination, or fingerprint-drifted rows. Target-neutral planning
  remains demand-free, and only the authoritative `LayoutPlan` may close it.
  The first address-free relocation-planning rung is complete. Nominal
  placement validation keeps the callback handler's inbound entry plan
  separate from the registrar operation's outbound realization, selects the
  latter by exact registration-operation symbol plus static ordinal and
  satisfaction row, and retains its complete ordered context, validated plan,
  fingerprint, binder, requirement, and `NativePlace` destination. Backend
  planning joins that row one-to-one and in placement order with the emitted
  private thunk/root schedule and independently replays missing, duplicate,
  reordered, or identity-drifted catalogs. `CallbackPrivateRelocationDemand`
  remains strictly address-free: it owns no target operation, byte, offset,
  object relocation, runtime storage, address, registration authority, or
  lease. Actual object relocation/emission and lifetime binding remain
  subsequent.
  The registrar-occurrence join is now complete through the next address-free
  seam. `HostCallPlan` and `AbstractBoundarySummary` retain the exact statement
  or expression site, registrar operation and canonical overload, state/
  statement/call ordinal, lowering identity, and ordered native-parameter rows
  carrying `NativeParameterId` plus semantic-formal identity where one exists.
  Backend planning binds each private demand to one
  exact registrar occurrence and root native argument; nested `Field` places
  preserve their layout identity and full ordered slot path, and distinct paths
  may share one root argument. Independent replay rejects site, target,
  overload, coordinate, lowering, order, cardinality, parameter, layout, or
  path drift. The binding still owns no physical home, field offset, target
  operation, bytes, object relocation, address, registration authority, or
  lease.
  The next target-closed placement rung is complete. Backend planning joins
  each exact registrar argument to the outbound `CallPlan` parameter's
  `ValuePlacement`. The direct field form retains exactly one authoritative
  `LayoutPlan` private-demand row. The bounded nested form instead retains one
  exact rooted path: a domain-separated semantic field identity for one inline
  named record field, its exact root/child layout identities and field-layout
  snapshot, followed by one child-owned terminal private-demand row. Layout
  closure and independent replay both prove that field belongs to the root,
  rejoin the exact child record, and checked-compose the field-relative and
  child-relative offsets while rechecking child and root bounds/alignment.
  Existing one-slot layout identities remain stable. Missing, duplicate, or
  colliding roots, children, fields, terminal rows, reversed/short/long paths,
  formal or ABI-placement drift, target-architecture mismatch, and identity or
  geometry drift reject. Reference, array, variant, and deeper field descent
  remain fenced engineering extensions. Direct-parameter declaration and
  target closure are now live through the normalized boundary signature and
  placement application; joining that direct row to a source registrar
  occurrence, selected callback, assigned operand, and emitted call remains a
  later engineering slice. This carrier still owns no selected/assigned
  operation, object symbol, relocation kind, bytes, runtime address,
  registration authority, or lease. The exact assigned-operand
  prerequisite is now complete for the custom/unknown outbound registrar
  branch. Selection retains the exact source host-call handle, call/operation
  ordinals, and ordered native-parameter-to-abstract-operand rows, with optional
  semantic-formal identity, while excluding the result pseudo-argument. Target
  lowering rejoins exactly one
  registrar occurrence and boundary edge and preserves exact abstract/target
  operand handles; backend planning binds those to the prior physical
  destination and assigned instruction/operand identity. Independent replay
  rejects same-coordinate call collisions, cardinality/order/identity drift,
  stale handles, and operand-shape substitution. Generic host operations
  remain outside this opt-in path. The carrier owns no object symbol,
  relocation, bytes, runtime address, registration authority, or lease. The
  first object-relative request rung is complete for the production one-slot
  `Field` form whose exact assigned operand is `RuntimeStorageAddress`.
  Backend planning binds the target-closed slot geometry to one canonical BSS
  storage-region symbol and the exact private callback text symbol, preserving
  region/base/slot/destination offsets, pointer extent, alignment, and both
  complete symbol snapshots. Independent replay rejects missing, duplicate,
  reordered, substituted, out-of-bounds, misaligned, `DataAddress`, direct-
  parameter, bounded-two-hop, or object-symbol-drifted rows. The exact
  address-store operation
  rung is now complete for that production shape. Backend orchestration inserts
  one `WriteFunctionAddressToRuntimeStorage` immediately before the exact
  registrar operation, preserves the registrar source coordinate and function
  span, rederives target/assigned handles after both insertion and the
  program-storage wrapper rebuild, and extends the validated root
  `CompilerBodyPlaceAddressWrite` footprint. Both ISAs encode the function and
  storage bases symbolically; relocation planning emits the exact x86-64
  `Absolute64` pair or AArch64 `Page21`/`PageOffset12` pairs, and final replay
  independently rejoins the private function identity, canonical BSS symbol,
  sites, kinds, addends, origin, cardinality, and unchanged instruction bits.
  `DataAddress`, direct parameters, bounded two-hop paths, registration,
  invocation, callback lifetime/lease, and publication authority remain
  fenced. The installation-entry manifest rung is now complete. Each ordered
  non-Clone row retains the full private object-store request, complete checked
  placement identity and requirement, domain-separated callback `EntryStubId`,
  exact Text function interval, canonical BSS snapshot, encoded address store,
  and architecture-specific relocation records; retained native artifacts
  independently replay that manifest. Deployment projects the sealed entries
  into artifact entry rows, binds one complete manifest entry to the exact
  unrelocated/materialized installed bytes, architecture, entry offset, and
  installed-code occurrence, and requires that attribution to match the root's
  entry and requirement before installation. Pending, live, failure, cleanup,
  and successful-quiescence carriers preserve the complete attribution rather
  than a compact report key. Missing, duplicate, reordered, symbol/geometry,
  instruction-kind, relocation, byte, entry, requirement, architecture, or
  installed-occurrence drift rejects with retry custody. This still grants no
  resolved address, registrar invocation, source-level `Registration`, live-
  registration capacity, lease, or publication authority; `DataAddress`,
  direct parameters, and bounded two-hop physical paths remain fenced.

  The first source canary cohort is live: the exact target-selected registrar
  closes two explicitly named, nonoverlapping nested private slots for two
  nominal binders while an independently named, uncited third-party
  `PrivateCallbackSlot` conformance remains inert. Focused negative canaries
  reject an uncited demand assumption, an overloaded signature-free requirement
  path, a cited conformance whose layout subject differs from the active policy
  owner, a cited slot whose callback requirement differs from the binder,
  duplicate cited placement, and a machine requirement substituted for the
  required named slot conformance. Two distinct cited slots at one physical
  extent also reject as overlapping supply. The first direct-source and
  application-v2 rung is complete under the settled design. A bodyless
  boundary requirement may interleave `native callback procedure from Binder`
  with ordinary parameters; syntax, resolved, and typed signatures retain its
  separate authored native position and exact nominal binder while source-call
  arity omits it. Target closure derives a requirement-and-declared-name-owned
  `NativeParameterId`, supplies the target function-pointer shape, merges the
  exact ordered native telescope, publishes `NativePlace::Parameter`, and
  validates the telescope one-to-one against the physical plan. The v2
  boundary-application identity commits the exact owner requirement, ordered
  nominal entries and origins/shapes, parameter placements, callback demands,
  and physical plan; callback-placement replay retains both its strong
  commitment and report coordinate. Focused canaries cover interleaving with
  no source runtime argument, rejecting a non-boundary declaration or unknown
  binder, and reissuing identity for a same-shaped reorder or renamed nominal
  parameter. Old ordinal-derived IDs remain a distinct v1 domain and are not
  translated heuristically.

  Remaining direct-form engineering is to bind a selected callback at an
  actual source registrar occurrence, carry its native-only row through exact
  assigned-operand/call emission and downstream replay, and complete the
  negative matrix for authored `addr`, undeclared insertion, inferred or
  duplicate/wrong binder/requirement, policy-created parameters, and stale v1
  or application-v2 evidence. Add the same
  target-neutral requirement placed at different x86/x64 offsets once the
  target catalog gains its missing 32-bit x86 engineering support; its present
  native targets are X86-64 and AArch64 only. The raw-offset canary now proves
  that an authored integer equal to the physical slot offset cannot substitute
  for compiler-issued parameter, layout, or slot identity while another exact
  demand remains valid. Semantic projection/read and assignment already reject
  because the private slot is absent from the layout data's source field
  schema; serialization walks only that declared schema and therefore has no
  private slot to name. Replay drift is already rejected independently by the
  nominal consumer's exact-context replay, checked placement-fingerprint
  binding, and late thunk-emission plan replay. These are internal retained-
  evidence seams, not source-authorable shapes.
- **REGISTERED-CALLBACK-LIFETIME — implement the runtime protocol.** A
  successful registrar call establishes one future external root represented
  by a linear `Registration`; rejection establishes none. Successful
  unregister ends the root before releasing code/component leases and returns
  the exact live-registration capacity occurrence. Rejection returns capacity
  unchanged, and an unsuccessful unregister retains it. Capacity bounds live
  runtime registrations, not statically emitted thunk count. `build.omg`
  selects and admits the realization/resources; ordinary Omega control flow
  performs registration. The deployment-owned custody rung is now live below
  that source protocol. It transactionally installs an independently admitted
  callback root into the exact deployment ledger before the ordinary registrar
  call, retains the installed root and ledger in a non-Clone pending carrier,
  and admits only the later exact successful provider receipt. The live carrier
  then owns reclaimable registration plus ledger custody until exact provider
  unregister and root quiescence return the original slot authority. Every
  installation, receipt, unsuccessful-unregister, and nonquiescent rejection
  returns complete retry custody; a false registrar result can explicitly
  remove the still-unregistered pending root. The emitted callback store/demand
  catalog is now joined through one sealed installation manifest entry to the
  exact installed bytes, entry offset, root entry, and requirement, and that
  complete attribution survives every pending/live/terminal result. Invoking
  the registrar and creating the source-level linear `Registration` remain
  engineering rungs. The deployment admission now also consumes one exact
  non-clonable provider-bound live-registration capacity occurrence only with
  the matching successful registrar receipt. Registration rejection returns
  the occurrence unchanged, the live callback retains it across every
  unsuccessful unregister or nonquiescent removal, and successful unregister
  plus root quiescence returns that exact occurrence beside the reclaimed root
  slot. This capacity is neither a lifetime budget nor a static thunk count.
- **FOREIGN-RETAINED-ARGUMENT-BACKING — generalize outside callbacks.** Keep
  argument backing and retention off callback-materialization rows. Specify the
  ordinary outbound-plan dispositions for call-scoped storage, public
  lifetime-bound borrows, moved stable custody, and private snapshots. Require
  exact stable-root/range/access/lifetime/revision provenance for every retained
  pointer edge; unknown provenance rejects, recursive graphs require aggregate
  arena/extent custody, and copying requires an explicit semantic snapshot
  contract. Provider backing may not change a pinned public result type;
  concurrent foreign writes use External placement, while exclusive mutation
  returns the requirement's declared preserved/invalidated content outcome.
- Implement the narrow Windows `user32` canary without exposing a raw code
  address. Derive `Atomic::interruption_fence` same-context evidence from the
  installed external-root route and reject it elsewhere.

Acceptance: changing a normalized plan changes lowering or rejects; forbidden
state introduced anywhere in final executable text rejects; a registered
callback cannot outlive its registration/code lease or smuggle application
state through a raw address.

### P5 — Cathedral bring-up over general Omega primitives

#### BUMP-ALLOCATOR-CANARY — package allocator

- After P1 conservation is source-usable, implement a package-level bump
  allocator over one qualified `Extent`. Two allocations must coexist; release
  cleans and returns the exact subextent without restoring bump-tail capacity;
  reset succeeds only after full recomposition; finish returns the original
  backing.
- Implement owned `Vec<T>` and then `Vec<u8>::Utf8` only after choosing the
  allocator contract needed for cleanup, authority return, and capacity reuse.

#### Address translation

- Build Cathedral's page-table hierarchy, validation states, installation, and
  teardown in Omega source using `source/drivers/facts/x86_page_table_entry.omg`.
- Use pre-reserved storage for the fixed bootstrap table; dynamic hierarchy
  allocation waits for the package allocator. Cathedral now represents one
  512-entry page candidate and validates its complete exact-zero starting
  state with a checked bounded scan. It also retains and independently replays
  one exact four-level numeric walk, then validates the selected 4-KiB roles:
  the upper three entries are present non-large-page links and the PT entry is
  present, while the PT PAT bit and every other permission/cache field remain
  uninterpreted. The exact descriptor returns unchanged and grants no backing,
  mapping, placement, TLB, installation, CR3, or machine-control authority.
  A following ordinary Cathedral carrier binds one requested virtual page base
  and physical frame base to that role-consistent descriptor. It requires the
  virtual endpoint to equal the retained canonical address with zero page
  offset and the physical endpoint to equal the PT entry's retained target,
  then returns both exact endpoints with the validated descriptor. This records
  numeric 4-KiB intent only; identity/higher-half layout and permissions remain
  unchosen, and no Extent, backing, hierarchy-page, mapping, placement, TLB,
  installation, CR3, or machine-control authority is granted.
  Cathedral now also validates one complete 512-entry page image against one
  retained walk step. It replays the exact step geometry, requires the selected
  entry to be present and field-for-field identical at its retained index, and
  checks with decreasing fuel that all 511 other entries preserve the complete
  zero encoding. Success returns the exact step and page unchanged; every
  permission/cache/software/PAT field remains uninterpreted, and no backing,
  placement, hierarchy, mapping, TLB, installation, CR3, or machine-control
  authority is granted. An ordinary Cathedral aggregator now replays
  endpoint/role consistency, independently validates one complete single-entry
  image for each PML4, PDPT, PD, and PT level, and binds every image step to its
  named descriptor step by exact table address, index, entry address,
  target/PFN, and full uninterpreted PTE equality. Success returns the exact
  endpoints, descriptor, and four pages unchanged. It still grants no backing,
  placement, hierarchy, mapping, TLB, installation, CR3, or machine-control
  authority and chooses no layout or permission policy. Physical backing,
  address-space-profile hierarchy, authority-bearing mappings, installation,
  and teardown remain. Do not restore a compiler-owned page-table model.

#### Exception roots and first timer

- Materialize fatal/diagnostic entries for every architectural exception before
  enabling interrupts. Cathedral now has a checked fixed-work internal leaf
  that records one normalized 0–31 vector in preallocated atomic state,
  publishes its validity, and unconditionally aborts. Generated per-vector
  stubs, admitted internal-state binding, physical entry plans, stacks, gates,
  and IDT installation remain.
- Provision dedicated per-CPU double-fault/NMI/machine-check stacks and one
  non-nesting maskable-IRQ stack class; preserve the selected `StatePlan`.
  Cathedral now authors and validates the complete four-role class/IST policy;
  WCSU-derived byte sizing, a source-level `StackLease`, storage provisioning,
  and installed-root binding remain.
- Bring up PIT+PIC first and LAPIC as the production provider. The hard root only
  acknowledges, records time, publishes a coalesced wake, and returns; fan-out
  runs in an ordinary task.
- **BOUNDED-INSTALLATION-REACH-ROWS.** Implement bounded installation reach
  rows on installation-bound boundary requirements.
  - **Live:** `reaches <= Bound` parses only on fresh bodyless boundary
    requirements, currently declared inside boundary traits or through the
    transitional top-level `boundary machine` spelling; syntax through typed snapshots retain the
    marker. Checked inference propagates exact requirement dependencies
    separately from the conservative bound and from concrete reach.
    Conformance rejects a provider
    outside the bound. Provider selection derives and fingerprints each exact
    `InstallationReachResolution`. External-root admission now accepts only a
    `ResolvedRootServiceReach`, substitutes `concrete + selected rows`, rejects
    absent selections, and retains the resolutions in the installed record.
    The preselection capability manifest publishes every unresolved exact
    requirement identity with its conservative upper bound rather than
    pretending the bound is the selected provider row. Postselection manifests
    join those identities to the exact selected rows and provider-plan
    identities, rejecting absent resolutions or bound drift. Attached Unit
    roots now lower a source-handle-free Terminal Psi closure containing the
    concrete row separately from exact bounded dependencies. The canonical
    codec and verifier retain and validate both, and final root admission
    resolves that closure against selected provider facts while rejecting an
    absent selection or changed bound. The Terminal verifier independently
    reconstructs the entry's concrete service closure and used installation
    dependencies from executable calls and operations. Missing, padded, stale,
    or unused rows reject; a direct concrete use remains concrete even when it
    also appears in an abstract bound. Result-bearing boundary roots now use
    the same closure: nominal static-machine calls retain their exact bounded
    requirement, primitive results no longer require an unrelated custody
    transfer, and codec/verifier canaries reject deletion, drift, or padding.
    Explicit top-level installation-bound requirements now lower the
    same exact dependency through their normalized machine-overload identity;
    trait requirements retain their normalized trait-requirement identity.
    Missing, ambiguous, wrong-kind, duplicate, changed-bound, and unused rows
    fail closed through lowering, codec replay, or independent verification.
    Boundary-operator provider slots remain independently validated against
    their exact typed operator schemas and never enter this installation-reach
    resolver.
    Installation-bound internal machines publish the conservative bound only
    inside their Terminal closure; ordinary private effectful machines still
    reject without an authored ceiling. Neither lowering nor verification
    reconstructs concrete reach by subtracting bounds.
  - **Remaining:** reject unresolved rows escaping ordinary callable package
    or component contracts when that export/interface carrier lands. Do not
    reject internal inferred callers globally: the installed-root closure is
    their legitimate resolution scope. The other current structural root
    producers contain no service-bearing boundary operation, so there is no
    additional producer row to populate today; verifier reconstruction remains
    the fail-closed fence if one gains such an operation.
    Exact checked satisfiers and typed provider selection now close explicit
    top-level requirement dependencies. External satisfiers and the final
    carrier-owned invocation path remain under
    **TOP-LEVEL-BOUNDARY-REQUIREMENTS**; they must replay the installed
    execution and era rather than performing ambient redispatch. Until that
    invocation path lands, the core completion declaration retains its
    transitional fixed `PortIo` row rather than claiming executable selected
    dispatch that checking cannot yet provide.
    Installation settlement no longer depends on that placeholder: its sealed
    completion route replays the exact installed reach resolution and binds the
    selected provider plan/execution, entry receipt, acknowledgement policy,
    invocation, and token. PIC and LAPIC canaries retain distinct `PortIo` and
    `MachineControl` rows, and coordinate drift returns retry custody.
  - **Constraints:** `+` is union. Do not infer one shared row from equal sets or
    add negation, subtraction, lower bounds, exclusive-or, named row variables,
    or cross-requirement correlation.
- `InterruptEntry::enter` publishes its bounded installation row beneath
  `MachineControl + PortIo`; the PIC-shaped provider-plan canary proves that the
  conservative bound resolves to the selected provider's exact `PortIo` row.
  Top-level boundary operations now carry the same abstract-row representation
  and checked unresolved-row identity, but `InterruptAcknowledgement::complete`
  remains temporarily hardcoded to `PortIo`: migrating it before provider
  coherence is represented would make checked PIC implementations appear to
  reach the full conservative union. Bind entry/completion coherence through
  the exact installed provider execution, acknowledgement policy, operation,
  and token lineage rather than row equality. That installation-only route is
  now sealed and replayed at acknowledgement settlement; migrating the public
  completion declaration to its distinct bound remains blocked on
  **TOP-LEVEL-BOUNDARY-REQUIREMENTS**. PIC
  completion resolves to `PortIo`; LAPIC/x2APIC completion resolves to
  `MachineControl`. Checked and terminal artifacts retain the
  selected provider, operation, bound, resolved row, and refinement evidence
  without granting authority from reach.

Acceptance: QEMU installs Cathedral-owned memory/interrupt structures, reports
timer ticks over owned serial output, and halts between ticks. No customer-shaped
compiler concept is introduced.

## Parallel compiler and language lanes

### Frames, reach, and trust

- **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW.** Replace package review's
  filename-and-trait keyed dangerous-authority classifier with the settled
  service/terminal containment model. Preserve normalized service reach as the
  package-stable preinstallation axis. Independently traverse each exact
  selected-provider closure to static target-qualified terminal bindings,
  normalize `StringBackedImportBootstrap` through the ordinary foreign-locator
  identity, classify exact bindings under closed compiler/target policy, and
  reject or report any exercised class outside the exact service identity and
  schema's permitted set. Unknown terminal mechanisms fail closed; risk labels,
  package names, aliases, paths, and model verdicts grant nothing. Cite the
  authored `providers/external_leaf_syscall_compile` canary and the rejection
  in `providers/via_runtime_binding_rejected` as the static-coordinate fences,
  and require the same property of every future binding kind.

  Land this redirect and delete the blessed filename table before faceting
  `FilesystemHost`. Then migrate its requirements and the existing
  `reaches FilesystemHost` rows to portable content-read, content-write,
  metadata-query, directory-enumeration, namespace-mutation, and
  metadata-mutation facets while retaining exact requirement/method identity.
  Split flag-polymorphic operations or conservatively publish the union of all
  enabled facets. Evidence over the current raw `i32` descriptors may claim
  operation classes only. Add typed unforgeable descriptor handles and checked
  attenuation before claiming that reads or writes are confined to objects
  opened with corresponding authority. The redirect is owner-blocked on Q7's
  first closed target/binding classification table; the containment formula
  alone does not determine authority for current syscall, import, firmware,
  table, or checked-physical coordinates.
- **R5:** continue exact inferred may-write summaries and relational candidates.
  Exact frames compose through transparent returns/helpers, caller-isolated
  scratch locals, statement/value positions, stable mutable aliases, and direct
  alias replacement; rebinding leaves earlier reborrows intact. The bounded
  non-reference direct-call expression class is complete through depth two,
  including member projection and one or more independently bounded indexes;
  typed non-reference assignment-value call trees extend through depth four.
  A direct primitive scalar assignment value may wrap complete caller-isolated
  call producers in up to fourteen unary, binary, primitive-cast,
  member-projection, or indexing shells without widening that call budget. A
  fifteenth direct scalar shell remains fenced; aggregate fields and projected
  concrete record, selected-case, or fixed-array literals retain their
  separate two-shell computation budget.
  One top-level concrete primitive-only record or selected-case literal may
  likewise contain an independently bounded non-reference call tree in each
  direct common or payload field while publishing every write. Direct typed
  assignment values may nest those concrete aggregates through depth three;
  every primitive leaf obeys the same rule, and this rail does not widen the
  depth-four call budget. A declared primitive field at any admitted level may
  wrap independently bounded call operands
  in up to two nested scalar-computation shells made from unary/binary
  operators, primitive value casts, member projections, or indexing without
  widening that budget. Literal-length caller-isolated fixed-array assignment
  values preserve the same relation through two nested array levels; every
  element retains the same call and primitive-computation budgets. Within that
  same three-level direct aggregate budget, fixed arrays may contain concrete
  record or selected-case literals, and concrete record or selected-case
  fields may contain literal fixed arrays. A primitive scalar assignment value may also
  select one direct member from a concrete caller-isolated record or
  selected-case literal whose effectful primitive fields are bounded
  direct-call trees or use one scalar-computation shell around those calls;
  every field publishes its writes. One additional outer scalar shell is
  admitted only when the fields do not consume that remaining shared
  computation-depth-two budget. The literal receiver may use the existing
  two-level aggregate budget while carrying that reduced computation budget
  unchanged; a third aggregate level remains fenced. The same primitive
  assignment may directly index a fixed-array literal, including one nested
  fixed-array level, whose eagerly evaluated elements contain bounded calls or
  use the one remaining scalar-computation shell; all element and independently
  bounded index-call writes publish even when a selected index is constant. The
  nested literal consumes the existing aggregate-depth-two rail without
  resetting the call or computation budgets. A third array level and
  reference-valued or opaque/recursive elements remain fenced. One outer scalar
  shell may instead consume the remaining computation budget; combining both
  remains a third-shell fence.
  Indexing irreversibly coarsens to the nearest backing collection while
  preserving independent index-call writes. Finite named-state SCCs accept only
  bijective write-capable parameter permutations. Primitive-only concrete
  record/sum locals remain isolated through nested fixed arrays.

  Continue with representable relational candidates. Boundary,
  beyond-per-position-budget, binding-reborrow, reference-valued/opaque,
  escaped, non-bijective, generic, recursive or reference-bearing aggregate
  literals, a fourth direct aggregate level, a third projected aggregate or
  aggregate/literal computed shell, a fifteenth direct scalar computed shell,
  other computed field shapes, and out-of-isolated-root shapes remain
  conservative fences. Do not restore
  authored `stores` clauses or treat lifetime elision as evidence; Git carries
  individual evidence cohorts.
- **TPR6 — finish subject-bearing progress-premise normalization and coverage.**
  Public schemas, exact-call instantiation, private-helper and named-state
  substitution, exported-body coverage, local admitted-receipt discharge, and
  structured provider-plan/trust retention are implemented. Mentioning or
  carrying a qualified value creates no premise; nominal static-machine
  binders remain pinned to their named requirement contract; private ranking
  witnesses remain outside public identity.

  Checked progress and ranking now share one canonical target-state rule for
  named local transitions. A back-edge to machine entry names the machine
  symbol while a subordinate transition names its state symbol; both remain
  edges within one activation. Progress-subject correspondence uses the same
  resolved state's formal parameters, so measured entry recursion retains its
  proven `Terminates` summary and exact premise lineage rather than falling to
  `NoGuarantee`.

  TPR6-A now preserves call-specific provider-receiver demands separately from
  caller premises, closes them through the exact selected entry and checked-
  adapter call graph, joins each to one exact selected provider plan/schema
  row, and stores the result in a canonical component-progress manifest. Each
  provider premise now also retains the profile owner's exact normalized
  `established by` requirement-route set; that set enters provider-plan and
  component-manifest identity and is rendered for audit. These are authorized
  establishment relationships, not receipts. The capability manifest only
  renders the canonical carrier. Final/native composition rejects pending rows
  after checked artifacts are available; selected-plan, authorized-route, or
  trust-report identity is never treated as an establishment receipt.

  TPR6-B now seals the complete selected-plan set to exact provider
  occurrences under the one installation registry, admits a progress receipt
  only after replaying its distinct subject and issuer occurrences, exact
  boundary route, and one grant invocation, and closes the component manifest
  transactionally. Receipt facts may serve several matching call sites but
  cannot be rebound by compact identity, profile, route, occurrence, or
  invocation. Terminal installation format 36 commits the manifest, structural
  access modes, and opaque
  acceptance identities into the canonical installation bytes, which the
  terminal artifact manifest already fingerprints. Runnable component-era
  publication now binds the complete terminal object and image, canonical
  installation record, the linear `InstalledCode` claim itself, and opaque
  progress closure into one non-clone carrier. The claim can no longer be
  retired independently while the runnable carrier is live. Publication
  retains it until successful retirement; every rejected binding,
  publication, or retirement returns the exact installed-code custody,
  candidate, receipt, and evidence without partial commitment. A
  source-derived progress-free Terminal-Psi canary now crosses lowering,
  verification, object/image emission, installation, runnable binding, and
  component-era publication while retaining that exact custody. A selected-
  entry source-derived progress-bearing canary now crosses the same path: its
  exact `self.field` premise remains in the installation manifest, the
  boundary receiver selects the provider occurrence without becoming an ABI
  argument, and publication rejects when the committed opaque acceptance is
  omitted before retaining it with the installed code. The production compiler
  now stages an exact selected entry into a non-visible terminal component
  candidate containing canonical semantic/proof bytes, object and image,
  selected provider-plan closure, owned provider-execution identity
  projections, and any nonempty progress manifest. Staging rejects target
  substitution, missing or duplicate boundary settlements, and executions
  outside the selected plan closure. The candidate carries no output path,
  visibility receipt, provider occurrence, progress receipt, or installed-code
  authority; compilation cannot mint deployment custody.

  The production `omega-component-deployment` owner now consumes that
  candidate beside one real `InstalledCode` occurrence. It validates exact
  unrelocated/materialized bytes before the one-shot registry claim, returns a
  staged session for every later retry, seals the complete selected-plan set
  to provider occurrences, admits exact progress attestations, canonically
  builds and decodes installation metadata, and binds the complete
  object/image/code join. Selected but unexecuted plans remain in installation
  identity; executions must be a selected subset and must exactly cover image
  settlements. Complete installed-root teardown now consumes the full owned
  root/receipt set transactionally, preflights every row and the exact private
  live root/evidence/slot/interrupt state before mutation, and returns both the
  same emptied already-claimed ledger and its slot authorities in caller order.
  Rejection returns the untouched ledger and every original row for exact
  retry; successful teardown preserves sealed required-slot, provider,
  progress, cohort, and one-shot interrupt history. Deployment can consume that
  exact emptied ledger beside its matching `InstalledCode` and candidate
  without attempting a second registry claim. Runnable binding retains the
  live registry and checks its canonical exact-empty predicate and selected
  closure even for progress-free artifacts. Successful era retirement is the
  sole public decomposition path for runnable code and registry custody. The
  deployment owner now also exposes the flat-file endpoint needed by the later
  compiler handoff: it consumes only a finalized runnable, replays the canonical
  installation/image join, atomically stages and publishes the exact sealed
  image bytes with executable mode, and returns a non-clonable receipt beside
  the still-owned runnable. The receipt binds the installation and image
  fingerprints, byte count, sealed filename, and path; any publication failure
  returns the exact runnable and requested path for retry, and later file drift
  invalidates replay. Progress-free and progress-bearing source canaries cross
  this endpoint without losing the accepted manifest or installed custody.

  The authority-free terminal component candidate now lives in the neutral
  `omega-component-candidate` crate. Both the compiler and deployment
  owner depend on that carrier, and deployment no longer depends on the
  compiler. This removes the crate cycle that prevented the compiler from
  invoking deployment while retaining private candidate fields, consuming
  decomposition, and zero installation or publication authority.

  The compiler output owner now has one real deployment handoff for the final
  filesystem step. `write_finalized_terminal_component_output` accepts only a
  deployment-finalized runnable, derives the canonical build-directory path
  from the image's sealed filename, and delegates consuming publication to
  `omega-component-deployment`. It cannot manufacture installed code, provider
  occurrences, progress acceptance, or a profile decision. Rejection returns
  the exact runnable and derived path through the deployment error; the
  progress-free and progress-bearing source canaries cross this compiler seam.

  A typed compiler deployment transaction now spans the remaining owner APIs
  without weakening them. `TerminalComponentDeploymentInputs` owns the staged
  candidate, one real `InstalledCode`, exact provider-occurrence bindings,
  exact progress attestations, and the profile decision; the compiler consumes
  it through deployment begin, provider closure, progress closure,
  finalization, and the established output seam. Its error is stage-typed and
  retains the exact current deployment carrier plus every unconsumed later
  input, rather than collapsing a linear failure into diagnostics. Source
  canaries pin both begin-stage installed-byte rejection/recovery and complete
  progress-bearing provider/progress/profile publication through this
  transaction.

  `CompileReport` can now retain that transaction result without erasing its
  custody. The report is non-clonable and admits either the legacy compiler
  publication receipts or one complete terminal deployment result, never both.
  Terminal report construction replays the deployment before taking ownership;
  rejection returns the whole published runnable, while success exposes only a
  borrowed view, a validated native path, or a consuming transfer back to the
  next owner. A source canary drifts the visible file, recovers the rejected
  deployment, republishes it, and transfers the repaired custody through the
  report.

  The production-driver tail now accepts independently acquired deployment
  values without pretending the compiler owns their acquisition. A
  `TerminalComponentDeploymentSupply` contains one real `InstalledCode`, exact
  provider-occurrence bindings, exact progress attestations, and the profile
  decision, but not the compiler's staged candidate. The driver binds that
  supply to the candidate once, runs the typed deployment transaction, and
  constructs the non-clonable terminal-deployment `CompileReport`. Deployment
  rejection retains the current typed stage plus unused report metadata;
  report-admission rejection returns the deployment together with root/source
  and build evaluation/observation metadata for exact recovery. A progress-
  bearing source canary crosses this supplied-input driver tail.

  The production driver now has an explicit live-owner acquisition boundary.
  `TerminalComponentDeploymentInputOwner` is invoked against the exact staged
  candidate and may return only the authority-bearing supply above. Acquisition
  rejection returns the unchanged owner, candidate, source count, and build
  evaluation/observation metadata; success immediately enters the established
  typed deployment/report transaction. A source canary rejects a substituted
  target, recovers and corrects the same owner, then proves that a later
  installed-byte rejection still returns typed deployment custody. The
  progress-bearing source canary reaches report custody through the owner call.

  Ordinary terminal candidate staging is now connected to that acquisition
  boundary by one strictly compositional driver. `TerminalComponentStagingInputs`
  retains the exact target, subsystem, admission profile, and provider
  settlements; the driver calls the established staging operation, then owner
  acquisition, then the typed deployment/report transaction without re-owning
  any of their policy. Staging rejection returns those inputs, the deployment
  owner, source count, and build evaluation/observation metadata intact. A
  progress-free source canary crosses staging, the real installation ladder,
  deployment, publication, and report custody through this composition.

  The connected driver no longer accepts caller-restated checked facts.
  `TerminalComponentStagingInputs::from_checked` binds the exact selected native
  target from the owning `CheckedCompilation`; a targetless checked result
  returns subsystem, admission profile, and every provider settlement intact.
  The driver likewise projects build evaluation usage and observation summary
  from that same checked owner instead of accepting parallel report copies.
  This is an explicitly temporary cutover adapter: it projects target/report
  facts only and never realizes executable semantics. `stage_terminal_component`
  remains the sole checked-result consumer that creates the authority-free
  terminal candidate. A source canary recovers targetless binding inputs, binds
  them to an exactly targeted checked result, and proves the checked metadata
  reaches terminal-deployment report custody unchanged.

  The ordinary Psi-owned checked frontend now has a separate typed terminal
  compile handoff without entering another backend coordinator.
  `CheckedCompilation` retains its exact consumed source count and build-
  selected image subsystem; `TerminalComponentCompileRequest` owns compile
  options, optional package inputs, the externally borrowed admission profile
  and provider settlements, and the external deployment-input owner. Frontend
  rejection returns that complete request. The request now transactionally owns
  checked staging-input settlement: targetless rejection returns both the exact
  checked result and original complete request, while success yields one bound
  owner retaining options, package inputs, checked/staging evidence, and the
  deployment owner. The free driver no longer decomposes a five-part request
  tuple or reconstructs its custody; later rejection uses the established
  staging/deployment custody and retains options and package inputs beside it.
  The successful source canary proves the
  checked-owned three-file count reaches report custody rather than the former
  fixture-restated count of one. This handoff never calls ordinary
  `Compiler::compile` or a publication operation and cannot manufacture
  installation, provider-occurrence, progress, or profile-decision authority.

  Remaining TPR6-B engineering: retire the legacy compiler's temporary final-
  output rejection only after a concrete non-test installation/deployment owner
  implements this acquisition boundary and an ordinary production caller
  supplies that owner to the typed terminal handoff. No such provider exists in
  the compiler today. The current legacy path publishes a native executable
  directly and carries neither the manifest nor an installation acceptance, so
  removing the fence there would erase the obligation; selected plans and
  authorized routes remain insufficient. This is missing platform/provider
  engineering, not a language-design block.
  The checked-only qualification-correspondence carrier is now live for
  existing statement transfers between exact parameter-rooted structural
  Field/Case places and nested exact in-bounds `FixedIndex` paths through
  literal-length fixed arrays. A separate deterministic proof ledger retains
  the source and destination fact handles, the fact's source place, the exact
  contextual source occurrence, destination place, formation point, unchanged
  qualification payload/domain, and exact `CheckedTransformation` evidence.
  Emission and checked-progress replay independently require structural place
  equality rather than label fallback, source-before-destination construction,
  exact statement-transfer origin/point, valid formation ownership, and
  identical payload/evidence. They also independently walk exact data members,
  machine attachments, and literal array element types and reject every
  out-of-bounds index. Every retained source, contextual source
  occurrence, and destination parameter root must belong to the formation
  machine or exact formation state; same-shaped foreign-machine and sibling-
  state parameter substitution rejects independently at production and replay.
  Unknown, expression, type, runtime-indexed/ranged, nonliteral-length, local,
  generic, label-only, invalid-member, or mismatched source occurrences remain
  fail closed.
  This carrier grants no admission or Terminal authority. Broader authored
  qualification-preserving transitions outside this narrow existing flow shape
  remain engineering work.
Acceptance: contract axes normalize independently, wrappers cannot launder
reach or trust, and private proof improvements do not change public identity.

### Multiplicity, tasks, and execution

- **CML4:** construct the complete `EdgeCleanupPlan` after outgoing-value
  materialization and transfer-map commitment. Current Unit/scalar and bounded
  acyclic slices retain reverse-declaration cleanup, partial-record transfer of
  prefix-disjoint all-field paths, maximal-residual disposal, nominal helper
  calls, shared targets, edge/action ownership, and direct-Boolean contextual
  obligations through terminal verification, interpretation, fuel, and all
  native artifact paths. Nominal scalar cleanup admits finite continuation
  chains whose stages contain arbitrarily nested finite short-circuit Boolean
  decisions. One finite parameter/constant decision tree, including Boolean
  equality against a constant, can instead feed a typed shared terminal-Psi
  convergence value and one native cleanup tail. Extend contextual
  cleanup beyond the current receiver-independent Boolean subset, finite
  continuation trees, and that narrow shared-convergence shape; add
  wider structural partial values, repeated-cycle resource composition, and
  conservation/backend-ledger reporting. This is not yet a general conditional
  CFG, complete cleanup plan, or conservation witness.

  The mixed scalar/affine record rung is closed for claim-free records whose
  fields are bounded Terminal scalars, exact binary32/binary64 float leaves,
  exact-capacity bounded-owned byte-sequence leaves, or affine structural
  subtrees. Scalar, float, and bounded-byte fields remain ordered
  structural/type and partition identity, but contribute no moved path,
  residual cleanup, runtime action, or fuel. Projected moves still select only
  affine structural fields, and maximal live structural residuals are cleaned
  recursively in reverse declaration order. Checked lowering, Terminal
  verification, machine emission, and object/installation replay independently
  reject scalar/float/byte/structural classification drift and preserve the
  bounded carrier's exact capacity and `N + 8` native layout. Borrowed byte
  views, first-class byte roots or byte moves, provider-backed or erased values,
  sums, claims or content, qualified roots, contracts, and nominal drop remain
  fenced from this rung.

  One independently ordered fixed-array rung is also closed: a claim-free
  affine unqualified root exactly `[T; 2]`, where `T` is an affine structural
  checked record with no nominal cleanup, may make one ordinary one-parameter
  Unit call that moves exactly literal index `0` or `1`; the opposite element
  is the one exact no-code residual on the one-fuel return edge. Typed and
  checked planning, Terminal verification/codec/interpreter, target and machine
  lowering, and object/image/installation replay retain the exact element type,
  index, length-two native layout, element stride, byte offset, cleanup action,
  and unchanged fuel.

  Its exact no-residual checked/Terminal successor is also closed. Two ordinary
  one-parameter Unit calls may move literal indices `0` and `1` exactly once
  each, in either authored order, and then use an ordinary Unit return with no
  residual cleanup. Checked replay and Terminal shape/frontier verification
  independently require the exact length-two record-element root, set
  `{0, 1}`, ownership, lack of claims/content/qualifications, and empty return
  cleanup; checked production also excludes contracts. Codec and interpreter
  replay preserve authored call order, reject duplicate/missing/path/length
  drift, and charge the exact five closure units. Target and machine lowering,
  object/image validation, and installation replay now independently rederive
  one exact function-level two-call `{0, 1}` witness. Both authored orders cross
  all five targets with canonical element stride/offset custody, an empty
  ordinary-return cleanup ledger, and exact operation/edge fuel ordinals;
  missing, duplicate, reordered, wrong-layout, or cleanup-bearing artifacts
  reject without weakening either projected call in isolation.

  One exact wider residual-bearing successor is closed. Under the same
  claim-free affine, unqualified, record-element, and no-nominal-cleanup
  restrictions, `[T; 3]` may move exactly two distinct literal indices through
  two ordinary one-parameter Unit calls in authored order. The complement is
  the one exact typed no-code residual on `ReturnUnitPartialAffine`. Checked
  production and Terminal shape/frontier replay reject one move, three moves,
  duplicate or nested paths, scalar parameters, extra control, claims/content,
  qualifications, contracts, and residual/type drift. Codec and interpreter
  replay preserve the two calls, singleton residual, authored order, and exact
  five closure units. All five target pipelines independently retain the
  length-three layout, canonical element stride and offsets, operation/edge
  fuel, two-call custody, and the one object/image/installation cleanup action;
  target, assigned-machine, object, and installed tampering fail closed. The
  singleton complement makes no array cleanup-order choice.

  The general fixed-array order is now settled: literal construction establishes
  increasing indices; ordinary cleanup-bearing edges emit a static decreasing-
  index sequence over the exact live residual set; nesting recurses; authored
  moves retain authored order; and trap/nuclear-abort edges clean nothing.
  The first multiple-residual rung is closed under the existing claim-free,
  unqualified, affine record-element restrictions: `[T; 3]` may move one exact
  literal index through one ordinary Unit call, then discard the two live
  complement indices in decreasing order on `ReturnUnitPartialAffine`. Checked
  production and replay, Terminal shape/frontier verification, codec and
  interpretation, target and machine lowering, optimization replay, and
  object/image/installation partition validation all independently reconstruct
  the exact two-residual complement. Producer-authored increasing order rejects,
  and interpreter fuel remains the exact three closure units.

  The exact wider flat successor is also closed: `[T; 4]` may move exactly two
  distinct literal indices through two ordinary Unit calls in authored order,
  then discard the two-index complement in decreasing order. Checked and
  Terminal replay, codec and interpretation, target lowering, optimization,
  machine emission, object/image validation, and installation replay retain
  the length-four layout, canonical stride and offsets, two calls, two cleanup
  actions, and exact five closure units. Duplicate, missing, out-of-bounds,
  reordered-cleanup, wrong-layout, and wrong-move-count tampering reject. This
  remains one straight-line static plan with neither runtime liveness flags nor
  a cleanup loop.

  The first recursively nested multiple-residual rung is closed under the same
  static rule. A claim-free, unqualified affine root exactly `[[T; 3]; 2]`,
  where `T` remains a checked record with no nominal cleanup, may move exactly
  one direct leaf from each outer element through two ordinary one-parameter
  Unit calls. Authored moves retain their order; cleanup emits the four live
  leaves with outer indices decreasing and, within each outer element, inner
  indices decreasing. Typed and checked planning, Terminal verification,
  codec/interpreter replay, optimization ownership validation, target layout,
  machine emission, object/image validation, and installation replay all
  independently retain the exact two-index paths and nested type graph. The
  five native targets retain the 48-byte outer carrier, 24-byte outer stride,
  exact offsets, four cleanup actions, and canonical two-call/return fuel
  ordinals. Same-outer, missing, duplicate, reversed-cleanup, and artifact-order
  tampering reject. The interpreter still charges exactly five closure units:
  residual count changes static frontier custody, not executable cleanup work.

  The exact nested successor is also closed. The same carrier admits
  `[[T; 4]; 2]` with one distinct literal leaf move from each outer element and
  six live residual leaves in decreasing outer-then-inner order. Every custody
  and replay layer retains the nested type graph, authored two-call order,
  64-byte outer layout, 32-byte outer stride, exact leaf offsets, and six
  cleanup actions on all five native targets. Missing, duplicate, same-outer,
  out-of-bounds, reordered-cleanup, wrong-length/layout/stride/offset, codec,
  object, image, and installation tampering reject. Static residual count again
  does not change the exact five call/return fuel units; no runtime liveness
  bitmap or cleanup loop is introduced.

  The following exact nested successor is also closed. The same carrier admits
  `[[T; 5]; 2]` with one literal leaf move from each outer element and eight
  live residual leaves in decreasing outer-then-inner order. Every custody and
  replay layer retains the authored two-call order, nested type graph, 80-byte
  outer layout, 40-byte outer stride, exact leaf offsets, and eight cleanup
  actions on all five native targets. Missing, duplicate, same-outer,
  out-of-bounds, reordered-cleanup, wrong-length/layout/stride/offset, codec,
  object, image, and installation tampering reject. Fuel remains exactly five
  call/return units without runtime liveness state or a cleanup loop.

  The next exact nested successor is now closed. The same carrier admits
  `[[T; 6]; 2]` with one distinct literal leaf move from each outer element and
  ten live residual leaves in decreasing outer-then-inner order. Checked
  production through Terminal codec/interpreter replay, Omega lowering,
  optimization validation, five-target layout and machine emission, object,
  image, and installation replay retain the authored two-call order, 96-byte
  outer layout, 48-byte outer stride, exact offsets, and five fuel units.
  Missing, duplicate, same-outer, out-of-bounds, reordered-cleanup, wrong-
  length/layout/stride/offset, codec, object, image, and installation mutations
  reject. `[[T; 7]; 2]` and wider forms remain fenced without runtime liveness
  state or a cleanup loop.

  The first construction-prefix ordinary-abandonment rung is closed. An
  uninitialized mutable `[T; 3]`, with `T` the exact empty, unqualified,
  claim-free affine record carrier, may establish literal indices `0` then `1`
  in authored order and fall through to an ordinary Unit return. Typed and
  checked planning retain two zero-ABI construction-element locals with their
  common root identity; Terminal verification and codec replay, Omega lowering,
  optimization identity, target assignment, native emission, object/image
  validation, and installation encoding independently retain establishments
  `[0, 1]`, cleanup `[1, 0]`, and exact two-operation/one-edge fuel ordinals.
  Index/root/length/order tampering rejects. Initialized, third-element,
  dynamic-index, nonempty, nominal, qualified, claimed, and wider construction
  forms remain fenced.

  The exact next construction-prefix rung is also closed. Under the same
  restrictions, an uninitialized mutable `[T; 4]` may establish literal
  indices `0`, `1`, then `2` and abandon that prefix on an ordinary Unit
  return. Checked through installation replay retain the three ordered
  zero-ABI locals, reverse cleanup `[2, 1, 0]`, common length-four root, and
  exact three-operation/one-edge fuel ordinals. Missing, reordered, duplicate,
  wrong-root, wrong-length, index, cleanup-order, and artifact mutations reject.
  At that rung, no wider prefix, runtime liveness state, or cleanup loop is
  admitted.

  The following construction-prefix rung is closed as well. The same exact
  carrier admits `[T; 5]` with establishments `[0, 1, 2, 3]` and ordinary
  cleanup `[3, 2, 1, 0]`. Checked production, Terminal verification and
  interpretation, machine emission, object/image validation, and installation
  replay retain the common length-five root and exact four-operation/one-edge
  fuel ordinals. Missing, reordered, redirected-root, wrong-length, index, and
  cleanup-order mutations reject without runtime liveness state or a loop.

  The next bounded construction-prefix successor is now closed. The same exact
  carrier admits `[T; 6]` with establishments `[0, 1, 2, 3, 4]` and ordinary
  cleanup `[4, 3, 2, 1, 0]`. Checked production, Terminal verification, codec
  and interpretation, machine emission, object/image validation, and
  installation replay retain the common length-six root and exact five-
  operation/one-edge fuel ordinals. Missing, reordered, redirected-root,
  wrong-length, index, cleanup-order, and artifact mutations reject; wider
  prefixes at that rung remain fail closed without runtime liveness state or a
  loop.

  The following bounded construction-prefix successor is now closed. The same
  exact carrier admits `[T; 7]` with establishments `[0, 1, 2, 3, 4, 5]` and
  ordinary cleanup `[5, 4, 3, 2, 1, 0]`. Checked production, Terminal
  verification, codec and interpretation, machine emission, object/image
  validation, and installation replay retain the common length-seven root and
  exact six-operation/one-edge fuel ordinals. Missing, reordered, redirected-
  root, wrong-length, index, cleanup-order, and artifact mutations reject
  without runtime liveness state or a loop.

  The next bounded construction-prefix successor is now closed. The same exact
  carrier admits `[T; 8]` with establishments `[0, 1, 2, 3, 4, 5, 6]` and
  ordinary cleanup `[6, 5, 4, 3, 2, 1, 0]`. Checked production, Terminal
  verification, codec and interpretation, machine emission, object/image
  validation, and installation replay retain the common length-eight root and
  exact seven-operation/one-edge fuel ordinals. Missing, reordered, redirected-
  root, wrong-length, index, cleanup-order, and artifact mutations reject
  without runtime liveness state or a loop.

  The following bounded construction-prefix successor is now closed. The same
  exact carrier admits `[T; 9]` with establishments `[0, 1, 2, 3, 4, 5, 6, 7]`
  and ordinary cleanup `[7, 6, 5, 4, 3, 2, 1, 0]`. Checked production, Terminal
  verification, codec and interpretation, machine emission, object/image
  validation, and installation replay retain the common length-nine root and
  exact eight-operation/one-edge fuel ordinals. Missing, reordered, redirected-
  root, wrong-length, index, cleanup-order, and artifact mutations reject
  without runtime liveness state or a loop.

  The next bounded construction-prefix successor is now closed. The same exact
  carrier admits `[T; 10]` with establishments
  `[0, 1, 2, 3, 4, 5, 6, 7, 8]` and ordinary cleanup
  `[8, 7, 6, 5, 4, 3, 2, 1, 0]`. Checked production, Terminal verification,
  codec and interpretation, machine emission, object/image validation, and
  installation replay retain the common length-ten root and exact nine-
  operation/one-edge fuel ordinals. Missing, reordered, redirected-
  root, wrong-length, index, cleanup-order, and artifact mutations reject;
  wider prefixes at that rung remain fail closed without runtime liveness state
  or a loop.

  The following bounded construction-prefix successor is now closed. The same
  exact carrier admits `[T; 11]` with establishments
  `[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]` and ordinary cleanup
  `[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`. Checked production, Terminal verification,
  codec and interpretation, machine emission, object/image validation, and
  installation replay retain the common length-eleven root and exact ten-
  operation/one-edge fuel ordinals. Missing, reordered, redirected-root,
  wrong-length, index, cleanup-order, and artifact mutations reject;
  `[T; 12]` and wider prefixes remain fail closed without runtime liveness state
  or a loop.

  Extend recursive coverage beyond the exact `[[T; 6]; 2]` rung and extend
  construction-prefix cleanup beyond `[T; 11]` to deeper canonical fuel/action
  ordinals.

  Dynamic/mixed projections, scalar/float/byte/linear/nominal/qualified/content
  elements, arrays with claims, sums, joins, and cycles remain separately fenced.
  Admitting nominal element cleanup must preserve the same outer decreasing-index
  order and each element hook's internal source order.
- **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP.** Authored selection of the
  exact owner-attached `T::drop` hook is now closed for every retained source
  selection kind. The package-agnostic selection ledger rejects the exact hook
  machine or one of its states independent of how source reached it; method and
  qualified calls, static-machine arguments, static paths, and forwarded
  selections are covered, while unrelated free `drop` machines and same-owner
  nonreserved names remain ordinary declarations. Receiver-bearing static
  machine arguments currently fail even earlier at refinement, and the exact
  ledger canary keeps the cleanup gate closed when that restriction is later
  relaxed.

  Add the ordinary consuming `omega::core::drop<T>(value)` machine after generic
  cleanup-row lowering is available. Its empty body is checked once against a
  symbolic contextual-cleanup row; concrete death edges substitute exact
  prerequisites, effects, reach, work, guarantees, Type-side eligibility, and
  authored diagnostic origins without inventing caller requirements.

  Extend owned erased-value descriptors with the exact structural cleanup plan
  and transfer it with payload/storage custody. Erasure into an auto-cleaned
  owner must prove the plan eligible under retained invariants or carry the
  stable facts and authority it needs; borrowed erased views acquire no cleanup
  responsibility. Linear values admit automatic death only through an exact
  owner-authorized plan satisfying all cleanup restrictions. Add static,
  generic, dynamic, foreign-owner, moved-from, double-cleanup, and conditional-
  premise canaries. Keep executable hook-body widening under CML4.
- **EXTERNAL-ENTRY-STACK-EPOCHS — finish provenance for the settled root
  realization.** Target-neutral arrival contexts and finite enter/body/exit
  epochs normalize into one validated, fingerprinted realization. The composer
  resolves path-relative `Interrupted`, joins body WCSU only at the body epoch,
  sums aligned concurrent demand, takes maxima across sequential epochs and
  contexts, and closes cyclic nesting through declared finite depth.

  Each realization now retains an exact context-to-body-domain closure. Fixed
  public stack dispositions must agree in every context; `ProviderSelected`
  may close differently per context but may never remain unresolved. Opaque
  adapters now require one admitted complete context-set claim bound to the
  root, provider, target, artifact, entry, boundary contract, and receipt; that
  set must equal the realization's contexts, so a bare receipt cannot license
  an omitted or padded epoch set. They also bind the domain closure and body
  evidence. Direct generated entries bind the same facts to emitted Terminal
  evidence without a provider receipt. The external-root ledger and canonical
  artifact report preserve the complete contexts, epochs, body domains,
  per-domain demand, and evidence origin behind compact fingerprints. The old
  scalar admitted composer is not an admission path.

  A sealed x86-64 target rule now derives 24/32-byte same-privilege and
  40/48-byte privilege/IST arrival frames from exact vector, mechanism,
  privilege, and hardware-switch facts; providers supply neither frame sizes
  nor error-code bits. The direct-entry binder replays the exact target,
  artifact, installed code, entry offset, boundary contract, context set, and
  Terminal body before the result enters composition. Arrival, adapter, and
  body provenance are independent in canonical identity and reports.

  Remaining: produce the x86 fact carrier directly from the installed
  gate/TSS realization rather than a test target-fact fixture; add other target
  arrival rules as their installation facts land. The first nontrivial
  generated-adapter rung now replays the existing receiver-free x86 semantic
  ProgramStorage wrapper's canonical template and resolved private-continuation
  call, binds the exact installed artifact/entry, boundary ABI, Terminal body,
  and context closure, and derives ordered `Enter`/`Body`/`Exit` epochs with
  the live 72-byte outgoing frame retained in each epoch. Exact emitted bytes
  and call coordinates remain behind a generated origin; mutations,
  installed-subject substitution, and ABI substitution reject before
  composition. A sealed installed-entry interval comparison now proves that
  the validated resolved wrapper bytes equal the corresponding bytes in the
  exact frozen installed image; installed-byte mutation rejects even under its
  own otherwise-consistent Terminal body evidence. Extend this derivation only
  when other generated adapters land. It proves neither firmware invocation
  nor an actual stack mutation. Add no
  architecture-specific frame vocabulary to source and do not infer adapter
  transitions by pattern-matching raw bytes.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and expand the suspension-safe-loan subset. Add one
  `TerminalSuspensionCallPlan` per exact possibly suspending call, keyed by its
  `OperationId` and existing `SuspensionCrossingId`. It retains the source-free
  live place/claim frontier and carry demands without adding a local CFG
  terminator or cleanup edge. The separate activation realization inherits
  CPU/thread preservation into `ActivationCarryObligations` and joins the WCSU
  stack, lease, and provider evidence; neither layer rederives liveness.

  Parking leaves the call incomplete and establishes no result. Resumption
  continues that same invocation before its one ordinary successor becomes
  reachable. `request_cancel()` changes an eventual ordinary safe-point result
  and consumes nothing; it cannot add a parked-state disposal edge, and only
  `finish(self)` consumes the external task claim. Retain
  `NoFiniteGuarantee(Edge(edge), UnboundedWait)` at waits lacking accepted
  finite-response evidence.

  The implemented terminal-image replay retains exact Unit, scalar, and
  acyclic-conditional frame/link/temporary, call, crash-terminal, provenance,
  fuel, custody, structural-place, multiplicity, qualification, affine-cleanup,
  relocation, and target-generated division evidence through decoded
  installation and artifact-wide closure composition. It independently
  reconstructs x86-64/AArch64 register mappings, ABI placements, stack and
  return-link mutations, structural field reads and returns, internal-call
  copies, conditional/division regions, executable spans, relocation closure,
  and final image output.

  Current private peak composition handles nested acyclic decisions and
  mutually exclusive source-distributed convergence calls with one
  depth-independent conditional-tree carrier. A bounded Boolean carrier also
  accounts ordered actual unconditional native join branches plus the final
  fallthrough into one affine-cleanup tail. Remaining: extend that accounting
  to general shared native joins and general affine cleanup rather than
  claiming convergence from duplicated leaves, then complete the WCSU,
  `StackPlan`, lease, preservation, cancellation, transfer, park/resume,
  Terminal suspension-call retention, and suspension-safe-loan work above.

  The task-plan foundation now projects a sealed fixed-stack shape only from
  `ComposedTaskStackDemand`, retaining the exact composition/root, byte count,
  alignment, frame validations, admitted same-stack contributions, and selected
  `StackRepresentationId`. WCSU-specific activation validation rejects forged
  projection identities and shape or representation substitution, and folds
  the projection identity into the activation identity. The compiler's legacy
  locally constructed `StackPlan` remains explicitly unbound (`None`) rather
  than being mislabeled as WCSU evidence. Compiler whole-call-graph collection,
  lease provisioning, and the remaining lifecycle joins stay open.

  Complete external-root `StackPlan` production depends on implementing
  `EXTERNAL-ENTRY-STACK-EPOCHS`, not on an owner decision. Zero-byte internal
  closures remain inadmissible until the entry realization is joined.
- **BLOCKEXEC:** implement an ordinary package-level blocking executor with
  bounded queues, moved custody, linear completion claims, suspension, and
  provider selection. A hung in-process worker cannot be killed safely;
  bounded recovery requires process isolation.

Acceptance: linear debt cannot disappear through cleanup or aggregation;
CPU/thread-restricted activations require selected preservation evidence; task
and allocation handles expose no compiler-owned stack/control storage.

### Propositions, quotients, and mathematics

Owner: `wiki/design_briefs/law_bearing_relations_and_quotients.md`.

Remaining N6/N8 work:

- **SELECTED-WITNESS-EVIDENCE — finish executable proof-output calls.** The
  unconditional lane is implemented. `let (value; public_slot: local_term) =
  call()` selectively captures copyable Prop outputs, `let (; ...)` supports an
  empty Type lane, and omitted selectors contribute facts without minting local
  terms. The retired aggregate-package spelling rejects with directed guidance.
  Checked and Terminal Psi retain the exact call, selector,
  proposition/interface, producer provenance, and optional caller-local term
  identity. Pure Unit proof producers erase; runtime Unit and scalar producers
  retain exactly one linked ordinary call. The canonical Terminal
  representation distinguishes all three shapes and rejects a missing or
  mismatched runtime link.

  Ordinary value-argument substitution and explicit erased `requires` inputs
  are implemented through checked and Terminal Psi. Each input retains its
  target lane, source term, and instantiated proposition. Outputs retain their
  formal and instantiated propositions; direct input forwarding aliases the
  exact supplied witness without inventing producer provenance, while a fresh
  produced witness remains distinct. Codec and verifier tamper gates cover the
  argument, source, proposition, and forwarding disposition.

  Closed generic producers are also live. The proof-output target identity
  composes the selected machine's checked specialization fingerprint, including
  the exact closed conformance application; two applications with the same
  callable shape but different conformance selections cannot collide. All
  non-lifetime conformance arguments remain explicit and ordinary lifetime
  elision resolves before this identity is recorded.

  The outcome-specific declaration is settled. Implement it in bounded stages:

  1. parse `ExactResultCase -> { guarantees }` inside `ensures`; resolve the
     path only against the declared result sum, normalize the exact nominal case,
     reject a non-sum result, Boolean guard, duplicate case group, duplicate
     machine-wide public selector, and case-literal-shaped ambiguity, and give
     the braces no value/package/group identity;
  2. admit named and unnamed rows inside the group. On every ordinary exit
     producing that case, require each named row to receive one exact evidence
     assignment and prove each unnamed proposition after substituting the
     concrete result payload. Other cases assign neither, crash exits produce no
     result lane, and a shared proof at a join covers only when every qualifying
     incoming path establishes it;
  3. extend result-arm patterns with the existing `runtime payload; named proof
     selectors` split. All guarded propositions enter the fact catalog only
     under the matching case; selected named terms bind there, while omitted
     selectors and unnamed rows mint no local term and never leak to siblings;
  4. derive each guarded fact/term validity as the intersection of its result
     occurrence, normalized referenced occurrences, and evidence-interface
     scopes. Preserve borrow and revision invalidation under intersecting writes;
  5. retain the exact case on checked and Terminal guarantee rows, public
     signature compatibility, canonical codec identity, derivation provenance,
     and verifier replay without adding runtime operations.

  Stage 1 is live. The parser admits only `Result::Case -> { guarantees }`,
  gives the group no identity, enforces one exact guarantee per row, and rejects
  Boolean/case-literal ambiguity, duplicate case groups, and duplicate machine-
  wide public selectors. Resolution accepts only the declared nominal result
  sum, stamps its exact data/case symbols after declaration assignment, and
  rejects non-sum, foreign, or unknown cases. Typed and checked trees retain the
  guarded rows separately from unconditional facts. Stage 2 is also live on the
  producer side: named guarded rows own checked evidence-term identities without
  entering unconditional or caller-visible lanes, and case-aware path replay
  requires one exact assignment on every matching ordinary exit while rejecting
  assignments on other cases. Forwarded evidence is checked after substituting
  the exact concrete result, including payload; unnamed rows require their
  substituted proposition on every matching path. Crash exits contribute no
  result lane, dynamic results reject when their exact case cannot be classified,
  and distinct join predecessors remain distinct so one-arm evidence cannot cover
  the merge. Caller-arm availability, validity intersections, and exact Terminal
  retention/replay remain fail-closed until stages 3-5 land. Stage 3 is live for
  an exact result-case arm whose subject is a direct call captured once in an
  immutable local, including the compiler-generated capture for direct
  transition subjects. The payload/proof `;` split retains erased named
  selectors through syntax, resolution, and typing. At checking, every guarded
  row enters the fact catalog only at the matching arm coordinate; an explicitly
  selected named row additionally binds one caller-local evidence term there,
  while omitted named and unnamed rows remain fact-only. Sibling arms cannot see
  either facts or terms, and selectors add no runtime statement or second call.
  Computed, mutable, or otherwise untracked result origins reject. Validity
  stage 4 is now live: every guarded row retains a structured descriptor for
  the exact saved-result occurrence, every proposition occurrence, and the
  carrierless evidence-interface scope before normalized labels erase caller
  places. The shared contract-occurrence walker also retains a dynamic indexed
  place's selector occurrence. Semantic publication instantiates those handles
  through the exact producer call and saved-result override, groups each row's
  dependency roots into one validity intersection, and keeps independent rows
  in independent contexts. Intersecting result/reference writes invalidate the
  affected fact and selected term; unrelated writes and sibling arms preserve
  only their own applicable rows. Exact Terminal retention/replay remains
  fail-closed until stage 5 lands. The first bounded stage-5 carrier rung is
  now live: Terminal contracts retain outcome-specific guarantees in a table
  disjoint from unconditional `ensures` and evidence lanes. Every row carries
  its exact structural result type, nominal case, dense per-case position,
  obligation, and proposition; a named row additionally embeds its exact
  evidence term and public selector while an unnamed row has no evidence
  endpoint. Terminal format 28/vocabulary 30 preserve that guard independently
  in canonical identity. Structural validation rejects a foreign/non-sum case,
  noncanonical or non-dense rows, cross-table obligation or selector collisions,
  mismatched named terms/interfaces, and orphan terms. Guarded rows add no
  operation or fuel-bearing site. The next bounded prerequisite is also live:
  a zero-input `[copy]` machine over a closed payloadless sum can now lower one
  exact case construction followed by `ReturnStructural`. Terminal format 29 /
  vocabulary 31 encode the structural operation canonically, validation binds
  it to an exact payloadless member of the result sum, reconstruction emits the
  corresponding case-membership fact, the interpreter returns the exact case,
  and fixed fuel is exactly two units (one construction plus one return).
  Resumable exhaustion at either charge cannot replay the construction, and
  proof metadata still adds no runtime operation or fuel. Payload-bearing cases,
  parameters/locals/internal calls, qualifications, unconditional or non-result-
  case machine contracts, and wider structural-return shapes remain fenced;
  Omega/native lowering rejects
  this target-neutral construction explicitly until tagged sum materialization
  lands. The next bounded stage-5 producer rung is now live on that exact
  carrier. Guarded-only contracts no longer disqualify the checked constructor;
  lowering retains canonical per-case rows while keeping guarded evidence terms
  out of unconditional `ensures` lanes. `ReturnStructural` rebases the exact
  constructor membership fact onto the machine-result place, and verifier replay
  activates only rows for the returned case. Matching named rows require exact
  fresh producer provenance or an exact forwarded required-term identity;
  matching unnamed `true` rows require their own proof route. Nonmatching named
  and unnamed rows are vacuous and reject stray provenance or proof routes.
  Codec round-trip, missing/stale evidence tampering, result-case activation
  swaps, interpreter execution, and fixed fuel pin the proof-bearing producer to
  the same one-construction/one-return runtime shape and two-unit ceiling as its
  proof-free peer. Verifier replay now also admits a Terminal-valid acyclic CFG
  when every ordinary exit returns an exact unrestricted, unqualified,
  claim-free payloadless case construction and the machine contains no calls.
  It intersects facts independently across all exits of each reached case, so
  case-local result membership never leaks into the unconditional all-exit
  intersection; matching unnamed rows receive that per-case proof context and
  matching named rows activate each required producer term once even when the
  case has multiple exits. Cases with no exit remain vacuous and reject stray
  proof or producer evidence. The checked planner/lowerer still authors only the
  single exact constructor carrier, and wider or unclassified structural exits
  retain the replay fence. The first exact caller-import rung is now live in
  Terminal Psi. A zero-input `CallStructural` may invoke one direct
  unrestricted, unqualified, claim-free payloadless producer whose ordinary
  contract lanes, crash surface, custody transfers, and evidence contract lanes
  are empty. Each callee guarded row is imported only as
  `case-membership(call-result, case) -> substituted row`; neither its raw
  conclusion nor a selected case is invented, and `ReturnStructural` rebases
  both sides to the caller result. Semantic codec validation keeps this surface
  disjoint from the legacy linear claim-transfer form. The interpreter
  transports the exact case and fixed fuel is four units (call, construction,
  callee return, caller return), including resumable exhaustion without replay.
  Omega lowering rejects the unrestricted call itself until tagged sum
  materialization exists. The first caller selected-term custody carrier is now
  live on this exact bounded call shape. One optional binding names the
  exact guarded callee case, dense row position, obligation, public selector,
  atomic proposition, callee term/interface, distinct caller-local output term,
  and source-handle-free result-root validity intersection. Codec format 33 /
  vocabulary 35 retain those coordinates canonically. Validation rejects an
  unnamed or nonmatching row, identity/interface/dependency drift, missing
  producer provenance, duplicate output, and any unconditional contract-lane or
  evidence-projection reuse of the guarded output. Omission remains fact-only,
  reconstruction still imports only the guarded implication, and the binding
  adds no runtime operation or fuel. The matching checked/source carrier is now
  live for one attached zero-input direct caller and payloadless producer over
  the same exact attachment. The caller captures the call once in an immutable
  local and every exhaustive case arm returns that saved result unchanged; the
  checked plan replays the exact flow coordinate, target/receiver, symbol-root
  association, case coverage, and result-root-only validity. Any subset of
  named rows may now be selected, while omission remains fact-only. Checked
  planning and Terminal lowering canonicalize that subset by guarded callee-row
  coordinate, reject duplicate or reordered retained rows, emit the exact
  two-machine closure, retain every sibling guarded row and producer provenance
  on the callee, rejoin each selected row to a distinct caller-local term, and
  preserve the four-unit call/construction/two-return runtime with no selector
  charge. Terminal format 36 / vocabulary 39 retain the counted selection
  vector and verifier replay rejects row, output, interface, validity, and
  order tampering independently. The next exact selected-evidence rung is now
  live for a proposition whose sole ordinary argument is the complete callee
  result. Checked planning records whether that one result occurrence is
  substituted; Terminal format 37 / vocabulary 40 retain distinct callee and
  caller proposition applications plus a structured argument-position,
  callee-result-place, and caller-result-place row. Validation independently
  rejoins the declaration, binders, evidence interface, dependency roots, and
  exact call endpoints; reconstruction imports only the guarded implication
  concluding the caller application. Source, codec, verifier, optimizer
  identity, and tamper canaries preserve the same four-unit runtime. The next
  bounded rung permits exactly one later use of that bound whole-result term:
  the matching payloadless arm may pass it as the sole named `requires` input
  to one direct tail state whose sole ordinary argument is the saved result and
  whose body returns that argument unchanged. Terminal retains an independently
  resolvable third machine with the exact requirement lane and identity-return
  shape, plus one selected-use row naming its machine, input position,
  proposition applications, evidence terms, and caller/target result places.
  Codec format 42 / vocabulary 45 and the verifier reject omitted, duplicated,
  redirected, interface-drifted, or non-identity uses while fixed fuel and
  interpretation retain the existing four-unit runtime. Payload projections,
  multiple or partial-result substitutions, multiple evidence arguments or
  uses, later invalidation, erased proof-output linkage, wider structural
  calls, and tagged-sum execution remain fail-closed.

  Requirement guarantees are inherited and satisfiers author additions only;
  omission never weakens the requirement, exact restatement rejects, and direct
  concrete calls may see the stronger merged row set while requirement calls see
  the pinned requirement surface. Outcome-specific implementation must preserve
  the general contract representation needed by `TRAIT-NAMED-WITNESS-CONTRACTS`
  but may continue to reject named requirement rows until that complete trait
  path lands.

  Never infer evidence from visible facts or attached state names. Runtime Type
  results retain their own multiplicity independently of the proof lane, and
  proposition, term, and provenance identities remain distinct. Acceptance:
  outcome-guard tampering rejects independently; a selector is unavailable in
  every nonmatching arm; omitted copyable outputs add no runtime work, cleanup,
  or fuel. The complete contract is in
  [`law_bearing_relations_and_quotients.md`](wiki/design_briefs/law_bearing_relations_and_quotients.md).
- **TRAIT-NAMED-WITNESS-CONTRACTS — preserve complete erased proof-call
  surfaces through traits.** Trait machine requirements admit the same named
  `requires` inputs and named `ensures` selectors as concrete machines. The
  requirement owns ordered proposition applications, evidence interfaces, and
  public output-selector identities. Incoming aliases remain satisfier-local;
  outgoing selector names are pinned public proof API. Named rows introduce no
  hidden value binders: every referenced subject must close over the
  requirement's parameters, result, static telescope, or declared proposition
  parameters, and ordinary borrow/revision validity still applies.

  Land the feature in dependency order. The
  `named_witness_concrete_lane_compile` source canary now proves that the
  existing concrete lane can introduce a witness-bearing proposition, select
  its named output after `;`, pass it through a named input, and eliminate it
  through its declared carrierless evidence interface. The first concrete
  non-generic conformance-inheritance rung is also live. A satisfier's named
  inputs may use local aliases, while named outputs preserve the requirement's
  exact public selectors; both lanes preserve cardinality/order, proposition,
  and evidence-interface identity. Concrete output strengthening may append
  rows only after that pinned prefix. Inherited checked facts now reuse the
  exact satisfier evidence terms, including their lane positions, rather than
  dropping term custody. The first static-call rung is also live. An attached
  or free generic caller may select one exact concrete named conformance through
  an explicit proof-static binder and call its direct concrete, non-generic,
  one-state Unit requirement. The public requirement may own any finite ordered
  set of subjectless named `requires` lanes, including none, and must own at
  least one subjectless unconditional named `ensures` lane; every public row in
  this rung is named. The plural source canary retains three inputs and three
  outputs while omitting one selector at the call site.
  Monomorphization retains the call-local closed application and exact
  requirement-to-realization row instead of replacing public proof identity
  with the executable satisfier. The same bounded lane now accepts the
  selected conformance's exact trait-default realization as well as an inline
  realization. Default rows remain owner-scoped: two conformances reuse the
  authored default through distinct closed-application commitments and
  generated realization identities, while an exact inline override wins.
  Checked call composition imports only the
  requirement contract, while the runtime call still targets the private
  realization. The captured output is a fresh opaque requirement-level term:
  satisfier-local input aliases, concrete strengthening selectors, forwarding
  identity, and producer provenance do not escape. Terminal codec format 33 /
  vocabulary 35 retain the normalized public requirement plus an independently
  replayed owner-scoped closed-conformance/runtime realization link. The
  verifier rejects row, application, identity, runtime-callee, or freshness
  drift, and differential coverage keeps ABI, storage, operation shape, and
  fixed fuel unchanged. Existing package review already publishes the exact
  requirement lanes: incoming alias renames are compatible and outgoing
  selector renames are breaking, while private call-site dispatch remains
  implementation content. Generic trait/requirement/satisfier or proposition
  telescopes, inherited requirement rows, direct named-conformance calls,
  scalar results, subject-bearing lanes, unnamed public contract rows, and
  dynamic dispatch remain fail closed.
  Next extend broader static calls and runtime trait dispatch. A satisfying
  machine must assign every inherited output on each applicable ordinary exit
  and may not omit, rename, weaken, or replace it. Direct concrete calls may
  retain authored strengthening; calls through the requirement expose only the
  pinned requirement surface.

  Dynamic dispatch produces an opaque requirement-level witness. Its declared
  proposition and evidence interface are available, while the selected
  satisfier's private producer conformance, term identity, and varying
  projections never escape. If the current erased evidence representation
  cannot express that abstraction, dynamic named-witness calls reject until it
  can; do not synthesize implementation evidence after checking or add runtime
  dictionary fields.

  Extend checked state signatures, Terminal proof lanes, canonical codecs,
  package review, compatibility comparison, and diagnostics together. Renaming
  an incoming alias is stable; changing lane order, proposition/interface, or an
  outgoing selector is breaking. Add pass/fail canaries for static and dynamic
  calls, missing/duplicate producer assignment, fact-only named rows, unbound
  subjects, selector rename, satisfier-private leakage, tampered lane identity,
  and zero runtime ABI/fuel/storage impact. Until every layer lands, retain the
  current fail-closed package-review rejection rather than publishing a partial
  trait contract.
- **QUOTIENT-THEOREM-LIFT — admit explicit lifted operations.** The settled source
  forms are ordinary quotient-owner bodies containing
  `Quotient::lift<F, Congruence>(...)`,
  `Quotient::lift<F, Congruence, Transport>(...)`, or
  `Quotient::define<F, Congruence>(...)`. Every form selects one exact
  representative machine application and one exact named, resultless checked
  congruence theorem. The three-argument `lift` additionally selects one exact
  forward-precondition transport theorem. `lift` proves authored
  public-precondition implication for both representative applications;
  `define` proves
  equivalence, position-preserving runtime argument correspondence, and
  unchanged result flow. There is no `lifts` clause, operation map, visibility
  discovery, structural witness search, variadic `Respects` interface,
  `ArgumentMode`/`Fixed<T>` surface, or call-site selection.

  The fail-closed planning spine is implemented. Quotient formation requires
  one explicit closed `Equivalence<C, R>` conformance and checks its sealed
  Reflexive/Symmetric/Transitive rows plus transitive anti-axiom provenance.
  Typed lift/define requests retain their exact operation kind, representative
  entry, and theorem application. The typed boundary now rejects a conformance
  or structurally discovered proof in the theorem slot; relation planning
  resolves one exact bodyful checked resultless theorem entry, closes and
  retains its complete static application, ordinary telescope, and contract
  rows, and records its termination, purity, and crash-route eligibility. This
  is selection and eligibility only: it proves no congruence schema and grants
  no executable authority. Checked planning derives the exact relation
  at every quotient-bearing runtime position, one shared occurrence at every
  ordinary pass-through position, and the exact result relation; attached
  `self` is position zero and proof-static binders are excluded. It closes
  type/`const`/static-machine substitution and retains the representative
  telescope.
  Direct `define` additionally checks one-to-one parameter correspondence,
  mode/multiplicity and carrier agreement, exact alpha-renamed `Q`/`P`
  precondition equivalence, and unchanged result flow through a straight-line
  immutable alias chain or a finite unconditional sibling-state graph. The
  representative's whole checked call closure must be pure and unconditionally
  terminating. Unsupported, adapted, open, ambiguous, cyclic, conditional, or
  representation-observing shapes reject.

  The expected-theorem-schema rung is also live. The relation plan now derives
  exact ordinary theorem parameters from every representative position:
  quotient-bearing inputs produce left/right binders plus their exact nominal
  relation, while ordinary inputs produce one shared binder. Structural
  application vectors retain both exact representative calls. Every machine-
  and state-level representative `requires` fact is retained by owner/contract/
  fact coordinate for substitution into both applications, and the structural
  conclusion joins those applications through the exact result relation.
  Parameter access, relation, requires coordinate, application mapping, and
  conclusion mutations remain distinct even when their row counts agree.
  The selected-theorem-schema verification rung is now live too. Checked
  planning compares the exact explicitly selected theorem against that derived
  schema after substituting both closed static applications and retains a
  certificate pairing every derived parameter, relation premise, legality
  premise, and conclusion with the selected theorem's exact symbols and
  machine/state contract-fact coordinates. Extra or missing premises,
  finer/wrong relations, redirected/duplicated/omitted representative calls,
  rebound shared arguments, const/attached/mode/type parameter drift, named
  evidence lanes, result-case/crash lanes, and conclusion drift reject
  independently. Selection, eligibility, and exact schema verification prove
  no congruence implication and grant no executable authority.

  The bounded stage-3 correspondence rung is now live. For `lift`
  requests whose authored runtime arguments directly name public parameters
  or are contract-separable closed literals,
  checked planning proves structural `Q => P` inclusion independently for the
  selected theorem's left and right representative applications. `lift` may
  explicitly omit, permute, and repeat those parameters: every occurrence row
  maps the actual public symbol to its distinct representative position. Public
  `Q` dependency partitioning follows that map rather than declaration order,
  and repeated occurrences share one exact instantiated value per theorem side
  without collapsing positional theorem parameters, relation premises,
  representative calls, or legality coordinates. Facts over an omitted
  formed-quotient parameter remain additional dependent `Q`, while facts
  depending only on omitted ordinary parameters remain fixed. Heterogeneous
  relation/type drift still rejects. Every dependent representative `P` fact
  must match one public `Q` fact after exact value/static substitution; one `Q`
  coordinate may discharge multiple distinct `P` rows and extra `Q` facts are
  permitted. Each row retains exact public, representative, and
  verified-theorem contract-fact coordinates, and the plan composes them with
  the verified theorem and exact runtime correspondence into a non-executable
  certificate. Closed booleans, in-range integers whose explicit suffix or
  exact concrete representative target supplies the landing, and floats whose
  explicit suffix or exact `f32`/`f64` target supplies the format may feed an
  immutable non-receiver representative parameter only when its primitive type
  and arithmetic domain/format agree. An anonymous numeric scalar lands once
  at that exact target; derived integer width/signedness/domain or float format
  is retained rather than inferred again. Literal value, canonical spelling,
  and landing ride the occurrence identity; the input relation is exact
  equality. A quoted raw-byte literal may additionally feed an exact shared
  `&[u8]` representative position or an exact constrained `[u8; N]` named
  value-domain buffer when its payload fits. Its immutable-image bytes and
  exact target identity are retained; planning neither selects an encoding
  domain nor adapts to a different buffer shape. An exact-width quoted byte
  literal that ordinary contextual typing has already landed as a canonical
  closed `[u8; N]` array may likewise feed that exact representative target.
  The canonical retained shape is one unsuffixed decimal `u8` value per array
  element. Its ordered bytes and normalized array identity remain evidence;
  the quotient planner performs no padding, truncation, element coercion, or
  contextual landing of its own. A direct closed Boolean array may likewise
  feed only its exact literal-width `[bool; N]` target. Every element must be a
  Boolean literal; values, order, and normalized array identity remain
  occurrence and proof-substitution evidence. A direct fixed integer array may
  likewise contain only integer literals at an exact literal-width primitive
  `[I; N]` target. Each element independently follows the scalar landing rule:
  its explicit landing must agree, or the exact element target supplies one,
  and the value must fit. Ordered spelling/landing evidence and normalized
  array identity are retained without element coercion. A direct fixed float
  array may likewise contain only float literals at an exact literal-width
  `[f32; N]` or `[f64; N]` target. Each element independently follows the
  scalar format rule; ordered spelling/format evidence and normalized array
  identity are retained without evaluating computed elements. A direct exact
  depth-two Boolean array may likewise feed only its exact literal-width
  `[[bool; M]; N]` target. Every row is a direct exact-width array literal and
  every leaf is a Boolean literal. Row boundaries, ordered values, and the
  normalized outer array identity remain evidence. Proof-value array traces
  delimit every container, so a nested array cannot collide with a flat array
  carrying the same ordered leaves. An exact depth-two fixed-byte array may
  likewise feed only its exact literal-width `[[u8; M]; N]` target. Every row
  independently uses the canonical fixed-byte rule: exactly `M` unsuffixed
  decimal `u8` leaves with no coercion or computation. Ordered bytes, row
  boundaries, and normalized outer array identity remain evidence. A direct
  depth-two fixed integer array may likewise feed only its exact literal-width
  `[[I; M]; N]` target for a direct primitive integer `I` other than `u8`.
  Every leaf independently follows the scalar landing and range rule. Ordered
  spelling/landing/domain evidence, row boundaries, and normalized outer array
  identity remain evidence; all `u8` matrices stay exclusively in the
  canonical fixed-byte lane. A direct depth-two fixed float array may likewise
  feed only its exact literal-width `[[f32; M]; N]` or `[[f64; M]; N]` target.
  Every leaf independently follows the scalar format rule. Ordered spelling/
  format evidence, row boundaries, and normalized outer array identity remain
  evidence without evaluating computed leaves. A direct depth-three Boolean
  tensor may likewise feed only its exact literal-width
  `[[[bool; K]; M]; N]` target. Every plane and row is a direct exact-width
  array literal and every leaf is a Boolean literal. Plane/row boundaries,
  ordered values, and normalized outer identity remain evidence. Remaining
  exact primitive fixed-array literals now close recursively: depth-three
  canonical-byte, non-`u8` integer, and float arrays plus every exact primitive
  array at depth four or greater retain every container boundary and the
  already-settled leaf evidence in one recursive tree with normalized outer
  identity. Existing flat, matrix, and depth-three Boolean owners retain
  priority; this fallback neither reclassifies their evidence nor admits data
  aggregates.
  Exact structural `Q => P` substitution now permits a
  dependent representative `P` fact to mention a literal-fed position only
  when public `Q` contains the identical post-substitution fact. Boolean value,
  integer spelling, landed type and arithmetic domain, and float spelling and
  format are all proof-value identity even when rendering would erase a
  difference. Literal-only facts remain fixed ordinary call obligations.
  When no exact fact match exists, one strict authored-implication rung now
  admits integer `ProofFact::Expression` goals that the existing arithmetic
  contract engine proves from the complete ordered dependent public-`Q`
  expression roster after exact left/right symbol, representative-static
  `const`, and integer-literal substitution. Resolved symbols, not display
  names, select atoms; only exact integer carriers/domains participate, every
  hypothesis and goal must be inside the engine language, and only `Proven`
  succeeds. Exact matches retain priority. Each arithmetic row retains the
  full ordered public premise coordinates plus its representative and distinct
  theorem-side legality coordinates for later replay. Unknown, refuted, mixed
  membership/proposition, float, member-path, proof-view, operator/domain, or
  identity-drifted judgments remain fail-closed.
  Fixed representative call preconditions now use a separate bounded
  certificate. One exact substituted fixed-`Q` match, or one strict integer
  `Expression` proof from the complete ordered fixed-`Q` roster, discharges the
  one representative call performed at runtime; this proof is never
  duplicated into two calls. Each row nevertheless retains both distinct
  verified theorem-legality coordinates, so later replay cannot collapse the
  theorem's left/right hypothetical applications. Direct resolved symbols,
  exact representative-static `const` values, and exact integer literals are
  the only arithmetic bindings, and only `Proven` succeeds. `define` permits
  no such weakening: its fixed facts now join the same exact one-to-one
  position/static-substituted `Q <=> P` bijection as its dependent facts.
  Mismatched or out-of-range integers, mismatched floats, mutable/non-byte,
  undersized or otherwise constrained byte-string targets, byte-string values
  not already context-landed for a bare fixed array, noncanonical or
  heterogeneous byte/Boolean arrays, mismatched or out-of-range integer arrays,
  mismatched or computed float arrays, noncanonical byte matrices, mismatched,
  out-of-range, or computed integer matrices, mismatched or computed float
  matrices,
  noncanonical or mismatched recursive primitive arrays, ragged arrays, other
  data nested arrays, other aggregates, zero-value,
  casts, calls, computed expressions, constrained/generic targets, mutable or
  attached targets, and literal arguments to `define` remain fail-closed.
  `define` remains strictly position-preserving at exact public
  arity and retains its exact `Q <=> P` bijection. Fixed facts without an exact
  match that require membership/proposition transport, float or computed
  implication, a mixed premise roster, unresolved identity, or argument
  adaptation remain fail-closed; generic owner substitution, general adapted
  lift arguments, non-arithmetic logical implication, and executable canonical
  Terminal replay remain fail-closed, so stage 3 is not complete and stage 4 remains
  open. Arithmetic `Expression` entailment and the checked-only explicit
  transport-schema lane are implemented; executable transport of quotient-
  domain membership and opaque proposition families remains blocked on later
  source-erasure and operation admission. The
  two-argument `lift` keeps the complete built-in exact/arithmetic implication
  route. The three-argument form selects one resultless checked theorem at the
  operation request and verifies it against the compiler-derived complete
  ordered `Q => P` schema for both representative sides. A selected transport
  is authoritative even when the built-in engine could prove the same
  implication; there is no mixed per-row automatic/theorem route. Ambient
  domain linking, visibility search, or an opaque solver verdict cannot supply
  transport authority.

  Replace the three singular congruence retention fields with one canonically
  role-ordered theorem-evidence collection. Each entry carries an explicit
  `QuotientTheoremRole` identity input, its exact selected application, a
  role-specific correspondence payload, and the shared checked-body, pure-
  closure, unconditional-termination, and crash-free eligibility. Require
  exactly one `Congruence` entry, and zero or one
  `ForwardPreconditionTransport` entry according to the authored `lift` arity;
  current structural `define` admits no transport entry. Reject duplicates,
  noncanonical order, missing or surplus required roles, and every unknown
  role tag. Unknown tags are artifact-version incompatibilities and must never
  be skipped by an older decoder. Do not reserve a reverse-transport role until
  theorem-mediated `define` is separately designed.

  The collection migration is implemented through sealed typed requests,
  checked relation planning, the proof-only total-direct `define`
  correspondence, canonical Terminal identity/codec/replay, and mutation
  canaries. Terminal format 42/vocabulary 45 bind the role discriminant before
  its role-specific payload and reject missing, duplicate, reversed, surplus,
  role/payload-mismatched, and unknown-tag evidence. Three-argument `lift`
  retains `Congruence, ForwardPreconditionTransport` in canonical order.
  Checked planning now verifies the transport theorem's exact left/right/shared
  parameter roster, complete fact-major public-`Q` `requires` and
  representative-`P` `ensures` rosters with adjacent Left/Right substitutions,
  exact role and selected closed static application, and both theorem entries'
  checked-body, pure-closure, unconditional-termination, and crash-free
  eligibility. It produces a distinct non-executable transport-backed lift
  certificate with no automatic implication or fixed-call rows; dependent and
  fixed `P` are both covered by the complete theorem roster. Terminal still
  does not replay that checked transport certificate, and package review still
  has no quotient operation record because quotient contract calls remain
  blanket-rejected, so those representation and admission migrations remain
  open.

  A failed built-in implication diagnostic must print the expected public and
  representative fact coordinates and point directly to
  `Quotient::lift<F, Congruence, Transport>(...)`. Extend typed requests,
  canonical correspondence identity, Terminal codec, package review, verifier
  replay, and mutation canaries together; the role discriminant itself feeds
  identity before the role-specific payload.

  The deliberately non-executable stage-4 preparation seam is now module
  canonical for the narrow total direct `define` case. A separate
  all-or-nothing validation API erases source handles into package-qualified
  callable/type identities, parameter ordinals, contract-fact coordinates,
  the exact positional relations and theorem expansion, eligibility
  certificates, and the direct result edge. It admits only monomorphic
  one-state operations with empty static telescopes, empty public/
  representative preconditions, no theorem legality premises, immutable
  non-attached parameters, complete checked purity/termination/crash evidence,
  and an exact direct result. `TerminalModule` owns the strictly ordered
  retained rows; the canonical codec binds every certificate and rederives its
  canonical identity on decode, and normal representation validation
  independently reconstructs the theorem parameters, relation premises,
  representative applications, conclusion, runtime correspondence, result
  flow, row identity, uniqueness, and order. The explicit proof-only Terminal
  producer attachment consumes only the complete extractor batch and is not an
  ordinary `lower_machine` path. Ordinary validation still rejects every
  quotient request, one unsupported request prevents returning a partial
  batch, and no row owns a machine or executable operation. This
  closes the module-retention prerequisite only: stage 3 is still incomplete,
  while checked executable authorization and stage-4 operation/result
  lowering remain open.

  Every request intentionally remains non-executable. Complete admission in
  bounded stages:

  1. derive the selected resultless theorem's exact ordinary parameter,
     `requires`, and `ensures` schema. Quotient-bearing positions become
     left/right representative binders, ordinary positions become one shared
     binder, partial calls require legality on both sides, and the conclusion
     applies the exact result relation to the two exact representative calls;
  2. verify that exact explicitly selected theorem after substitution. Reject
     extra premises, finer relations, operation redirection, duplicated or
     omitted representatives, rebound pass-through arguments, admitted proof
     dependencies, or any runtime-result/dictionary interpretation;
  3. finish general authored `Q => P` checking for `lift` on both representative
     applications, including the role-tagged explicit transport theorem form,
     while keeping `define` at structural `Q <=> P`, and retain the exact
     theorem/correspondence certificate; and
  4. lower the admitted operation, representative application, theorem
     selection, positional relations, operation kind, contract correspondence,
     and result flow into canonical Terminal identity with independent verifier
     replay.

  Acceptance: changing any representative argument, positional relation,
  selected theorem, precondition correspondence, or result-flow edge
  rejects independently; no quotient operation observes representative
  structure or acquires effects/custody beyond the initial integration fence.
- Suppress every synthesized representation observer on quotient formation.
  Resolved-to-typed lowering now rejects runtime `==`/`!=`, a direct
  `Equatable` conformance, and synthesized container equality through a quotient
  field. It also rejects proof-contract `zero_value<Quotient>()`; a retained
  representative is not a compiler-verified canonical default. Build-time
  layout/access schema reflection rejects a quotient directly and refuses to
  derive a zero-byte nested record layout for one. Record and arm destructuring
  reject quotient subjects before field/case analysis, so an empty or rest
  pattern cannot become a representation observer. Struct and case literals
  likewise cannot forge a quotient value; casting an exact carrier instance
  with `as Quotient` is the sole construction path. That nominal fence runs
  before generic field-shape deferral, so a parameterized quotient head cannot
  bypass it. Logical proof-position equality remains raw for the exact
  quotient-congruence judge; it never lowers to representative bytes. Add
  quotient-owned executable equality through an ordinary lifted operation with
  `DecidesEquivalence`; derive its ordinary result-congruence theorem, and bind
  its optional `==` token only through the settled fixed-operator declaration
  head.
  Keep ordering, canonicalization, hashing, and later observer roles on
  explicit role-correctness contracts until each earns a named interface.
- Enforce the initial quotient integration fences: lifted representative
  machines are pure and terminating, and quotient carriers contain no
  affine/linear `Type` content or owned/routed custody. Effectful lifting waits
  for a complete observable-behavior relation; custody-bearing quotients wait
  for exact occurrence-preservation machinery. Formation now walks the exact
  recursive proof-carrier graph once and rejects any contained ordinary Type
  whose multiplicity is not unrestricted, as well as references, slices,
  dynamic traits, const-expression type shells, and explicitly carried data.
  Recursive proof-only nodes and contained structurally copy data remain
  admissible. The direct operation plan now also retains the exact
  representative machine/state when its locally checked termination summary is
  unconditional. A missing guarantee or one requiring progress-profile
  premises retains the termination fence rather than treating those premises
  as discharged. The same validation now consumes the shared whole-call-graph
  operational and service-reach fixed points once on the rejecting quotient
  path. It retains the exact representative machine/state as pure only when
  recursive service reach, suspension, blocking, mutable/out parameters, and
  unresolved concrete call targets are all absent. It does not run a second
  expression-local effect inference. Formation expands transparent domain
  aliases and rejects every establishment-routed qualification in the carrier
  graph with a custody-specific diagnostic. Content-bearing qualifications
  remain excluded by the required linear-carrier fence; custody-bearing
  quotients still wait for exact occurrence-preservation machinery.
- Add exact-pair-selected heterogeneous constructor lifts. Dependent records
  lift in order and generate checked transport obligations for coarser earlier
  fields. Extend R6 carrier-family binders for reusable proposition-valued
  relators; add no global carrier role or default relator.
- Gate runtime deciders whose lifted relation depends on erased `Type` content;
  require determination by the runtime projection or report the component.
- Continue total specification arithmetic. Prop rejects direct Trapping
  arithmetic/conversion while preserving total comparison, bitwise,
  classification, Wrapping, and Saturating terms. Exact formation uses only
  prior facts; flow-sensitive entry, branch, fallthrough, out-parameter,
  incoming-edge, merge, and invalidation analysis preserves intervals,
  dependent subtraction, bounded products, and call-write fences.

  Fixed-width integer/address `embed` returns proof `Int` with exact carrier
  range facts; `Int as Nat` requires nonnegativity. `Nat - Nat` is Exact and
  discharges `right <= left` through independently selected
  `Nat::less_or_equal`; missing evidence rejects. Clamping is the separately
  named `Nat::saturating_sub`, used consistently by dependent mathematics and
  measured recursion. `Granted::content` and normalized content projections
  retain explicit proven `Int as Nat` conversions and `IntervalSet<Nat>`;
  signed runtime embeddings reject at the closed projection boundary.
  The shared integer-policy catalog covers add, subtract, multiply, divide,
  remainder, and shifts across Exact, Wrapping, Saturating, and Trapping, with separate
  result laws, formation conditions, primitive trap predicates, and shift-count
  laws. Division keeps zero and signed-minimum/-1 distinct; shift count failure
  stays distinct from overflow. Specification/expression analysis, bounded
  checked operations, proof-bearing Terminal rows, structural-crash replay,
  and the dedicated Exact-division/remainder lane consume the catalog. Add
  remaining bridge integrations only as
  their owning operation surfaces land.
  The float catalog fixes exact `meaning32`/`meaning64` projection: finite
  values map to exact nonzero rationals, signed zero/infinity survive, NaN
  payloads erase, and cross-format projection rejects. Checked interpretation
  consumes the catalog. Current source binding validates an ordinary tokenless
  `Float::meaning32`/`meaning64` path and signature but does not yet retain the
  recognized core declaration as contract owner. Checked and Terminal rows
  retain proof-position invocation, format, equality, and projection-table
  coordinates, and
  replay rejects missing, reordered, substituted, noncanonical, or cross-format
  evidence. Runtime Booleans, machine operations/contracts, native lowering,
  and complete proof-kernel discharge remain open under
  [`total_specification_arithmetic.md`](wiki/design_briefs/total_specification_arithmetic.md).
  **IMPLEMENTATION — D40 FLOATMEANING PCC CORRESPONDENCE.** Add the closed
  FloatMeaning proof term and carrier-specific `FloatMeaningEqual` proposition
  to canonical proposition validation, encoding, substitution, the proof
  kernel, the authoritative low-rung checker, and the current independent
  diagnostic comparator. Retain a verifier-reconstructible
  source term for contract parameters/results, Terminal values, structural
  float leaves, and exact-bit literals; bind its format, projection operation,
  exact recognized core declaration, and numeric-catalog version. Canonically
  deduplicate equal tuples to one `ProofValueId` while retaining occurrence/span
  provenance separately. Require an explicit theorem for distinct terms. Add
  controls for NaN reflexivity, signed-zero distinction, IEEE-relation
  separation, lookalike declarations, cross-format/catalog substitution,
  arbitrary producer IDs, false coalescing, and proof erasure from runtime.
- Then migrate suffix law discovery to propositions plus explicit conformances,
  and expand the checked `Nat`/`Int`/`Rat`/Cauchy/approximation corpus. `Real`
  remains proof-only and core-level.

Acceptance: an admitted axiom cannot license quotient formation; selected
Reflexive/Symmetric/Transitive evidence and every operation-congruence theorem
are explicit;
different witnesses establish one stable proposition identity and eliminate
through its declared interface; quotient operations select their exact proof
in the quotient owner's body; canonical definitions cannot hide wrappers; and
no structural observer, effect, or custody occurrence crosses the quotient
boundary without its corresponding checked law.

### Boundary realization and evaluated binding data

- **EVALUATED-FOREIGN-BINDINGS — replace the remaining string-backed import
  bootstrap with ordinary typed compile-time values.** The intrinsic binding
  value shape is complete: `CompilerIntrinsic` is payloadless, while exact
  realization symbol, signature, and target select a sealed catalog entry.
  The first production entry and native handoff now physically cover Linux
  `Console::exit_process(i32) -> Unit`; under D39 this is not yet semantic
  external-termination authority. The remaining intrinsic catalog, the
  explicit checked completion-kind join, and source-form migration stay open
  under the provider and observation-profile work above. For imported leaves,
  add const-generic typed object-format locator cases over ordinary fixed byte
  arrays (`PeByName`, `PeByOrdinal`, versioned ELF symbols, and later peers).
  Target-scoped ordinary machines construct complete `Binding` values; `via`
  evaluates them under the existing hermetic evaluator. The satisfied
  requirement's `Calling<C, Policy>` relationship already produces ordinary
  evaluated `CallPlan` data; remove the duplicate plan operand and do not add a
  parallel `CallingPlanId` registry.

  Carry the normalized locator, evaluated plan, producer closure, target
  applicability, and evaluation/materialization fingerprints through checked
  provider identity, Terminal/artifact identity, admission, object planning,
  and backend emission. Keep each physical locator atomic so independently
  named library/export values cannot be accidentally paired. Raw foreign bytes
  are typed target-package data, never Omega symbol names, requirement keys,
  ambient lookup strings, or `build.omg` redirection inputs. A changed locator
  changes every dependent final artifact and requires relink plus fresh
  admission.

  The dependency-light locator foundation is implemented in `omega-target`
  with compatibility reexports from `omega-effects`.
  One sealed target-bound carrier validates and fingerprints atomic `PeByName`,
  `PeByOrdinal`, and `ElfVersioned` candidates, rejecting empty/NUL coordinates,
  zero PE ordinals, UEFI/PE conflation, and non-Linux versioned ELF use. Every
  target, case, byte, length boundary, and ordinal participates in its normalized
  identity. The first coordinated representation join is now live:
  `ProviderBinding::Import` owns the whole normalized locator, validates its
  target against the provider plan, and carries its atomic identity through
  selected-plan/opaque-TCB facts, package-review format 57, and manifest JSON
  with exact raw coordinates. Trust artifacts now retain the same sealed locator
  and render target, case, normalized identity, and raw hex coordinates/ordinal;
  target drift rejects before report installation. The atomic calling-
  convention bridge is now live: the sealed locator survives compiler provider
  extraction through `ExternalBindingKind`, `HostImportLocator`, and
  `HostBindingMechanism`, with target drift rejected before host-ABI insertion.
  Object planning now retains an atomic locator side table keyed by the exact
  object symbol, deduplicates exact rows, rejects target drift, and carries the
  normalized case into final-image import identity. Relocation planning joins
  normalized calls by exact locator equality and rejects missing or ambiguous
  rows. PE final emission consumes raw `PeByName` library/export bytes without
  UTF-8 reconstruction and emits `PeByOrdinal` through the ordinal flag in both
  lookup tables; malformed coordinates, duplicate rows, and target drift reject
  before mutation. Versioned ELF and Mach-O normalized rows remain explicit
  fail-closed boundaries rather than being reconstructed as text. Ordinary
  authored outbound machine emission now retains the complete `HostImportLocator`
  in all seven scalar/float/aggregate validation variants, and final instruction
  replay joins normalized calls to the exact object-side locator handle. Raw
  non-UTF-8 coordinates are never reconstructed as text; locator mutation or an
  ambiguous side-table row rejects. Specialized open/create and runtime-I/O
  adapters remain explicitly string-backed and fail closed on normalized rows.
  Versioned ELF locators now pass object planning with exact deduplication and
  reach a canonical final-image request retaining their Linux profile,
  normalized identity, raw object/symbol/version bytes, symbol handle, and every
  relocation site. Wrong cases, target drift, duplicate handles, reused or
  identity-colliding locators, and missing physical plans reject. The first
  dependency-light loader-plan input rung now seals one target/deployment-owned
  exact `PT_INTERP` pathname for Linux x86-64 or AArch64. It preserves raw
  non-UTF-8 bytes, requires a nonempty absolute NUL-free path, and binds the
  exact Linux profile plus length-framed bytes into deterministic identity.
  The first ELF-owner join now consumes one exact final image beside that input,
  replays the canonical referenced-import request, and accepts only a nonempty
  set of normalized `ElfVersioned` rows for the identical Linux profile. The
  non-clone carrier privately retains every symbol handle, raw locator,
  normalized identity, and relocation site; any target drift, string-backed or
  unused input, or canonical-request failure returns the original image and
  interpreter unchanged. These carriers grant no loader, section, publication,
  or admission authority. The first complete address-free table plan now
  consumes that preflight and independently validates an exact NUL-terminated
  `PT_INTERP` payload, canonical raw-byte `.dynstr`, the reserved undefined
  `.dynsym` row plus one sorted undefined global function row per import, one
  concrete System V `.hash`, parallel `.gnu.version`, grouped
  `.gnu.version_r`, private import-to-symbol/version indexes, and the exact
  `DT_NEEDED` string-index roster. Exact byte deduplication and stable sorting
  make table contents and their fingerprint independent of import insertion
  order. Its invariants come from the primary System V ABI [program-header
  rules](https://gabi.xinuos.com/elf/07-pheader.html), [string-table
  rules](https://gabi.xinuos.com/elf/04-strtab.html), [symbol-table
  rules](https://gabi.xinuos.com/elf/05-symtab.html), and [dynamic-hash
  rules](https://gabi.xinuos.com/elf/08-dynamic.html#hash-table), plus the [LSB
  symbol-version requirement
  format](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html).
  The next sealed rung now serializes those structures into six exact ELF64
  `ELFDATA2LSB` payloads: `.interp`, `.dynstr`, 24-byte `Elf64_Sym` rows in
  `.dynsym`, the word-oriented `.hash`, half-word `.gnu.version` rows, and
  16-byte `Elf64_Verneed`/`Elf64_Vernaux` chains in `.gnu.version_r`. A
  separate bounds-checked decoder replays every row, exact length, hash index,
  linked version offset, dynamic-string reference, and section-kind boundary
  before sealing deterministic payload identity. Truncation, trailing bytes,
  endian drift, invalid counts/indexes, offset cycles, or payload mutation
  reject without losing the validated structural plan. This follows the
  primary System V ABI [ELF64 data sizes and
  alignment](https://gabi.xinuos.com/elf/01-intro.html#sixty-four-bit-data-types)
  and [least-significant-byte-first
  encoding](https://gabi.xinuos.com/elf/02-eheader.html#data-encoding).
  A following sealed descriptor rung now binds those six payloads to six
  address-free semantic section-kind rows with their exact ABI type, flags,
  payload size, alignment, entry size, semantic link, and `sh_info` meaning.
  It independently replays every name, payload relationship, symbol/version
  count, System V hash chain count, and version-need object count before
  sealing deterministic descriptor identity while retaining payload custody
  on failure. An append-only section-name seed fixes the current `.interp`,
  `.dynstr`, `.dynsym`, `.hash`, `.gnu.version`, and `.gnu.version_r` name
  offsets and reserves `.shstrtab`, but deliberately supplies neither a
  `.shstrtab` descriptor nor final numeric section indexes. These rules follow
  the primary System V ABI [section-header, type, flag, link, and info
  rules](https://gabi.xinuos.com/elf/03-sheader.html) and the LSB [GNU section
  type assignments](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/sections.html).
  The first address-free procedure-linkage relocation rung now consumes those
  descriptors and joins every private import binding back to its exact
  canonical request and source call sites. It admits only intact unresolved
  x86-64 `CALL rel32` or AArch64 `BL` placeholders with the matching four-byte,
  zero-addend text relocation, assigns one canonical logical PLT/GOT slot per
  imported dynamic symbol, and seals one semantic RELA `JUMP_SLOT` requirement
  per slot (`R_X86_64_JUMP_SLOT` or `R_AARCH64_JUMP_SLOT`). Multiple calls to
  one import share that slot; target drift, non-procedure uses, malformed or
  overlapping sites, and binding/slot/relocation drift reject while returning
  the exact descriptor carrier. Because every unresolved use is accounted for
  as a procedure call, this plan proves that the current admitted image needs
  no general `.rela.dyn` row. It follows the primary [x86-64
  psABI](https://gitlab.com/x86-psABIs/x86-64-ABI) and [AArch64 ELF
  ABI](https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst),
  but assigns no address or physical GOT/PLT/section index and emits no bytes.
  The next sealed target-template rung consumes that semantic linkage and emits
  only the fixed ELF64-LSB bytes for `.plt`, `.got.plt`, and `.rela.plt` plus
  exact zero placeholders. The x86-64 small/medium lazy-binding policy records
  the sole `_DYNAMIC` semantic fixup in GOT[0], leaves GOT[1]/GOT[2] reserved,
  and binds each import slot to its PLT lazy tail. The AArch64 standard lazy
  policy instead leaves all three GOT header words zero/reserved and binds each
  import slot to PLT0; it never fabricates an AArch64 `_DYNAMIC` GOT[0]. Typed
  fixups name every PLT/GOT/RELA and source-call target, while explicit signed-
  displacement, page-delta, low-12 alignment, and branch-range constraints
  retain the placement obligations. An independent replay checks the exact
  opcodes, relocation symbol/type rows, zero mutable fields, nonoverlap,
  semantic targets, constraints, and deterministic identity, returning the
  original linkage plan on rejection. These target templates follow the
  primary [x86-64 dynamic-linking
  rules](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/dl.tex)
  and [AArch64 procedure-linkage-table
  rules](https://github.com/ARM-software/abi-aa/blob/main/sysvabi64/sysvabi64.rst#procedure-linkage-table).
  A following linkage-descriptor rung now retains those exact templates while
  extending the existing section-name seed append-only from 69 to 93 bytes and
  sealing the semantic `.plt`, `.got.plt`, and `.rela.plt` rows. It binds each
  target template's exact payload size to its ABI section type, flags,
  alignment, and entry size. The AArch64 `.plt` additionally retains
  `SHF_AARCH64_PURECODE`; `.rela.plt` retains typed `.dynsym` `sh_link` and
  `.got.plt` `sh_info` meaning plus `SHF_INFO_LINK`, rather than premature
  numeric indexes. Independent replay checks the unchanged six-row prefix,
  exact appended names and offsets, row order, every metadata field, target
  distinction, and deterministic identity while preserving template custody on
  rejection. This produces a nine-semantic-section address-free view, not a
  final ELF section roster.
  The next semantic `.dynamic` rung now consumes that nine-section carrier and
  retains the significant exact `DT_NEEDED` prefix followed by the complete
  owned fixed tag roster and one final `DT_NULL`. Literal rows bind exact
  `.rela.plt`/`.dynstr` sizes, `Elf64_Sym` entry size, RELA kind, and version-
  requirement count. Seven zero-valued typed address obligations name
  `.got.plt`, `.hash`, `.dynstr`, `.dynsym`, `.rela.plt`, `.gnu.version`, and
  `.gnu.version_r`; they assign no pointer or numeric section index. The plan
  independently replays tag order/multiplicity, raw library-name offsets,
  relocation closure, target-specific future-`.dynamic` GOT policy, every
  literal and obligation, deterministic identity, and exact descriptor
  custody. General RELA, GNU-hash, bind-now, text-relocation, init/fini,
  runpath, soname, and target-optional tags remain absent because the sealed
  inputs own none of those meanings. The following serialization rung now
  consumes that plan into exact 16-byte ELF64-LSB `Elf64_Dyn` rows: signed
  `d_tag` followed by unsigned `d_un`, both little-endian. Literal values are
  copied exactly, while address-bearing values and the final null value remain
  zero. The seven address obligations become typed eight-byte fixups at the
  exact value-field offsets. An independent bounded decoder requires the exact
  row count with no trailing bytes and replays endianness, row order and
  values, fixup bounds/non-overlap/targets, deterministic identity, and plan
  custody. The next address-free descriptor rung extends the exact 93-byte
  append-only section-name seed with `.dynamic\0` at offset 93 and retains one
  semantic descriptor with `SHT_DYNAMIC`, writable/allocated flags, exact
  payload size, alignment eight, entry size sixteen, a typed link to
  `.dynstr`, and no info relationship. Independent replay checks the complete
  102-byte seed, raw name, unique semantic link, every metadata field,
  deterministic identity, and payload custody. It assigns no final numeric
  `sh_name`/`sh_link`, section index, address, or placement authority. The
  section-name-table rung now consumes the unchanged exact 102-byte seed as its
  complete payload: the `.shstrtab\0` name reserved once at offset 59 remains
  the unique name, and `.dynamic\0` ends the table at byte 102. Its semantic
  descriptor uses `SHT_STRTAB`, no flags, size 102, alignment one, zero entry
  size, and no link/info relationship. Independent bounded replay walks every
  contiguous NUL-framed name, rejects any byte or metadata drift, and preserves
  exact `.dynamic`-descriptor custody. It still assigns no numeric index,
  `e_shstrndx`, address, or placement. The numeric-roster rung now consumes
  that owner into twelve closed rows: null index zero; the retained six base
  rows at 1–6; `.plt`/`.got.plt`/`.rela.plt` at 7–9; `.dynamic` at 10; and
  `.shstrtab` at 11, which is also the exact `e_shstrndx`. It preserves every
  `sh_name` offset and literal `sh_info`, resolves semantic `sh_link` and the
  relocated-section `sh_info` to exact in-roster indexes, and independently
  replays order, unique coverage, every metadata field/reference, identity,
  and name-table custody. The closed numeric roster still assigns no address,
  file offset, or payload placement. The section-header serialization rung now
  consumes that exact roster into twelve 64-byte ELF64-LSB `Elf64_Shdr`
  templates (768 bytes total), copying every numeric field while leaving
  `sh_addr` and `sh_offset` as zero placeholders. Twenty-one typed placement
  fixups name the eleven non-null file-offset fields and the ten allocated
  virtual-address fields. An independent bounded decoder requires the exact
  table length with no trailing bytes and replays every field, row, fixup
  coordinate, zero placeholder, bound, non-overlap relationship, identity,
  and roster custody. It assigns no placement or `e_shoff`. The indexed-payload
  rung now consumes those templates and joins every numeric
  row to its exact already-owned bytes: null, the six base dynamic payloads,
  PLT/GOT.PLT/RELA.PLT, `.dynamic`, and `.shstrtab`. Each row is byte-identical
  to its upstream serializer and size-identical to its numeric descriptor.
  Separate typed fixup families retain every procedure-linkage/source-text
  obligation and all seven dynamic-table obligations while mapping section
  storage and semantic targets to their exact numeric rows; source text remains
  an explicit non-section storage domain, and the twenty-one section-header
  placement fixups remain in their original owner. Independent replay checks
  exact row coverage, bytes, sizes, fixups, masks, zero placeholders,
  constraints, storage bounds and targets, deterministic identity, and complete
  header-template custody. It resolves no address or placement. A relative
  payload-layout rung now consumes the indexed roster and classifies every non-
  null row into exact read-only, read-execute, read-write, or file-only domains
  from retained `sh_flags`. It packs each domain independently in numeric roster
  order with checked `sh_addralign`, exact relative offsets and spans,
  deterministic identity, and recoverable replay of every row and geometry.
  These offsets begin at zero per domain and are not `sh_offset` or `sh_addr`.
  The absolute-load rung now consumes that exact owner and binds both admitted
  Linux profiles to the fixed `0x400000` image base and a target-derived 64-KiB
  maximum-page alignment. It closes the canonical `PT_INTERP`, R/RX/RW
  `PT_LOAD`, `PT_DYNAMIC` order with exact `p_paddr == p_vaddr`, strict W^X,
  retained source text/data/aligned-BSS placement, file-only `.shstrtab` and
  section-header-table coordinates, and all twenty-one `sh_addr`/`sh_offset`
  resolutions. Independent replay checks the segment/section/source extents,
  congruence, alignment, special-header aliases, exact upstream fixup coverage,
  and deferred procedure-placement envelopes while keeping AArch64 relocation
  pages at 4 KiB. The section-header application rung now consumes that exact
  absolute owner, copies the retained 768-byte template, and applies only the
  eleven resolved file offsets and ten resolved virtual addresses to their
  typed zero placeholders as little-endian `u64` values. An independent bounded
  decoder replays all twelve roster rows, every unchanged field, exact ledger
  order/coordinate/kind/value, null and file-only `.shstrtab` semantics,
  deterministic content-bound identity, and load-layout custody. It still
  chooses no `e_shoff`, serializes no ELF or program header, mutates no final
  image, and claims no runnable authority. The internal `.dynamic` application
  rung now consumes that placed-header owner, copies only indexed roster row
  ten, and resolves the exact seven `DT_PLTGOT`, `DT_HASH`, `DT_STRTAB`,
  `DT_SYMTAB`, `DT_JMPREL`, `DT_VERSYM`, and `DT_VERNEED` address obligations
  from their allocated section virtual addresses. It patches only the typed
  zero `d_un` fields as little-endian `u64` values. Independent replay rejoins
  the semantic tag plan, serialized fixups, indexed storage, target section
  identities, every literal/null and unchanged byte, deterministic identity,
  and the complete placed-header/load-layout custody chain. A dynamic file-
  envelope rung now consumes that resolved owner and serializes the exact
  64-byte ELF64-LSB header plus the five already-planned 56-byte program-header
  rows. It binds `e_entry`, `e_shoff`, target machine, fixed table geometry,
  every program-header field, and the exact applied 768-byte section-header
  table retained as a file fragment at that offset. An independent bounded
  decoder replays both fragments against the entry symbol, absolute load
  layout, and placed-header owner; rejection returns the complete upstream
  owner. The carrier remains explicitly non-runnable and performs no payload
  copying, relocation application, or image mutation.
  Exact procedure/source address-fixup application is now complete as a
  separate non-runnable rung. It copies only the retained source `.text`,
  `.plt`, `.got.plt`, and `.rela.plt` fragments, applies every indexed fixup
  from the absolute load layout, and retains an exact typed application ledger.
  Independent replay rederives every semantic target, range/alignment rule,
  mutable mask, encoded field, and unchanged byte; rejection returns the
  complete file-envelope owner and the retained `FinalImage` is not mutated.
  Exact non-runnable file assembly is now complete as a following consuming
  rung. It places the header/program-header prefix, retained source text/data,
  all eleven non-null section payloads, resolved procedure-linkage and
  `.dynamic` fragments, file-only `.shstrtab`, and the applied section-header
  table into one owned buffer at their absolute file offsets. A typed fragment
  ledger and independent replay check exact source bytes, file extents,
  non-overlap, complete coverage, and canonical zero-filled alignment/page
  gaps; BSS remains memory-only, rejection returns the complete resolved-
  linkage owner, and the retained `FinalImage` remains immutable. The final-
  byte admission rung now consumes that assembly, recovers the exact retained
  `FinalImage` through the complete ownership chain, applies only the already-
  resolved source-text bytes, and independently rejoins the complete assembled
  file, target-specific format, image statistics, and placed executable-region
  inventory. Rejection retains the intact assembled owner; success retains the
  mutated image beside exact `ExecutableImageOutput` bytes and grants neither
  publication nor an execution event. The first production-emitter integration
  now consumes only that admitted carrier and independently rejoins its exact
  final image, import/relocation counts, final-text relocation envelope,
  executable-region inventory, target, and bytes to the borrowed source-free
  `ObjectArtifact`. Target or artifact drift rejects with the admitted carrier
  intact. Success remains a distinct non-installable custody type rather than
  `ExecutableImage`, so it cannot enter existing installation/publication APIs
  and grants no execution event. The complete source-free chain driver is now
  live for one exact import-bearing `ObjectArtifact` plus one consumed
  `NormalizedElfInterpreterPlan`: it constructs the final image and advances
  through every existing section, linkage, placement, fixup, assembly,
  admission, and production-bridge owner in order.
  Its stage-tagged failure enum retains the exact typed carrier returned by the
  rejecting rung; even a mid-chain opcode-placeholder failure preserves the
  original image, interpreter, and normalized imports through that owner.
  Exact x86-64/AArch64 replay is deterministic. The driver result remains the
  same non-installable custody type and grants no publication or execution
  authority. The first production object-retention prerequisite is now live:
  `MachineCodeFunction` owns ordered source-free `ForeignCallRelocation` rows
  containing the exact normalized locator, semantic call owner, and native
  relocation field. Ordinary `build_object_artifact` independently replays the
  x86-64 `CALL rel32` or AArch64 `BL` placeholder, target applicability,
  semantic provenance, ordering, uniqueness, and non-collision before it
  deduplicates exact locators into unresolved `SymbolKind::Import` rows, the
  atomic `normalized_imports` side table, and exact text relocations. The
  dynamic-ELF driver can therefore start from the ordinary object builder for
  both Linux profiles without a private `ObjectArtifact` fixture, and repeated
  calls to one locator share one import symbol while retaining distinct call
  sites. The first preceding production path is now closed for a
  `Unit`-returning normalized import leaf with no scalar arguments, one fixed-
  width 8/16/32/64-bit integer literal argument, or exactly two, three, or four
  such literal arguments, provided every evaluated placement is
  register-resident. Checked compilation
  retains the extracted external-binding rows before consuming typed trees;
  native settlement rejoins one unique retained row to the complete selected
  `ProviderPlan`, exact selected-plan evidence, and admitted same-stack
  contribution. Compact report fingerprints are never used as plan authority,
  and an equal-report substitute plan rejects. A distinct target operation
  survives assignment; ordinary machine emission produces an x86-64 `CALL
  rel32` or AArch64 `BL` placeholder and retains the locator, provider
  execution, evaluated call plan, admitted contribution, and exact physical
  `Unit` stack evidence in `MachineCodeFunction::foreign_calls`. In the
  literal-bearing cases, each occurrence-specific row retains its source value,
  integer type and immediate, parameter index, evaluated register placement,
  and exact materialization byte interval. With two through four arguments,
  those rows and byte intervals remain in parameter order, and every interval
  ends exactly where the next begins. Machine emission independently rejoins each
  custody row to its preceding constant and emits compact x86-64 or AArch64
  register materialization; object construction replays the complete ordered
  plan, placements, bytes, semantic call ownership, and physical stack custody
  before consuming the rows. Both Linux profiles advance from the exact native
  rejoin through target, assignment, machine, ordinary object, and the complete
  dynamic-ELF driver; stripped, reordered, or drifted source/type/value/index/
  register/byte/plan/stack custody rejects. Runtime-derived arguments, five or
  more arguments, any stack argument, result-bearing signatures, complete
  task-stack-budget composition, optional `.gnu.hash`, and ordinary source
  `via` evaluation remain engineering work.
  The generic contextual byte-literal rung is also live for owned direct
  `[u8; N]` destinations used by final results, locals/owned initializers,
  exact resolved call arguments, and record/case fields. It copies source bytes
  into an ordinary array only when `N` is a resolved integer literal and the
  byte count matches exactly; non-`u8`, short/long, const-parameter, and
  const-call widths reject, while borrowed/text carriers keep their prior
  meaning. Hermetic build-time evaluation observes the resulting raw-byte
  `Array` value without text reconstruction.
  The existing source evaluator is isolated behind an explicitly temporary
  `StringBackedImportBootstrap` variant. Ordinary source `via` evaluation and
  complete dynamic-ELF section, relocation, and loader-plan realization remain
  engineering work.

Acceptance: the same boundary requirement can select a checked test provider or
a target intrinsic without editing its declaration; final artifacts contain no
primitive-provider or foreign-endpoint registry; an intrinsic lowering is
selected only by exact realization symbol/signature/target; audit manifests
enumerate the actual evaluated locator and plan; and changing raw foreign bytes
changes the evaluated binding and every dependent target/artifact identity.

### Float providers

Owner: `wiki/design_briefs/float_semantics.md`.

Remaining F7 work:

- provide feature-qualified x86-64 FMA or a checked binary32/binary64 software
  implementation, then select the generic x86 FMA slots;
- complete the wider proof/`Real` connection under N6/N8.

The mechanical x86-64 prerequisite is live: `omega-isa-x86_64` owns exact
register-only VEX encoders for scalar `VFMADD132SS` and `VFMADD132SD`, including
extended-register and invalid-register coverage. A bounded source-free custody
seam is also live: `omega-target` owns the immutable, profile-bound AVX+FMA3
requirement; machine emission retains exact format/register/interval identity;
and object construction independently decodes `VFMADD132SS`/`VFMADD132SD`,
replays the target and deployment profile, and rejects stripped, substituted,
overlapping, or identity-drifted custody. The ordinary object builder rejects
feature-requiring FMA without an explicit profile, and executable-image
construction rejects it because no admitted provider discharges the retained
requirement yet. This seam grants no target feature admission, accepts no
Terminal/source operation, and remains unselected by generic FMA lowering;
that still requires the feature-qualified provider or checked software
realization above.

The generic Linux and Windows x86-64 baselines now retain target-specific
semantic-edge suites. Each checked half pins the exact 36 nearest arithmetic,
comparison, classification, min/max, square-root, negate, and separately
rounded multiply-then-add plans. Their artifact halves build the explicit
`linux_x64` or `windows_x64` root twice and require respectively byte-identical
ELF or PE/COFF images under retained host-independent identities; the Windows
suite also replays the DOS/PE signature and AMD64 machine header. Native
execution is a separate leg and runs only on a matching Linux or Windows x86-64
host. Directed and fused operations are deliberately absent, so this validation
evidence grants no generic FMA or feature admission. Linux AArch64 now has its
own comprehensive target receipt over the existing 56-plan semantic-edge twin,
including nearest and directed arithmetic, directed and nearest FMA, and
fused-versus-separately-rounded behavior. Its checked/interpreted half exits 70,
two explicit `linux_arm64` roots produce byte-identical ELF images that name
`EM_AARCH64`, and native execution is retained separately on a matching Linux
AArch64 host. Together with the existing macOS AArch64 semantic-edge suite, all
four currently admitted native profiles now carry target-specific evidence.
This closes evidence coverage only; it does not authorize FMA on the generic
SSE2 x86-64 baseline.

Proof-only Exact float-to-integer cast admission now lives in a focused 399-
line private owner. Finite expression intervals, declared range projection,
all-incoming guard meets, strict next-float bounds, comparison polarity, and
exact target-range checks retain fail-closed behavior and diagnostic order. The
source-value classifier and store-conflict diagnostics now live in a focused
299-line private owner. Scalar class inference, named-operator representation-
change refusal, concrete data identity resolution, and cross-class/nominal
diagnostics retain exact behavior and order across all consumers. Expression
operator type validation now lives in a focused 483-line private owner. Binary
check order, float/integer fences, cross-class/text/logical/struct/array
diagnostics, unary checks, and shared shape classifiers retain exact behavior.
Expression store-shape validation now lives in a focused 120-line private
owner. Array-versus-scalar and scalar-versus-data classification, text-carrier
exceptions, and exact diagnostics retain behavior and order across every store
surface. Expression cast validation now lives in a focused 285-line private
owner. Recast bypass, indexed qualification, proof embedding, quotient-mint
carrier checks, same-carrier erasure, scalar source fences, and Exact/Wrapping
float-policy diagnostics retain exact behavior and order. The natural 270-line
expression-type facade retains generic argument matching, bounded-text capacity
validation, expression-owner diagnostics, and type labels; crate-facing APIs,
identities, and the exact 51-function inventory remain unchanged.
Anonymous-float destination landing now lives in a focused 445-line owner.
Destination-format discovery, exact pre-landing rational and comparison
folding, one-time rounding, and runtime-tree stamping retain original order and
the public re-export. The documented u64-magnitude admission matrix now lives
in a focused 352-line owner: struct/assignment/local/guard/transition/proof-fact
blessing and exact rejection retain diagnostic and traversal order. The natural
221-line literal root retains cohesive suffix magnitude/destination validation
and owner re-exports, with the exact 16-function inventory unchanged.

Checked-result float/integer conversion remains blocked on the separate
checked-result arithmetic decision listed below.

### Lifetimes, dynamic traits, and build-time evaluation

- Finish outlives constraints, persistent/parameter-backed owners, aggregate
  borrow propagation, runtime-index expressions beyond exact immutable
  local/state-parameter forwarding, loan-root rebasing, and exact R5 facts.
  Direct helper-call results and moved/projected borrow-carrying aggregates now
  retain exact source loans, enclosing field/fixed-index paths, and polarity
  when nested inside another literal. Same-carrier denotation-preserving value
  casts also retain those loans at root and nested positions across moved,
  helper-produced, and literal operands. Validated shared/mutable recasts over
  whole name/member places now publish the exact source loan too; indexed
  byte-region recasts with one exact literal offset into a fixed byte array and
  either a fact-free primitive target, one nonzero closed acyclic tree of
  quotient-free, all-relevant fact-free records, or one recursively nonzero
  literal fixed array ending in either an exact primitive or eligible record
  shape now publish the
  complete validated half-open target footprint as one fixed-range loan.
  Primitive-terminal arrays require an exactly tiled normalized representation;
  record-terminal arrays and eligible records containing recursively literal
  array fields retain the complete normalized padded record extent. Zero array
  fields participate only when their terminal independently qualifies and the
  containing record remains nonzero; their element alignment can still induce
  protected padding.
  Fully specialized type plus scalar-integer `const` or exact-replayed acyclic
  structured-data `const` instances participate through their exact
  synthesized symbol, validated base/argument origin, and substituted fields.
  Scalar const origins require an unbound canonical decimal leaf within the
  exact declared integer carrier. Structured origins require one bounded,
  completely decoded compiler-only atom replayed in declaration order against
  the exact resolved monomorphic record or pure-sum carrier, including selected
  cases, ordered payloads, nested literal arrays/records/sums, and exact
  integer/Boolean leaves. Layout still comes only from substituted instance
  fields. One direct erased-lifetime application around an otherwise eligible
  synthesized record instance now participates when it carries the exact
  nonempty declared lifetime arity, no residual runtime arguments, and no
  recursively lifetime-bearing field. Raw checked lifetime spellings remain
  distinct while the sealed exact synthesized symbol supplies physical layout;
  ordinary recast validation and precise loan sizing share that resolver.
  Generic normalization rewrites
  concrete-machine cast targets and synthesizes recursively nonzero literal-
  array type arguments. Record lookup
  and recursion use exact symbol identity. Repeated-leaf capacity
  overflow fails closed before allocation. First/last/padding-byte mutations
  reject while immediate siblings remain disjoint. Runtime or merely bounded
  offsets, slices, total zero-size targets, open/unresolved, mixed, recursive,
  or custom-canonical structured-const origins, nested/array lifetime-generic
  shapes, malformed or nonphantom lifetime applications, machine/proposition
  generic instances, invariant-bearing/erased/cased records, and other indexed
  recasts remain conservative.
  Scalar recast representation-set normalization and
  implication now live in a focused 377-line private owner. Exact integer two's-
  complement bit-pattern intervals, same-carrier float intervals, domain-
  conjunction implication, mutable bidirectional equivalence, and five focused
  tests retain behavior and order. Aggregate recast representation
  normalization now lives in a focused 471-line private owner. Exact record/
  array geometry, plan-laid stored-width and repeated-field normalization,
  stable leaf order, exact tiling, fallible repeated-leaf allocation, shared
  implication, and mutable bidirectional equivalence retain behavior and
  identity. Interior-byte recast
  offset proof now lives in a focused 622-line private owner. Per-edge upper/
  lower meets, constant and self-forwarding routes, guard/equality symbolic
  composition, declared ranges, boundary `ensures`, and write-frame
  invalidation retain fail-closed order and exact results. Semantic-domain
  qualification now lives in a focused 579-line private owner. Exact domain
  lookup and vacuity, literal/range/`requires` mint discharge, contextual
  statement/expression traversal, and diagnostic emission order retain
  behavior. Raw interior-byte recast admission now lives in a focused 199-line
  private owner. Exact source-shape recognition, literal/range/guard offset
  evidence, offset diagnostics, and recursive fact-free target eligibility
  retain behavior and order. The natural 711-line recast root retains
  validation/root-position orchestration, the primary scalar judgment and
  slice continuation, reference-pun closure, and shared borrow unwrapping;
  public APIs, diagnostics, identities, and the exact 49-function inventory
  remain unchanged. Remaining computed aggregate expression forms still
  need the same propagation law.
  Structural four-axis carry derivation now lives in a focused 189-line owner.
  Generic-bound substitution, recursive transparent stored-shape traversal,
  strict cycle/opaque fallback, and per-axis intersection remain shared by
  declaration validation and checked carry facts; the property coordinator is
  now 374 lines with the exact 18-function/method inventory unchanged.
  Static-machine contract-fact refinement now lives in a focused 270-line
  private owner. Requires/ensures/boundary variance, crash-route bucketing,
  tautology elision, proposition/membership rendering, positional alpha-
  normalization, diagnostic order, crate APIs, and the exact 26-function
  machine-parameter inventory remain unchanged. Static-machine binder-aware
  type refinement now lives in a focused 213-line private owner. Positional
  binder substitution, generic binding reuse, reference mutability,
  constrained/slice/generic recursion, fixed-array const-binder identity,
  normalized fallback identity, crate APIs, and that same inventory remain
  unchanged. Static-machine callable-shape refinement now lives in a focused
  535-line private owner. Generic parameter kind/property checks, positional
  binder setup, parameter/return shape matching, service and invocation
  ceilings, suspension/blocking/termination refinement, nested machine-contract
  recursion, trait reverse checks, contract-fact handoff, APIs, diagnostic
  order, and the 26-function inventory remain unchanged. Nominal static-machine
  admission now lives in a focused 120-line private owner. Exact forwarded-
  binder authority, concrete entry ownership, unique authored satisfaction
  rows, canonical requirement-overload identity, rejection diagnostics/order,
  crate APIs, and that same inventory remain unchanged; the natural 475-line
  parent retains traversal, operational inference, call/data selection
  transactions, and recursive contract lookup.
- Materialize dynamic descriptors for pass-through, rebound, and escaping
  borrows from the retained exact conformance rows and declaring-trait symbol.
  Bodyless/bare requirements do not license `dyn`; ambiguous same-carrier
  boundaries name the exact complete conformance.
  The first pass-through rung is live for an immutable local selected by one
  exact closed conformance and forwarded to a compatible bare dynamic
  parameter. Validation requires the earlier same-trait selection rather than
  searching visible conformances. Checked selection facts retain the source,
  trait, conformance, and complete normalized row map through state graph and
  control flow. Each row now carries the complete normalized requirement-
  overload and selected realization-callable identities; checked-to-state
  validation independently reconstructs both and rejects identity drift before
  state-call argument planning rejoins that exact descriptor identity to the
  bare parameter's closed candidate catalog. Trait drift,
  unselected dynamic arguments, bodyless conformances, and ambiguous concrete
  arguments remain fail closed. Target-data planning now validates those rows
  again and emits one deduplicated, pointer-aligned private data object per
  logical selected conformance. Each runtime slot is one zero-filled pointer
  word paired with its exact address-free realization target; normalized-
  identity drift rejects before any bytes publish. Relocation planning now
  revalidates zero slot bytes, alignment, strict normalized requirement order,
  and the private data symbol, then binds every retained realization `StateKey`
  to exactly one private function symbol with a data-section `Absolute64`
  materialization record. Missing or duplicate function identities fail
  closed. The abstract-data bridge now retains the table's exact trait,
  conformance, normalized row identities, and private object handle rather than
  degrading it to ordinary data; a unique symbol-keyed lookup fails closed on
  missing, duplicate, or malformed bindings for transitional instruction
  selection. The first direct-place pass-through construction now writes the
  exact two-word runtime ABI carrier: one place-address operation stores the
  retained concrete instance and one single-word data-address operation stores
  the selected private table. Target lowering, both native encoders, final-byte
  validation, and the table/frame relocation pair preserve that distinction;
  failed exact joins never fall through to copying the unmaterialized erased
  local. Instruction selection now independently reconstructs each checked
  table row, validates normalized requirement/realization identity and exact
  control-flow `StateKey`, and emits one standalone private function containing
  that retained state body. Repeated exact realizations deduplicate, entry-state
  realizations reuse the existing entry identity, and missing, duplicate, or
  mismatched demands fail before machine bytes. Both native targets now
  complete the table relocation and full-image pass-through canary. The first
  runtime indirect slot-call rung is also live for an immutable bare-dynamic
  parameter. Selection requires the exact parameter symbol/kind and two-word
  descriptor, one normalized requirement row at one common slot in every
  retained candidate, and an identical authoritative `CallPlan` for every
  realization; parameter/result shape drift, missing or duplicate rows, and
  representative-first coincidence reject. That plan alone owns receiver,
  arguments, and result, so indirect lowering cannot also replay a direct/
  spliced result producer. Private calls retain a closed validation identity
  distinct from foreign table calls and therefore add no foreign floating-
  control envelope on either ISA. Every private realization's complete
  prologue/body/result/return span contributes to the one root transitive
  footprint certificate, and ceiling/span failures return diagnostics rather
  than panicking or omitting evidence. Mach-O publishes loader rebase opcodes
  for the exact typed data-to-private-function `Absolute64` sites and replays
  their preferred pointers before publication, so a distinct-instance native
  canary truly executes the relocated table slot under ASLR. The first mutable
  rebind rung is now live for one local initialized and reassigned by exact
  direct-place casts naming the same carrier, trait, conformance, and normalized
  row map, then forwarded to that bare-dynamic parameter. Selection facts are
  statement-versioned; recast validation blesses only the exact admitted
  assignment RHS; checked-to-state replay reconstructs the target, cast, and
  prior selection; and call planning selects the latest prior version.
  Instruction selection writes a fresh instance address and the unchanged exact
  table address into the existing two-word local slot before generic mutation
  handling. A decoy-to-selected native canary proves that the reassigned
  instance, not stale initializer state, reaches the indirect slot on both
  Linux targets. The exact same-conformance local can now also call one
  requirement directly after rebinding. State-call planning retains a closed
  `ReboundLocal` receiver with the unanimous exact binding and latest selection
  statement, refuses malformed/colliding versions without devirtualization
  fallback, and makes the dispatch itself own the table-materialization demand.
  Instruction selection independently rejoins that latest selection, its sole
  conformance candidate, the existing two-word local slot, one common normalized
  requirement row, and its authoritative `CallPlan` before reusing the private
  table-call lowering. A distinct decoy/selected canary executes the direct
  rebound call natively and under both Linux target replays. A changed
  conformance or carrier, non-cast assignments, aggregate erased calls,
  stored/joined/escaping descriptors, and component crossing remain fail
  closed.
- **TARGET-SEMANTIC-APPLICATIONS — close typed target observations and selected
  realizations.** Complete hermetic evaluation with crash refinement, target
  capsule, separate result/usage identities, deterministic progress, and
  runtime equivalence.
  Publish `Hermetic | Receipted | Volatile` ceilings and realized provenance.
  Generalize `const` evaluation beyond the current scalar/record corpus to
  fixed arrays, copy-eligible sums, and aggregates containing them. Admit exact-
  width literal-to-fixed-array copying in owned result positions and keep
  temporary evaluator references internal; do not add a special byte-blob type.
  Derive value-sensitive `ConstEvaluable` and `ConstMaterializable` judgments;
  walk only the realized active case, retain component/origin diagnostics,
  zero layout padding, reject underdetermined observable encodings such as an
  unfixed NaN payload at runtime materialization, and preserve an exact carried
  quotient representative without demanding canonicalization. Retain target-
  dependent const application identity through target-neutral intermediates.

  AArch64 runtime equivalence now covers signed 64-bit Saturating division and
  remainder without narrowing. The retained nonzero formation certificate
  remains target-neutral; ISA lowering independently splits on divisor `-1`,
  maps `i64::MIN / -1` to `i64::MAX` with `NEG`/comparison/conditional select,
  maps the corresponding remainder to zero, and leaves every other divisor on
  the ordinary `SDIV`/`MSUB` path. Unsigned Saturating division remains `UDIV`.
  Direct-write and recursively evaluated operand byte widths replay the exact
  emitted sequences, both Linux targets emit hermetically, and native AArch64
  coverage executes both the overflow corner and an ordinary `-101 / 5` /
  `-101 % 5` branch. Division by zero remains forbidden before target lowering.

  The first opt-in `ConstEvaluable(T, value)` result boundary is live.
  Explicit admission APIs preserve operational/common-floor checking and
  interpreter execution, then structurally validate the exact declared result
  against its returned owned snapshot. They admit Unit, primitives, literal
  fixed arrays, closed `[copy]` records, and only the realized case/payload of a
  closed sum; constraints reuse their already-checked carrier. Reference,
  slice, Text, dynamic/open/generic/opaque, atomic, affine-record, and malformed
  type/value shapes reject with a component path and no panic. The first
  position-owner migration is live: zero-argument integer machines used by
  fixed-array lengths and const-generic arguments now cross the opt-in boundary
  after exact invocation-custody/common-floor admission, decode only an exact
  integer snapshot, and preserve the owning position's nonnegative and host-
  width conversion checks. The second position-owner migration is live:
  machine-backed facts used to discharge concrete const-domain membership now
  validate their exact integer-to-Boolean signature, cross the same
  source-custodied opt-in boundary, and decode only an exact Boolean snapshot.
  Other compiler-owned plan evaluators do not opt in implicitly while their
  affine result vocabularies remain unchanged. The first bounded opt-in
  `ConstMaterializable(value, layout)` carrier is also live for closed
  non-generic `[copy]` records composed only of integer/Boolean/non-NaN
  binary32/binary64 fields, literal fixed arrays, and nested records of the same
  kind. It retains the exact typed owned value, schema name and identity, full
  validated layout plus normalized fingerprint, target byte order, exact zero-
  initialized staged bytes, and one deterministic identity. Construction and
  replay independently revalidate the schema/member placements, derived
  offsets, fixed extent and alignment, value shape, byte order, and exact bytes
  through the existing atomic aggregate writer; failed replay or a short
  destination leaves the destination unchanged. Float leaves retain exact IEEE
  format bits; binary32 custody rejects a retained binary64 snapshot that would
  require narrowing. Signed zero and infinity remain exact. Every NaN still
  rejects until canonicalization or an exact raw-NaN realization fixes its
  representation. The first pure-sum rung is now live separately over the
  compiler-owned conventional runtime representation. `omega-layout` projects
  the authoritative four-byte authored-order tag and complete all-case payload
  overlay into an exact report; Psi independently rejoins every case/payload
  identity and target-independent geometry, then walks and writes only the
  selected payload into zero-initialized staging. The carrier retains selected
  case identity/ordinal, exact layout and fingerprint, byte order, bytes, and
  deterministic identity; replay uses hash-free semantic-member equality, so
  stable-numbered case/payload renames are presentation-only while ordinals and
  geometry remain exact, and replay or short-copy failure is atomic. This does
  not extend programmable `Layout` with tagged case placement. The first
  direct nested rung is now live for the complete nonempty authored-order set
  of direct runtime-relevant conventional pure-sum fields in one closed
  non-generic `[copy]` record. `omega-layout` projects the whole-field outer
  layout and one exact field-identity plus conventional nested-layout row per
  occurrence from the same target runtime plan. Repeated occurrences of the
  same sum type stay distinct and may select different cases. A distinct
  non-clone carrier keeps the outer typed value and layout beside every ordered
  nested sum's complete layout and selected case identity/ordinal, target byte
  order, and zero-initialized final bytes; replay rejects missing, extra,
  reordered, and duplicate occurrence rows, compares every layout hash-free,
  stages every nested buffer, and performs one atomic outer copy. Erased sum
  fields remain outside the runtime occurrence set. The direct array rung is
  live for the complete nonempty authored-order set of direct nonzero literal
  fixed-array-of-sums fields in the same closed record cohort. Each compact
  target row retains one exact outer field identity, count, stride, and one
  complete conventional all-case layout; a distinct non-clone carrier retains
  every field occurrence and literal index with independently selected
  case/value/bytes without duplicating the complete layout per element.
  Projection rejoins each exact array descriptor, symbol, extent, and alignment
  from one runtime plan and rejects fragment, stored-integer, or target-
  dependent repeated placement on every outer field. Replay reconstructs every
  indexed sum and the zero-padded outer image before one atomic copy. The
  singular producer and consumer remain exact-one wrappers and reject a plural
  cohort.
  The first one-level record-path rung is now live separately for the complete
  nonempty authored-order set of direct outer fields naming closed non-generic
  `[copy]` records with direct conventional pure-sum fields. Its compact plural
  report retains the outer whole-record layout once and one exact occurrence,
  inner whole-record layout, and complete child-sum row set per field, all from
  the same target plan. Repeated uses of one inner type remain distinct and may
  select different cases. A distinct non-clone carrier retains one existing
  validated inner carrier per occurrence, reconstructs every zero-padded inner
  image and the outer image, replays every report hash-free, and performs one
  atomic outer copy. The singular producer and consumer remain zero-allocation
  exact-one wrappers and fail closed when another occurrence exists.
  Stable-numbered field/case/payload renames remain presentation-only while
  identities, order, geometry, selected values, and bytes remain exact.
  Projection and replay use a fallible memoized bounded graph walk and a linear
  authored-order occurrence cursor, so recursive or oversized aggregate paths
  reject without host recursion or shared-subgraph amplification.
  The complete plural depth-two record-chain rung is now live for
  `Outer -> Middle -> Leaf -> direct sums`. Its compact report retains the
  outer whole-record layout once and one exact row per authored-order outer
  occurrence; each row's unchanged plural one-level report owns the complete
  middle-to-leaf set.
  The non-clone carrier likewise retains one existing validated plural
  one-level middle carrier per outer occurrence, rebuilds every leaf and middle
  image plus the outer zero-padded image in order, replays every layout and
  occurrence identity hash-free, and performs one final atomic copy. Repeated
  uses of the same middle or leaf type remain occurrence-distinct and may
  select different cases. The earlier singular producer and consumer remain
  exact-one wrappers and reject a plural outer or middle cohort. Shallower,
  deeper, recursive, array-mediated, or direct-sum-coexisting paths reject, as
  does target-dependent placement at any layer.
  One further singular fixed-depth rung now admits exactly one
  `Outer -> First -> Middle -> Leaf -> direct sums` chain and no other sum-
  reachable field anywhere in the outer schema. Its compact report retains the
  new outer whole-record layout and exact field occurrence beside the unchanged
  singular depth-two report. The distinct non-clone carrier likewise retains
  the complete depth-two carrier, rebuilds the inner and outer zero-padded
  images, replays every layout and occurrence identity hash-free, and performs
  one final atomic copy. Shallow, deeper, plural, recursive, array-mediated, or
  direct-sum-coexisting paths reject, as does target-dependent placement at any
  of the four record layers. Existing direct, one-level, singular depth-two,
  and plural depth-two APIs remain unchanged.
  The complete plural depth-three fixed-depth rung now admits a nonempty
  authored-order set of `Outer -> First -> Middle -> Leaf -> direct sums`
  chains. Its compact report retains the outer whole-record layout once and
  one exact row per outer occurrence; each row owns the unchanged plural
  depth-two report for that occurrence. The non-clone carrier composes the
  corresponding plural depth-two carriers, rebuilds every nested image and the
  outer zero-padded image, replays all layouts and occurrence identities
  hash-free, and performs one final atomic copy. Repeated nominal types remain
  occurrence-distinct. The singular depth-three API remains exact-one, while a
  shared memoized bounded walk, fallible storage, and a global leaf-occurrence
  ceiling bound the plural producer and consumer. Shallower, deeper,
  recursive, array-mediated, or direct-sum-coexisting paths still reject, as
  does target-dependent placement at every layer.
  Zero-length or nested sum arrays, direct-sum coexistence, paths deeper than
  three records, plural paths deeper than three records, mixed common-field/case
  shapes,
  target-dependent inactive-case geometry, generic/opaque/quotient records,
  references, slices,
  Text, dynamic values, atomics, non-copy data, and malformed shapes remain
  rejected without narrowing the legacy materialization API. This custody is
  not evaluator admission, a target capsule, quotient canonicalization, an
  origin-chain proof, or proof authority. Quotient snapshots/materialization,
  sums nested through deeper aggregates, mixed shapes, target
  capsules/observations, complete origin diagnostics, and broader representation
  bytes remain subsequent.

  Materialize one compiler-owned versioned typed capsule shared by evaluator and
  backend. Expose only its closed subject-qualified observation vocabulary; do
  not add a runtime reflection object or ordinary provider for primitive carrier
  meaning. Land `addr::Bound: Int` as the exclusive one-past address bound and
  migrate core's transitional `boundary machine no_wrap(...) -> bool` into the
  transparent proposition
  `embed(base) + embed(length) <= addr::Bound`. Domain `requires` rows resolve to
  `Prop`; never coerce a Boolean-returning machine into a predicate. Preserve
  eligible total pure machine calls only as denotational terms inside an actual
  proposition.

  Permit target observations in every existing canonical const position,
  including array lengths and const-generic applications, while adding no
  conditional field/case, multiplicity, or declaration-splice mechanism.
  Retain symbolic applications until target closure. Track exact
  `ObservationApplication` and `SelectedRealizationApplication` dependencies
  through constants, proofs, plans, public signatures, caches, Terminal
  certificates, manifests, and artifacts; folding may remove an expression but
  never its target-closure receipt. Keep whole-capsule plus whole-realization
  keying as the conservative gate until fine-grained replay covers both kinds.
  Record a compact provenance DAG so incompatible independently closed artifacts
  identify both closures and the alias/const/generic/plan path that introduced
  the mismatch. Treat a changed public target-dependency set as a breaking API
  revision and a private change as target-artifact invalidation.

  Native plan sizes may feed ordinary const types when otherwise eligible, but
  calling-plan staging or plan-sized extents remain the representation-aware
  route. Field/case-set variance uses distinct nominal target schemas behind a
  stable requirement; build selection never mutates a declaration. Add canaries
  for target-indexed arrays, folded-dependency retention, selected-plan
  dependency without an observation call, cross-target replay rejection,
  conditional-field rejection, and the inclusive one-past narrowing case:
  `no_wrap` alone does not prove a `2^32` length fits `u32`, while an independent
  `length < addr::Bound` proof does.

  Admit evaluation through either a certified maximum-logical-work ceiling or
  deterministic unobservable metering, with corresponding temporary-memory and
  result-size budgets and published-build receipts.
  Recursive build-time call-closure admission now lives in a focused 299-line
  private owner. Ordinary termination traversal, authored-precondition
  rejection, linear runtime-carrier exclusion, callable-contract validation,
  exact call paths, and diagnostic order remain unchanged. The 282-line parent
  retains operational-axis projection, common-floor aggregation, and
  evaluation; public APIs and the 107-function production inventory are
  unchanged. Closed constant-domain proof-expression evaluation now lives in a
  focused 127-line private owner. Checked integer arithmetic, shifts, bitwise
  and logical operations, comparisons, unary operations, `self` substitution,
  unsupported-expression fallback, diagnostics, and evaluation order remain
  unchanged.
  Recursive constant-domain membership discharge now lives in a focused 227-
  line private owner. Nested-domain traversal, cycle fallback, direct-`self`
  machine-fact selection, common-floor admission, exact signature checks,
  proof-fact order, and diagnostics remain unchanged. The natural 122-line
  coordinator now owns only concrete-membership discovery and transactional
  fact replacement; public APIs and the 107-function production inventory are
  unchanged. The wire-policy value bridge now lives in a focused 153-line
  private owner. Exact schema-fact materialization, fixed-capacity padding,
  common-floor policy admission, returned `FieldPlan` decoding, full-width tag
  preservation, diagnostics, and tag-sorted agreement remain unchanged. Its
  cohesive 191-line parent retains schema classification, derived-plan
  comparison, encode-obligation construction, and transactional installation;
  public APIs and the 107-function production inventory are unchanged.
- **MEMBER-REFLECTION — design blocked.** Settle `Self::fields`, field/case and
  payload splices, their legal constant positions, and proof visibility of
  generator-expanded bodies. The owning open-design note is
  `wiki/language_guide/chapter_14_traits.md`'s `build-time-open` footnote;
  implementation must not choose this surface or its expansion rules.
- Complete the ordinary `Build` API/executor with exact dependency aliases,
  package-scoped providers, no ambient filesystem escape, and generated-source
  rechecking under consumer ceilings.
- **BUILD-ADMISSION-CHECKPOINT:** implement D18 in the maintained Rust
  comparator. Retain the existing coherent `CheckedFrontend`, prepared static-
  machine-specialized build projection, operational/service-reach plans, source
  commitment, and authority verdict as one activation-local checkpoint. Run
  the selected root build from that projection, then continue final checking
  from the retained base. Give own generated source a later scope stratum whose
  declarations can see authored declarations but can never resolve an authored
  occurrence. Delete the current full frontend rebuild and nominal
  `(source_span, name)` build-machine rebind once the continuation exists. Pin
  exact build-helper reach, generated overload/conformance non-interference by
  scope, dependency-bundle no-rerun, no-source-reread, and configuration/
  evidence retention canaries.
- Harden resolution with content/revision checks, archive containment, limits,
  scoped writes, receipts, and one dependency/build/trust lock. Any imported
  claim-set diff invalidates root acceptance; release providers are hermetic or
  receipted, and volatile observations cannot pass source-rebuildable release.

### Components and executable trust

- **COMPONENT-SUBSTRATE:** implement the settled independently selected
  provider path, keeping Cathedral update policy outside the compiler:
  - replace transitional bare boundary-trait runtime values with the explicit
    affine `Service<R> in Bound` carrier and routed installation/publication
    establishment; fused selection may erase it, while independent calls
    acquire and leave one exact era;
  - extend typed `Build::select_provider<Service, Provider>` with fused versus
    independent mode, exact closed-requirement slot identities, multiple roots
    per package, and compiler-derived closure/import/export validation; compute
    the transitive fixed point of concrete implementation, selected executable
    conformance, layout, cleanup, state, and custody edges, and require explicit
    owner acceptance when that fixed point enlarges the requested replacement
    cohort;
  - emit deployment-agnostic component capsules containing canonical Terminal
    Psi, reconstructed obligation evidence, symbolic imports/exports, target
    dependencies, resource/lifecycle demand, and optional native realizations;
  - check the three execution routes explicitly: verified Psi interpretation,
    checked Psi-to-native realization whether prebuilt or lowered locally, and
    exact disclosed admission of opaque native executable-TCB content;
  - create the initial stable slot, first era, bounded service handles, linear
    supervisor update authority, and frozen deployment-local acceptance
    envelope during the owner-controlled build;
  - represent future candidates existentially behind the service contract and
    let the runtime verifier accept only candidates fitting that envelope;
    graph or authority widening requires a new build/composition transaction;
  - implement linear staged/publication/freeze/retirement/release transitions,
    era-pinned calls and returned custody, continuity-proof selection, and
    graph-cut diagnostics for shared mutable state or linear custody; reject
    `[copy]` on directly era-pinning carriers, maintain exact creation/move/
    explicit-duplication/terminal-release pin accounting for affine and linear
    carriers, and require zero active entries plus zero era pins for reclaim;
  - retain an attributable cause for every closure edge so source builds report
    an exact span and prebuilt artifacts still report the consuming artifact and
    declaration, selected conformance or implementation, and transitive cohort
    expansion path; and
  - implement a durable deployment journal with `Prepared`, `Activated`, and
    `Finalized` restart reconciliation. The journal retains accepting envelope,
    evidence, admissions, slot history, and live-era state; it is not assumed
    atomic with in-memory publication. The Rust product implementation now owns the canonical
    versioned journal record and typed `Prepared` -> `Activated` -> `Finalized`
    transitions. It retains exact slot/era/artifact, entry-plan/admission,
    accepting-envelope, disclosed-admission, and canonical installation
    evidence; decode is report-only, failed publication returns all custody,
    and restart reconciliation exposes rather than chooses rollback or
    roll-forward policy. A generic checked durable-storage adapter now consumes
    one canonical phase record and a caller-selected new path, stages and
    synchronizes the exact bytes, atomically publishes through a same-directory
    no-clobber hard link, removes the stage, and synchronizes the directory.
    Success retains a non-clonable replay receipt; rejection returns the record
    and path and distinguishes unpublished from visible-but-cleanup-or-sync-
    incomplete state. Existing destinations are never replaced, restart load
    independently decodes and re-encodes exact bytes, and the adapter selects no
    rollback, roll-forward, path, retention, or cohort policy. Add Cathedral's
    path/retention selection and restart-to-runtime reconciliation around this
    checked core.
- Replace the provider-switchboard fixture's transitional `clock: ClockHost`
  field with `Service<ClockHost> in Bound` once that carrier lands. Keep its
  provider as checked Omega code. Real foreign protocol tables use validated
  named `Binding::VtableField` leaves; the parser rejects authored numeric
  `VtableSlot` while downstream artifact enums/codecs retain compatibility
  under **BOUNDARY-OPERATOR-FAMILY-SELECTION**.
- **FFIVAL:** run the narrow Windows `user32` boundary-coherence slice after
  ENT4, using existing activation, custody, registration, stack, and reach
  machinery.
- The first exact component stack-needs column is live. A Terminal component
  candidate retains the emitter-derived `StackDemand` for its canonical
  object entry, independently rederives the complete target-specific internal
  call-graph closure during candidate admission, and rejects even a
  same-architecture demand from another object format. Candidate decomposition
  and failed deployment paths preserve that exact evidence. It excludes the
  external entry adapter and grants no provision, stack lease, installed-root
  admission, or external headroom. Extend component artifacts with mapping
  cohorts, two-sided import/export checks, service-carrier multiplicity, custody
  receipts, enumerable roots, and the remaining resource/lifecycle columns.
  Concrete drain/coexistence algorithms, scheduler/device quiescence,
  update-cohort policy, rollback, mappings, and provisioning remain
  Cathedral/runtime work.
- Implement serialized capability attenuation/revocation only after the
  component carrier and custody rules are complete.

### Atomic ordering and device protocols

- **ATOMIC-EVENT-MODEL — design blocked.** Define the formal portable event
  model and x86-64/AArch64 refinement before enabling general protocol
  verification or global-order fences. Placed atomic accessors, checked ISA
  barriers, and installed-root same-context evidence do not wait for it.
- **DEVICE-OPERATION-COMPLETE-DMA-SLICE — client gated.** Under D27, retain the
  existing five-role access-plan carrier as non-authorizing test scaffolding;
  no source/checker row may be synthesized from it. The first source-admitted
  implementation is one complete DMA service boundary for a named driver,
  firmware service, or hosted/native compatibility customer. Its selected
  provider keeps publication, cache maintenance, MMIO notification,
  posted-write completion, and acquisition private; checked source gains no
  authority to compose their proofs independently. `build.omg` admits the
  exact opaque provider and contract rather than pretending its internal
  protocol is checked Omega.
- That vertical slice derives an exact external loan from the typed mapped
  extent and schema/device correspondence. The installed provider, not build
  selection or source spelling, issues each sealed runtime device/queue/session
  scope occurrence; source cannot construct, inspect, or compare it. A
  pre-commit rejection returns every consumed candidate unchanged. Accepted
  submission yields one linear pending value bound to the per-transfer
  `ExternalLoanId`. Acquisition returns Stable CPU custody plus device status
  only with exact release evidence; otherwise it returns the still-live
  pending loan and completion candidate. Missing coverage rejects compilation
  or installation and has no runtime outcome.
- When a concrete checked driver needs to compose the five protocol roles,
  design their typed source operations from that client and replace the
  provisional uniform row with role-specific payloads. Preserve exact private
  mapping/grant context, schema/device correspondence, runtime scope occurrence,
  and the role discriminant as a canonical-identity input. A role may require
  distinct data, descriptor, doorbell, read-back, request, and completion
  coordinates. Do not infer that eventual ABI from the current fixtures.
- For that future checked-driver surface, bind publication evidence to exact
  range/write state so intersecting writes invalidate it. Acquisition consumes
  request-, loan-, scope-, and instance-bound completion evidence. Terminal Psi
  retains the actual ordering event; erased proof values and generic call
  effects are not lowering barriers.

### Wire runtime and executable installation

- Wire declaration/schema validation now lives in a focused 413-line owner for
  stable numbering and retirement, version-scope and adjacent-era
  compatibility, field-type and bounded-carrier admission, and erased-aware
  nested-cycle rejection. Encode/decode now share a focused 214-line value-
  field owner for nested-message shape matching, exact repeated carrier/
  element/capacity replay, decode range establishment, and named-data
  resolution. Encode-call validation now lives in a focused 400-line owner for
  schema-field classification, runtime-sized order, exact worst-case output
  budgeting, value/schema matching, and output/written argument admission.
  Decode-call validation and the exact `WireVerdict` zero/tag contract now live
  in a focused 351-line owner. Schema-field admission, value matching, range
  establishment, buffer/read/verdict checks, and diagnostic order remain
  unchanged. The natural 130-line root is a dispatcher/test facade; crate APIs,
  protocol identities, and the exact 22-function inventory remain unchanged.
- Extend repeated encode/decode to `Vec<T>` after allocator obligations land.
  Packed scalar decode into `&[T]` remains unsupported because variable-width
  encodings cannot form a zero-copy scalar view.
- The retained selected provider plan, sealed provider execution, exact
  installed entry, and post-handoff writer context now join to the matching AOT
  fragment. That bound invocation now consumes one exact activated mapping plus
  a provider receipt for nonempty write rights, pinning, and non-publication,
  and returns a written-but-still-unpublished destination while failed linear
  transitions return every input. The written carrier retains and hash-free
  replays the complete exact destination image produced by that successful
  invocation before any existing consumer may observe it. Implement consumer
  semantic validation and publication, physical AOT invocation, trusted/PCC
  and final-footprint validators, target W^X/coherence reporting, and
  uninstall/replacement joins.
- Keep arbitrary runtime bytes-to-code, JIT, and raw executable addresses
  unsupported.

Acceptance: only an admitted reusable artifact plus consumed placement authority
can produce installed code; validation binds exact final bytes and placement.

## Blocked index

These are pointers to the owning question or open design item, not duplicate
specifications:

- **SUM-MATERIALIZATION:** tagged-case placement vocabulary in
  `wiki/language_guide/appendix_open_questions.md`.
- **ATOMIC-EVENT-MODEL:** portable atomic axioms and target refinement choices
  in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** public carrier ruling for failure-returning
  checked arithmetic.
- **IMPORTED-CRASH-CAPSULES:** realization/import/certificate identity in
  `wiki/language_guide/appendix_open_questions.md`.
- **MEMBER-REFLECTION:** source surface, constant-position rules, and proof
  visibility in `wiki/language_guide/chapter_14_traits.md`'s
  `build-time-open` footnote.

## Platform-gated verification

- Run the Linux host/time/filesystem and `IntegerAt` metadata paths natively on
  AArch64. x86-64 WSL and cross-target structural coverage already exist; do
  not claim runtime verification without the host.
- Build and run the Windows GUI callback canary through ENT4; do not pass a raw
  code address or add a Win32-only escape.
- Keep unavailable hosts structurally tested and report the missing runtime
  leg explicitly.

## Deferred until a real customer

- cross-package operational recursion: design a compositional published
  termination interface for a call-graph SCC without exposing or inspecting
  any member's private `terminates by` measure; unsupported cross-package SCCs
  continue to reject until that interface exists;
- matching logic as a proof/semantics interchange: under the settled
  subject-qualified operational-refinement root, encode one fixpoint-free
  Terminal Psi obligation, reconstruct its
  theory and goal independently from canonical artifact subjects, and compare
  total trusted bridge size, certificate size, checking cost, constructive
  assumptions, and positive/negative results against the current route; then
  attempt one explicit structural-induction correspondence and one external
  arithmetic-proof import before considering any boot-lattice role; see
  `wiki/design_briefs/matching_logic_proof_research.md`;
- fault-tolerant component restart: define closed-custody component closure,
  explicit owner-death protocols for shared resources, external device reset or
  transaction obligations, and target-supplied isolation evidence together;
  abandonment-frontier reports alone must never license survivors;
- concurrent whole-system composition proofs for deadlock, starvation, memory,
  and response bounds;
- richer measured-recursion guards and multi-subject lexicographic cycles;
- reduced-rational divisibility theory beyond current quotient work;
- asynchronous extent revocation beyond provider quiescence;
- non-blocking executable-visibility tokens;
- runtime-generated host code, JIT, and arbitrary self-modifying code;
- independent final-byte CFI certificates and optional
  CET/PAC/shadow-stack hardening;
- universe levels before a full math-library replay goal;
- reusable fragmented allocation until a growable-container/backend customer
  states its retirement, authority-return, and immediate-reuse demands; and
- an optimizing SSA/register-allocation/SIMD backend beyond current correctness
  requirements.
