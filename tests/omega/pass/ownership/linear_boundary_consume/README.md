# linear_boundary_consume

`data Receipt [linear]` + `boundary trait ReceiptAck` (`ack(receipt:
Receipt)`) — a by-value linear parameter consumed through a requirement:
the call edge transfers custody and the requirement discharges the claim
(`Receipt` authors no `drop` plan, so no edge cleanup may retire it).
Binding spellings: `DllImport::ElfVersioned { object, symbol, version }`,
`DllImport::PeByName { library, export }`,
`DllImport::MachODylibSymbol { install_name, symbol }`.

## Learnings

- `ForeignBinding<family, n, m>` counts the import's byte lengths —
  MachO keeps the `_` prefix (`_abs` → 4); a miscount reports "quoted
  byte literal has 4 source byte(s), but owned fixed byte array
  `[u8; 3]` requires exactly 3".

Compile-checked only: `core::external_binding` is depend-free.
