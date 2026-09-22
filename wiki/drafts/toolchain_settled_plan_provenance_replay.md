# Toolchain-settled provider plans fail candidate provenance replay

Status: diagnosis record with a handoff, not a repair. The fix is a design
choice inside the provider-settlement feature and belongs to that lane.

All measurements on macOS arm64 with ONE pinned `dev` binary built at
`d782d2a5ee`. Cross-run comparison in this family is only valid against a
pinned binary — see the caveat at the end.

## Symptom

An ordinary program that depends on the standard library can fail inside
`omega-language-std` itself, before the program under test is reached:

```
cannot check prepared project: review projection failed for package
`omega-language-std` with 11 diagnostic(s)
  error: ProviderPlan `linux_x86_64::omega::toolchain::filesystem_host::satisfies::FilesystemHost`
         retained realization symbol Handle { arena_index: 0, generation: 0 }
         resolves to 0 exact typed machines
```

An earlier binary reported the same configuration as
`routed service field 'Filesystem::host' has no exact Fused
selected-provider-plan join`. Both are this defect seen from different
sides; the `ProviderPlan` wording is the sharper one.

## Why it happens

`Handle { arena_index: 0, generation: 0 }` is the default, invalid handle,
and that is **deliberate**. `2704dd0edb` ("omega: mint the toolchain-settled
FilesystemHost provider plan") mints the canonical `FilesystemHost` plan from
the reviewed per-target leaf table because that boundary "admits no authored
`satisfies` conformance or `select_provider` row", and its own message states
the minted plan "enters review provenance as a UniqueCoveringCandidate with
**invalid realization symbols**".

`build-evaluation/tests/canonical_filesystem_host_settlement.rs:1-9` states
the intent even more plainly: the minted table joins the selected closure
through `with_toolchain_settled_plan`, **"outside candidate provenance"**.

It does not stay outside. `selection_provenance.rs:219` calls
`validate_derived_provider_plan_provenance` for every selected plan, and
`provenance_replay/candidate_validation.rs:381-399`
(`exact_provenance_realization`) requires each retained realization symbol to
resolve to exactly one typed machine. An invalid handle resolves to zero, so
every minted row rejects — 11 rows, 11 diagnostics.

Two halves of one feature disagree: minting records invalid realization
symbols by design, and provenance replay demands they resolve.

## Why this is not a one-line fix

The precise discriminator already exists — `effects.rs:195`
`SelectedProviderPlanFacts::is_toolchain_settled(plan)` — but it is **not
reachable at the rejection site**.
`selection_provenance.rs:146 selected_provider_plan_facts_with_independent_components`
receives `Vec<SelectedProviderPlanWithProvenance>` and *produces* the
`SelectedProviderPlanFacts` that carries the toolchain-settled identities, so
it cannot ask the question while validating. Someone has to either thread the
minted identities in or mark the row itself.

**Do not key the skip on `ProviderSelectionProvenance::UniqueCoveringCandidate`
instead.** It is tempting: line 226 already does
`UniqueCoveringCandidate => continue` for the declarations check, immediately
after the provenance call. But ordinary unique-covering candidates also carry
that provenance and DO have real realization symbols, so skipping replay for
the whole variant would stop validating plans that should be validated. That
is a weakening of a soundness-adjacent check, not a fix.

## Caveat on measuring this family

`omega --check` on a reduction that fails *inside* `omega-language-std` never
reaches the root program, so such a run cannot answer any question about the
root program. Several bisection variants in the sibling record
([checking_contract_exit_fact_cost.md](checking_contract_exit_fact_cost.md))
are uninformative for exactly this reason.

Worse, a long bisection can straddle a rebase. Two runs of one unchanged
fixture gave `Filesystem::host has no exact Fused selected-provider-plan join`
and then `ProviderPlan ... resolves to 0 exact typed machines` — not
nondeterminism, but ~23 upstream commits and a rebuild between them. Pin one
binary (`cp target/debug/omega /tmp/omega-baseline`), record its commit, and
re-run every row of a comparison against it.
