# Standalone Omega compiler request

The Epsilon-written bootstrap compiler D and Omega-written compiler C share this
logical request and observable wire contract. Their private representations and
algorithms need not match. Rust compiler objects are not a wire specification.

**Incomplete physical specification:** the outer framing, semantic contents,
commitment preimage, validation order, and publication rules below are settled.
Complete ordered field/tag, failure-code, phase, and scalar-resource tables still
need assignment and implementation under OMEGA-D/OMEGA-C in the
[bootstrap board](../../../TASKS_BOOTSTRAP.md#p4---epsilon-to-omega-and-self-hosting).
Neither implementation may claim a complete interoperable V1 boundary yet.

## Logical inputs

| Section | Contents |
| --- | --- |
| `OmegaCompilationSubject` | Resolved package graph, explicit selected root and role, requester-local aliases/edges, stable package keys with separate immutable resolutions, and complete build-visible snapshots. |
| `OmegaInvocation` | Explicit requested product, canonical target profile, canonical external admissions, and exact subject commitment. |

Bootstrap Alpha tape is an ordinary explicit product, never inferred from a
filename, host, or first discovered entry. Package instances, locks, and source
resolution do not certify compiler artifacts. V1 accepts resolved source, not
preaccepted compiled dependency instances; adding the latter requires a new
request version, not a zero or omitted row interpreted as acceptance.

Discovery, retrieval, ambient traversal, and host defaults are outside the
standalone compiler. Compiler-injected build vocabulary belongs to compiler
identity, not host-supplied source. The compiler derives providers, roots,
generated source, subsystem, and optimization choices by admitted build
execution; the request cannot supply those conclusions. See
[build execution](execution.md) for checkpoint, authority, and dependency handoff.
Completed dependencies contribute durable bundles and evidence, not a graph-wide
collection of live half-compiled checkpoints.

## OCREQ v1 framing

| Offset | Encoding |
| --- | --- |
| 0 | Eight identity bytes: `4F 43 52 45 51 01 00 00`. |
| 8 | Little-endian `u32` subject byte length. |
| 12 | Little-endian `u32` invocation byte length. |
| 16 | Exact subject section, then exact invocation section, then exact end. |

There is one flat version. Sections have no independent nested versions;
representation changes revise the outer version atomically. Variable bytes are
length-framed; tables have explicit counts; graph indices are zero-based;
variants have assigned numeric tags. Reserved slots are zero, including identity
bytes 6–7. Counts, lengths, indices, and coordinate-producing extents are at most
`INT32_MAX`. Language names use their specified UTF-8 grammar; snapshot paths,
file contents, and link targets are raw bytes. Enum order, serde, pointers,
report fingerprints, and private re-encodings cannot define wire bytes.

## Canonical subject

Package rows are strictly ordered by recomputed `PackageKeyIdentity`. A row
contains structural name/lineage, separate exact revision/tree/content
resolution, role, and snapshot. Recompute key identities from structural fields;
a supplied digest does not establish them. Graph references index the validated
table. Dependency rows are ordered by requester index and local alias.

Duplicate, dangling, foreign, unreachable, cyclic, role-inconsistent,
identity-mismatched, and noncanonically ordered rows reject. Alias spelling is
local name-resolution syntax, not package identity; one requester cannot rename
an alias inside another package. [Source acquisition](../packages/sources.md)
defines selection and lineage separately from this encoding.

Each snapshot is a complete deterministic virtual filesystem. Its closed rows
are directories, regular files with executable state and exact bytes, and
symbolic links with exact target spelling. Validate root directory, parent
closure, unique raw paths, metadata-policy limits, and exact aggregate content.
Rows follow raw-path order. Absence is the complement of the complete set;
direct-child enumeration, lengths, and canonical metadata derive from the rows
and payload. Do not serialize those derived facts a second time.

An external-local canonical path in lineage is identity text only: the compiler
never dereferences it. Ambient environment, clocks, randomness, network state,
or a prior build-operation trace cannot replace snapshot facts. BuildOutput
starts as a fresh activation-local tree; any later replay record describes that
execution, not the complete input filesystem.

The invocation's 32-byte subject commitment is:

```text
SHA-256("omega.ocreq.subject.sha256.v1\0" || exact subject-section bytes)
```

Validate outer framing and subject canonicality before checking this binding.
It remains mandatory when the two sections share a frame, and does not replace
local package-key checks. Hashing a reconstructed object is not equivalent.

## Validation and failures

Validate identity/reserved bytes, encoded high bits, and exact end before any
ceiling. Compare each length against the remaining extent subtractively; never
form a potentially trapping `16 + subject_length + invocation_length` sum.
A short stream declaring a huge body rejects; an exact huge frame may exceed a
provision. Then validate capacities, fields, package identities/order, graph,
snapshots, admissions, and commitment before source tokenization and build work.

| Halt tag | Outcome | Publication |
| --- | --- | --- |
| 0 | `Complete` | Unwrapped requested artifact. |
| 1 | `Reject` | Invalid/noncanonical request or ordinary source/build/checking refusal. |
| 2 | `Incomplete` | Named private provision exceeded; no verdict on unexamined source. |
| 3 | `InternalFailure` | Compiler contradiction after input admission; no artifact authority. |

Every failure publishes only its closed OCOUT frame, never an artifact prefix.
The eight-byte identity is `FF 4F 43 4F 55 54 01 00`; the common header is 40
bytes. Its outcome must match the halt tag. Coordinate space 4 alone appends
eight bytes: little-endian `u32` canonical package and source-unit ordinals;
the header's `u64` coordinate holds the source byte offset. Other spaces have
no tail. Unknown codes, illegal code/coordinate pairs, nonzero reserved slots,
and noncanonical tails reject.

Select diagnostics by fixed phase, then request-byte offset for framing, or
canonical package/source-unit order and byte offset for source. Use the first
byte of the primary retained span: declaration start for declaration failures,
token/expression start for those failures, source extent for EOF. A generated
unit failure uses a generated-source reason anchored at the request-resolvable
authored `BuildOutput::include_source` call. Its generated path/internal offset
may appear in local diagnostics, not as an invented request ordinal.

Each private capacity has one named scalar resource and one `(limit, requested)`
pair. Public limits, requested amounts, and primary coordinates lie in
`0..INT32_MAX`; their physical `u64` high words are zero. Every selected limit is
strictly below `INT32_MAX`. Publish nonnegative quantities as
`min(exact quantity, INT32_MAX)`, independent of the selected limit.
Epsilon uses `Exact(nonnegative i32) | Overflowed` with pre-operation checks;
Omega may compute wider exact values and normalize only at publication.

Both compilers use the same diagnostic selection under the same profile.
Differential agreement is an oracle, not a replacement for refinement proofs.
A future explicitly selected smaller bootstrap coverage profile must report
unsupported valid Omega as a named `Incomplete` provision, not invalid source.
A construct count can use limit zero and its actual positive requested count.
This option does not change the currently required full-Omega scope of D.

Closed tables are checked projections of constants embedded independently in
the offline compilers, never runtime host sidecars. Completion requires matching
exact/adjacent vectors, unknown-tag rejection, phase selection, bounded
arithmetic, and Complete-only publication controls as well as all assigned tables.
