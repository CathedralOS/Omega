AtomicU32 and AtomicU64 locals execute loads, stores, fetches, swaps and
compare-exchange. AtomicBool locals execute loads and stores. This completes
the executable-entry requirement of
[ATOMIC-MEMORY-MODEL](../../../../../TASKS.md).

[main.omg](main.omg) retains the original operations and sum of 265. The
program checks every observed value and final stored value; its entry checks
the returned sum, then exits zero. A wrong value traps. Checking, Terminal
execution and native code carry the actual variable or field address. AArch64
updates use baseline exclusive loads/stores. Strong compare-exchange retries a
failed store and completes on mismatch without an update; single-attempt
exchange remains one attempt.

| Matching host | Measured compiler/program source | Native / Terminal exit |
| --- | --- | --- |
| Linux x86-64 | `af33106a1e7` | 0 / 0 |
| Windows x86-64 | `7b9ecb41a26f` | 0 / 0 |
| macOS ARM64 | `a1da0d793346` | 0 / 0 |
| Linux ARM64 | `a1da0d793346` | 0 / 0 |

The integrated Linux run at `af33106a1e7` has six checked positives, nineteen
intended refusals, six matching exit-zero results from native and Terminal
execution, and two ordinary CLI agreements. Nine sample invalid controls
reject their actual bad stores. All six existing tests for tracking atomic
writes across branches and loops pass. The original four runtime programs
challenge a real field alias and untouched sibling, strong compare-exchange
mismatch and argument order, and bounded bitwise updates. Two additional
programs from incoming main check both branch writes and repeated loop
updates. Those new alternatives have Linux observations only.

Earlier macOS ARM64 and Linux ARM64 results each include four matching
exit-zero results and two ordinary CLI agreements. Windows x86-64 has two
matching exit-zero results and the ordinary global project. Those observations
retain their measured sources; the incoming target-neutral changes preserve
the ISA, ABI, layout and interpreter consumers for those measured programs.
They supply no new-host execution of the branch or loop alternatives.
Profiles, source bundles, raw reports and independent reviews are retained
with [PR #1865](https://github.com/CathedralOS/Omega/pull/1865).

The original integrated report at `524d0e57db23` remains failed because an old
reject helper permitted values through 101 but only received 41. Closed
private call inference correctly proved that safe call; a separate baseline
checked all nine old corpus twins. Input source `29b899640ddf` changed only
the bad inputs to 101 in the nine corpus and nine sample controls. The same
compiler rejected all eighteen at the actual store into Cell's unchanged bound
of 100. Current `af33106a1e7` independently repeats those refusals. Positive
programs still store 41 and check 59. Original reports and program hashes
remain retained.

Controls also reject ordinary scalar atomic access, ordinary writes through
shared references, incompatible recasts and invalid orderings. The write-only
stored-pointer control rejects reference formation before any observation. It
supplies no observed read result.

Records keep actual source stamps. The nine changed control inputs have fresh
source provenance but preserve earlier golden timings because their rejection
outcomes match. Raw reports retain the actual fresh timings. The Linux combined
record file remains absent; its full report is retained separately.
The Linux native record uses
its genuine native-only probe at `2bb67af4e82e`. Other native-only pins retain
their original lane. The first `af33106a1e7` workflow attempt passed the
compiler check, then failed in its Python wrapper before preparing a producer;
the corrected run supplies the observations above. No execution of the final
metadata commit or corrected inputs on another host is claimed.

The original unlowerable local, metadata-selection failures and ARM LSE-form
refusal remain historical observations. Shared-receiver acceptance and any
required std repair, other admitted access and representation cases, forced
store reservation failure, concurrent witnesses and formal memory-model
refinement remain open. No timing or progress guarantee is claimed.
