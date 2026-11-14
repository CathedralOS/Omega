# Explicitly installed entry storage

Owning task: **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
This row tests the entry-memory and lifetime obligations of [entry roots](../../../../wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge).

**Fact:** An explicitly installed private `Registers` allocation lends its exact
exclusive placed view, reading status 41 and writing transmit 9 into the same backing
before handback. The nine examples perform the same read and write. The code byte 9
travels through each place before the same placed write consumes its proved contract.
Each invalid example supplies 8 and rejects at the unchanged write contract. No extra
caller condition supplies the missing fact.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [unproved write](invalid_local/main.omg) |
| field | [program](field/main.omg) | [unproved write](invalid_field/main.omg) |
| parameter | [program](parameter/main.omg) | [unproved write](invalid_parameter/main.omg) |
| call_result | [program](call_result/main.omg) | [unproved write](invalid_call_result/main.omg) |
| match_join | [program](match_join/main.omg) | [unproved write](invalid_match_join/main.omg) |
| const_name | [program](const_name/main.omg) | [unproved write](invalid_const_name/main.omg) |
| other_state | [program](other_state/main.omg) | [unproved write](invalid_other_state/main.omg) |
| other_machine | [program](other_machine/main.omg) | [unproved write](invalid_other_machine/main.omg) |
| target_body | [program](target_body/main.omg) | [unproved write](invalid_target_body/main.omg) |

The target-body place calls `TransmitCode::value()`. Its Build companion selects an
ordinary named implementation for each hosted target. Each positive implementation
returns 9; each twin implementation returns 8 and reaches the same consuming write
requirement. These equal provider values demonstrate preservation of this storage fact,
not distinct operating-system behavior.

The match-join place derives both branch inputs from actual external status reads,
preventing a literal input from erasing the runtime selection. Each branch carries the
same proved code byte 9 and a distinct tag (11 or 13); after the unchanged placed write,
the program traps if the selected tag differs from that input. Thus an inverted or
constant branch cannot pass merely because both code fields are 9. The reject twin
changes only both code fields to 8 and still refuses the write contract; the tag check
never establishes the code requirement.

The nine valid examples also have corpus tests under `tests/omega/pass/facts/` as
`installed_entry_storage_through_<place>_exit`. The invalid examples remain sample
projects. Their corresponding invalid corpus tests are still missing, so this row is not
complete. Every positive binds all four hosted targets. `entry_ram.txt` explicitly
requests initialization for source occurrence 0; `expected_entry_ram.txt` lists the
expected bytes in the original memory after execution. The portable test entrypoints
consume those same files:

```sh
omega run --both --keep --entry-ram 0=2900000003000000 main.omg
python3 tools/sample_native_gate.py --filter facts/installed_entry_storage
python3 tools/corpus_gate.py --both --filter installed_entry_storage
```

`entry_ram.txt` rows are `source-position=hex-bytes`, never addresses. The runtime
checks each initialization entry against its scalar encoding, complete size, alignment,
required padding, placement, access and cleanup that runs no program code before
construction. Absence provides no storage. This grants neither MMIO nor host pointers,
callbacks, external objects, compound/atomic/erased custody or unsupported
qualifications.

Establishment uses those actual typed bytes, not Build selection or a mutable
parameter's shape. The runtime keeps separate writable memory for the loader's saved
return state, the checked application stack, each original allocation and each
observation header. The callee receives only its declared occurrences. No ambient
mutable global or hidden receiver enters source. Ordinary lifetime, loan, qualification,
termination and cleanup checks remain in force.

After normal return, the runtime retires the entry loans and observes the same original
allocations. The private observation protocol (FD 3 on Unix; a process-bound named pipe
on Windows) binds the canonical artifact, placement seal, occurrence and complete byte
count; guest stdout/stderr are not this evidence. A partial write, wrong header,
missing/extra frame or absent normal return cannot pass. The public CLI compares actual
native and Terminal backing independently of their exit agreement. With `--keep`,
inspect `native.entry-ram.0.bin`, `terminal.entry-ram.0.bin`, `native.entry-ram.frames`,
`native.identity.txt`, `program.oterm` and `terminal.effects`; both backing files must
contain `29 00 00 00 09 00 00 00`.

Checked-to-Terminal lowering retains the exact access loan, scalar read/write and
selected entry identity. Terminal installation constructs its own private allocation
from the checked recipe and observes the original after execution. Provider settlement,
layout geometry and ABI planning retain source/physical coordinates and each pointer's
exact record geometry. Native realization charges the callee's worst-case stack usage to
a separate stack region. ISA selection and final Mach-O/ELF/PE emission establish
initialization before lending and observation after return. A separate instruction
reader reconstructs the final pointer addresses, argument registers, exact semantic
call, stack switch/restore, full writes and loader continuation, and the final image
header must select that exact adapter.

The [two-occurrence
control](../../../../tests/omega/pass/build/installed_entry_ram_two_occurrences_exit/main.omg)
additionally checks argument order and distinct backing: occurrence 0 remains
`2900000003000000`, and occurrence 1 changes from `3500000005000000` to
`3500000009000000`. The existing rejection tests cover:

