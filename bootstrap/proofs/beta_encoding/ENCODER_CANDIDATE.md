# Paused complete encoder candidate

This temporary engineering note preserves one concrete integration candidate
for the [complete encoding subject](ACCEPTANCE.md). It is not current Beta
semantics, a demonstrated resource fit, or permission for isolated helper work.
Remove it when an integrated source-owned encoder supersedes the recipe, or an
explicitly assessed replacement strategy displaces it while preserving the
required full-subject proof. Implementation status belongs on the bootstrap board.

## Proposed computation

One total encoder entrance sequences complete raw-source envelope/capacity
admission, token-at-a-time scanning, one EOF flush with no pending operands,
and successful output-fragment flattening. The checker validates explicit
equalities; it does not run the encoder. Root reconstruction owns raw source,
tape, limits, and theory independently of proof production.

Use a mechanical raw-byte tree `Empty | Leaf(Byte) | Join(Source, Source)`,
not token- or instruction-selected partition boundaries. Recursion consumes an
unchanged immediate child. Incoming state carries comment mode, reversed pending
token, operand expectation, checked output count, and sticky failure, but no
completed-token list or accumulated output history.

Valid tokens have at most 19 bytes including an assertion colon. This is not a
malformed-token resource bound: absent early rejection, pending tokens and their
reversals may span the complete admitted source. Cost and containment must cover
that case without inventing a token-capacity language outcome.

Each scan result contains outgoing state and a fragment:
`Empty | Chunk(ByteList) | Join(Fragment, Fragment)`. A token emits at most eight
bytes; source joins combine fragments in source order while threading only state.
No empty-join simplifier is required. At final success:

```text
flatten(Empty, tail) = tail
flatten(Chunk(bytes), tail) = append(bytes, tail)
flatten(Join(left, right), tail) = flatten(left, flatten(right, tail))
```

Append recurses on the unchanged list tail, flatten on unchanged fragment children.
Each output byte is appended once rather than once per source ancestor; explicit
flattening, projection, congruence, and root-equality proofs still cost work.

## Token transitions

Retain complete-token register/HEXWORD parsers and explicit reversal, including
leading zeroes, lowercase spelling, width, and full exhaustion. Do not introduce
a separate character-recognizer framework.

| Expectation | Token | Next |
| --- | --- | --- |
| R | Register | Ready |
| X | HEXWORD | Ready |
| RR | Register | R |
| RX | Register | X |
| RRX | Register | RX |

In Ready, all 21 mnemonic rows select exact opcode and expectation; emit the
opcode immediately, including operand-free `ret`. `dw` selects X without an
opcode. Assertions are legal only in Ready, equal the current output count,
and emit nothing. Unknown/inappropriate forms are Invalid. Registers emit one
byte; words use the shared eight-byte little-endian serializer.

Intermediate fragments are not publication, so later wrong/missing operands
prevent Success without a whole-instruction buffer. Check nonwrapping count
growth and output capacity at each emission, retaining sticky exhaustion;
EOF-only checking could replace an earlier capacity refusal with a later syntax
failure. Separators flush tokens; semicolon flushes then enters comment mode;
CR/LF end comments. EOF flushes once and requires Ready, also after comments.
Complete envelope admission includes late comment bytes after a parser defect.
The total failure-selection equations still require audit against the selected
profile; this candidate does not amend compiler diagnostic precedence.

## Continuation condition

Pause isolated helper and provision expansion until one complete definition
package and integrated recipe has a defensible feasibility account. Cost all
theory declarations (including unused clauses), owner source/tape/limits/root,
admission, scan joins and projections, reversals/parsers, opcode dispatch,
serialization, count/limit/assertion checks, failure propagation, fragments,
flattening, equality, framing, cumulative allocation, depth, and execution time.
State-dependent composition cannot assume that shared local token facts eliminate
its work. Distinguish measured costs, algebraic recipe scenarios, and missing
quantities; there is no established complete cost or justified larger profile.

Use the existing theory entrance with source admission, token recognition,
instruction/emission transitions, and output composition as subordinate owners.
Keep arithmetic shared and create files only when actual definitions require
them. Select a coherent profile only after the complete package and recipe
expose all costs and their sharing assumptions. Package completion alone does
not close the source-owned certificate, mutation/resource controls, or admission.
The pause does not abandon P1, authorize a new language/checker rule or host
producer, or block independent bootstrap work. Ordinary engineering choices
remain engineering choices, not unresolved language decisions.
