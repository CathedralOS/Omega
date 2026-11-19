# Published-object verification

The native driver passes the finished image and its retained stage outputs to
the requested validators. The validator reports results for the root object,
its private callable bodies and the runtime's component objects. Each body is
matched to its originals by function identity or component slot.
[Validator requests and results](../stage_validators.rs) · [Object checks](src/lib.rs)

Target-operation correspondence reads the execution plan retained in the object.
Fixed-frame correspondence checks the original allocation, physical machine,
encoding and baseline layout. Selected-instruction rewrite replay checks the
recorded rewrite run when the compilation applied one. A request that needs
missing originals reports a diagnostic.
[Allocation correspondence](src/allocation.rs) · [Machine-code representation](../../representations/machine-code/)
