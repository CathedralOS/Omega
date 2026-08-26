# Bridge compilation envelope, version 1

The bridge source bundle preserves exact labels and source bytes, but labels are
custody metadata rather than package or module authority. `omega-bootstrap`
therefore receives a second, resolver-produced artifact that binds those bytes
to the already reconciled package/source graph. The compiler does not scan
`build.omg`, infer package identity from repository paths, or rediscover mutable
dependencies.

This envelope is a private bootstrap transport. It is not Omega syntax, a
package format, a lock-file replacement, or a stable product ABI. Its package
keys are opaque commitments supplied by orchestration; merely encoding one, or
structurally validating the envelope, grants no authority. Compilation
acceptance is conditional on recheckable package evidence and accepted lock
state that independently establish the exact source closure and commit to the
canonical envelope. A resolver- or compiler-issued verdict is review metadata,
not that evidence. The bridge and lower-rooted checker must compare the SHA-256
digest of all envelope bytes with the expected digest reconstructed from the
accepted closure. A digest carried only inside this untrusted envelope would add
nothing.

Version 1 deliberately carries no boundary/provider bindings and is accepted
only for a selected program whose closed call graph requires none. Structural
pack/verify tools may validate the transport before that semantic authority
join, but must describe that result as structural validity rather than
compilation acceptance.

## Integer and extent rules

All multibyte integers are unsigned little-endian. Counts, lengths, offsets,
and IDs must fit `0..=2^31-1`. Every multiplication and addition is checked
before use. The exact checkpoint-000001 transport ceilings are:

- 1 through 16 source units;
- at most 131,072 content bytes per unit and 262,144 in aggregate;
- at most 64 bytes per source label and 1,024 aggregate label bytes;
- 1 through 16 packages and at most 32 requester-local aliases;
- at most 64 canonical strings and 2,048 aggregate string payload bytes; and
- at most eight `::`-separated components and 64 bytes per component.

The resulting maximum nested bundle is 263,312 bytes. The framing preflight
ceiling for a complete envelope is 267,280 bytes, including 64 string lengths
as well as the maximum string payload and fixed tables. The all-strings-used
relation makes 64 distinct strings unrealizable with only 16 source, 32 alias,
and two root references; this conservative preflight bound does not waive that
later canonicality check.

The relation-dependent effective maximum uses at most 50 string length headers:
`64 + 16*48 + 16*20 + 32*16 + (50*4 + 2048) + 263312 =
267224`. Thus 267,280 remains the simple framing/storage preflight ceiling, not
a claim that a canonical envelope can attain that exact length.

## Current structural implementation

`omega-bootstrap-compilation-check.alp` independently implements this
document's bounded wire, table, graph, string, nested-bundle, resource, and
exact-EOF rules in Delta. Its native/self-built and canonical-Gamma gates
require empty output and exact `0`, `251`, and `252` observations. Exact-bound
fixtures cover the independently realizable ceilings. The 64-string and
267,280-byte exact encodings are relation-dependent impossibilities, while an
aggregate label extent of 1,025 cannot precede the 16-by-64 per-label ceiling;
their preflight/adjacent exhaustion behavior is tested without manufacturing a
false canonical positive.

This checker is structural transport evidence only. It does not consume or
validate the independently supplied SHA-256 commitment, accept a resolver/lock
receipt, resolve source names, inspect source semantics, compare CKIR, or
authorize an emitted artifact. Those joins remain mandatory below.

The companion bounded
[`omega-bootstrap-sha256.alp`](omega-bootstrap-sha256.alp) producer and
[`SHA-256 contract`](OMEGA_BOOTSTRAP_SHA256.md) close exact hashing of any raw
envelope extent through this transport ceiling. Their fixed-vector,
native/self-built, and lower-rung evidence proves only the hash computation and
fixture-digest consistency. The expected digest still must be independently
reconstructed from the future accepted closure; hashing a receipt supplied
beside untrusted envelope bytes grants no authority.

Exceeding a declared ceiling is checked exhaustion (`252`). Malformed,
noncanonical, inconsistent, or unsupported input is rejection (`251`). No
normalized source or artifact may be published on either failure. Arithmetic
overflow while checking a purported encoding is malformed input (`251`), as is
an ID outside the count it references. Once a validated public extent selects
`252`, later inspection may not downgrade that result to `251`. Exact EOF and
all output extents are preflighted before any byte is published.

## Header — 64 bytes

```text
offset  width  field
0       8      magic: ASCII "OMGCOMP\0"
8       u16    schema major: 1
10      u16    schema minor: 0
12      u16    target: 1 = Linux x86-64 System V
14      u16    flags: zero
16      u32    total envelope byte length
20      u32    nested bridge-source-bundle byte length
24      u32    canonical-string-table byte length
28      u32    canonical-string count
32      u32    package count
36      u32    source count
40      u32    alias count
44      u32    selected root package ID
48      u32    selected root source ID
52      u32    selected root owner-name string ID
56      u32    selected root machine-name string ID
60      u32    reserved: zero
```

The fixed tables follow immediately in this order, then the canonical string
table, then one exact version-1 bridge source bundle, with exact EOF:

```text
64 + 48*package_count + 20*source_count + 16*alias_count
   + string_table_byte_length + source_bundle_byte_length
```

The computed size must equal both the header length and exact input length.
There is no compatible-minor-version rule.

## Package row — 48 bytes

