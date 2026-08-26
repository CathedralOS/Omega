# Provisional Ωself profile — checkpoint 000001

Checkpoint 000001 is the first coherent Omega-written product compiler source
snapshot. It implements Psi source custody, final token/lexical-diagnostic
representations, Unicode 17 XID classification, and the current source-to-token
machinery. The hosted adapter reads one source unit, exits with its accepted
status 0, rejects lexical errors with status 251, and rejects source capacity
exhaustion with status 252.

This is a mechanically enforced provisional profile, not the final `Ωself`
freeze. It records general facilities used by the exact manifest in
`checkpoint-000001.json`; `profile-000001.json` is the canonical admission
artifact and this document explains its evidence and unresolved decisions.
Later product checkpoints rerun the census; the Delta bridge supplies the cost
evidence needed to settle retain-versus-refactor decisions. “Unused” below means
absent from this lexical checkpoint only. Such a facility may be rejected by
this checkpoint's provisional profile, but it is not finally excluded from
`Ωself` while later compiler phases remain unwritten.

The census is now compiler-produced rather than hand-inferred. Run
`omega-source-snapshot --feature-census` against the checkpoint entry and each
declared target. Snapshot schema v3 retains machine target qualifiers,
`bodyless`, `satisfies`/`via`, generic conformance bounds, ranking arguments and
ranges, data `where` facts, cast domain/form, case construction/projection,
local mutability, call/transition qualification flags, and reference lifetimes
instead of silently dropping them. All four
checkpoint targets currently yield the same feature and resource census.

## Exact observed shape

The closure has 12 source units, 115 root items, 178,366 source bytes in total,
and a largest source unit of 78,952 bytes. Its root items are 24 data
declarations, 76 machines, four targets, one trait, and ten imports. The machine
surface includes 20 target-qualified machines, 18 `satisfies` clauses, 16
bodyless target leaves, and 16 `Binding::CompilerIntrinsic` realizations. These
forms were invisible in snapshot v1 and are retained candidates required by the
current source, not incidental zero-count possibilities. Public declarations
are now observed directly through the reconciled toolchain build prelude.

The largest observed compositional resources are:

| Resource | Observed maximum | Provisional general ceiling |
| --- | ---: | ---: |
| source units / total bytes / bytes per unit | 12 / 178,366 / 78,952 | 16 / 262,144 / 131,072 |
| root items / data members / variant payload fields | 115 / 42 / 3 | 128 / 64 / 4 |
| machine states / state parameters / state statements | 64 / 11 / 43 | 128 / 16 / 64 |
| call arguments / static arguments / transition arguments | 10 / 2 / 5 | 16 / 2 / 8 |
| path components / identifier bytes | 6 / 78 | 8 / 128 |
| array-literal elements / declared fixed-array length | 806 / 65,536 | 1,024 / 65,536 |
| struct-literal fields / string-literal bytes | 3 / 18 | 4 / 32 |
| normalized expression nesting depth | 8 | 8 |

The ceilings are rounded, path-independent admission candidates, not exact
closure fingerprints. The profile evaluator now has exact-limit and adjacent-
over-limit mutation teeth for every resource. The ceilings nevertheless remain
provisional until the Delta bridge supplies capacity, exhaustion/publication,
and assurance evidence. The census proves normalized source shape only;
resolution, typing, selected-target completeness, lowering, and runtime
capacity remain separate gates.

| Facility | Checkpoint use | Provisional disposition |
| --- | --- | --- |
| modules and authored dependency aliases | product Psi modules plus hosted `psi` alias | retain candidate; general name/import rules required |
| ordinary named records and nested field access | source units, spans, tokens, streams, lexer state | retain candidate; positional duplication rejected |
| payload-free and payload-bearing sum data | token vocabulary, numeric bases, diagnostics, console reads | retain candidate; general tagged layout required |
| fixed arrays, slices, string/byte literals, runtime indexing | source/decoded/token buffers, Unicode tables, keyword spelling | retain candidate with explicit capacity and exhaustion rules |
| concrete scalar ranges | source/token lengths and standard-library byte results | measure; currently pays directly for bounded indexing without dependent bounds |
| concrete Trapping arithmetic and casts | cursor math, UTF-8/scalar arithmetic, byte conversion | measure against narrow checked helpers in the bridge |
| state machines, scalar state parameters, mutation, and calls | every lexical scan and hosted adapter loop | retain candidate; branching value-machine results are deliberately unnecessary |
| explicit result fields for branching operations | bounded appends and lexical predicates | retain candidate source convention pending general bridge call-cost evidence |
| boundary traits and target-selected realizations | hosted byte input and process exit | retain candidate for the product source forms actually used; do not confuse this Omega source facility with Delta's separately sealed bridge-host interface |
| static provider path arguments | sealed `select_provider<Console, ConsoleNativeProvider>` calls in the transitive console closure | measure against a non-generic sealed provider binding; this checkpoint contains no general generic declarations and does not admit them by implication |
| generated ordinary-Omega data | Unicode XID range arrays | retain generated-source closure rules; generator and external data stay pinned inputs |
| propositions, proof facts, proof contracts, quotients, and proof-program mathematics | unused in checkpoint | reject provisionally for this checkpoint; final disposition belongs to the complete source/bridge join |
| termination/ranking clauses | one ranking clause | retain candidate; ranking is executable compiler control evidence and must not be swept into the proof-surface exclusion |
| dependent bounds and linear types | unused in checkpoint | reject provisionally for this checkpoint; distinguish dependent/proof-indexed forms from ordinary ownership when later source arrives |
| domains and authored generic domain families | unused in checkpoint | reject provisionally; the canary isolates a generic domain declaration, while typed semantic use remains unresolved |
| advanced authored generic constraints | unused in checkpoint | reject provisionally; final disposition awaits later source closures and bridge cost |
| specialization and reflection | no distinct accepted authored syntax to census | no profile claim yet; add a row only when Omega has an accepted source spelling |
| numeric/schema field tags | unused in checkpoint | reject provisionally; compare ordinary named fields if later source introduces them |
| mixed field-plus-case declarations | unused in checkpoint | reject provisionally; compare separate records and sums if later source introduces them |
| inline aggregate transition literals | unused in checkpoint | reject provisionally; aggregate-typed names/calls require typed census evidence and remain explicitly unresolved |

