# RC-SOURCE-SEMANTICS-CLOSURE — verify + record (z142)

Bare stub at TASKS.md (RC-SOURCE-SEMANTICS cluster, :9805-:9807).
Mines the [RC-SOURCE-SEMANTICS](rust_compiler_completion.md) release-matrix
row's closure state:

> Every accepted positive fixture reaches its promised checked or product
> stage; every negative fixture rejects; individual semantic integration
> tests pass.
> Gate: `mbx nextest run -p compiler --all-targets --no-fail-fast`.

## Measurement at `50559da3ab` (linux x86-64, cargo nextest)

`cargo nextest run -p compiler --all-targets --no-fail-fast` — 3123 tests
across 111 binaries.

**GATE RED — not closed.** Bounded run (same bound convention as
`rc_release_run_linux_x86_64.md`): **1362 run, 723 pass, 632 fail**, 7
terminated in flight on SIGTERM (~95 min wall). Prior full census on this
same gate (RC-NATIVE-MATRIX host-legs record): 3070 run, 1887 pass,
**1183 fail**, 24344s — this run reproduces the same dominant families.

### Failure census this run (by test binary)

- canary_suite: 618
- build_target_activation: 6
- build_config_granted: 3
- callback_terminal_custody: 2
- 1 each: calling_policy_plans, build_log_facet, application_type_equations

Panic-line census: 469 `rejoins 0 Terminal attachment identities`
diagnostics (family 1), 20 `requires a selected Fused provider` (family
3), bundled-windows-entry rejection legs (family 2).

### Failure families (matches the earlier census)

1. **ProgramEntry↔Terminal attachment rejoin** — `selected ProgramEntry
   establishment rejoins 0 Terminal attachment identities; expected one`.
   Dominant canary_suite family; entry/provider surfaces fenced by
   ENTRY-CONTENT-ROOTS (live claim, exp ~09:57Z).
2. **Windows authored-entry contract rejection** —
   `require either the exact bundled Windows x86-64 contract or one
   accepted package-owned Windows x86-64 binding, not
   .../std/targets/windows_x86_64/entry.omg`. The bundled std entry no
   longer satisfies authored-entry admission (observed directly this run,
   e.g. `x86_feature_admission::exact_x86_fma_demand_fails_closed_...`
   205.7s, `aarch64_fma_demand_is_not_an_x86_feature_association`
   154.3s).
3. **Selected Fused provider requirement** — `Service field Main::<field>
   requires a selected Fused provider for boundary ...` on fixture
   machines (Service<R> spelling migration; SELECTED-DISPATCH /
   ENTRY-CONTENT-ROOTS lanes).

## Live fences observed in claims status this wave

- `ENTRY-CONTENT-ROOTS` (Devin / linw2) — entry_settlement,
  service_custody, package authority fixtures; family 1/3 home lane.
- `RC-SOURCE-SEMANTICS-GATE` — sibling stub, Devin / z122 (exp 11:23Z).
- `SOURCE-SEMANTICS-SUITE` — dev-197 (exp 08:47Z).
- `DIVISION-CANARY-ENTRY-BINDING`, `UEFI-PHYSICAL-SEMANTIC-ENTRY` —
  entry-fixture surfaces.
- `COMPOSABLE-PAIR-DESCRIPTORS` — sis2sis selected_lowering/peepholes.

## Verdict

**RED / open.** Gate reproduces the earlier 1183-fail census families at
fresh origin/main; closure requires the entry-attachment rejoin, windows
bundled-entry admission, and fused-provider selection families to land
under their owning lanes. No unclaimed slice exists under the CLOSURE
name — it is the row's release-gate state, not a work slice.