```text
u32       dense package ID
byte[32]  opaque nonzero PackageKey commitment
u32       source-row start
u32       source-row count
u32       reserved: zero
```

Package rows are strictly increasing by raw PackageKey commitment bytes; the
row position is the dense package ID. Commitments are pairwise distinct.
Package source spans canonically partition the source table in package-ID order,
and every retained package has at least one source. Package ID does not encode
load order, a cache location, or a source path. The selected root package must
be a dense package ID, and every other package must be reachable from it through
the alias graph.

## Source row — 20 bytes

```text
u32  dense source ID
u32  owner package ID
u32  nested-bundle entry ID
u32  canonical module-path string ID
u32  flags: zero
```

The owner must agree with the containing package span. Within each package span,
source rows are strictly increasing by the raw label bytes of their referenced
nested-bundle entries; the resulting row position is the dense source ID.
Bundle-entry IDs form an exact permutation of `0..source_count-1`, so every
retained byte belongs to one and only one source row. A source label is used
only as a canonical row-order key and diagnostic custody label; it is never
interpreted as package or module identity.

The module-path field is resolver-owned logical placement under the accepted
package source root; it is not inferred from the custody label or cache path.
The empty path denotes the package-root module. When a source contains an
authored `module` item, that declaration must exactly agree with the row; more
than one module item or any mismatch rejects. A source without such an item
uses the resolver-owned row rather than silently becoming a root module. This
supports exact deterministic placement while the product closure migrates away
from legacy filename-derived loading, without granting the label semantic
authority.

Multiple files may contribute to one module. Their semantic order is source-ID
order followed by authored declaration order. Duplicate semantic declaration
identities reject, and resolution may not depend on traversal or load order.
The selected root source must belong to the selected root package.

## Alias row — 16 bytes

```text
u32  requester package ID
u32  local-alias canonical string ID
u32  target package ID
u32  reserved: zero
```

Rows are ordered by requester ID and then raw alias bytes. An alias uses the
canonical package-alias grammar: it begins with `a` through `z`, continues with
lowercase ASCII letters, digits, or `_`, and contains neither `__` nor a
trailing `_`. It is unique within its requester and cannot target the requester
itself. The directed package graph must be acyclic. Every non-root package must
be transitively reachable from the selected root for closure validation, but
source in one package may name only that package's own direct alias rows. It may
not acquire reach by walking another package's aliases. The same target may
have different aliases in different requesters; aliases are local names and
never package identity.

## Canonical string table

The table contains exactly `string_count` entries:

```text
u32    byte length
bytes  exact ASCII payload
```

Entries are unique and strictly increasing by raw byte order. Entry ID is its
dense table position. Every string must be referenced by a source row, alias
row, or selected-root field; unused strings reject. A string referenced in more
than one role must satisfy every role's grammar. The empty string is valid only
when all of its references are package-root module paths. All other strings are
either one identifier or a canonical `::`-separated path. Identifiers use ASCII
letters or `_` initially, then ASCII letters, digits, or `_`, with a maximum of
64 bytes. Alias references additionally obey the narrower package-alias grammar
above. The table extent, strict order, uniqueness, use set, path component
count, component size, and aggregate payload ceiling are checked independently.

The selected root owner and machine entries must each be one identifier. They
name an authored machine; the compiler must resolve that exact symbol rather
than infer an entry from source order or a zero-parameter heuristic. Version 1
requires exactly one such declaration in the selected source and asserted
module, with a mutable or shared attached receiver, no explicit parameters, a
scalar `u8`, `u32`, or `bool` result, and no boundary/provider requirement. Its
CKIR entry block has zero block parameters. An overload or a declaration found
only in another contributing source rejects. This is the existing CKIR1
conformance-entry profile, not the final product `ProgramEntry` ABI. A later
entry profile may add an explicit normalized-signature selector instead of
weakening this uniqueness rule.

## Nested source bundle

The remaining bytes are one exact
[`OMEGA_BOOTSTRAP_BUNDLE.md`](OMEGA_BOOTSTRAP_BUNDLE.md) version-1 bundle.
Its source count must equal the envelope source count. Its canonical entry order
defines bundle-entry IDs. Each label and content extent must also satisfy the
profile ceilings above. UTF-8 and tokenization are validated independently
within each source extent; no delimiter or token may cross a source boundary.

The resolver/orchestrator is an untrusted encoder. Acceptance requires the
independent resolver/lock commitment above and an independent lower-rung
envelope reconstruction, then source-level checking. A first path component
that could denote both a same-package top-level module and a dependency alias is
ambiguous and rejects. Each `use` resolves only through the requester's direct
alias rows or its own modules; transitive-only reach rejects. Cross-package
imports additionally require `pub`, while same-package private access follows
the ordinary module visibility rules. Missing, duplicate, inaccessible, and
otherwise ambiguous names reject.

Resolved semantic order is package/source order above, then authored
declaration order; fields, states, and body operations retain authored order.
That order supplies global record and machine IDs to the private CKIR. The
lower-rooted checker independently parses every participating source, resolves
symbols, reconstructs the order, and compares every CKIR row as well as the
selected root and emitted artifact.

CKIR1 itself has no package/module rows and no machine-call operation. The first
two-unit artifact may therefore erase package/module names only after resolution
and may import a nominal data declaration or another form CKIR1 already
represents. A cross-unit machine call requires an explicit versioned CKIR
widening and corresponding backend and checker work. Those semantic joins—not
possession or structural validation of this envelope—grant compiler authority.
