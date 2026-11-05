AtomicU32 and AtomicU64 locals execute loads, stores, fetches, swaps and
compare-exchange. AtomicBool locals execute loads and stores. This completes
the executable-entry requirement of [ATOMIC-MEMORY-MODEL](../../../../../TASKS.md).

[main.omg](main.omg) retains the original operations and sum of 265. The program
checks every observed value and final stored value; its entry checks the returned
sum, then exits zero. A wrong value traps. Checking, Terminal execution and native
code carry the actual variable or field address. AArch64 updates use baseline
exclusive loads/stores. Strong compare-exchange retries a failed store and
completes on mismatch without an update; single-attempt exchange remains one
attempt.

| Matching host | Measured compiler/program source | Native / Terminal exit |
| --- | --- | --- |
| Linux x86-64 | `524d0e57db23` | 0 / 0 |
| Windows x86-64 | `7b9ecb41a26f` | 0 / 0 |
| macOS ARM64 | `a1da0d793346` | 0 / 0 |
| Linux ARM64 | `a1da0d793346` | 0 / 0 |

Linux, Mac and ARM each have four actual zero pairs and two ordinary CLI
agreements. Windows has two zero pairs and the ordinary global project. The
additional programs challenge a real field alias and untouched sibling, strong
compare-exchange mismatch and argument order, and bounded bitwise updates.
Profiles, source bundles, raw reports and independent reviews are retained with
[PR #1865](https://github.com/CathedralOS/Omega/pull/1865).

The integrated Linux report remains failed because an old reject helper permitted
values through 101 but only received 41. Incoming private-call inference correctly
proved that safe call. All nine old corpus twins now check. Input source
`29b899640ddf` changes only their bad inputs to 101 in the nine corpus and nine
sample controls. The same compiler, `524d0e57db23`, rejects all eighteen at the
actual store into Cell's unchanged bound of 100. Positive programs still store
41 and check 59. The original failed reports and source hashes remain retained.
Together with ten unchanged controls checked at `524d0e57db23`, the current
Source evidence covers nineteen intended refusals. No runtime observation was
repeated or relabeled for this input correction.

Controls reject ordinary scalar atomic access, ordinary writes through shared
references, incompatible recasts, invalid orderings and stores that violate the
stored value's declared bound. The write-only stored-pointer control rejects
reference formation before any observation. It supplies no observed read result.

Actual incoming Source consumers were checked and executed on Linux at
`524d0e57db23`. Other hosts retain their measured sources and physical evidence:
the incoming changes preserve the native layout, ISA and ABI owners for these
explicit variables and parameters. Records keep actual source stamps. The Linux
combined record file remains absent; its full report is retained separately.
The Linux native record uses its actual native-only probe at `2bb67af4e82e`.
Other native-only pins retain their original lane. No execution of the final
metadata commit or of corrected inputs on another host is claimed.

The original unlowerable local, metadata-selection failures and ARM LSE-form
refusal remain historical observations. Shared-receiver acceptance and any
required std repair, other admitted access and representation cases, forced
store reservation failure, concurrent witnesses and formal memory-model
refinement remain open. No timing or progress guarantee is claimed.
