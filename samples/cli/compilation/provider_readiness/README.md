# Explicit Build provider readiness

This program exercises PIPELINE-ARCHITECTURE-ENFORCEMENT and TERMINAL-BUILD-SEED:
an actual project Build acquires an explicitly named dependency implementation,
installs its checked Terminal code, resumes the same activation, and joins all
dependency requests before Product compilation.

The root waits for `backing::TestImage` to satisfy the already-known
`ProgramImage` requirement and observes `64`. The dependency also exports
`OtherImage`, returning `128`; there is no default-provider search. A wrong
result attempts a second application declaration and must fail. The executable
exits with zero. `before readiness` and `after readiness` must each appear once
per root Build invocation, including a multi-target compilation.

`namespaced/` selects `chosen::choices::Observations`: the dependency alias,
package name and nested implementation path are deliberately different.
`nested/` makes the backing await another dependency before it can supply the
root. Both retain the same result and four hosted entry bindings.

The intended-invalid projects retain exact refusal oracles:

- `invalid_missing/` and `invalid_private/`: no public implementation at the
  designated path in the acquired dependency.
- `invalid_wrong_requirement/`: the selected implementation satisfies a
  different exact boundary requirement.
- `invalid_unadmitted/`: the implementation calls an unbound external slot;
  installation cannot manufacture a host provision.
- `invalid_cycle/`: nested readiness returns to an already paused source.
- `invalid_replacement/`: another backing cannot replace an installed static role.
- `invalid_product_dependency/`: a Product dependency cannot supply a Build slot.

Run these projects through the ordinary CLI and existing sample gate:

```text
python3 tools/sample_native_gate.py --filter cli/compilation/provider_readiness --jobs 1
omega run --both samples/cli/compilation/provider_readiness/main.omg
```

The gate prepares normal local package acceptance for positive programs and
checks invalid controls without granting acceptance. Use explicit `run --both`
for native/Terminal agreement; the sample gate's ordinary native leg alone
does not establish it.

These are runnable project integration witnesses, not check-only probes. The
legacy corpus source harness statically projects dependencies and cannot drive
this scheduler; moving that harness onto executed project discovery remains
part of the parent cleanup. No readiness-specific harness or source-shape
recognizer substitutes for production execution.

Applied/inherited multi-slot readiness, helper dependencies of unselected
workspace-catalog members, and other-host execution remain unverified. Local
observations and exact producer identities belong in the qualification record,
not a claim that those remaining requirements are complete.
