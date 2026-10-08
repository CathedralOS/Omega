# Descriptor prefixes with stored reference custody

The public command is:

```sh
omega run --both tests/omega/pass/host/runtime_descriptor_reference_record_prefix_exit/main.omg < tests/omega/pass/host/runtime_descriptor_reference_record_prefix_exit/input.txt
```

Both engines exit 0 and emit `BC` followed by a newline. The record contains an
eight-byte integer header, a shared reference to original caller storage, an
atomic cell and a scalar tail. Reading one input byte through a helper changes
only the header's low byte. Shared output sees the header before and after that
write; the header's other bytes, the reference's original referent, and the
scalar tail stay intact.
Zero-count and EOF reads also preserve that original storage.
Atomic helper events observe the untouched cell, update it, and observe the
same cell again; lending the enclosing record grants no ordinary byte access to
its atomic resident or a copied atomic snapshot.
A later atomic store through the stored reference is observed through the
original owner, so a same-valued snapshot of the referent cannot satisfy the oracle.

Borrowing this record is not an owned copy or a loan of its reference's referent.
The retained native scalar geometry and interpreter capture stop at descriptor
leaves. Access into the uncovered pointer interval is unavailable, not synthesized
from an interpreter identity. Opaque mutation summaries remain opaque; their
incompleteness cannot prevent emitting an independently checked earlier store.
