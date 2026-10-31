# Fragmented storage compositions

These are composition witnesses for the fixed-layout representation mechanism, not additional nine-place rows.

[Forwarded semantic mutation](forwarded_mutation/main.omg) updates mode and a split code field through an ordinary mutable call, then independently observes mode 7, code 2, the preserved present flag, and the complete encoded byte 47.

[Forwarded byte mutation](mutable_full_byte/main.omg) writes byte 165 through an unrestricted mutable byte view of a complete non-atomic representation. It independently observes the decoded payload 90, untouched neighbor 195, and both physical bytes. Its [domain twin](mutable_full_byte/invalid/main.omg) changes only the semantic payload type to Nonzero: arbitrary byte writes cannot establish that stronger predicate, so reverse representation validity must reject before execution.

The positives bind all four required hosted targets and have authored exit zero. Exact producer87 (`bc022a133a9ab10d6ba88e5eb9fa893ee72e8ad2`) observed native and Terminal exit zero on the pinned paths; the domain twin rejected for missing two-way validity. These observations establish the compositions, not C ABI agreement, general conversion families, or completion of the parent task.