The checkpoint deliberately binds branching computations to fields before
dispatch. This is ordinary Omega and avoids depending on implicit arm-value
materialization in the bridge. It is a source-profile simplification, not a new
language or a semantic exception.

## Functional gate

Run `source/compiler/omega/source-checkpoints/checkpoint-000001.sh`. The checkpoint is
accepted only when all of the following hold:

1. `python3 source/compiler/omega/source-checkpoints/verify_profile.py` composes the manifest
   gate, replays every target census, validates the domain-separated profile
   digest and exact catalog partition, enforces resource limits, proves every
   profile canary is valid checked Omega, applies admission expectations, and
   rejects the built-in profile mutations.
2. The hosted entry compiles through native emission for its selected target.
3. Empty input, identifiers, integers, punctuation, whitespace, representative
   Omega source with a Unicode identifier, nested block comments, and
   consecutive cooked/raw/non-string tokens accept with status 0 and publish
   the exact version-1 lexical observation independently encoded by Rust.
4. Invalid UTF-8, unterminated nested comments, invalid cooked-string escapes,
   unsupported punctuation, the 16,384-token boundary, and the 65,536-byte
   source boundary reject with their specified status while preserving the
   exact diagnostic coordinates, retained source bytes, and completed token
   prefix. A tampered observation must fail the byte comparison.

These observations do not settle conflicting language-surface claims. Unicode
XID identifiers contradict the guide's ASCII-transparent/source-payload-only
wording; the current lexer accepts `\u{...}` and encodes the scalar as UTF-8
while the guide explicitly forbids that escape; raw-string delimiter/content
rules are not normative there. The product source now uses `u64` for byte
coordinates, collection counts, and scan indices throughout this checkpoint,
while Unicode scalar values remain `u32`; no implicit cross-carrier indexing or
`.len` comparison remains. The checkpoint still records the unsettled Unicode
identifier, escape, and raw-string behaviors without treating them as
full-Omega lexical authority. `TASKS.md` owns those remaining language rulings.

The profile artifact's `unresolved_decisions` array records gaps owned by the
profile/bridge join; it is not an exhaustive registry of language-design
questions. The remaining lexical conflicts above remain explicit product-
language blockers even though adding them to the hashed profile would
not settle them. This distinction prevents a bootstrap profile refresh from
silently acting as a language ruling.

The standard gate now enforces the normalized-syntax and resource portion of
this provisional `Ωself` profile. Negative fixtures are valid full-Omega
programs rejected only by profile admission; the positive fixture composes
retained facilities without matching product filenames or exact occurrence
counts. Typed semantic distinctions, ABI/layout, lowering coverage, Delta
capacity behavior, and measured bridge costs remain explicitly unresolved and
are not claimed by this artifact.

The adapter publishes a versioned structural lexical observation beginning
with `OMGLEX1\0` and version 1. It records acceptance, diagnostic identity and
source coordinates, exact retained source bytes, token kind metadata, token
coordinates and raw spelling, and exact decoded string bytes. The encoding is
checkpoint tooling rather than an Omega ABI or enum-layout claim. The Rust
producer independently maps its token vocabulary into the same schema; the
complete gate compares accepted, rejected-prefix, invalid-UTF-8, token-capacity,
and source-capacity observations byte for byte and includes a tamper tooth.

## Measured performance

On the checkpoint host, one 12-source native compile spent about 14% in typed
to checked trees and 84% in control flow to abstract operations; source loading,
parsing, report generation, and native image writing were not the dominant
cost. Unicode data uses two fixed array literals rather than roughly 1,500
indexed initializer statements, while the measured backend still expands the
large aggregate heavily. This is a checkpoint observation, not an active work
order. The selected bootstrap slice and its semantic, resource, and refinement
acceptance are tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md); that implementation evidence
does not by itself freeze records, arrays, or generated data into final
`Ωself`.
