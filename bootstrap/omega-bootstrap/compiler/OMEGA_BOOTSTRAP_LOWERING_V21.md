# Omega bootstrap lowering frame v21

`OMGLOWL21` pairs one `OMGCOMP1` compilation with its `OMGRSWC12` witness
and lowers the selected full TokenStream slice to canonical `CKIR20`.
`omega-bootstrap-token-stream-to-ckir.alp` validates the pairing before it
publishes any checked-IR byte.

The 32-byte little-endian outer header is `8s + 4u16 + 4u32`: magic
`OMGLOWK\0`, major 21, minor/patch 0, header size 32, total bytes,
compilation bytes, witness bytes, and selector 12. The frame must contain
exactly those two adjacent payloads with no trailing byte. The witness is
exactly 7,520 bytes and must report the same compilation length and selected
source extent.

Validation includes dense table identity, source/name/body span custody,
call-target pairing, record and pure-sum copy policy, active TokenKind payload
facts, Token/observation store ownership, Float constructor 78 with payload
bits `true,false,true`, and observation tag 70. Cross-paired compilation and
witness frames are rejected.

Canonical output is CKIR major 20, target 1, flags 1, entry machine 2:

- 13,704 bytes, SHA-256
  `ee418b04a4c661d329fe55198ae2b1063c86f5ed421711b9b0ab88f5eff6351a`;
- 24 types, 8 records/29 fields, 5 sums/105 cases/8 payloads;
- 3 machines/11 parameters, 14 blocks/11 block parameters;
- 183 operations, 180 operands, 14 terminators, 9 case arms and 8 arm args;
- `TokenKind` layout 12/4, `Token` 56/8, observation 40/8,
  TokenStream/Main 1,638,456/8, and rounded BSS 1,642,496;
- independent CKIR interpretation returns observation tag 70.

Status 251 is a semantic or pairing rejection. Status 252 is an outer,
compilation, source, or input resource exhaustion. Neither status publishes
partial CKIR.
