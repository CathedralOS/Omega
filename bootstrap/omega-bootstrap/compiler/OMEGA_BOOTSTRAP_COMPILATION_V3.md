# Build-source-bearing bridge compilation envelope, version 3

OMGCOMP3 is the bounded structural successor to
[`OMGCOMP2`](OMEGA_BOOTSTRAP_COMPILATION_V2.md). It preserves the exact 64-byte
header, package/source/alias/string tables, nested source bundle, Linux x86-64
target, native-provider-substitution configuration, resource ceilings, and
status behavior. Its only new relation is one explicit authoritative build
source inside the root package.

This is role custody, not package or compilation authority. The selected build
source is authoritative *within the requested compilation envelope*: later
source semantics must harvest provider selection only from that source. The
envelope remains untrusted until external accepted-lock/closure evidence joins
its exact source subjects and digest. A source label, filename, module path,
readable `build` name, declaration order, or unique provider candidate never
creates the role.

## Identity and role encoding

The exact identity is magic `OMGCOMP\0`, schema major 3, schema minor zero,
target 1 (Linux x86-64 System V), flags zero, and selected configuration 1
(native provider substitution). Header word 60 retains that configuration.

The existing source-row flags word has the following complete version-3
meaning:

```text
0  ordinary compiled source
1  authoritative build source
```

Exactly one source row has flags 1; every other row has flags zero. Unknown
bits, no build row, or multiple build rows reject with 251. The build source
must be owned by the selected root package. It may equal the entry source, but
the roles remain independently encoded. OMGCOMP1 and OMGCOMP2 continue to
require every source flags word to be zero, so every cross-version pair rejects.

The manifest names the role by exact package key and nested-bundle source
label. The deterministic packer resolves that pair to one source row and emits
the role bit. Decoding reports the row ID and its package, label, and logical
module; no later stage may rediscover the role from those readable fields.

## Bounded carrier and exclusions

The focused fixture carries one root-package build source and entry source plus
the portable six-requirement `Console` declaration and one Linux-x64 provider
source. Its source bytes include the explicit
`Build::select_provider<Console, ConsoleNativeProvider>` spelling so the next
resolver milestone can consume the already-custodied role. OMGCOMP3 treats all
source bytes as opaque and therefore does not resolve that call, derive a
`ProviderPlan`, validate complete provider coverage, authorize an intrinsic,
lower a boundary call, or establish provider/package/artifact authority.

Malformed framing, role bits, role cardinality, role ownership, version or
configuration cross-pairs, graph inconsistency, and trailing bytes select 251.
Exceeding an inherited public transport ceiling selects 252 before publication.
The Delta checker emits no bytes.

[`omega_bootstrap_compilation_v3.py`](omega_bootstrap_compilation_v3.py) is the
deterministic pack/verify/inspect tool.
[`../gates/delta-compilation-envelope-v3.sh`](../gates/delta-compilation-envelope-v3.sh)
compares it with an independent encoder, exercises role and resource controls,
runs the Delta checker built through both native and self-hosted routes, and
retains the complete OMGCOMP2/V1 regressions.
