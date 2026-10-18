# Bounded products

Bounded `u64` operands `a [0..=50]` and `b [0..=50]` make the exact product
`a * b` fit `u64 [0..=2500]`; the body stays a plain `a * b`, with no
`Wrapping` domain and no `as` widening.

Every place performs the same operation: multiply two operands whose declared
range is `0..=50` and return the product as `u64 [0..=2500]`. The oracle runs
both the interior `30 * 40 == 1200` and the upper endpoint `50 * 50 == 2500`
and exits 0 only when both hold. Only the transport of the two operands changes.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | both bounded parameters are copied into locals |
| field | `field/` | bounded record fields carry the operands |
| parameter | `parameter/` | the multiplying machine reads its bounded parameters |
| call result | `call_result/` | calls with declared `u64 [0..=50]` results supply both operands |
| match join | `match_join/` | both arms keep the operands in range, and both arms run |
| const name | `const_name/` | `const CEILING: u64 = 50` bounds both operands |
| another state | `other_state/` | a successor state receives both bounded operands |
| another machine | `other_machine/` | a forwarding machine transports the operands |
| another target body | `target_body/` | each of four target bodies multiplies |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/bounded_products/local/main.omg
omega run samples/cli/facts/bounded_products/field/main.omg
omega run samples/cli/facts/bounded_products/parameter/main.omg
omega run samples/cli/facts/bounded_products/call_result/main.omg
omega run samples/cli/facts/bounded_products/match_join/main.omg
omega run samples/cli/facts/bounded_products/const_name/main.omg
omega run samples/cli/facts/bounded_products/other_state/main.omg
omega run samples/cli/facts/bounded_products/other_machine/main.omg
omega run samples/cli/facts/bounded_products/target_body/main.omg
```

Each place has an `invalid/` child that removes one operand bound (the consistent
carrier occurrence included), so the exact product is no longer provably in
range. The control must reject with `may overflow`; the const-name control keeps
the declared product range and widens the ceiling to 100, so it rejects with
`not provably within its declared range`. Check a twin with
`omega --check <place>/invalid/main.omg`.

The row is pinned by `tests/omega/{pass,fail}/facts/bounded_products_through_*`
fixtures; every positive records native exit 0.
