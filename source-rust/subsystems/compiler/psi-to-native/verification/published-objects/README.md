# Published-object verification

The native driver passes the finished image and its retained stage outputs to
the requested validators. The validator reports results for the root object,
its private callable bodies and the runtime's component objects. Each body is
matched to its originals by function identity or component slot.
[Validator requests and results](../stage_validators.rs) · [Object checks](src/lib.rs)

Target-operation correspondence reads the execution plan retained in the object.
It is the only reader of stage 02's abstract program and stage 04's target
operations after instruction selection: it proves each access reference stage
04 formed against the backing the selected program allocates, and checks that
every emitted function kept the calling contract stage 04 lowered for it.
Image emission does not depend on either form.
[Access backing geometry](src/backing_geometry.rs)

Fixed-frame correspondence checks the original allocation, physical machine,
encoding and baseline layout. Selected-instruction rewrite replay checks the
recorded rewrite run when the compilation applied one. A request that needs
missing originals reports a diagnostic.
[Allocation correspondence](src/allocation.rs) · [Machine-code representation](../../representations/machine-code/)
