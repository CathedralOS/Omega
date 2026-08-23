# Design Brief: Package Manager First Draft

Status: draft, 2026-08-23. This is a small starting point for the Cargo-like
service under the `omega` binary. It should be folded back into
`build_and_package_model.md` once the command shape and lock schema settle.

## Intent

Omega needs a package workflow with the convenience of `cargo add` and
`cargo update`, but without a hosted package registry and without ambient trust.
The package manager fetches source from user-named locations, pins exact
content, compiles packages to derive normalized capability evidence, and refuses
updates that change that evidence until a reviewer accepts the diff.

This tool is part of the compiler orchestration surface. It must not become a
parallel language, parser, policy engine, or package registry.

## Existing Constraints

- A package is a directory with `build.omg`.
- The package is the dependency reach boundary. A package can name only aliases
  declared by its `build.omg`.
- Pins live in `build.omg`; the lock artifact is machine-written resolved
  evidence, not a second authored dependency file.
- `reaches` is a symbol-resolved boundary-service ceiling. It is separate from
  authority possession, provider trust, suspension, blocking, crashes,
  termination, resource contracts, and recoverable failure.
- Capability authority flows through ordinary values plus domain evidence.
  Current compiler evidence already reports `uses`, `stores`, `acquires`,
  `returns`, and `derives`.
- Dependency source retrieval is resolver-owned authority. Downloaded package
  code does not inherit resolver authority or the root package's build-host
  providers.
- Package policy admits the final transitive reachable-authority set, not each
  dependency edge in isolation.

## Source Model

The first implementation should accept:

```text
omega install <alias> <url-or-path> [--rev <rev>]
omega update [alias...] [--to <rev>]
omega audit packages
```

The source value should be transport-neutral. GitHub, GitLab, HTTPS, SSH, and
local paths are just ways to obtain bytes. Identity comes from the resolved
commit/tree/content hash and the normalized package evidence, not from the URL.

Mutable refs may be accepted as an update convenience, but a build consumes the
resolved immutable identity. Release policy can reject mutable-ref provenance
even when the lock records the resolved commit.

For the first slice, disable or fail closed on Git submodules unless each
submodule is represented as its own pinned dependency edge. Archive support
should wait until path containment, size limits, symlink handling, and expansion
receipts are part of the resolver evidence.

## Package Capability Manifest

Each resolved package should produce one normalized manifest. It is derived by
the compiler; package authors do not hand-write it.

Suggested fields:

- package identity and source identity;
- dependency aliases and exact source pins;
- public API contract identity;
- exported service-reach ceilings;
- build-machine service reach, observation ceiling, realized observation class,
  and receipts;
- provider requirements, provider selections, provider origins, and trust
  receipts;
- routed qualification routes and accepted establishment evidence;
- capability-flow facts grouped by capability and verb;
- synchronous invocation, suspension, blocking, crash, and termination
  summaries for published boundaries;
- unresolved installation-bound rows and their upper bounds;
- reproducibility verdicts for replay-from-record and rebuild-from-source.

The manifest fingerprint is the default update guard. If the fingerprint
changes, `omega update` rejects before mutating `build.omg` or the lock.

## Update Policy

Default update policy:

1. Resolve the candidate source to immutable content.
2. Build or check the candidate in package-admission mode.
3. Derive the candidate package capability manifest.
4. Compare old and new manifests.
5. Update the source pin only if the manifest is unchanged.
6. Otherwise reject and print a capability diff.

This is stricter than "only reject added capability" by design. Removing a
capability can be an API or deployment compatibility change, and moving a
provider/trust receipt can change the TCB even if the service row is the same.

The acceptance path should be explicit, for example:

```text
omega update <alias> --accept-capability-change <receipt-reason>
```

Exact spelling is unsettled. The important rule is that acceptance records the
old and new source identities, old and new manifest fingerprints, reviewer
identity if available, the diff, and the reason. The package being updated
cannot approve its own imported claims.

## Audit Guidance

The diff should rank changes by supply-chain risk:

- critical: new root authority, executable installation, dynamic loading,
  process control, DMA/IOMMU, interrupt publication, signing, secret access, or
  provider-owned backing;
- high: new filesystem, network, environment, clock/randomness without
  replayable receipts, wider build-host service reach, or new opaque provider;
- medium: new synchronous invocation, suspension/blocking, crash route,
  provider origin, routed qualification, retained/stored capability, or
  returned/acquired authority;
- low: removed capabilities, narrower operational ceilings, or public API
  shape changes that still require dependent review.

Human or LLM audit guidance should be concrete:

- inspect the dependency path that introduced the capability;
- read the changed `build.omg` and public boundary declarations first;
- review provider/trust receipts and any accepted opaque implementation;
- check whether capability-bearing values are stored, returned, or derived;
- prefer splitting optional powerful behavior into a separate package so most
  dependents can keep a smaller reach set.

## `build.omg` Relationship

The command should edit `build.omg` as the authored source of truth. A likely
future API shape is:

```omega
machine build(builder: &mut Build) {
    builder.dependencies.bind(
        "json",
        Source::Git(
            "https://github.com/example/json.omg",
            commit("012345...")
        )
    );
}
```

The precise library spelling is not fixed here. The durable requirements are
that the alias is package-local, the source is pinned, and the resolved lock
records the compiler-derived capability manifest.

## Placement

The current Rust on-ramp should add the first resolver/orchestration code under
`bootstrap/onramps/omega-rust/omega/orchestration/omega-packages/`. The CLI
under `apps/omega-cli` should remain thin: parse `install`, `update`, and
`audit packages`, then call the orchestration API.

Long term, the Omega-written product implementation belongs under
`compiler/omega/`, with the same ownership split: Psi derives language evidence;
Omega resolves, admits, audits, installs, and emits artifacts.

## Open Questions

- Exact `Build` library method names for dependencies.
- Whether acceptance should live in `build.omg`, `omega.lock`, or a separate
  signed review receipt referenced by the lock.
- How much source editing the first CLI should do versus printing a proposed
  patch when `build.omg` uses unsupported patterns.
- Workspace-level shared pins and ceilings.
- Whether an install should run dependency `build.omg` immediately or first
  perform a static manifest preflight with no build-host providers.

## First Test Packages

The implementation should start against local package directories before adding
remote Git transport. Use deliberately small packages. External package
identity names, repository names, and lock package keys use hyphen-separated
lowercase words. In-code aliases use the target language's identifier spelling
when needed, usually underscore-separated Omega identifiers. The resolver must
canonicalize package identity spelling so `foo-bar` and `foo_bar` cannot name
different packages in the same package namespace.

- `arithmetic-kernels`: pure checked library;
- `generated-table`: generated-source library with scoped `build.omg` file
  read/write;
- `file-journal`: library that reaches filesystem;
- `network-overreach`: library that declares unused network reach;
- `axiom-ledger`: library with an accepted axiom/boundary claim;
- `provider-switchboard`: provider-selection library;
- `capability-vault`: capability-flow library;
- `graph-workbench`: transitive root wrapper that exposes the dependency path
  for every new capability.

These packages are the acceptance corpus for `install`, `update`, and
`audit packages`. After local resolution works, mirror the same packages as
Git repositories under `CathedralOS` and pin tests to exact commits.
