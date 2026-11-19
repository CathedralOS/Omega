# Native verification

Verifies current physical forms and the originals an explicit audit requested.
The ordinary pipeline supplies completed records; it does not install callbacks
into source or earlier compiler stages. Missing original inputs refuse replay.

Allocation and physical replay retain their issued, immutable original objects.
The check implementation reads a private projection of their exact instructions,
assignments, register environment, preservation and publication records. Raw
records cannot replace these originals: private constructors carry guarantees
that a content hash alone does not establish. Relaxation likewise retains its
optimizer-issued custody and exposes an immutable record to its checks. Encoding and
layout replay receive immutable current forms, without their producer wrappers.
The checks of encoding, layout and relaxation live in `encoding-admission`.

Selected-instruction rewrite replay still imports its checker from the old
selected-rewrite stage. Allocation and physical originals now come from
`physical-planning`, where their constructors and operations moved together
behind private modules. Relaxation custody remains with its optimizer pending
the final layout owner. Widening a constructor or replacing issued custody with
raw fields is not a valid shortcut.
