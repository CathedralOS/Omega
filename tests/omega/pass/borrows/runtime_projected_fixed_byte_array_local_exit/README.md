# Captured fixed-byte borrow over projected storage

The record's array is projected into a local `&[u8; 65536]`. Both direct indexing
and a helper observe the first and last values, 17 and 29; the original ranked countdown then returns
zero while forwarding that same view. The driver also checks the surrounding
fields, 71 and 83. Native and Terminal execution must both exit zero.
The same observations over a neighboring `u32` array exercise ordinary stored
reference geometry, including a helper reading the suspended caller's backing.

```sh
omega run --both tests/omega/pass/borrows/runtime_projected_fixed_byte_array_local_exit/main.omg
python3 tools/corpus_gate.py --both --filter borrows/runtime_projected_fixed_byte_array_local_exit
python3 tools/corpus_gate.py --filter borrows/projected_fixed_array_shared_source_mutable_local
```

Ordinary package/entry review still applies; no acceptance policy is included.
Runtime qualification here is macOS ARM64, not evidence for the other bindings.

Before repair, the same composition refused reference formation because the
destination was not Reference-shaped. Formation now uses the shared carrier
producer, and later projections use the existing exact borrowed-referent
geometry relation. Fixed byte carriers borrow like dynamic views, but store
one pointer rather than a pointer and length. Native custody and publication
retain that declared width; access remains independently checked. The borrowed
call ABI takes the address of the descriptor, not its backing pointer.

Local observation and reference formation classify the reference shell rather
than selecting slice/named source families or the ordinary pointer roster.
The shared carrier producer chooses the declared descriptor operation; native
reads and retirement use the same declared geometry for arrivals and produced
locals. A fixed view's extent is a type-owned constant, never a length-word load.

The paired invalid program forms a mutable fixed-array local from shared
record access and must reject. Neither matching extent nor descriptor
construction supplies mutable authority.
