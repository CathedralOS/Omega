# Reuse the completed application review when Cathedral resumes

The package manager already reused unchanged dependency reviews, but checked the
application again after its review decisions were accepted. The repair lets the
application reuse its completed review when every recorded input still matches.
On the frozen Cathedral project, normal resume and lock publication fell from
506.68 seconds to 7.90 seconds (64.11×), with the same reviewed decisions.

## What changed

The existing review cache now includes the application. Root acquisition also
returns the source object carrying the resolver's cache locations, instead of
discarding that returned object. The source resolver still chooses those
locations and validates the original request, immutable content and limits.

Reuse compares the complete existing key: compiler and bundled library identity,
source graph and dependency edges, target and execution profile, selected entry,
Build snapshot, generated inputs, bindings and component descriptions. After
record recovery, the pass rechecks the exact consumed source against current
inputs. Root service audits, complete decision comparison and normal lock
publication remain required. A cached review supplies findings, not native code
or a runtime grant. Application records remain outside the executable component
candidate roster.

A completed Independent component description can differ from the inputs at an
early lookup. That remains a full-key miss; the repair does not omit descriptions
to obtain a hit. Cache format, checksum, size, count and source limits are unchanged.

## Matched customer comparison

Fixed compiler base: `df2275ff1472b31fccc9c0b990636a2cc051196f`.
The optimized baseline and candidate use one frozen Cathedral source copy at the
same path, the same initial lock and byte-identical bundled libraries. Each
producer first prepares its own ordinary review. Their complete generated
review documents are byte-identical, including comparison identity and all
required decisions.

Copying the project changes seven local package identities. Compared with the
previous live-project audit, nine changed policy decisions become removed/added
pairs, producing 255 policy decisions plus one explicit copied-root replacement.
Both complete before/after policy row multisets match the prior audited proposal
after mapping only those explicit package identities. The same 256 decisions are
accepted in both legs through the ordinary editable document.

The original row-only editor missed the separate root replacement decision. Its
baseline resume correctly returned pending without changing the lock after
518.447 seconds. That incomplete attempt is retained and excluded from the
normal-publication comparison. The corrected preparation enumerates every
`decision` category and verifies zero pending decisions before both resumes.

The corrected quiet comparison passed:

| Producer | Resume | Peak resident memory | Each review pass |
| --- | ---: | ---: | --- |
| Baseline | 506.683 s | 6.42 GiB | 1 compiled, 8 carried |
| Candidate | 7.903 s | 1.17 GiB | 0 compiled, 9 carried |

Both exited 0 and published all nine packages. Published `build.omg` bytes are
identical; the complete locks differ only in their nine exact compiler-identity
fields. The candidate uses 1.56% of the baseline time, exceeding the predefined
requirement to halve this unchanged resume. Native image compilation is separate.

Cold preparation times (738.168 seconds baseline, 689.560 seconds candidate)
overlap other work and are not a performance comparison. The candidate's actual application record
has a 594,030-byte payload and a verified 32-byte checksum; its persistence was
observed directly, not inferred from a diagnostic saying only that encoding ran.

## Focused controls

- The unchanged two-package ordinary fixture carries both reviews with zero
  fresh compilations. The baseline still compiles its root.
- Changed root and dependency contracts reject the intended false obligations.
- A new target, changed bundled-library content and changed required/optional
  Build inventory compile fresh records.
- The existing generated-source fixture carries its unchanged review; changing
  only its Build asset from `73` to `74` creates a new key and fresh review.
  Normal publication writes generated source byte-identical to the changed asset.
- Independent composition retains its full-key miss and rejects a changed
  dependency's false guarantee.
- Ordinary and Independent versions both execute with native exit 0 and
  Terminal exit 0. These are normal compilations, not execution of cached reviews.
- Existing compiler-identity key and corrupt/retired-record data tests each pass.
  Scoped package-manager/orchestration Clippy, formatting and repository rules
  pass. Clippy reports one existing unrelated conversion warning in validation.

The public source control reuses
[`captured_source_generated_constant`](../../../tests/fixtures/pass/build/captured_source_generated_constant/).
The ordinary composition control removes only the Independent selection flag in
an ignored copy. No language syntax or contract is weakened to produce a hit.

## Replay and limits

Optimized immutable CLI SHA-256 identities:

- Baseline: `c762fe888070920384e43390771912667da3b9b173957598b3c8b9e5797d0730`.
- Candidate: `1f3fda4e8c8ac07cd766943b904371f30fb903675285d73ed3c733df0a163816`.

Both report optimization level 1. The four submitted acquisition/cache source
files match the frozen candidate exactly. A later independently owned constant
callback repair does not change these consumers; scoped Rust checks include
that shared source without relabeling the earlier timing producer.

Raw source, library, producer, command, review, lock and timing receipts are under
ignored `build/root-review-reuse/`. The source copy records Cathedral HEAD
`2c287634938b9f004d97932bbc9656a42516ffef` and every copied file digest; the manifest,
not the HEAD alone, identifies the frozen working-tree bytes. No live Cathedral
source or lock is changed by the comparison.

This is one matched unchanged-review workflow on this host. It does not promise
a universal speedup, speed up native image compilation or remove any required
package decision. The independent source review covers the complete key and
consumer path, both reuse and invalidation, and the exact focused results.
