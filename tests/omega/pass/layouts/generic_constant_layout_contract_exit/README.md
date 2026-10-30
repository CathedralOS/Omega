# Constant contracts across specialization

`require_geometry<u8>` and `require_geometry<u64>` have separate lexical
applications of the same complete `size_of<Cell>() + align_of<Cell>() == 24`
contract. The unused binder deliberately isolates copying and admission from
dependent layout: this program already runs on the pre-migration producer.
The runtime layout check independently traps on an incorrect result.

This witnesses the `PIPELINE-ARCHITECTURE-ENFORCEMENT` migration. Both executions
must exit zero, and the sibling failure fixture
`../../../fail/layouts/generic_constant_layout_contract/` changes only Cell's
field count and must reject the same false contract. A missing evaluation entry
is unfinished implementation, not an acceptable rejection for either fixture.
