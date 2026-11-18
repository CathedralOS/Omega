# Storage layout records

This owner carries chosen storage geometry, layout reports, field and symbolic
coordinates, placement constraints, stored-integer fit contracts and symbolic
write records. Canonical fingerprints and shape checks belong to these records;
neither chooses a default layout nor writes storage.

`layout-planning` owns integer defaults and conventional record/sum packing.
`storage-materialization` owns symbolic write derivation, byte application and
decoding. The existing layout behavior tests live with the materializer and use
these public records directly. The representation's normal dependency graph cannot call them.

The field identity and stored-integer modules expose exact record indexes and
validation to consumers. Their values confer no source acceptance, provider
admission or live storage authority.
