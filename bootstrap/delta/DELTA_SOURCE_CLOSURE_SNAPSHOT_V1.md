# Delta source-closure snapshot, version 1

This contract records a deterministic Delta source closure without making a
filesystem path, repository layout, source label, working directory, or
symlink spelling part of source identity.  It is evidence about exact source
and build custody.  It is not package acceptance, compiler authority, a Delta
v1 language freeze, or a claim that the provisional `omega-bootstrap` slice is
the complete bridge.

The canonical transport is UTF-8 JSON with schema
`omega.delta-source-closure-snapshot.v1`.  Objects are key-sorted, use two-space
indentation, and end in one newline.  Unknown or missing fields reject.  All
row IDs are nonempty logical identities; IDs and roles are sorted and unique
where the schema declares a set.  Canonical strings must not contain `/`, `\`,
`.alp`, `.json`, an absolute path, or a `.`/`..` path component.

Repository discovery is a separate diagnostic sidecar with schema
`omega.delta-source-closure-locations.v1`.  Each source locator names a
caller-supplied repository role and a canonical relative path below that role.
The verifier receives role roots explicitly; the shell gate obtains them from
`bootstrap/paths.sh`.  Locators are excluded from both commitments.  Moving or
renaming a source, changing the current working directory, or selecting an
equivalent symlink locator may change the sidecar but cannot change the source
identity or either snapshot digest.

## Snapshot schema

The exact top-level fields are:

```text
schema, snapshot_id, status, claim, profiles, sources, source_edges,
generated_inputs, build_units, tool_artifacts, artifacts, artifact_edges,
content_set_sha256, closure_sha256
```

`status` is `canonical_compiler_root` or `provisional_capability_slice`.
The latter must say only which bounded capability DAG it records and must not
claim a complete `omega-bootstrap` closure.

A profile row contains:

```text
id, kind, target, configuration, abi, resource
```

`kind` is `build_host` or `final_target`.  Build units name one profile of
each kind.  A host on which a Delta tool executes is therefore never confused
with the target/ABI emitted by that tool.

A source row contains:

```text
id, roles, byte_length, sha256
```

The digest covers the exact raw source extent.  A source role does not follow
from a filename.  Source edges contain `from, to, relation`; version 1 admits
only `depends_on`.  The current canonical compiler has no source edge because
it is one translation unit.  Standalone bridge executables likewise remain
separate roots; an artifact handoff does not invent a Delta import.

A generated-input row contains:

```text
id, role, recipe, inputs, byte_length, sha256
```

Each input is `kind, id, ordinal`.  The first recipe is
`ordered-source-bytes-plus-lf-v1`: append each exact raw source followed by one
LF in ordinal order.  This records the Rust on-ramp's translation-unit
transport.  It does not authorize comment stripping, whitespace collapsing,
token rewriting, or any other source normalization.  Other generated inputs
must use a separately versioned recipe before publication.

A build-unit row contains:

```text
id, roles, input_kind, input_id, compiler_tool, output_artifacts,
build_host_profile, final_target_profile
```

`input_kind` is `generated_input`.  `compiler_tool` is either a tool-artifact
ID or `none` for a source-image-only snapshot that makes no compilation claim.
Output artifact IDs are sorted and unique.  A build with outputs must name an
imported compiler tool; a source-image-only build must name neither a tool nor
outputs.

An imported tool-artifact row contains:

```text
id, role, byte_length, sha256, manifest_sha256, build_host_profile
```

The artifact bytes and its separately canonical artifact manifest are both
committed.  A filesystem lookup or loader name is never tool identity.  A
snapshot may contain no imported tool only when every build is explicitly
source-image-only.  This is the bounded first compiler-root milestone; exact
native artifact replay requires a later imported artifact manifest.

On Darwin, ad-hoc code-signature bytes are installation/staging metadata and
are nondeterministic across otherwise identical builds.  The focused V1 bridge
therefore commits the exact unsigned Mach-O content image obtained by the
versioned `detach-adhoc-code-signature-v1` projection.  The gate builds twice,
detaches both signatures, and requires byte identity before using a signed
staging copy.  It never commits a nondeterministic signed hash or treats a
signature-path spelling as executable identity.

An artifact row contains:

```text
id, role, byte_length, sha256, producer
```

`producer` names a build unit or `external` for a committed input carrier.
Artifact edges contain `from, to, relation`; admitted relations are
`materializes`, `runtime_input`, and `produces`.  All endpoints must exist and
the artifact graph must be acyclic.  These are action/data-flow edges, not
source dependencies or compilation authority.

## Commitments and resources

`content_set_sha256` is SHA-256 over the domain
`omega.delta-source-content-set.v1\0` followed, in source-ID order, by
`source\0`, the ID plus NUL, u64 little-endian extent, and exact bytes of every
source, then the same sequence in generated-input-ID order under the group tag
`generated_input\0`.  `closure_sha256` is SHA-256 over
`omega.delta-source-closure-snapshot.v1\0`, u64 little-endian canonical compact
JSON length, and canonical compact JSON with only `closure_sha256` omitted.
The compact projection includes `content_set_sha256` and every semantic row.

Version 1 bounds the canonical manifest and locator sidecar at 65,536 bytes;
128 sources, 512 source/artifact edges, 64 generated inputs, 32 build units,
32 tool artifacts, 128 artifacts, and 32 profiles; 524,288 bytes per source or
generated input and 2,097,152 aggregate committed content bytes.  Malformed,
noncanonical, inconsistent, missing, or digest-mismatched data returns 251.
Crossing a declared resource ceiling returns 252.  Failure publishes no
stdout bytes.

## First snapshots and gate

`source-closures/canonical-compiler-v1.json` binds the exact current
`delta.compiler.lowermachine` raw source and its explicit LF-delimited
translation-unit image.  Its only build unit is source-image-only: it validates
closure machinery without pretending the disposable Rust producer or an
ambient `clang`/`codesign` lookup is a canonical imported compiler artifact.

The intended second snapshot is the explicitly provisional three-root
full-width fixed-buffer capability DAG:

```text
OMGCOMP1 -> OMGRSWA10
OMGCOMP1 || OMGRSWA10 -> OMGLOWJ19 -> CKIR18
CKIR18 -> conservative Linux x86-64 ELF
```

Its roots are the focused resolver, focused lowerer, and focused CKIR18
backend.  It may be published only after all three physical Delta sources,
their contracts, exact tool-artifact manifest, and native/self producer gate
are frozen.  Handcrafted fixture bytes do not satisfy that condition.

`source-closure-snapshot-v1.sh` verifies the canonical snapshot, exact source
and generated extents, relocation/path/symlink invariance, strict mutation and
resource teeth, and no-publication 251/252 behavior.
