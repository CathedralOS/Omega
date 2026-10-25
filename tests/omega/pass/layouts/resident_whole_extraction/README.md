# Whole resident extraction

`attempt` is a public embedding entry, not a hosted application. Export it with
`omega inspect-terminal --machine attempt --target uefi_x86_64 --output <artifact>
main.omg`. Supply an admitted vacant extent at base 4096 with 16 bytes of backing
initialized to `0xa5` and an actual Stable read/write profile.

The source initializes Word17, transports the resident through a state and an
ordinary helper, extracts it, checks the returned value, and returns the original
vacant extent plus Word. The embedding observation must retain the original
buffer, extent geometry, origin and lineage; its first eight bytes encode 17 and
the remaining eight stay `0xa5`. A second vacant handback must refuse.

With actual write-only supply, initialization is allowed but extraction must
refuse the read/write description's mismatch with that original admitted supply
without retiring the resident. A provider
description claiming read/write cannot amplify the actual allocation's rights.

The companion `resident_aggregate_extraction` exercises arrays, nested records
and linear content custody. This embedding fixture alone does not establish
native execution. Cathedral's UEFI witness separately checks a complete-width
read/write resident through initialization, mutation and extraction.

`OMEGA_CORPUS_HOST_PROVISIONS=1 python3 tools/corpus_gate.py --filter
pass/layouts/resident_whole_extraction` executes the public embedding provision
on both borrowed and loaded Terminal artifacts. The declared 3873-cell ceiling
admits the complete journey; one cell less refuses without publishing a result
or replaying effects. Resumption uses one-unit fuel grants. Failed instances
retain their custody until teardown, which releases all live cells. Successful
handback releases the allocation record and its descriptors and cannot repeat.
These are logical record counts, not byte-buffer capacity or RSS limits.
