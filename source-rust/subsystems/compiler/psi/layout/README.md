# Psi storage layouts

This owner carries chosen storage geometry, layout reports, field and symbolic
coordinates, placement constraints, stored-integer fit contracts and symbolic
write records. Canonical fingerprints and shape checks belong to these records;
neither chooses a default layout nor writes storage.

[Geometry](geometry.rs) describes sizes, alignments and field offsets.
[Packing](packing.rs) computes offsets for records and sums from the supplied
field geometry. The [integer defaults](integer-defaults/integer_defaults.rs)
choose the default geometry for a standalone integer or a record field.

The interpreter's [materializer](../../../interpreter/memory/materialization/src/lib.rs)
reads and writes bytes using a chosen layout. Its tests use the same layout records.

The field identity and stored-integer modules expose exact record indexes and
validation to consumers. Their values confer no source acceptance, provider
admission or live storage authority.
