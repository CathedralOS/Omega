# Beta compiler bootstrap observable

This document fixes the whole-program observable that the `bc.beta` cold-start
edge must preserve. A fixed point, matching output from another compiler, or
agreement on a finite input corpus does not establish this relation.

## Subject of the claim

Let:

- `S` be any finite byte stream supplied as Beta source on standard input;
- `B` be the explicit finite resource profile under which the compiler runs;
- `A` be one exact, fingerprinted Alpha tape claimed to implement
  [`bc.beta`](bc.beta).

The completed cold-start claim is:

```text
for every S and every supported B:
    observe_beta(bc.beta, S, B) = observe_alpha(A, S, B)
```

`A` includes the exact result of Beta lowering and Alpha assembly. The claim is
about that artifact, not about the producer that happened to emit it.

Malformed, truncated, oversized, and otherwise rejected byte streams remain in
the quantification. Validation may divide the input space into canonical cases,
but it may not silently restrict the theorem to successful compiler inputs.

## Maximal observation

An observation is the maximal ordered standard-output byte stream together with
exactly one terminal classification:

```text
CompilerObservation = {
    stdout: finite or infinite sequence<Byte>,
    terminal:
        Halt(u32)
      | Trap(TrapKind)
      | Exhaust(ResourceKind, limit, requested)
      | Diverge
}
```

- `stdout` includes every byte emitted before termination, trapping, or checked
  exhaustion. Equality is byte-for-byte, in order, over the complete stream.
- `Halt(u32)` retains Alpha's full low-32-bit halt value. A Unix shell's low
  eight bits are only a projection and cannot close this edge.
- `Trap(TrapKind)` distinguishes the traps assigned meaning by the Alpha and
  Beta semantics, including division by zero, signed division overflow, and an
  invalid opcode. A host signal number is evidence about a realization, not the
  canonical trap identity.
- `Exhaust(ResourceKind, limit, requested)` is a checked semantic outcome. It
  names the exhausted resource and the declared limit, and records the size or
  reservation that could not be admitted. Exhaustion must occur before an
  overlapping write or acceptance of a truncated compiler input/output.
- `Diverge` means the canonical machine takes infinitely many internal steps
  without another terminal outcome. Its `stdout` may be a finite prefix or an
  infinite stream. A test timeout is not by itself a proof of divergence and
  must never be reclassified as a trap or exhaustion.

Standard error, wall-clock duration, host addresses, and platform wrapper bytes
are not language observables. Input EOF is: after the last byte of `S`, Alpha
`read` and Beta `read_byte()` yield the canonical all-ones sentinel.

## Resource profile

`B` makes every finite compiler resource relevant to the claim explicit:

- source-byte capacity;
- symbol/local/state table capacities;
- emitted-output capacity, if output is buffered by a realization;
- Alpha tape, data-memory, and call-stack extents;
- evaluator or proof fuel where a lower-rooted checker is intentionally
  fuel-bounded.

Changing `B` changes the quantified run, not the language program. A supported
profile must either admit an operation or produce `Exhaust`; unchecked memory
overflow and silent truncation have no acceptable observation.

The current `bc.beta` source arena is the disjoint byte interval
`[2097152, 3145728)`, hence its first declared source limit is 1,048,576 bytes.
The name tables begin at `3145728`; a source byte may never overwrite them.
The current process projection of `Exhaust(SourceBytes, 1048576, 1048577)` is
an empty output stream and exit status 253, pinned by `source-exhaustion.sh`.
The eventual lower-rooted proposition retains the semantic `Exhaust` identity
rather than treating that host status as its definition.

### Supported profile `B_bc1`

The first supported whole-compiler profile is now frozen to the exact source and
artifact committed together:

- `bc.beta`: 32,045 bytes, SHA-256
  `8beb76feda2783e6597998e5e7b41889c049faa2f604e627aead0f9d2b0cdbc8`;
