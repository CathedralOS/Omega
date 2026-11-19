# Physical planning

Allocates one selected program, chooses its physical instruction alternatives,
executes the selected physical optimizations, and fixes its frame recipes before
encoding. `plan_physical_program` is the entrance. Allocation, spilling,
reanalysis and physical alternative selection are private operations; their
previous stage packages and public entrances are removed.

The output contains physical instructions, checked frame geometry and immutable
frame recipes. No frame bytes are produced here. The common encoder reads the
completed recipes. The native coordinator currently closes callback and entry
bodies and binds their resources around this operation; moving that whole
closure into its final owner remains part of the integrated migration.

An explicit audit may retain the issued allocation and physical-plan originals.
Their private constructors and full operations moved together. Raw fields or a
matching hash cannot substitute for their solver and completed-run guarantees.
The explicit verifier reads them without calling allocation or instruction
selection. Borrowed atomic counters record entered operations, including failed
attempts; these observations do not influence admission or become program data.

`frame-admission` retains independent keyed storage reconstruction and frame
geometry checks. Its byte check shares the existing target frame recipes; that
check is not an independent implementation of the ISA encoder.
