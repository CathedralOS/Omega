# Conversions that use a caller's buffer

These examples encode a pair of bytes in two formats using storage supplied by
the caller. They check that enough space guarantees success and that failure
returns the buffer without exposing an invalid result.

Fact: **Selected buffered conversions preserve caller output custody and establish a destination only with sufficient capacity.**

This is part of **LIBRARY-REPRESENTATION-FAMILIES** under the
[conversion contract](../../../../wiki/spec/layouts/codecs.md).
The nearest completed row is [representation_agreement](../representation_agreement/README.md),
whose conversions return fixed owned frames. This row instead passes an exclusive
borrow of the caller's byte buffer through requests, successful results and
failures. It has no hidden allocator or service.

`BufferedEncoding` adds a sufficient-space operation to the ordinary fallible
`TryFrom` interface. The operation requires an output length of at least three
bytes and promises an `Ok` result for every `Pair`. It calls the same conversion
implementation; it does not make all `TryFrom` operations pure or infallible.

The two formats use different tags and field orders. `compose` decodes the first
into a `Pair`, then writes the second using the same buffer. Failure returns
that buffer and prevents the second conversion from running. A matching byte
sequence supplies no new permission to access storage.

The [local program](local/main.omg) checks unequal field values, byte endpoints,
both formats, short output, malformed input and unchanged failure storage. It
also checks nonzero canaries around each buffer. Its
[invalid twin](local/invalid/main.omg) changes only the first encoder's capacity
guard from three bytes to two. The write helper still requires three bytes, so
the twin must reject at that call.

The public capacity requirement establishes the success fact before conversion.
Calls carry the exact buffer, its lifetime and its exclusive access. The result
checks use the actual selected implementations. No caller guard supplies a
missing success proof after the conversion has finished.

## Current result

The first place is unfinished. The compiler now proves the five success
postconditions that previously failed, and it can put the incoming slice into
the inline `Request`. It still stops while planning cleanup for an encoder
branch: it cannot determine which part of the original request remains after
its fields are moved. Native and Terminal execution have not run.

[Run 41505955288](https://github.com/CathedralOS/Omega/actions/runs/41505955288)
used source `1a9062c50669d96745270ae0f77f35fd7a5caec3` and an authenticated,
optimized Linux x86-64 compiler. The original positive reached this error:

```text
cleanup cannot reconstruct its remaining region partition
```

The two supporting positive controls checked. All five invalid controls rejected
for their expected reasons: the short-buffer guard, wrong lifetime arguments,
shared-to-mutable access, and two cases where an input parameter named `result`
must keep its own meaning. The mutable-to-shared control checked. This is source
checking evidence, not a runtime result.

The frozen program, Build files and guard-only twin remain unchanged from
`10fba736d8eb1ed0de892a360d8fb6dba4882599`. The earlier baseline
[41489778010](https://github.com/CathedralOS/Omega/actions/runs/41489778010)
rejected the five success proofs; the subsequent return-binding repair removed
those refusals. Raw reports and compiler identities are retained outside git in
`/Users/zcanann/Omega-build/representation-resource-20261102/`.

[PR #1461](https://github.com/CathedralOS/Omega/pull/1461) remains a draft. Its
next step is to finish cleanup for the unchanged encoder and run the local
program with both engines, then complete the nine-place row and invalid twins.
No completed place or canonical success record is claimed.

The broader resource-backed `Representation` interface remains open, including
its agreement laws, reflection-based adapters, other resources and formats,
other hosts and the Omega-written compiler.