- [missing backing](../../../../tests/omega/fail/build/installed_entry_ram_missing_backing/main.omg);
- [wrong source occurrence](../../../../tests/omega/fail/build/installed_entry_ram_wrong_occurrence/main.omg);
- [incomplete initialized extent](../../../../tests/omega/fail/build/installed_entry_ram_invalid_extent/main.omg);
- [noncanonical initialized padding](../../../../tests/omega/fail/build/installed_entry_ram_invalid_padding/main.omg);
- [overlapping exclusive root uses](../../../../tests/omega/fail/build/installed_entry_ram_overlapping_exclusive_roots/main.omg).

The initial frozen macOS ARM64 producer at `436add4eac90` refused the valid root at
native admission because it had no matching installed-argument lender. The repaired
macOS bridge executed the root, all nine places and two-occurrence control with
native/Terminal exit 0 and exact original-backing observations; all twins and storage
controls refused their own obligations. Final producer-bound qualification records
retain actual sources, target and engine observations. Linux x86-64 and Windows x86-64
qualification records execute the row and the source-access customer with actual
native/Terminal agreement and independent original-backing readback. The Windows lender
follows the Microsoft ABI, keeps platform calls on the original loader stack, and uses a
separate checked activation stack for the application. Its final PE adapter has an
independent instruction reader. A process-bound private pipe carries a bounded complete
observation after normal return; stdout/stderr and child execution are bounded
separately.

The [stack-argument
witness](../../../../tests/omega/pass/build/installed_entry_ram_stack_occurrence_exit/main.omg)
lends five distinct occurrences, crossing the first Windows stack-argument boundary
after four register arguments. Only occurrence four changes its transmit byte to 9.
Linux ARM64 uses the same exact-allocation lifecycle and AAPCS64 X0–X7 pointer
placement, with Linux write and exit-group syscalls and an independent final-byte
reader. Qualification records distinguish named QEMU/PRoot execution on Linux x86-64
from physical ARM hardware execution. The current macOS ARM64 run below observes these
existing programs. Current Windows x86-64 and Linux ARM64 migration execution remains
unverified. Unsupported native placements still refuse explicitly. This is private
hosted RAM, not a device-provider or Atomic establishment claim.

The companion [free local-state row](../free_entry_local_state/README.md) proves
ordinary local ownership and executable cleanup without supplied parameters. [Applied
zero establishment](../applied_zero_establishment/README.md) retains invalid
initialization controls when construction moves into ordinary locals. These establish
separate obligations of the same parent outcome.

Measured validation uses the following sources:

| Source and run | Observed scope |
| --- | --- |
| Linux x86-64, compiler `0b53c8a4e109`, [original baseline](https://github.com/CathedralOS/Omega/actions/runs/42168270932) | 18 source checks, 13 native/Terminal exit-zero pairs with original-backing agreement, ten ordinary positives and eight matching invalid expectations. The nested invalid rejected 8 against 9 but missed its required diagnostic wording, so the full run failed. |
| Linux x86-64, compiler and programs `ab235e9fbe19`, [migration qualification](https://github.com/CathedralOS/Omega/actions/runs/42172876301) | Target source check and native/Terminal exit-zero pair with original-backing agreement; ordinary target positive and its invalid twin; nested invalid with the unchanged expected wording; five existing refuted-precondition controls and one existing dependency-recovery integration test. |
| Compiler and programs `062d0753ff08`, observer `7e32952871ae`, [Mac ARM64 qualification](https://github.com/CathedralOS/Omega/actions/runs/42190647957) | 13 programs pass source checking and five invalid storage tests reject; 13 native/Terminal exit-zero pairs with original-backing agreement; ten sample programs run on both engines; nine controls reject. |

The Mac run observes 19 original corpus allocations and the expected backing for
each ordinary positive. Its 27 unit-test results belong to the authenticated
original compiler preparation. These are measurements of the listed compiler
and program sources, rather than execution of a newer main revision.

The qualification runs keep the 120-second corpus limit and the ordinary sample gate's
300-second limit per CLI call, with one worker and profiling off. All nine Mac invalid
samples reject at the consuming `code == 9` requirement. The diagnostic repair adds
requires-contract context without changing any checking decision or removing the
previous message.

For Linux x86-64, the other eight places and storage controls retain their actual
original baseline observations. Their authored inputs are unchanged; the compiler change
only adds text to a diagnostic after a precondition is already refuted. It changes no
accepted result, storage geometry, access, runtime code or backing observation. These
retained measurements remain measurements of compiler `0b53c8a4e109`, rather than
execution of the final source.

Current matching-host validation of this migration on Windows x86-64 and Linux ARM64
remains open. The historical host observations above do not establish current execution.
The invalid examples still lack corresponding corpus tests. The broader Build-owned
selection requirement, including distinct per-target behavior from one portable
artifact, also remains open.
