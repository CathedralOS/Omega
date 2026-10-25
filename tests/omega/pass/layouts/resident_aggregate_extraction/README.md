# Aggregate resident extraction

Export `attempt` using `omega inspect-terminal --machine attempt --target
uefi_x86_64 --output <artifact> main.omg`. The embedding supplies an admitted
vacant extent at base 4096 with 40 actual bytes initialized to `0xa5` and a Stable
read/write profile.

The source initializes and extracts a nominally linear Word containing a scalar,
a two-element array and a nominally linear nested record. It checks 17, 23, 29
and 31 respectively, then returns the vacant extent and intact Word. The result
must carry two distinct whole claims: the original extent and the original Word.
Word's nominal claim retains its nested obligation; it is not a claim minted from
the decoded bytes. The host checks the original allocation identity, geometry,
origin and lineage, the 32-byte encoded image and the unchanged eight-byte tail.

The opt-in corpus host provision executes both borrowed and loaded artifacts
with a 3884-cell ceiling and 64-byte placement-image payload ceiling. It checks
one-cell-short, one-byte-short and one-byte-only refusals; a root reservation
made after the initialization image retires independently forces extraction to
refuse. Actual write-only backing also refuses the source provider's read/write
description at extraction. One-unit fuel resumes never replay the operation;
sticky faults retain custody until teardown, which releases all reservations.
Metadata and host-supplied backing capacity are not placement-image payloads.

This is interpreter lifecycle evidence through the public embedding API, not a
native application or Cathedral boot claim. Cathedral's separate native witness
exercises a scalar record; it does not establish this aggregate's native oracle.
