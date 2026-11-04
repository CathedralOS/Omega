# Load and restore the segment-table pointer

The Omega body saves GDTR, loads a table owned by the CPU harness, reads it back, restores the original GDTR and returns. The caller retains one exclusive borrow of the table and descriptor buffers throughout the call. Each instruction receives exactly ten plain bytes; the loaded address grants no Omega pointer authority.

The [CPU harness](../../../../../tools/native_entry_cpu/README.md) independently checks the original GDTR, the loaded base and limit, all ten stored bytes, six immediate byte canaries, outer word canaries, RSP, callee-saved registers and IF/DF. Eight successful ordinary calls produce `B........P` and QEMU exit33. The hosted application only demands the compiled private entry and is never executed.

This is a scoped descriptor-load test in a closed x86-64 environment. It excludes asynchronous delivery, single stepping and hardware breakpoints during the temporary load. It does not reload segment registers, load a TSS, publish an IDT, install a timer or issue interrupt acknowledgements. Terminal interpretation requires a GDTR environment and currently refuses this operation explicitly. Native CPU execution is the oracle.
