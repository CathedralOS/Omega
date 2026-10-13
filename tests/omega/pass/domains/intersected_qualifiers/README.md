# intersected_qualifiers

`Extent in Granted & Vacant` — intersected qualifiers: a `Granted &
Vacant` value carries the dormant-content qualification into a
`Vacant`-qualified field. `held.e as Extent` strips the qualifier set on
read; only the qualifier-free source shape is an implicit strengthening.

Compile-checked only: `core::extent` is depend-free.
