# Saved indexed IEEE observation across a disjoint store

The guard observes one completed array selector and establishes finite bounds for
unqualified `I32::from_f64`. A later store changes another field of the same plain
record. The captured element remains unchanged because the complete write frame
is disjoint from its entire containing collection, rather than because of a
reconstructed selector or a guard added after conversion.

The runtime witness checks finite, out-of-range and NaN inputs, reads the changed
sibling field after conversion, and binds all four hosted ProgramEntry targets.
Both native and Terminal execution must return zero.

[Fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md) requires
exact value/storage identity; [numeric values](../../../../../wiki/spec/language/numeric_values.md)
requires finite, representable truncation. The
[reject twin](../../../fail/float/ieee_collection_write_retires_guard/main.omg)
changes only the store target to the observed collection element. The frozen
pre-repair positive refused the conversion; the twin refused finite/truncation
admission too. Fresh repaired-engine agreement remains the qualification gate.
