# Generated ordinary-source custody, version 1

This contract joins one deterministic generated-source recipe to an ordinary
source extent in the existing OMGCOMP1 bridge carrier. It is bounded bootstrap
cost evidence, not an Omega language feature, package format, accepted lock,
signature policy, or compilation-authority receipt.

The generated output remains ordinary Omega source. After this custody join,
the existing resolver, CKIR3 lowering, conservative backend, and OMGRFN4
refinement relation consume exactly the same bytes as any authored source. No
consumer may recognize the recipe ID, source path, generator name, external
input name, generated declarations, or generated array dimensions.

## Canonical recipe

The canonical JSON artifact has schema
`omega.bootstrap.generated-source-recipe.v1`. Unknown or missing fields reject.
Objects use the exact fields below; arrays use the stated canonical ordering.

```text
top level:
  schema, recipe_id, status, resource_profile, runner, generator,
  repository_inputs, external_inputs, output, omgcomp1_join, closure_sha256

runner:
  kind, working_directory, workspace_manifest, package, binary,
  locked, offline, arguments, environment

environment row:
  name, value

generator and repository-input row:
  role, path, byte_length, sha256

external-input row:
  identity, kind, name, version, source, content_sha256

output:
  path, byte_length, sha256, media_type

OMGCOMP1 join:
  package_key, owner, machine, root_label, generated_source_id, sources

join source row:
  kind, label, module
or, for a repository source:
  kind, label, module, path, byte_length, sha256
```

`status` is exactly `bounded_cost_evidence`; `resource_profile` is exactly
`omega.bootstrap.generated-source-custody.v1`. The only runner admitted by this
version is `cargo-stdout-v1`: repository-root working directory, an exact
workspace manifest, package and binary, `--locked`, `--offline`, no ambient
arguments, and a sorted unique list of explicit environment values. The runner
is sealed construction tooling. The recipe never supplies an arbitrary command
or shell fragment.

Repository paths are canonical relative POSIX paths. Generator and repository
input rows bind exact bytes by length and SHA-256. They are sorted by path and
have unique paths and roles. Version 1 requires exactly one dependency-lock,
workspace-manifest, and package-manifest input. Every external Cargo-registry
input must match one exact name/version/source/checksum row in that dependency
lock. External rows are sorted by identity and unique.

The output row binds exact committed bytes. The verifier executes the sealed
runner twice, with bounded stdout and stderr capture, and requires both results
to be byte-identical to that output. It never overwrites the committed source.
A nonzero runner status, timeout, output disagreement, or digest disagreement
rejects before carrier construction.

The OMGCOMP1 join contains one nonzero opaque package key and one through four
canonically label-ordered source rows. Exactly one row has kind
`generated_output`; the other rows bind exact repository bytes. The root label,
owner, and machine select the existing bounded conformance entry. The
`generated_source_id` must resolve through the decoded OMGCOMP1 source row to
the nested bundle entry whose complete content equals the twice-reproduced
output. Labels and recipe paths remain custody metadata; neither supplies
package identity or logical module authority.

The `closure_sha256` is lowercase hexadecimal SHA-256 over:

```text
"omega.bootstrap.generated-source-recipe.v1\0"
|| u64le(canonical_json_byte_length)
|| canonical_json_without_closure_sha256
```

Canonical JSON uses UTF-8, sorted object keys, compact separators, and no
trailing newline for the digest projection. The committed file itself uses
two-space pretty JSON plus one final newline.

The digest detects recipe mutation; it is not a signature and grants no
authority. Adding a signing key would establish only custody over a review
decision and would improperly invent a second admission surface.

## Resource and status rules

The fixed version-1 ceilings are:

- at most 16,384 recipe bytes;
- at most eight repository inputs and four external inputs;
- at most 128 bytes and 16 components per repository path;
- at most eight runner arguments and eight explicit environment rows;
- at most 8,192 generator bytes;
- at most 65,536 bytes per repository input and 65,536 bytes across the
  generator plus all repository inputs;
- at most 64 bytes per external identity and runner/environment scalar;
- at most 131,072 generated stdout bytes and 65,536 stderr bytes;
- one through four OMGCOMP1 join sources; and
- the inherited OMGCOMP1 ceilings of 16 sources, 131,072 bytes per source,
  262,144 aggregate source bytes, 263,312 nested-bundle bytes, and 267,280
  envelope bytes.

Malformed, noncanonical, inconsistent, failed, or nondeterministic recipes are
status 251. Crossing a declared capture or recipe resource ceiling is status
252. Status is selected before publication and cannot later be downgraded.

The materializer buffers both generator observations and the complete OMGCOMP1
carrier. It writes the carrier once, only after recipe validation, both exact
reproductions, complete OMGCOMP1 decode, and the generated source-extent join.
Neither failure class publishes a prefix or carrier. Intermediate test files
remain staging artifacts and are never compilation acceptance.

## Required gate

[`../gates/generated-source-custody.sh`](../gates/generated-source-custody.sh)
must prove:

1. exact canonical recipe and domain-digest validation;
2. two locked/offline reproductions equal the committed output;
3. exact OMGCOMP1 construction, decode, and generated source-extent identity;
4. mutation rejection for schema, digest, roles, paths, lengths, hashes,
   ordering, lock/external linkage, runner policy, output, and join custody;
5. exact-limit capture succeeds, adjacent capture selects 252, a prefix followed
   by runner failure selects 251, nondeterministic observations select 251, and
   no failing case invokes publication; and
6. materialization itself publishes only one complete canonical OMGCOMP1.

The existing CKIR3 and OMGRFN4 gates remain responsible for general constant
aggregates, source and IR resources, Rust-free meaning, conservative artifacts,
and lower-rooted reconstruction. This contract adds no second generated-data
lowering path and makes no claim about the stale enclosing product checkpoint.
