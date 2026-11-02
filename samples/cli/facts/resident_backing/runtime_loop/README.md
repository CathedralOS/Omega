# Allocation descriptors through a loop

Parent: **BUMP-ALLOCATOR-CANARY / VEC-NATIVE-GROWTH**. This is another transport
production of the existing `resident_backing` fact, not another size family.

The loop carries two distinct, qualified allocation descriptors unchanged.
Its result promises both original bases and lengths. The caller uses those
promises to rejoin the allocations, then writes and reads actual backing with
the runtime-cell production's neighbor checks. Zero and nonzero iteration
counts execute. The invalid twin swaps the returned descriptors and must reject
the original-base and original-length guarantees.

The flow owner must establish each invocation-to-loop-parameter identity on
every arrival. Return checking observes the completed result under that loop
header's own contract, rather than unfolding the cycle or assuming its desired
postcondition. A swapped backedge has no original-parameter identity to import.
The boundary providers supply storage and permissions only, not these equalities.

Run `omega run --both samples/cli/facts/resident_backing/runtime_loop/main.omg`;
check `invalid/main.omg`. The baseline rejects the two base-address guarantees;
the repair executes with exit 0 in both engines on Windows x86-64. All four
hosted targets are bound, but other hosts have not been executed here.
The row's existing nine places and rejection twins remain the transport controls.
This prerequisite does not close Vec prefix-copy, growth, release or reset.
