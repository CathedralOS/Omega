# Generic scan with borrowed results

The program scans borrowed collections with statically selected predicates and
explicit context. It counts matches over integers, bytes and records, forwards a
predicate through a generic batch, and returns the first matching integer through
the bundled `Optional<&T>` type without copying its backing. Expected exit: 0;
a wrong count or first match crashes.

Run after ordinary package preparation:

```sh
omega run --both samples/cli/collections/generic_scan_count/main.omg
```

The repaired compiler runs this program with exit 0 natively and in Terminal
interpretation on macOS arm64. Replacement disposes the old descriptor after
capturing the right-hand side; the corpus audit retains disposed parent ancestry
while a child survives. Returned recasts preserve their access and complete view
history through aliases, replacements, records and state transfer.
The corpus exercises replacement and returned-view composition separately from
invalid controls for bounds, access, and backing lifetimes.

The language's [returned-view contract](../../../../wiki/spec/language/lifetimes.md#returned-views)
requires the result to retain its input lifetime and exact backing. A fixed-size
scan, copied result or boolean-plus-payload API is not acceptance of this program.
