# Bounded byte loan reproducer

This complete application intentionally demonstrates a current compiler gap,
not the intended-invalid range control. On unmodified main
`30ec6a20142f4e79e236c20912455939df40ea96`, `omega update --project` rejects
its valid shared fixed-byte-array loan on a guarded state edge:

```text
Main::main has no executable body
state graph: terminator: guarded jump successors: loan transfer
```

The full sources and normal std build binding are retained here. Its intended
behavior is stdout `TEST\n`, empty stderr and exit 0. The failure was reproduced
with a compiler built from that exact base, independently of active workers.
The successor planner requests a Reference carrier for the bounded byte view,
whose structural identity already names its own byte descriptor. No custody
guard or type identity was altered. The delivered scheduler keeps input storage
in its private named `Input` record and uses ordinary named-record state loans.

```text
mkdir -p samples/cli/independent/resource_scheduler/build/reproduction
cp samples/cli/independent/resource_scheduler/validation/reproduction/main.omg samples/cli/independent/resource_scheduler/build/reproduction/main.omg
cp samples/cli/independent/resource_scheduler/validation/reproduction/build.source.omg samples/cli/independent/resource_scheduler/build/reproduction/build.omg
OMEGA_LIBRARY_ROOT="$PWD/source/library" /path/to/verified/omega update --project samples/cli/independent/resource_scheduler/build/reproduction --target linux_x86_64
```

This is retained as a source-backed compiler-gap report. It is not counted as
a passing application test or an intended source rejection.

The metadata fixture is named `build.source.omg` to keep normal passing-sample
discovery from treating this retained compiler-gap witness as a passing sample.
