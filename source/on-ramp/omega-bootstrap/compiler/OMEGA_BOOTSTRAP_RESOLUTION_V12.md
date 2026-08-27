# Omega bootstrap resolution witness v12

`OMGRSWC12` is the focused resolved-source contract for the complete
`TokenStream::push` bootstrap slice. It is produced by
`omega-bootstrap-token-stream-resolve.alp` from one `OMGCOMP1` unit.

The 192-byte little-endian header is `8s + 4u16 + 44u32`: magic
`OMGRSWC\0`, major 12, minor/patch 0, header size 192, followed by total and
compilation byte lengths and these table counts:

| table | count | row bytes | byte offset |
| --- | ---: | ---: | ---: |
| unit | 1 | 20 | 192 |
| type | 22 | 32 | 212 |
| record | 8 | 32 | 916 |
| field | 29 | 24 | 1172 |
| sum | 5 | 32 | 1868 |
| case | 105 | 28 | 2028 |
| case payload | 8 | 24 | 4968 |
| machine | 3 | 56 | 5160 |
| machine parameter | 11 | 24 | 5328 |
| block | 14 | 40 | 5592 |
| block parameter | 11 | 24 | 6152 |
| call | 2 | 36 | 6416 |
| store | 15 | 40 | 6488 |
| store path | 20 | 4 | 7088 |
| call argument | 11 | 32 | 7168 |

The exact witness is 7,520 bytes. IDs are dense within every row-bearing
table. Header-selected record IDs 0..7 are `SourceId`, `Span`, `SourceSpan`,
`Token`, `LexDiagnostic`, `TokenObservation`, `TokenStream`, and `Main` by
resolved role, not spelling or declaration position. Sum IDs 0..4 are
`NumericBase`, `KeywordKind`, `PunctuationKind`, `TokenKind`, and
`LexDiagnosticCode`; their declaration-order case counts are 4/30/42/9/20.
All six leaf/product records and all five pure sums preserve copy policy.

The selected `TokenKind` cases are globally dense: Integer 77, Float 78,
Keyword 80, and Punctuation 81. The eight payload rows preserve
`Integer(NumericBase,bool,bool)`, `Float(bool,bool,bool)`,
`Keyword(KeywordKind)`, and `Punctuation(PunctuationKind)`.

The witness owns the exact 16,384-element Token and observation arrays, the
65,536-byte decoded buffer, the 1,638,456-byte derived owner, ten-parameter
push, full/retain branch, fifteen stores and twenty nested store projections,
Exact `+1`, and the complete indexed Float readback. Root arguments encode
`SourceId { value: 4 }`, `Float(true,false,true)`, 5/6/7/8,
70/1/2/3, and read index 0. The selected entry is machine 2; push/read are
machines 0/1.

Status 251 means malformed compilation or a semantic mismatch and publishes
no bytes. Status 252 means a declared input/owner resource ceiling was
exceeded and also publishes no bytes. The focused owner ceiling is 2 MiB;
the adjacent derived-owner control is independent of the 16,385-element
semantic mismatch.
