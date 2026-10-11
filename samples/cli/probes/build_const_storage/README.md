# build_const_storage

Const eval inside build.omg: `Buffer<leaf()>` instantiates const-generic data through a machine call; the build program inspects `result.buffer.slots.len` to choose the binding arm; `BuildProduct::entry(&builder.product, ...)` queries the product (pure-call discard now emits a dead-code warning).

Compile-checked only: run a check target, not native.
