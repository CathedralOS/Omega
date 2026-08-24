# Provisional Ωself census — checkpoint 000001

Checkpoint 000001 is the first coherent Omega-written product compiler source
snapshot. It implements Psi source custody, final token/lexical-diagnostic
representations, Unicode 17 XID classification, and complete source-to-token
spelling. The hosted adapter reads one source unit, exits with its accepted
status 0, rejects lexical errors with status 251, and rejects source
capacity exhaustion with status 252.

This census is provisional evidence, not the final `Ωself` profile. It records
general facilities used by the exact manifest in `checkpoint-000001.json`.
Later product checkpoints rerun the census; the Delta bridge supplies the cost
evidence needed to settle retain-versus-refactor decisions. “Unused” below means
absent from this lexical checkpoint only. Such a facility may be rejected by
this checkpoint's provisional profile, but it is not finally excluded from
`Ωself` while later compiler phases remain unwritten.

| Facility | Checkpoint use | Provisional disposition |
| --- | --- | --- |
| modules and authored dependency aliases | product Psi modules plus hosted `psi` alias | retain candidate; general name/import rules required |
| ordinary named records and nested field access | source units, spans, tokens, streams, lexer state | retain candidate; positional duplication rejected |
| payload-free and payload-bearing sum data | token vocabulary, numeric bases, diagnostics, console reads | retain candidate; general tagged layout required |
| fixed arrays, slices, string/byte literals, runtime indexing | source/decoded/token buffers, Unicode tables, keyword spelling | retain candidate with explicit capacity and exhaustion rules |
| concrete scalar ranges | source/token lengths and standard-library byte results | measure; currently pays directly for bounded indexing without dependent bounds |
| concrete Trapping arithmetic and casts | cursor math, UTF-8/scalar arithmetic, byte conversion | measure against narrow checked helpers in the bridge |
| state machines, scalar state parameters, mutation, and calls | every lexical scan and hosted adapter loop | retain candidate; branching value-machine results are deliberately unnecessary |
| explicit result fields for branching operations | bounded appends and lexical predicates | retain candidate source convention pending general bridge call-cost evidence |
| boundary traits and target-selected realizations | hosted byte input and process exit | retain only the sealed compiler-host byte/exit surface needed by product entrypoints |
| basic explicit generic calls | standard provider selection in the transitive console closure | measure against a non-generic sealed provider binding |
| generated ordinary-Omega data | Unicode XID range arrays | retain generated-source closure rules; generator and external data stay pinned inputs |
| proof/program mathematics | unused in checkpoint | reject provisionally; likely final exclusion because implementing full-Omega proof checking does not require proof syntax in compiler source |
| dependent bounds and linear types | unused in checkpoint | reject provisionally; likely final exclusion, subject to later source closures |
| domain polymorphism | unused in checkpoint | reject provisionally; final disposition awaits later source closures and bridge cost |
| advanced generic constraints, specialization, reflection | unused in checkpoint | reject provisionally; final disposition awaits later source closures and bridge cost |
| numeric/schema field tags | unused in checkpoint | reject provisionally; compare ordinary named fields if later source introduces them |
| mixed field-plus-case declarations | unused in checkpoint | reject provisionally; compare separate records and sums if later source introduces them |
| complex aggregate transition payloads | unused in checkpoint | reject provisionally; compare scalar/index state parameters plus explicit context if later source introduces them |

The checkpoint deliberately binds branching computations to fields before
dispatch. This is ordinary Omega and avoids depending on implicit arm-value
materialization in the bridge. It is a source-profile simplification, not a new
language or a semantic exception.

## Functional gate

Run `compiler/source-checkpoints/checkpoint-000001.sh`. The checkpoint is
accepted only when all of the following hold:

1. `python3 compiler/source-checkpoints/verify_manifest.py` replays all declared
   target resolutions, validates the exact loaded-source/alias/import closure and
   provenance digests, and rejects the built-in closure mutations.
2. The hosted entry compiles through native emission for its selected target.
3. Empty input, identifiers, integers, punctuation, whitespace, representative
   Omega source with a Unicode identifier, nested block comments, and
   cooked/raw strings accept with status 0.
4. Invalid UTF-8, unterminated nested comments, invalid cooked-string escapes,
   and unsupported punctuation reject with status 251 and publish no token
   observation as success.

The adapter does not yet publish the complete canonical token/diagnostic byte
stream. That observation format and the Rust-comparator differential are the
next product-source checkpoint, not evidence claimed here.

## Measured performance

On the checkpoint host, one 12-source native compile spent about 14% in typed
to checked trees and 84% in control flow to abstract operations; source loading,
parsing, report generation, and native image writing were not the dominant
cost. Unicode data now uses two fixed array literals rather than roughly 1,500
indexed initializer statements, but the current backend still expands the
large aggregate heavily. Follow-up performance work should therefore measure a
general static/generated-data representation and Stage-08 expansion rather
than treating test parallelism or HTML report suppression as the primary fix.
