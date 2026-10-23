# Placement discovery from resolver-owned call inputs

The projected resolver preserves the measured placement demands while constructing
about 80% fewer expression rows in this early pass. Seventeen differential
comparisons preserve selected call declarations and complete ordered placement
records. Eight programs produce byte-identical native images and run successfully
in native and Terminal execution with the projected demands. These measurements
support retaining the change; production qualification is recorded separately
below. They do not establish a whole-compiler speedup or peak-memory reduction.

## Mechanism and boundary

The [dependency audit](../integration/discovery-audit.md) proposed extending the
existing resolver-owned lexical-input construction. The implementation indexes
declarations, retains preceding local headers and type/index inputs, and lowers
outer call expressions with their complete receiver and argument dependencies.
Nested calls use ordinary expression lowering and symbol selection. Non-call runtime expressions
and unrelated executable state bodies are not materialized for this product.

Call roots retain their machine owner through the **existing** conformance and
target-family routing visitors. The same declaration and expression symbol
selection, constant settlement, normalization and prototype validator remain in
use. This is a preparation-only product, not a second name resolver, body-typing
result or execution authorization. Normal receipt-required helper completion
remains part of settlement; ordinary final resolution and checking still run.
The invocation authority checks in placement admission are unchanged.

The two lexical preparation demands are explicit: type operands and call results.
Construction scratch never becomes executable state statements. Full resolution
and ordinary deferred completion pass an empty additional-root set to the shared
routing visitors. The existing full syntax-forest clone at the preparation
boundary remains; this change removes body materialization inside that clone.

The source audit also checked the original `body_is_present` flag: declaration
indexing retains it, and sealed factory validation reads it independently of
whether implementation statements have been materialized. Deferred bodies do
not masquerade as authored bodyless factories.

## Observed comparison

Both diagnostic producers start from
`b1493daf54dddd0257cac96412eda8df0b5b669f`, with their exact patches and effective
optimized profiles retained. The [summary](summary.json) links the counts to the
raw observations.

| Representative input | Full expression rows | Projected rows |
| --- | ---: | ---: |
| length/local | 5,575 | 1,078 |
| dynamic dispatch/local | 5,559 | 1,074 |
| nested record/local | 5,601 | 1,092 |
| returned placed view | 5,613 | 1,097 |
| forwarded placed view | 5,615 | 1,098 |

The [initial diagnostic](initial/instrumentation.patch.gz) compares complete ordered
selected-argument records: selected declaration source identity and generated
syntax, including use spans, closed policy/schema identities, template identity
and multiplicity. Both outputs begin from clones of the same syntax forest, so
generated child spans are comparable. All 14 comparisons match. This producer
always returns the full resolver's selections downstream and cannot establish
candidate runtime acceptance.

The [routed diagnostic](routed/instrumentation.patch.gz) additionally carries call
owners through final routing and compares the exact selected machine/state
source identity for every contributing call. Its `project` mode returns the
projected placement selections downstream. All 17 reached comparisons match;
checking outcomes and actual refusal diagnostics also match. It covers the
original five positive workloads, five intended refusals, nested results,
call custody, target-specific wrappers, owned/inherited conformance routing,
constant initializer machinery and the baseline failures below.

The eight successful runtime cases are length, dynamic dispatch, nested record,
returned view, forwarded view, nested view, target-return wrapper and inherited
conformance-return wrapper. Each has an actual standalone native exit of zero
and native/Terminal agreement in both full and projected runs. Native hashes
match per program. The target-return program runs its portable main; its unrequested target wrappers
establish resolution/checking only, not UEFI execution. The extra target and
conformance witnesses retain the same
placed-view operation and authored exit oracle; they test call selection's
ownership dependencies, not a new language capability.

## Refusals and limits

The five intended controls still reject wrong backing, overlapping loans,
incorrect attached policy ownership, attempted mutable access from shared
storage, and invalid length subtraction.

Additional observations are kept distinct from passing coverage:

- The existing policy-owner fixture rejects in both modes because `schema` is
  transferred before a later field read. Its selection records match, but it is
  not a passing checking or runtime witness.
- The projected-receiver draft rejects retired value-qualified machine-call
  syntax. Both modes preserve that refusal. It is a negative control, not
  evidence that the language admits value-qualified calls.
- The constant helper using placed reads/writes rejects before discovery because
  its accessors are not yet selected. The separate constant-capture draft refuses
  the factory requirement's build-time admission. Neither produces a differential
  discovery record or supplies runtime acceptance. The ordinary machine-constant
  fixture does reach discovery and has matching empty placement results.

The first regression-test comparer incorrectly assumed every selected call
names a machine; a conformance call can name a trait requirement. Its failed
attempt is preserved, and the comparer now uses the selected declaration's kind,
source identity and parent identity. A second attempt used a source-free request, which intentionally omits symbol
source metadata. The tests now carry a real SourceMap and compare those retained
declaration identities. Neither correction changes compiler selection.

No timing or memory claim is inferred from row counts. Builds and captures share
an active host; recorded wall times are diagnostic context. No cross-host or
full-corpus acceptance is asserted.

## Replay and research

`initial/` and `routed/` retain producer profiles, exact patches, capture/build
scripts and compressed raw records. `runtime.py` retains the actual compilation,
standalone execution and dual-engine commands. `witnesses/` preserves all
exploratory sources, including failed drafts. Replaying a diagnostic requires an
isolated checkout of its recorded base, decompressing and applying that directory's patch, copying
the witnesses to `build/placed-result-selection/witnesses`, and creating the
script's named output directory before building through `tools/build_slot.py`.
Use its freeze script before capture; never replace a historical executable.
The observed builds use optimization level 1 and two Cargo jobs.

Research reuses the [audit's primary-source log](../integration/discovery-audit.md#research-log)
and the [earlier query-dependency research](../placed_discovery/research.md).
The decisive local question was the dependency on lexical scope and body-owned
routing; the experiment adds neither query caching nor an independent selector.

## Production qualification

The production implementation removes diagnostic environment switches and the
full-resolver comparison route. Focused Rust tests compare actual call identities
for nested/local inputs, target siblings and owned/inherited conformances while
asserting that unrelated implementation bodies remain deferred. All four resolver tests pass (three new comparison cases and the retained
namespace case); scoped Clippy exits zero with warnings. Final production build,
integrated-main replay, qualification and landing receipts will complete this record.
