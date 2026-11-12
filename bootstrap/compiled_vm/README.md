# Compiled VM experiment

This peer explores removing the Gamma interpretation layer while retaining the
audited Alpha VM. `gamma.py` compiles the actual Gamma language to independent
Alpha tapes. Its first customer is the existing Gamma-written Delta compiler;
that compiler constructs the existing Delta-written Epsilon evaluator, whose
Gamma receipt can also be compiled to an independent tape.

**Python is currently an additional trusted semantic producer.** This is an
explicit development acceleration experiment, not a source-only bootstrap from
the Alpha seed. The generated tapes execute on Alpha without Python or the
Gamma evaluator. They contain the customer's logic, including its checks;
there are no source recognizers, receipt substitutions, or host callbacks.
No variant is selected by this experiment.

The production trust root is Python executing `gamma.py`, the checked-in native
[Alpha seed](../0_alpha/README.md), and the host OS. No admitted Beta tape runs
on that production route. Separately, the tests stamp the checked-in
[`gamma_evaluator_bytecode.tape`](../2_gamma/gamma_evaluator_bytecode.tape) as
their comparison oracle. They verify the recorded Beta source and tape
identities, but do not reconstruct that oracle through Beta; its
[existing construction](../2_gamma/README.md) remains a separate obligation.
Shell and Python packing/stamping helpers are byte-transport dependencies.
Hash pins and differential observations bind this evidence; they are not
compiler correctness proofs.

## Run

From the repository root, on an executable Alpha seed host with Python 3:

```sh
# Native differential controls and the real Delta Bytes compiler customer.
python3 bootstrap/compiled_vm/test.py

# Also construct the complete Epsilon evaluator and run its Console boundary.
python3 bootstrap/compiled_vm/test.py --epsilon

# Also feed the complete bootstrap compiler its existing Omega fixture.
python3 bootstrap/compiled_vm/test.py --omega --omega-timeout 1800

# Exercise the existing changed-source, rejection, and reset customer too.
python3 bootstrap/compiled_vm/test.py --omega-controls --omega-timeout 1800
```

Generated sources, tapes, executables, and requests go to ignored
`build/compiled-vm/`. Tests use the existing closure packer, identity checks, and
platform seed stamper. The host frames byte requests and compares observations;
Gamma, Delta, Epsilon, and Omega execution happens in the generated tapes or the
explicitly identified reference evaluator. Every native process has a timeout;
a timeout makes no language judgment. The Omega allowance is adjustable because
checking the complete compiler is a materially larger workload.

To compile another Gamma source without the test harness:

```sh
python3 bootstrap/compiled_vm/gamma.py source.gamma build/program.tape
```

The output is raw Alpha code; stamp it using
[`seed_env.sh`](../../tools/bootstrap/alpha/seed_env.sh). The executable accepts
only the source program's sealed input, without the reference Gamma evaluator's
four-byte source-length envelope. Invalid source leaves an existing output
artifact untouched. Successful compilation replaces it atomically.

## Meaning and storage

[`../2_gamma/LANGUAGE.md`](../2_gamma/LANGUAGE.md) governs the source language.
The compiler validates every function and both branches before publishing code.
It supports wrapping words, checked scalar boundaries, immutable pairs,
sequential grouped bindings, mutually visible functions, proper tail calls,
sealed byte input, and atomic buffered publication. Primitive names can be
ordinary bindings; list heads retain their primitive meanings. Numeric and
character tokens are literals, not supported declaration names. The reserved
application declaration must have its documented nullary form and cannot be
called as a user function. Unsupported or malformed source fails compilation.

Every value occupies a word plus an unforgeable scalar/pair tag. Pairs occupy
four words. A function frame holds its arguments, saved caller frame pointer,
and statically assigned lexical slots; expression temporaries follow the frame.
Non-tail calls use Alpha's return stack. A tail call evaluates all arguments,
moves them over the old frame, restores the caller frame pointer, and jumps to
the callee. Its return address and call depth remain unchanged, including calls
between functions of different arities. Pair allocation is monotonic; there is
no garbage collector.

The generated runtime owns disjoint regions of Alpha's 128 GiB address space:

