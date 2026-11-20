# Delta interpreter

This Gamma program executes a Delta source file and its byte input directly.
It uses the Delta compiler's source checker to reject invalid declarations
before execution. The interpreter then evaluates the retained Delta expressions,
including calls, constructors, pattern matches and byte operations. A Delta
program runs through this path without first producing a Gamma program.

The default [source list](sources.list) names every member relative to
`bootstrap/delta/`. It includes the shared checker, failure publisher and byte
rope constructors. Each member is LF-terminated. Join the exact member bytes
with one additional LF between successive files. From `bootstrap/delta/`:

```sh
awk 'FNR == 1 && NR > 1 { print "" } { print }' \
  $(cat interpreter/sources.list) > /tmp/delta-interpreter.gamma
```

A Gamma interpreter can run this source, or a Gamma backend can compile the
interpreter itself for Alpha or Linux x86-64. In each case, its sealed input is
a little-endian u32 Delta-source length, the source bytes, and the guest stdin
bytes through end of input. A successful program must define
`main : Bytes -> Bytes`. Halt zero publishes the returned bytes, including an
empty value. Malformed frames and static failures use the checker's DCOUT
records. Runtime traps and byte limits use the
[Delta application statuses](../LANGUAGE.md). The Gamma host also has its own
resource limits.

Two source configurations share the checker, evaluator and framing. The default
list includes `conformance.gamma`. To build for construction, replace that one
list member with `interpreter/construction.gamma`. The selected declaration
block supplies the same byte and syntax limits as the corresponding Delta
application profile. Source admission and guest input have separate limits.

| Source profile | Delta source | Syntax storage | Guest input | Guest output |
| --- | ---: | ---: | ---: | ---: |
| Conformance | 4 MiB | 114,294,752 bytes | 4 MiB | 4 MiB |
| Construction | 64 MiB | 1 GiB | 8 MiB | 128 MiB |

The host runner accepts `--profile construction` to select that member.
Supply `--source PROGRAM.delta` and an optional `--stdin INPUT` to execute
an application and save its exact result in `guest.output`. The runner's
process status reports the interpreter's exit. A Delta program may encode its
own status in those bytes. [Run an application](../../../tests/bootstrap/interpreted/delta/interpreter.md)

Evaluation processes arguments left to right. A call binds the resulting
values in a fresh environment. A selected branch, match arm, let body or
function body is evaluated in tail position. Runtime constructor tags are local
to the statically checked nominal type. Sealed input is a read-only rope leaf
whose base skips the interpreter frame and Delta source.

[Host comparison and reproduction](../../../tests/bootstrap/interpreted/delta/interpreter.md)
