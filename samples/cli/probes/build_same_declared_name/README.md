# build_same_declared_name

Two different source directories may declare the same package name:
`dep_one` and `dep_two` both declare `twin_kernel` under aliases `k1`
and `k2`. Declared names are diagnostic identity, not uniqueness-keyed
— the alias selects each occurrence, so `k1::main`'s `value()` and
`k2::main`'s `other()` compose in one product body.
