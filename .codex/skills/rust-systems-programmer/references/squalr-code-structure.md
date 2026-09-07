# Squalr: where decisions and state belong

Inspected at `e6538c99b2960f35a44727d09acf5f58ef2284ab` in `Y:/Development/Friends-and-Family/Zac/Squalr`. Paths are relative to that checkout. These are implementation patterns, not benchmark results or requirements to reproduce its framework.

## Prepare shared decisions; borrow them in local plans

`squalr-engine-api/src/structures/scanning/constraints/scan_constraint_finalized.rs`: `ScanConstraintFinalized::new` computes periodicity and unit size and prepares scalar plus 16/32/64-byte vector comparison functions.

`squalr-engine-api/src/structures/scanning/plans/element_scan/snapshot_filter_element_scan_plan.rs`: `SnapshotFilterElementScanPlan<'lifetime>` borrows that finalized constraint and adds alignment, tolerance, and a local planned scan type. Reusable preparation is separate from choices that depend on the current filter.

Transfer when repeated work shares expensive preparation but differs in local geometry. Ask which facts change per request, partition, and element. Keep each decision at its natural frequency. Eager preparation has a cost, including unused variants; do not create a planning layer for a one-shot operation without a reason.

## Branch once around the loop

`squalr-engine-scanning/src/scanners/vector/scanner_vector_aligned.rs`: `scan_region` matches `ScanFunctionVector::Immediate` versus `RelativeOrDelta` outside the vector loop. Immediate comparisons receive current bytes; relative/delta comparisons receive current and previous bytes. Each branch owns its loop and tail handling.

Transfer when the condition is invariant for the batch. Modest loop duplication can expose simpler iteration and avoid repeated mode decisions. Share the actual common kernel helpers, such as result encoding, rather than forcing every mode into a universal loop. Inspect generated code or measure before claiming the source shape improved machine code.

## Select dynamically; specialize a bounded dimension

`squalr-engine-scanning/src/scanners/element_scan_dispatcher.rs`: `aquire_scanner_instance` selects a scanner through `&'static dyn Scanner`, while concrete vector implementations use const-generic widths such as `ScannerVectorAligned::<16>`.

`squalr-engine-api/src/structures/scanning/comparisons/scan_function_vector.rs`: comparison functions remain `Arc<dyn Fn(...) + Send + Sync>`. The runtime-selected scanner and statically known width coexist with an indirect comparison call per vector. This is not fully static dispatch.

Transfer when a small fixed set of kernel dimensions benefits from specialization while the application still needs runtime choice. Balance dispatch frequency against code size and compile time. Do not monomorphize every configuration axis or promise that selecting a kernel removes callback overhead.

## Stateless algorithm, invocation-owned working state

`squalr-engine-scanning/src/scanners/snapshot_scanner.rs`: `Scanner: Send + Sync` accepts borrowed region, filter, and plan and returns an owned result vector. The aligned scanner is an empty struct; `scan_region` creates its run encoder locally. Dispatch can reuse a static scanner object without sharing its mutable working state.

Transfer by separating algorithm selection from execution state. Put scratch buffers and result builders with the invocation or owning worker; do not attach a mutex-protected scratch field merely because the algorithm is represented by an object. Reuse scratch across calls only when its lifetime and concurrency contract are explicit.

## Function signatures expose the real input modes

The comparison enum distinguishes a one-pointer immediate function from a two-pointer relative/delta function. The caller selects the mode before entering the loop, and implementations receive the input set their mode uses.

Transfer when modes differ in required inputs or ownership. Prefer a precise enum or signature to a catch-all context full of optional fields. These particular raw-pointer callbacks are not a reusable safety guarantee: their caller must establish readable ranges and lifetimes. In safe library interfaces, use slices or existing validated view types where appropriate.

## How to use this reference

Choose the decision boundary first: request preparation -> local plan -> selected kernel -> invocation-owned state -> output. Use only the pieces the destination needs. Names such as `Finalized` do not themselves prove immutability or validity; inspect construction and mutation. Existing source quirks, unsafe registries, and handwritten dispatch boilerplate are not style requirements.
