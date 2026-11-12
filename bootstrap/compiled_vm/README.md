# Compiled VM experiment

This peer removes the Gamma interpretation layer after constructing a Gamma
compiler from source. [`compiler.gamma`](compiler.gamma) and
[`frontend.gamma`](frontend.gamma) compile Gamma to independent Alpha tapes.
Their first customer is the existing Gamma-written Delta compiler, which
constructs the Delta-written Epsilon evaluator. Its Gamma receipt is compiled
to another independent tape and runs the Epsilon-written bootstrap compiler.
No variant is selected by this experiment.

The source construction starts from the checked-in native
[Alpha seed](../0_alpha/README.md), the separately admitted
[Beta compiler tape](../1_beta/AUDIT.md), and the host OS. It stamps that tape
into Alpha, reconstructs Beta from its source, and uses the reconstructed Beta
compiler to construct the Gamma evaluator from Beta source. Both tapes must
match their bound artifacts exactly. The constructed Gamma evaluator then runs
the authored Gamma compiler on its own source to construct `compiler0`.
`compiler0` and its successor each rebuild that source; all three resulting
tapes must be identical. Beta self-reconstruction is evidence about the admitted
seed; it does not derive the initial Beta tape from nothing.

[`gamma.py`](gamma.py) remains an explicit development accelerator and
independent implementation for comparison. Its route trusts Python as an
additional semantic producer. The normal source construction does not invoke
it. Generated tapes execute without Python or the Gamma evaluator, and contain
the customer's actual logic and checks. There are no source recognizers,
receipt substitutions, or host callbacks. Shell and Python packing/stamping
helpers remain trusted byte transport and validation dependencies. Hash pins,
differential observations, and reproducible rebuilds are evidence, not compiler
correctness proofs.

## Run

From the repository root, on an executable Alpha seed host with Python 3:

```sh
# Seed construction, compiler fixed point, Delta/Epsilon, and an Omega program.
python3 bootstrap/compiled_vm/source_test.py --omega

# Recheck construction/rebuilds/controls without repeating larger customers.
python3 bootstrap/compiled_vm/source_test.py --compiler-only

# Explicit Python-assisted development construction of the same compiler.
python3 bootstrap/compiled_vm/source_test.py --accelerated --omega

# Native differential controls and the real Delta Bytes compiler customer.
python3 bootstrap/compiled_vm/test.py

# Also construct the complete Epsilon evaluator and run its Console boundary.
python3 bootstrap/compiled_vm/test.py --epsilon

# Also feed the complete bootstrap compiler its existing Omega fixture.
python3 bootstrap/compiled_vm/test.py --omega --omega-timeout 1800

# Exercise the existing changed-source, rejection, and reset customer too.
python3 bootstrap/compiled_vm/test.py --omega-controls --omega-timeout 1800
```

Source construction artifacts and the exact source/request bytes go to ignored
`build/source-gamma-reference/`; the accelerated source run uses
`build/source-gamma-accelerated/`. Host-compiler tests use `build/compiled-vm/`.
`source_test.py --seconds N` sets a positive watchdog for compiler construction,
self-rebuilds, source compilation, and Omega execution (3,600 seconds by default).
A refused source compilation publishes no bytes. Malformed source deliberately
traps with status 249; negative controls require that status and empty output.
Other runtime statuses fail the gate.

Tests use the existing closure packer, identity checks, and platform seed
stamper. The host frames byte requests and compares observations;
Gamma, Delta, Epsilon, and Omega execution happens in the generated tapes or the
explicitly identified reference evaluator. Gamma compilation and customer
execution have process watchdogs; the short Beta reconstruction shell commands
have no watchdog. A timeout makes no language judgment. The Omega allowance is
adjustable because checking the complete compiler is a materially larger
workload.

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
under compiled Epsilon. The existing 180-byte Omega fixture compiled to 50 Alpha
bytes; executing that tape exited 42 with empty stdout and stderr. The first
60-second observation allowance was insufficient; a 300-second run completed in
95.329 seconds. This demonstrates an accelerated construction through a real
Omega executable. No Omega compiler self-rebuild has been demonstrated here.

The then-current `tests/bootstrap/omega-executable/controls_b.epsilon` customer also
passed through compiled Epsilon with the complete bootstrap compiler (575,227
source bytes). It then accepted six changed modulo, shift, and bitwise programs,
rejects the out-of-range literal 256 while asserting an unfinalized empty tape,
then successfully recompiles 42 on the same compiler instance. The published
observation matched the existing expected `C` plus 22-byte Alpha tape exactly;
that tape also executed with exit 42 and no output. This eight-invocation run
completed in 159.692 seconds under a 300-second allowance. Later review found
that those anonymous operator acceptances were wrong Omega semantics; this
historical passing observation is not correctness evidence for them. The current
compiler refuses those forms, and its corrected customer passed in 139.663
seconds through source-built Epsilon with the same exact reset artifact and
exit 42. That observation used bootstrap compiler SHA-256
`f765c26d704c41552739e32fc592232edcf814f5d57ee3990e92660aacb6152f`. The default smaller-customer Omega watchdog
is now 1,800 seconds; `--omega-timeout` retains an explicit experimental override.

