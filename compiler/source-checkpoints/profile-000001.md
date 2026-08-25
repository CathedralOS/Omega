# Provisional Ωself profile — checkpoint 000001

Checkpoint 000001 is the first coherent Omega-written product compiler source
snapshot. It implements Psi source custody, final token/lexical-diagnostic
representations, Unicode 17 XID classification, and complete source-to-token
spelling. The hosted adapter reads one source unit, exits with its accepted
status 0, rejects lexical errors with status 251, and rejects source
capacity exhaustion with status 252.

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

The closure has 12 source units, 103 root items, 158,663 source bytes in total,
and a largest source unit of 78,952 bytes. Its root items are 21 data
declarations, 67 machines, four targets, one trait, and ten imports. The machine
surface includes 20 target-qualified machines, 18 `satisfies` clauses, 16
bodyless target leaves, and 16 `Binding::CompilerIntrinsic` realizations. These
forms were invisible in snapshot v1 and are retained candidates required by the
current source, not incidental zero-count possibilities.

The largest observed compositional resources are:

| Resource | Observed maximum | Provisional general ceiling |
| --- | ---: | ---: |
| source units / total bytes / bytes per unit | 12 / 158,663 / 78,952 | 16 / 262,144 / 131,072 |
| root items / data members / variant payload fields | 103 / 42 / 3 | 128 / 64 / 4 |
| machine states / state parameters / state statements | 65 / 7 / 26 | 128 / 8 / 32 |
| call arguments / static arguments / transition arguments | 6 / 2 / 4 | 8 / 2 / 4 |
| path components / identifier bytes | 6 / 49 | 8 / 64 |
| array-literal elements / declared fixed-array length | 806 / 65,536 | 1,024 / 65,536 |
| struct-literal fields / string-literal bytes | 3 / 18 | 4 / 32 |
| normalized expression nesting depth | 7 | 8 |

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
| boundary traits and target-selected realizations | hosted byte input and process exit | retain only the sealed compiler-host byte/exit surface needed by product entrypoints |
| basic explicit generic calls | standard provider selection in the transitive console closure | measure against a non-generic sealed provider binding |
| generated ordinary-Omega data | Unicode XID range arrays | retain generated-source closure rules; generator and external data stay pinned inputs |
| propositions, proof facts, proof contracts, quotients, and proof-program mathematics | unused in checkpoint | reject provisionally; likely final exclusion because implementing full-Omega proof checking does not require proof syntax in compiler source |
| termination/ranking clauses | one ranking clause | retain candidate; ranking is executable compiler control evidence and must not be swept into the proof-surface exclusion |
| dependent bounds and linear types | unused in checkpoint | reject provisionally; likely final exclusion, subject to later source closures |
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

Run `compiler/source-checkpoints/checkpoint-000001.sh`. The checkpoint is
accepted only when all of the following hold:

1. `python3 compiler/source-checkpoints/verify_profile.py` composes the manifest
   gate, replays every target census, validates the domain-separated profile
   digest and exact catalog partition, enforces resource limits, proves every
   profile canary is valid checked Omega, applies admission expectations, and
   rejects the built-in profile mutations.
2. The hosted entry compiles through native emission for its selected target.
3. Empty input, identifiers, integers, punctuation, whitespace, representative
   Omega source with a Unicode identifier, nested block comments, and
   cooked/raw strings accept with status 0.
4. Invalid UTF-8, unterminated nested comments, invalid cooked-string escapes,
   and unsupported punctuation reject with status 251 and publish no token
   observation as success.

The standard gate now enforces the normalized-syntax and resource portion of
this provisional `Ωself` profile. Negative fixtures are valid full-Omega
programs rejected only by profile admission; the positive fixture composes
retained facilities without matching product filenames or exact occurrence
counts. Typed semantic distinctions, ABI/layout, lowering coverage, Delta
capacity behavior, and measured bridge costs remain explicitly unresolved and
are not claimed by this artifact.

The adapter does not yet publish the complete canonical token/diagnostic byte
stream. That observation format and the Rust-comparator differential are the
next product-source checkpoint, not evidence claimed here.

## Measured performance

On the checkpoint host, one 12-source native compile spent about 14% in typed
to checked trees and 84% in control flow to abstract operations; source loading,
parsing, report generation, and native image writing were not the dominant
cost. Unicode data now uses two fixed array literals rather than roughly 1,500
indexed initializer statements, but the current backend still expands the
large aggregate heavily. Follow-up performance work should therefore measure a
general static/generated-data representation and Stage-08 expansion rather
than treating test parallelism or HTML report suppression as the primary fix.
The selected bootstrap slice and its semantic, resource, and refinement
acceptance are tracked in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md); that implementation evidence
does not by itself freeze records, arrays, or generated data into final
`Ωself`.
