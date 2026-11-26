# Proposed memory-backed directories for interpreted runs

Status: proposal. The [runtime mapping decision](../../spec/build/command_line.md#runtime-directory-mappings)
covers live disk directories. This proposal concerns optional backing for
individual mappings in the same guest filesystem.

An editor may already hold unsaved buffers in memory. Its embedding caller
could supply those bytes under their project filenames so an interpreted tool
can read them through the ordinary filesystem provider. Another caller might
want an empty writable scratch directory whose contents disappear at exit.
In either case other guest directories can still map to live disk. For example,
a parser could read unsaved files at `/input`, work in memory at `/tmp`, and
publish its result through a writable disk mapping at `/out`.

A private copy of a populated directory is a distinct request: its initial
contents must be stable and writes must stay private. Disk, memory or a
copy-on-write implementation could satisfy that behavior. Read-only access to
live disk continues to observe changes made by other authorized writers.

Before adopting a memory-backed mode, decide how a caller supplies initial
contents, bounds storage, and retains or discards writes at exit. An embedding
caller can motivate this capability without a CLI encoding. No flag spelling
or default is proposed here. Build-stage source selection and child backing
remain [separate questions](../../../OWNER_QUESTIONS.md#build-product-source-view-selection).
