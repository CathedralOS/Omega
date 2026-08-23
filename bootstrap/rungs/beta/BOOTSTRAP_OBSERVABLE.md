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

- `bc.beta`: 32,064 bytes, SHA-256
  `f844d33e29814f1280bbeee2bf599db2bded2fb9469a7f1bfc870fac522c326d`;
- `bc.tape`: 51,602 bytes, SHA-256
  `835c44d1b99afc13be8da3f8ccc95fc6dde61aaa94dfba8b3920b0d34c4f99d9`;
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
boundary, the prefix rule, and a full-capacity trailing-`=` lookahead canary
that varies `NAMEOFF[0].low` between `=` and `>` and requires identical output.
A later simulation proof must establish that these
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
  helper macro. A following relational phase proves the helper's exact
  `M[p:p+len]` output and decreasing length rank once, then instantiates it at
  all 113 checked emit rows / 829 literal bytes; these conditional per-event
  clauses do not yet prove reachability or the complete ordered output trace.
  Two one-block procedure summaries now order the 55-byte `emit_prelude` and
  132-byte `emit_write_str` traces and prove their termination/restoration, but
  composing those calls from `main.ready` remains open;
- its source-derived frame phase checks all 70 prologues, 78 parameter/local
  slots, 27 parameter stores, callee arities, and 134 immediate pre-call pops;
  a source-name/slot phase additionally binds all 169 local reads and 73
  `let`/assignment writes to exact fp-relative macros; the earlier argument
  argument-value association, local access values, and the live stack-depth
  bound remain open;
- its raw-memory phase binds 61 source loads and 34 stores to exact byte/word
  opcodes and registers, including each store's immediate address pop. The
  reduced ranged-store phases close the Beta-source intervals and transfer the
  three selected Alpha address operands under valid callee entry relations. The
  selected machine-`NLOC` load/update participates. The later protected-writer,
  frame-summary, and potential-lift phases establish those selected entry
  relations globally; other raw loads, identifier/table payload values, and
  general address correspondence remain open;
- its grammar-derived raw-load partition now classifies those 61 loads as 54
  aligned fixed-global words, five indexed SRC bytes, and two indexed
  name-table words. An exhaustive row scan checks the exact adjacent Alpha
  immediate/load pair for every fixed-global site, closing all 54 fixed-load
  bounds while leaving the seven indexed span/index relations open;
- its first blockwise relational phase proves a cursor-zero conditional
  `slurp` summary with a compact `SRC[0:n] = input[0:n]` segment token, exact `Input(n)`/EOF
  lookahead and cursor transfer, the bounded endpoint append, all seven success
  globals, both returns, empty output, caller restoration, and a decreasing
  `1048576-n` rank. It distinguishes complete inputs through 1 MiB from the
  consumed-but-unstored 1,048,577th byte. The following root bridge establishes
  its actual prelude/main entry relation, carries return 1 to `main.ready`, and
  composes return 0 through `main` to canonical `Halt(253)` with empty output,
  without yet assigning the typed SourceBytes exhaustion identity;
- its BC11 grammar-composition pass further partitions every raw-store source
  address into 31 aligned fixed compiler globals, one exact source-buffer
  `base + n` spelling, and two exact local-name-table `base + s * 8` spellings;
- its ranged-store phase checks the complete `slurp`/`declare` source CFG slices,
  decoded predecessor closure, and exhaustive NLOC writers, inductively
  deriving `n <= 1048575` and `s <= 1023` on the source store paths. The three
  exact byte extents are nonwrapping, in 64 MiB, and numerically disjoint from
  the reserved global, explicit-stack, and hidden-return regions. A following
  witness-free row join carries these facts through the exact compiled local,
  primitive, push, and store chains. A two-cell executable tag/interval stack
  derives each selected arithmetic and address-pop result. Its decoded-CFG
  fixed point proves both call-free selected procedures restore their entry `(r15,r14)` pair with at
  most 32 relative bytes, conditional on a valid aligned entry frame and, for
  `declare`, the actual/source `NLOC` relation in `[0,1024]`;
- its expression-primitive phase binds all 581 decimal/character literals and
  55 arithmetic operators to exact immediate and stack-pop/operator macros and
  all 180 comparisons to exact signed-order/full-word-equality branch variants,
  operand order, targets, and complementary 0/1 results; an exhaustive artifact
  inventory reserves 360 comparison-result and 113 fixed-emit address immediates
  and requires ownership of every remaining candidate. This flat phase alone
  leaves recursive value composition, unique ordering among identical
  block-local primitives, arithmetic trap correspondence, and dynamic
  reachability open;
- its stack-push phase reconstructs and exhaustively owns all 235 binary-left,
  134 ordinary-call argument, and 34 store-address push macros. Their recursive
  value association, identical same-block order, and live stack bounds remain
  open in that flat phase;
- its expression-composition phase reparses all exact source expressions and
  statement continuations with Beta precedence, then requires the already-owned
  primitive/local/memory/effect/transition/push PCs in syntax-directed lowering
  order. It binds binary operands, nested loads, ordinary argument staging and
  reverse pops, store address/value staging, local stores, guarded transitions,
  and return epilogues. Each statement expression is relatively `r15`-balanced
  and the exact compiler's temporary high-water mark is two words. Flat-valid
  same-valued literal, argument-push, and store-push permutations reject here;
  absolute stack bounds, dynamic frames and leaf/callee values remain open, as
  does order between byte-identical complete same-block statements/effects;
- its BCT9 call-bound phase reconstructs all 310 ordinary source call edges,
  checked frame weights, per-call temporary heights, and the 113 fixed-emit
  helper calls. It checks finite 64-level expression/block recurrences, the
  rejected depth-65 probe costs, and the 19-level signed-positive `emit_dec`
  rank, deriving conservative root bounds of 12,720 explicit-stack bytes and
  662 hidden returns. The following protected-counter phase binds each of the
  64 potential rows to its exact live machine depth, exhaustively checks the
  `NLOC`/counter/resource writers, and rejoins the reset, guard, update, and exit
  chains. An exhaustive 607-store partition plus all-70-procedure call-cut CFG
  fixed points then protect saved-fp words, check exact local/call high-water
  marks, and prove conditional caller-pair restoration. The final potential
  induction establishes those conditions from the exact prelude and leaves
  explicit/hidden low-water marks 1,035,856/67,103,568, closing absolute
  `B_bc1` stack safety while leaving general values and reachability open;
- its BC11 stack-register phase constructs one unified owner map for every
  decoded write to `r14`/`r15` and every memory access through `r15`, deriving
  exactly 2,630 owned starts from the already checked prelude, prologues,
  epilogues, pushes, and pops. This closes orphan stack effects; the following
  phases prove the source ranged-address premise and selected compiled operands;
  the protected-counter, all-store/frame-summary, and potential-lift phases then
  close whole-artifact dynamic stack/frame bounds;
- Alpha out-of-range memory remains undefined in `alpha/SEMANTICS.md` and must be
  excluded by independently checked `B_bc1` bounds before whole-artifact closure
  (or Alpha must be hardened independently);
- divergence requires a checked progress/termination argument or a coinductive
  trace argument, not a timeout.

Those are the concrete obligations of the lower-rooted source-to-artifact
refinement edge.