- `bc.tape`: 51,647 bytes, SHA-256
  `e491073e0a27d50fb429d6f2a24158029ab5d8baa98e8855a4bd8771768c104b`;
- Alpha memory: 64 MiB, with the tape at byte zero and the hidden return stack
  starting at 64 MiB;
- stamped tape payload: at most 262,140 bytes inside the 256 KiB hole after its
  four-byte length prefix;
- compiler data stack: starts at 1 MiB and must remain within the reserved
  `[524288, 1048576)` interval;
- hidden Alpha call stack: the top 64 KiB of memory, or at most 8,192 live
  return addresses;
- source bytes: `[2097152, 3145728)`, exactly 1,048,576 bytes;
- per-procedure local-name metadata: 1,024 paired `NAMEOFF`/`NAMELEN` entries;
- live parameters and call arguments: at most four, matching `r0..r3`;
- recursive expression-codegen depth: 64 `gen_expr` activations;
- recursive block-codegen depth: 64 `gen_stmts` activations;
- output: streamed one byte at a time, with no finite compiler-owned output
  buffer.

The source compiler checks the source, name, argument, expression, and block
ceilings before the corresponding compiler-owned memory can overlap. Source
exhaustion projects to status 253 with empty output. The other checked compiler
resource failures project to status 252 and retain the deterministic maximal
output prefix already streamed. `source-exhaustion.sh` pins every exact/+1
boundary and the prefix rule. A later simulation proof must establish that these
syntactic ceilings keep both Alpha stacks inside the reserved extents; naming
the extents here does not substitute for that proof.

## Required reconstruction boundary

The authority that closes the edge must independently reconstruct, from the
exact `bc.beta` source and exact artifact bytes:

1. the input/resource quantification;
2. the complete output-stream relation;
3. every halt, trap, checked-exhaustion, and divergence case;
4. the Alpha small-step obligations for the artifact; and
5. the Beta source-meaning obligations for the compiler.

The resulting proposition is checked below `bc`. The producer may supply proof
search output, summaries, or certificates, but it may not select the observable,
omit terminal cases, or define the artifact's obligations.

## Current evidence and remaining gap

Existing self-host, corpus, differential, and instruction-level refinement gates
are valuable teeth, but they do not yet establish the quantified observation:

- `selfhost.sh` proves dependency closure and deterministic reproduction;
- the differential gates cover finite corpora and host-visible low-byte exits;
- the current symbolic refinement fragment returns one result term and does not
  model the compiler's complete byte stream or every terminal class;
- the lower-rooted artifact checker proves reachable instruction framing,
  direct-target boundaries, and static procedure-region/call-return discipline,
  but not the 8,192-frame dynamic call bound or frame/data-memory contents;
- the lower-rooted control-skeleton checker binds every exact source entry/state
  block and `to` site to decoded Alpha instruction starts and successor shapes,
  including guarded fallthrough, but not statement-local data/trace simulation;
- the same lower-rooted process gives every source call, return, read, write, and
  fixed-string emit site exact artifact custody, accounts for every effectful
  artifact opcode, and checks all 829 fixed literal bytes plus their output
  helper macro; it does not yet prove argument/value correspondence, dynamic
  frame behavior, reachability, or the complete ordered output trace;
- its source-derived frame phase checks all 70 prologues, 78 parameter/local
  slots, 27 parameter stores, callee arities, and 134 immediate pre-call pops;
  a source-name/slot phase additionally binds all 169 local reads and 73
  `let`/assignment writes to exact fp-relative macros; the earlier argument
  pushes/values, local access values, and live stack-depth bound remain open;
- Alpha out-of-range memory remains undefined in `alpha/SEMANTICS.md` and must be
  excluded by independently checked `B_bc1` bounds before whole-artifact closure
  (or Alpha must be hardened independently);
- divergence requires a checked progress/termination argument or a coinductive
  trace argument, not a timeout.

Those are the concrete obligations of the lower-rooted source-to-artifact
refinement edge.