## Source compiler implementation and evidence

The frontend retains immutable syntax nodes and source spans for names. It
uses an explicit list stack with the same 255-list authored nesting boundary
as the reference implementation. Decimal words accumulate modulo 2^64;
comments end at either CR or LF; character literals preserve the language's
restricted escapes. All source bytes, including comments, are validated.

The backend first censuses declarations and parameter scopes. It counts lexical
slots by traversing expression positions, then compiles through an explicit
action stack. Every function and branch is visited, including unreachable code.
Instructions are immutable records; a second pass builds an AVL map from
numeric labels to byte addresses. Only after successful validation and label
resolution does the compiler publish instruction bytes. Function and local name
lookup currently uses exact span comparison in lists; this deliberately simple
implementation can be measured and replaced if larger customers justify it.

The small integer tags in `c_kind` name syntax, not source shapes:
`1=def`, `2=Int`, `3=main`, `4=$application`, `5=if`, `6=let`,
`7..11=+,-,*,/,%`, `12=eq`, `13=lt`, `14=pair`, `15=read`, `16=write`,
`17=first`, `18=second`, `19=input`. All other names remain source spans.
Instruction opcodes are the existing Alpha opcodes; private record opcode 21
marks a label and emits no bytes. Label identifiers 1..13 belong to runtime
helpers, function identifiers start at 100, and subsequent labels follow them.

The source files total 33,329 bytes (851 lines); the one joining newline makes
the executed source 33,330 bytes, SHA-256
`7b439ef26f2cb8a27bb9c0a2b4d5bfca6e7efeed0a5bdf9d798c04b65370b946`.
On Linux x86-64 its actual reference construction completed in 62.362 seconds.
It produced a 457,252-byte compiler tape, SHA-256
`71f6650f8201294e6654217f38fcd431fd8daaa233e0330c5da29e9819ad7e71`.
Both subsequent self-rebuilds were byte-identical, as was the independently
Python-produced tape. The source-built compiler passed 32 native differential
controls and 15 source refusals, including a legal binder named `let` that
caught an earlier incorrect traversal of binding records as expressions.

The downstream execution below used an initial 31,890-byte spelling with decimal
ASCII operands in keyword tests (source SHA-256
`584d3d94e903998defa96d417f65ff43a98630b55211c66a0ca01fa46287c79b`).
Its reference construction took 62.386 seconds. Rewriting those operands as
readable character literals preserved the parsed program, and the final actual
reference construction reproduced the same compiler tape exactly. Thus the
following customers consumed the same executable as the final source produces.

The source-built compiler constructed the real Delta compiler tape and the
complete Epsilon evaluator tape. Both were byte-identical to the independently
Python-produced tapes described above. The Delta compiler took 9.705 seconds to
compile from Gamma source; it produced the exact 3,082-byte Bytes customer
receipt. It reconstructed the bound Epsilon receipt in 9.875 seconds, and the
compiled Epsilon exit, stdin, trap, rejection, and malformed-envelope controls
passed. The complete 591,392-byte bootstrap compiler then compiled the existing
180-byte Omega fixture to a 50-byte Alpha program; executing it exited 42. This
last stage took 105.255 seconds. These are contended functional timings, not
comparative benchmarks.

Completing the bootstrap compiler's needed Omega coverage and obtaining
reproducible Omega compiler self-rebuilds remain delivery work. The Gamma
compiler fixed point does not establish an Omega compiler fixed point.

The parameter/local/call customer can be requested with
`python3 bootstrap/compiled_vm/source_test.py --omega-values`. It executes the
current manifested bootstrap compiler, verifies source-owned refusals and
executes the actual emitted tapes. The older modulo/shift/bitwise observation
above is historical only: those anonymous forms lacked the required carrier
meaning and are now explicit coverage refusals. It is not evidence that the
older compiler preserved their Omega semantics.

An earlier values-customer snapshot ran through the source-built Epsilon tape in
1,050.206 seconds. Nine emitted Alpha executables returned the expected values;
30 source-owned refusals checked empty, unfinalized publication and reset.
The complete Epsilon customer was 601,461 bytes, SHA-256
`0465ac3c9ee18d7f59b62a4cf1ca7d8323a5c0324e9ac7f6fdeaea76cad64c32`.
The separately reconstructed Epsilon tape retains SHA-256
`5f12d53066179df5e92929ab7a41933a2ed3c0234400d9c7b3cda8718a469c12`.
These are functional observations under concurrent machine load, not controlled
performance comparisons. The bootstrap compiler still cannot construct the
Omega compiler source closure or demonstrate its self-rebuild.

