# Bounded write-only record fields

At historical inventory revision `7a9a7b8287e622137adeef7681b3143138d5724f`,
`Limited.value` and `Limited.next` each had type `u8 [0..=10]`. Both remain
`u8` and their containing data now requires `value <= 10 && next <= 10`;
the unsigned carrier supplies the zero floors. The three `depth` inputs of
`replace`, `fill` and `forward` retain `depth <= 10` as call requirements.
Those input requirements alone did not preserve either storage proposition.

The arithmetic-policy fields and deliberately named `Utf8` qualification retain
their separate meanings. Their declarations and the `fill`/`rename` bodies stay
checked. The executable entry calls `forward` and `replace` on the original
`Limited` storage, observing (7, 4), then (3, 4), and trapping on a mismatch.
Its authored native and Terminal exits are zero.

The companion [fact production](../../../../../samples/cli/facts/data_where_standing/write_only/README.md)
carries standing bounds after write-only field stores through all nine places,
with predicate-sensitive rejection twins and independent access/store controls.
