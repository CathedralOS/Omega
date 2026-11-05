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
| Linux x86-64 | `2bb67af4e82e` | 0 / 0 |
| Windows x86-64 | `7b9ecb41a26f` | 0 / 0 |
| macOS ARM64 | `a1da0d793346` | 0 / 0 |
| Linux ARM64 | `a1da0d793346` | 0 / 0 |

The integrated Linux scope checks four positives and eleven intended refusals,
runs four native/Terminal zero pairs, and runs two ordinary CLI projects.
Matching Mac and ARM scopes cover the same programs. Windows has two positive
checks, ten intended refusals, two zero pairs and the ordinary global project.
The additional programs challenge a real field alias and untouched sibling,
strong compare-exchange mismatch and argument order, and bounded bitwise
updates. Actual compiler profiles, source bundles and raw reports are retained
with [PR #1865](https://github.com/CathedralOS/Omega/pull/1865).

Controls reject ordinary scalar atomic access, ordinary writes through shared
references, incompatible recasts, invalid orderings and stores that violate
the stored value’s declared bound. The write-only stored-pointer control
rejects reference formation before any observation. It supplies no observed
read result.

Incoming Source changes were checked and executed on Linux at `2bb67af4e82e`.
The other hosts retain their measured sources: incoming changes affect Source
checking and retain the same native layout, ISA and ABI consumers for these
explicit variables and parameters. Records keep actual source stamps. The
separate Linux combined record file remains absent; its full combined report
is retained with the qualification. The Linux native record uses its actual
native-only probe; all other native-only pins retain their original lane.
Metadata changes do not claim execution of the final metadata commit.

The original unlowerable local, initial metadata-selection failures and prior
ARM LSE-form refusal remain recorded as historical observations. The
shared-receiver example and its required std migration, other admitted
access/representation cases, forced store reservation failure, concurrent
witnesses and formal memory-model refinement remain open under the parent
task. No timing or progress guarantee is claimed.