The changed legacy `controls`, `controls_c`, and `controls_d` bodies completed
with their exact reset tapes in a shared invocation (603,476-byte customer,
SHA-256 `64f98ca7aff4e1f651a3a347ea4cc3f955d74795b50370d2eb0088fa43e75a36`).
Each emitted tape executed with exit 42. That invocation then exposed the
previously unsupported ungrouped terminal call in `controls_i`; it was not a
successful complete batch. The repaired parser reused call-expression retention
instead of changing that witness. The unchanged `controls_i` body, four
structural-overload/duplicate refusals, a bare nested parameterized call and
reset then passed in 420.248 seconds. This 604,432-byte customer had SHA-256
`601f5013b8ff23c35f934fe5da184ea85e10ed4af187b4439f609b29754adf41`;
its three emitted artifacts executed with exits 42, 47, 43.

After restoring each body's diagnostic anchor following declaration census,
the final 598,333-byte compiler closure has SHA-256
`0c402370392af73910cf6caa05b0198dd7ccf0820397e26b85367a1dfcec9f72`.
Its focused early-body refusal checked canonical source coordinate 0 and empty
publication, then reset and emitted an actual executable returning 43. That
600,677-byte customer, SHA-256
`20ffacd689a6712f78070ccde506f14a4abe5f9edaaa3f4f9189893b05a620ed`,
passed in 76.013 seconds. The older observations above retain their own input
identities; the entire matrix was not rerun after this diagnostic-only repair.


## Compiler-source lexer customer

`--omega-control` constructs and executes the real lexer identifier predicates,
with their bodies and signatures unchanged, through the source-built Epsilon
tape above. The wrapper uses explicit `as +Trapping` casts and tests both
predicates across ASCII boundaries and the upper half of `u32`.

The runtime-control observation defaults to 7,200 seconds; `--seconds` on the
source-built route and `--omega-timeout` on the development route retain explicit
overrides. Timeout is an observation failure, not an Omega result. Additional
source-owned cases check staged parallel transfers, skipped looping Boolean
operands, nested calls/comparisons, domain casts, refusals and compiler reuse.
This extends the bootstrap compiler toward an actual successor source unit;
it does not construct the complete Omega compiler.

A canonical explicit-domain smoke completed in 478.167 seconds and emitted a
2,153-byte Alpha program that exited 42 without output. Its Epsilon source was
644,808 bytes, SHA-256
`449d795abd68930a95fc0f08ee190caa3bfb2519465851e96b693370f8a09b7a`,
including bootstrap compiler
`7f0540c05d1c31033f3164dc1fb0c552d41ae2e57764451387c2618e47de46e3`.
The exact 1,341-byte Omega customer had SHA-256
`d26ea6de810d23de785af5b4cfc9a7490b865431ddedf181b878343879268117`;
its tape had SHA-256
`58a5d2ff0e45242e9382c7f940fdfede2089742152d8bb747a61b633e7ccf0b7`.
This observation predates the final public crash-ceiling and visibility checks.

An earlier grouped run failed at its short-circuit call-expression customer
(Epsilon exit 12 after 1,114.986 seconds). Its two preceding tapes executed,
but the earlier lexer wrapper also omitted required explicit domain casts;
that acceptance is not conformance evidence. The repaired parser preserves
expression context around nested calls instead of weakening the looping-RHS
witness. Historical observations remain bound to their own source bytes.


The later boundary/control invocation used a 653,357-byte Epsilon source,
SHA-256 `6863d967b67ff9311dd6f121348cb9e725e287bcfd325ce54c0d4e46b923c621`,
including compiler `6459924a185619f637f16166b10b1f29e274692c3c4f7580fd7bc1e2032d22bf`.
All 25 source-owned refusal checks completed, including public crash-ceiling
propagation in both declaration orders and empty/unfinalized output. Its first
six emitted tapes were separately executed with exits `42, 42, 43, 44, 45, 46`
and empty output. These cover the real lexer boundaries, parallel transfers,
skipped infinite Boolean operands, upper-u32 comparisons, nested calls and
explicit domain casts. The 4,352-byte real lexer customer has SHA-256
`169002a5c460e9af180c4f3954fc28fd601132b131164483d459fe3cbda38c33`;
its 6,095-byte tape has SHA-256
`925bed6ed031e0d3ed5ddbb0c4f21fa2771cb7912e002c73755076784a924f59`.

That invocation is still a **failed complete matrix**: its final public/plain
positive exposed a missing call-primary path for a transition subject and
returned Epsilon exit 90. The whole 9,063-byte observation has SHA-256
`4253b9bb07f6debeb0f1db968f55ce7fe699667aafc222be23b21708139f4bbe`.
Elapsed time was 2,644.870 seconds, including roughly 208 seconds paused to
avoid exceeding the shared 32 GiB memory budget; it is not a benchmark.
The exact positive is retained while the shared parser path is repaired.

Separately, the 35-invocation parser customer reached its complete `A` marker
before the legacy batch stopped at an unchanged leading-`!1` refusal witness.
That 677,942-byte Epsilon source has SHA-256
`7b0c364de404419b64eedc5f5b502635ede0a00af78ba62347756fa084e060cd`;
the failed batch took 623.514 seconds and returned Epsilon exit 2 plus `A`.
The repaired parser routes statement-leading unary prefixes through the shared
expression machinery. Neither failed batch is promoted to evidence for later
source bytes; focused execution of the final repairs remains required.
