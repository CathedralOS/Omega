# Standalone Omega compiler request

The Epsilon-written bootstrap compiler D and Omega-written compiler C share this
logical request and observable wire contract. Their private representations and
algorithms need not match. Rust compiler objects are not a wire specification.

**Incomplete physical specification:** the outer framing, semantic contents,
commitment preimage, validation order, publication rules, outcome frame
layout, coordinate spaces, diagnostic phases, scalar-resource table, the
subject/invocation field/tag tables, and the assigned `Reject` inventory
below are settled. The syntax `Reject` inventory — including its distinction
granularity — and the remaining checking codes still need assignment, the
named `Incomplete` coverage provisions still need selection, and the semantic
phases over the decoded fields still need implementation, under
OMEGA-D/OMEGA-C in the
[bootstrap board](../../../TASKS_BOOTSTRAP.md#p4---epsilon-to-omega-and-self-hosting).
Neither implementation may claim a complete interoperable V1 boundary yet.

## Logical inputs

| Section | Contents |
| --- | --- |
| `OmegaCompilationSubject` | Resolved package graph, explicit selected root and role, requester-local aliases/edges, stable package keys with separate immutable resolutions, and complete build-visible snapshots. |
| `OmegaInvocation` | Explicit requested product, canonical target profile, canonical external admissions, and exact subject commitment. |

Bootstrap Alpha tape is an ordinary explicit product, never inferred from a
filename, host, or first discovered entry. Its product tag does not require the
Rust reference compiler to implement Alpha compilation; the
[bootstrap contract](../../../bootstrap/CONTRACT.md#selected-execution-chain)
places that obligation on D and C. Package instances, locks, and source
resolution do not certify compiler artifacts. V1 accepts resolved source, not
preaccepted compiled dependency instances; adding the latter requires a new
request version, not a zero or omitted row interpreted as acceptance.

External admissions cover the assumptions and authority required by the requested
compilation, including actual build execution. Ordinary artifact production does
not require ecosystem receiving-policy input or claim permission to execute the
result. Explicit receiver admission remains a separate operation under
[service permission](permissions.md#artifact-production-versus-receiver-admission).

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
starts as a fresh activation-local tree and is validated after its one execution.

### Subject field/tag table

The subject section is one positional field sequence; there are no skipped or
self-delimiting fields. Every `u32` is little-endian and at most `INT32_MAX`.
Every `bytes` field is a `u32` length followed by that many bytes. Every table
is a `u32` count followed by that many rows in the order its rule assigns.
The section layout is:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | `packages` | `u32` count, then that many package rows |
| 2 | `edges` | `u32` count, then that many dependency edge rows |
| 3 | `root_package` | `u32` index into the package table |
| 4 | `root_role` | `u32` tag: 1 `package`, 2 `application` |

The selected root is a package index plus its role because roots and entry
selection are derived by admitted build execution, not supplied by the
request. The explicit root role must agree with the root row's declared
`role` field; a workspace is never a root role.

Each package row is, in order:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | `name` | `bytes`, the declared package name |
| 2 | `lineage_kind` | `u32` tag: 1 `external_local`, 2 `git` |
| 3 | `lineage` | `bytes`, canonical lineage identity text |
| 4 | `revision` | `bytes`, exact selected revision resolution |
| 5 | `tree` | `bytes`, exact tree resolution |
| 6 | `content` | `bytes`, exact content resolution |
| 7 | `member` | `bytes`, member projection path; empty selects the repository root |
| 8 | `role` | `u32` tag: 1 `package`, 2 `application`; the row's declared role |
| 9 | `snapshot` | `u32` count, then that many snapshot rows |

The name, lineage, and resolutions supply the structural fields the
`PackageKeyIdentity` recomputation and ordering rules consume. Resolution and
lineage payloads are opaque custody text to the compiler; their
canonicalization is acquisition-owned.

Each snapshot row is, in order:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | `kind` | `u32` tag: 1 `directory`, 2 `regular_file`, 3 `symbolic_link` |
| 2 | `path` | `bytes`, the row's raw path |
| 3 | payload | `directory`: none; `regular_file`: `u32` `executable` tag (0 or 1) then `bytes` content; `symbolic_link`: `bytes` target |

Each edge row is, in order:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | `requester` | `u32` index into the package table |
| 2 | `scope` | `u32` tag: 1 `product`, 2 `build` |
| 3 | `alias` | `bytes`, the requester-local alias spelling |
| 4 | `target` | `u32` index into the package table |

Edge rows are ordered by requester index, then local alias, then scope.

### Invocation field/tag table

The invocation section is likewise one positional field sequence:

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | `product` | `u32` tag: 1 `check`, 2 `terminal_artifact`, 3 `native_artifact`, 4 `alpha_bootstrap_tape` |
| 2 | `target_profile` | `bytes`, the canonical target profile name |
| 3 | `admissions` | `u32` count, then that many admission rows |
| 4 | `subject_commitment` | exactly 32 bytes, the binding below |

Each admission row is a `u32` `kind` tag then `bytes` `data`. No admission
kind is assigned yet, so a canonical V1 request's admission table is empty
until this contract gains entries; a present row's unassigned kind rejects at
phase 6 like every other unassigned tag.

Field/tag shape checking is phases 2 and 6: extents, counts, `u32` bounds,
variant/tag membership, and the exact section end. Content rules — name and
alias grammar, package-key recomputation and strict ordering, graph checks,
snapshot row semantics, admission checks, and the commitment binding — remain
in their assigned phases.

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
| 3 | `InternalFailure` | Compiler malfunction or implementation-invariant violation after input admission; no artifact authority. |

Every failure publishes only its closed OCOUT frame, never an artifact prefix.

The requested product includes normalized Build's independent
[Psi/native PCC selections](../proofs/publication.md). `Complete` must deliver
every requested artifact/sidecar pair; an uncertified artifact is not a fallback.
The existing single-artifact payload does not define multi-file framing: extend
the owning product schema explicitly, without concatenating unframed files or
inventing halt tags. Search exhaustion is `Incomplete`, an invalid certificate
is `Reject`, and deriving a contradiction from admitted logical premises is not
by itself `InternalFailure`.

The failure frame's eight-byte identity is `FF 4F 43 4F 55 54 01 00`; its common header is 40
bytes. Its outcome must match the halt tag. The assigned common-header layout is:

| Offset | Encoding |
| --- | --- |
| 0 | Eight identity bytes: `FF 4F 43 4F 55 54 01 00`. |
| 8 | Outcome tag `u8`; equals the process halt value. |
| 9 | Coordinate space `u8`, from the assigned table below. |
| 10 | Two reserved zero bytes. |
| 12 | Outcome code, little-endian `u32`. |
| 16 | Primary coordinate, little-endian `u64`. |
| 24 | Selected limit, little-endian `u64`. |
| 32 | Requested amount, little-endian `u64`. |

Each outcome tag owns an independent code table. `Reject` codes name
request/source refusals; `Incomplete` codes are the scalar-resource table
below; `InternalFailure` codes name named invariant violations. Assigned codes
so far:

| Tag | Code | Name | Space | Coordinate | Limit/requested |
| --- | --- | --- | --- | --- | --- |
| 1 `Reject` | 1 | `malformed_request` | 1 request | first missing, incorrect, or trailing request byte under the validation order | zero/zero |
| 1 `Reject` | 2 | `invalid_utf8` | 4 canonical source | first byte of the malformed scalar's retained span | zero/zero |
| 1 `Reject` | 3 | `outside_lexical_profile` | 4 canonical source | first byte of the rejected spelling | zero/zero |
| 1 `Reject` | 4 | `unterminated_block_comment` | 4 canonical source | first byte of the comment's opening `/*` | zero/zero |
| 1 `Reject` | 5 | `unterminated_string_literal` | 4 canonical source | first byte of the string's opening quote | zero/zero |
| 1 `Reject` | 6 | `unterminated_string_escape` | 4 canonical source | first byte of the enclosing string's opening quote | zero/zero |
| 1 `Reject` | 7 | `unsupported_escape` | 4 canonical source | first byte of the escape sequence | zero/zero |
| 1 `Reject` | 8 | `unterminated_hex_escape` | 4 canonical source | first byte of the escape sequence | zero/zero |
| 1 `Reject` | 9 | `invalid_hex_escape_digit` | 4 canonical source | first byte of the offending digit | zero/zero |
| 1 `Reject` | 10 | `duplicate_name` | 4 canonical source | first byte of the later declaration's name span | zero/zero |
| 1 `Reject` | 11 | `missing_entry` | 0 none | zero | zero/zero |
| 1 `Reject` | 12 | `integer_literal_out_of_range` | 4 canonical source | first byte of the refused literal | zero/zero |
| 3 `InternalFailure` | 1 | `invariant_violation` | 3 internal row | implementation-owned row identity | zero/zero |

`Reject` outcomes carry zero limit and requested fields. `InternalFailure`
outcomes likewise carry zero limit and requested; the coordinate identifies an
implementation-owned internal row, not a source or request offset. The lexical
inventory above is complete: every lexical refusal names one of codes 2–9 at
the first byte of its retained diagnostic span under canonical-source
coordinates, and codes 10–12 name the declaration, entry, and literal
refusals. The per-diagnostic `Reject` inventory for syntax and the remaining
checking failures is assigned with the semantic phases over the decoded
fields, since its coordinates depend on subject package/source-unit binding;
diagnostics whose codes remain unassigned stay unpublished.

Coordinate spaces:

| Space | Meaning | `u64` coordinate | Tail |
| --- | --- | --- | --- |
| 0 | none | zero | none |
| 1 | OCREQ request | request byte offset | none |
| 2 | emitted artifact | artifact byte offset | none |
| 3 | internal row | implementation-owned row identity | none |
| 4 | canonical Omega source | source byte offset | eight bytes |

Coordinate space 4 alone appends eight bytes: little-endian `u32` canonical
package and source-unit ordinals; the header's `u64` coordinate holds the
source byte offset. Other spaces have no tail. The frame's total extent is
exactly 40 bytes for spaces 0–3 and exactly 48 bytes for space 4; any other
extent is noncanonical. Space 0 requires a zero coordinate; spaces other than
4 require zero tail ordinals in the producing record and emit no tail bytes.
Unknown codes, illegal code/coordinate pairs, nonzero reserved slots, and
noncanonical tails reject.

Select diagnostics by fixed phase, then request-byte offset for framing, or
canonical package/source-unit order and byte offset for source. The fixed
phases, in selection order, are:

| Phase | Stage |
| --- | --- |
| 0 | request framing: identity, reserved bytes, encoded high bits, section extents, exact end |
| 1 | capacities: declared section extents and table-count provisions |
| 2 | subject fields: per-row field/tag shape |
| 3 | package identities and strict ordering |
| 4 | dependency graph: indices, duplicates, dangling, unreachable, cyclic, role, alias |
| 5 | snapshots: VFS row order, parent closure, unique paths, metadata limits, content |
| 6 | invocation fields: product, target profile, admission structure |
| 7 | admissions: canonical external-admission checks |
| 8 | subject commitment: the 32-byte binding above |
| 9 | lexical |
| 10 | syntax |
| 11 | checking |
| 12 | product lowering and emission |

A phase is a diagnostic-selection order, not a serialized frame field. An
`Incomplete` or `InternalFailure` outcome belongs to the phase whose check
produced it. Use the first
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

The scalar-resource table assigns one `Incomplete` code per named private
capacity:

| Code | Resource | Limit | Space | Coordinate |
| --- | --- | --- | --- | --- |
| 1 | `request_subject_bytes` | 67,108,864 | 1 request | subject length field, request offset 8 |
| 2 | `request_invocation_bytes` | 1,048,576 | 1 request | invocation length field, request offset 12 |
| 3 | `parser_roots` | 4,096 | 4 canonical source | first byte of the refused root |
| 4 | `parser_states` | 4,096 | 4 canonical source | first byte of the refused state |
| 5 | `parser_path_members` | 16,384 | 4 canonical source | first byte of the refused member |
| 6 | `parser_data_members` | 16,384 | 4 canonical source | first byte of the refused member |
| 7 | `parser_type_nodes` | 16,384 | 4 canonical source | first byte of the refused type |
| 8 | `parser_type_depth` | 128 | 4 canonical source | first byte of the refused type |
| 9 | `parser_expression_depth` | 128 | 4 canonical source | first byte of the refused construct |
| 10 | `parser_statements` | 16,384 | 4 canonical source | first byte of the refused statement |
| 11 | `parser_expressions` | 16,384 | 4 canonical source | first byte of the refused expression |
| 12 | `tape_payload_bytes` | 16,777,212 | 2 emitted artifact | payload offset at refusal |
| 13 | `tape_fixups` | 1,864,134 | 2 emitted artifact | instruction start of the refused fixup |
| 14 | `tape_labels` | 16,777,212 | 2 emitted artifact | payload offset at allocation |

The request-extent provisions bound the declared subject and invocation
section lengths; both sit inside the fixed request header and are checked at
phase 1 after complete framing, before either section is interpreted. The
parser capacities are private D budgets over its fixed tables, not Omega source
limits. The tape capacities bound the emitted artifact's payload and
relocation records; their coordinates are payload-relative byte offsets.
Resource names, limits, and coordinate rules in this table are the same for
both implementations; a private capacity not yet represented here is added by
extending the table, never by reusing another resource's code.

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