| Region | Half-open address range | Boundary behavior |
| --- | --- | --- |
| Code | `0 .. 0x1000000` | Tape limited to the seed's 16 MiB hole minus its length word |
| Frames and temporaries | `0x02000000 .. 0x08000000` | Check every push and frame reservation |
| Alpha return stack | `0x0feffff8 .. 0x10000000` | At most 131,072 active source calls plus one runtime helper (1,048,584 bytes), above the frame region |
| Sealed input | `0x10000000 .. 0x18300000` | Check ingestion and every source read |
| Buffered output | `0x20000000 .. 0x28000000` | Check every append; flush only after valid main result |
| Immutable pairs | `0x30000000 .. 0x2000000000` | Check every 32-byte allocation |

Ordinary authored traps exit 2; storage limits exit 3. Application traps and
frame/heap/input/output limits use 249/250/252/253/254. Pair handles cannot be
forged by arithmetic or read from input; scalar operators reject tagged words.
The limits are this implementation's physical profile, not a claim to reproduce
the reference evaluator's resource envelope. Physical host memory remains an
execution dependency, just as it is for the reference seed.

## Evidence and remaining work

On Linux x86-64 at repository base `c930a0075d7`, the focused `--epsilon` run
passed 30 byte-exact native differential observations and 15 rejected-source
controls. These include 100,001 tail swaps, mixed-arity and pair-valued tail
calls, caller-frame preservation, scalar/tag traps, overflow division, lexical
refusals, and atomic application publication. Nine additional native controls
exercise exact and adjacent storage boundaries under a reduced compiled test
profile. CLI refusal also preserves an existing output artifact.

The 147,840-byte Delta compiler closure produced a 1,184,046-byte Alpha tape.
It compiled a real Delta Bytes duplication program into a 3,082-byte Gamma
receipt identical to the reference evaluator's result. The receipt compiled to
Alpha and duplicated empty, ASCII, and all-byte inputs correctly.

The same compiled Delta compiler reconstructed the complete canonical Epsilon
evaluator receipt: 729,060 bytes, SHA-256
`bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368`.
That is the existing bound receipt identity, not a newly substituted artifact.
Compiling it produced 7,876,070 Alpha bytes. Its Console exit, stdin, trap with
stdout prefix, rejected-source, and malformed-envelope observations passed.
Reconstruction took 11–17 seconds in contended functional runs; these are not
comparative performance benchmarks.

A separate cross-variant run used the exact 622,933-byte request from a fresh
reference reconstruction (SHA-256
`6386d0d5729a320e73b9865809751ae2962865f4ebc1b3472f2e828de9ab7f76`).
Compiled Delta produced the same 721,484-byte Epsilon execution-adapter receipt,
SHA-256 `71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082`.
This is actual byte correspondence, not a comparison between the two different
Epsilon entry receipts. Its contended 8.708-second execution and the reference's
354.968 seconds are not a controlled speedup measurement.

The complete 591,392-byte bootstrap compiler plus existing OCREQ entry then ran
under compiled Epsilon. The existing 174-byte Omega fixture compiled to 50 Alpha
bytes; executing that tape exited 42 with empty stdout and stderr. The first
60-second observation allowance was insufficient; a 300-second run completed in
95.329 seconds. This demonstrates an accelerated construction through a real
Omega executable. No Omega compiler self-rebuild has been demonstrated here.

The existing `tests/bootstrap/omega-executable/controls_b.epsilon` customer also
passed through compiled Epsilon with the complete bootstrap compiler (575,227
source bytes). It checks six changed modulo, shift, and bitwise programs,
rejects the out-of-range literal 256 while asserting an unfinalized empty tape,
then successfully recompiles 42 on the same compiler instance. The published
observation matched the existing expected `C` plus 22-byte Alpha tape exactly;
that tape also executed with exit 42 and no output. This eight-invocation run
completed in 159.692 seconds under a 300-second allowance. It checks source
generality, failure publication, and compiler reset. The default Omega watchdog
is now 1,800 seconds; `--omega-timeout` retains an explicit experimental override.

The next source-owned construction is a Gamma-written Gamma-to-Alpha compiler,
first run through the existing Beta-written Gamma evaluator, then compiled by
its resulting tape. The present explicit parser, tagged frame protocol, and
instruction emission provide a concrete comparison target. Porting must use
explicit traversal work stacks where needed for the reference evaluator's
bounded contexts, rather than translating Python recursion mechanically.
Closing that edge, completing the bootstrap compiler's needed Omega coverage,
and obtaining reproducible Omega compiler self-rebuilds remain delivery work;
this acceleration result does not discharge those obligations.
