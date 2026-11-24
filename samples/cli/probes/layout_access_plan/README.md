# Field policy and fixed layout

The entry returns normally when the access policy describes both fields of `Pair` and a separate `FixedLayout<Pair>` places their values at the selected offsets. It writes `17` to `hidden` and `3` to `readable` in an ordinary `LayoutStorage<Pair, PairLayout>` value. Its eight-byte view must contain `17` at byte 0 and `3` at byte 4.

To exercise the failure branch in a copy of the sample, change the expected byte at offset 4 from `3u8` to `2u8`. Both engines should trap.

Run each engine from the repository root.

```sh
mbx run -p omega -- run --engine native samples/cli/probes/layout_access_plan/build.omg
mbx run -p omega -- run --engine psi samples/cli/probes/layout_access_plan/build.omg
```
