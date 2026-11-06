# Keep 33 private callback selections in one calling plan

The checked registrar takes one borrowed envelope containing 33 private slots.
Its calling policy joins each destination to its exact callback requirement,
including a distinct second requirement. It invokes the first, second and last
selected handlers with distinct results and checks adjacent and outer fields.
Expected native and Terminal exits are both 0.

This existing fixture uses its ARM64 calling plan. A checked registrar calls
selected machines directly; it does not load the emitted private pointers.
The companion `layouts/private_callback_descriptor_fragments_exit` fixture
executes all 33 stored native pointers through a C registrar. Its negative twin
swaps two unlike callback requirements while keeping every binder unique.
